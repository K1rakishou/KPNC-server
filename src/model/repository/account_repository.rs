use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use tokio::sync::RwLock;
use tokio_postgres::Row;

use crate::{constants, info, warn};
use crate::helpers::hashers::Sha512Hashable;
use crate::helpers::string_helpers::FormatToken;
use crate::model::database::db::Database;

lazy_static! {
    static ref ACCOUNTS_CACHE: RwLock<HashMap<AccountId, Account>> = RwLock::new(HashMap::with_capacity(1024));
}

#[derive(Clone)]
pub struct Account {
    pub id_generated: i64,
    pub account_id: AccountId,
    pub firebase_token: Option<FirebaseToken>,
    pub valid_until: Option<DateTime<Utc>>
}

impl Account {
    pub fn is_valid(&self) -> bool {
        let firebase_token = &self.firebase_token;
        if firebase_token.is_none() {
            return false;
        }

        let valid_until = self.valid_until;
        if valid_until.is_none() {
            return false
        }

        let valid_until = valid_until.unwrap();
        let now = chrono::Utc::now();

        return valid_until >= now;
    }

    pub fn validation_status(&self) -> Option<String> {
        let firebase_token = &self.firebase_token;
        if firebase_token.is_none() {
            return Some("firebase_token is not set".to_string());
        }

        let valid_until = self.valid_until;
        if valid_until.is_none() {
            return Some("valid_until is not set".to_string());
        }

        let valid_until = valid_until.unwrap();
        let now = chrono::Utc::now();

        if valid_until < now {
            let message = format!(
                "Account is not valid, now: {}, valid_until: {}",
                now,
                valid_until
            );

            return Some(message);
        }

        return None;
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct AccountId {
    pub id: String
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct FirebaseToken {
    pub token: String
}

#[derive(Eq, PartialEq)]
pub enum CreateAccountResult {
    Ok,
    AccountAlreadyExists
}

#[derive(Eq, PartialEq)]
pub enum UpdateAccountExpiryDateResult {
    Ok,
    AccountDoesNotExist
}

#[derive(Eq, PartialEq)]
pub enum UpdateFirebaseTokenResult {
    Ok,
    AccountDoesNotExist
}

impl AccountId {
    pub fn new(account_id: String) -> AccountId {
        if account_id.len() != 128 {
            panic!("Bad account_id len {}", account_id.len());
        }

        return AccountId { id: account_id };
    }

    pub fn from_user_id(user_id: &str) -> anyhow::Result<AccountId> {
        if user_id.len() < 32 || user_id.len() > 128 {
            return Err(anyhow!("Bad user_id length {} must be within 32..128 symbols", user_id.len()));
        }

        let account_id = AccountId { id: user_id.sha3_512(constants::USER_ID_HASH_ITERATIONS) };
        return Ok(account_id);
    }

    pub fn test_unsafe(user_id: &str) -> anyhow::Result<AccountId> {
        let account_id = AccountId { id: user_id.sha3_512(constants::USER_ID_HASH_ITERATIONS) };
        return Ok(account_id);
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.id);
    }
}

impl FirebaseToken {
    pub fn from_opt_str(token: Option<&str>) -> anyhow::Result<Option<FirebaseToken>> {
        if token.is_none() {
            return Ok(None);
        }

        let token = token.unwrap();
        return FirebaseToken::from_str(token)
            .map(|token| Some(token));
    }

    pub fn from_str(token: &str) -> anyhow::Result<FirebaseToken> {
        if token.len() == 0 || token.len() > 1024 {
            return Err(anyhow!("Bad token length {} must be within 1..1024", token.len()));
        }

        let firebase_token = FirebaseToken { token: String::from(token) };
        return Ok(firebase_token);
    }
}

impl Display for FirebaseToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.token);
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Account(")?;
        write!(f, "account_id: {}, ", self.account_id)?;

        if self.firebase_token.is_some() {
            write!(f, "firebase_token: {}, ", self.firebase_token.clone().unwrap())?;
        } else {
            write!(f, "firebase_token: None, ")?;
        }

        write!(f, "valid_until: {:?}, ", self.valid_until)?;
        write!(f, ")")?;
        return Ok(());
    }
}

impl Account {
    pub fn firebase_token(&self) -> Option<&FirebaseToken> {
        return self.firebase_token.as_ref()
    }

    pub fn new(
        id_generated: i64,
        account_id: AccountId,
        firebase_token: Option<FirebaseToken>,
        valid_until: Option<DateTime<Utc>>
    ) -> Account {
        return Account {
            id_generated,
            account_id,
            firebase_token,
            valid_until
        }
    }

    pub fn from_row(row: &Row) -> anyhow::Result<Account> {
        let id_generated: i64 = row.try_get(0)?;
        let account_id: String = row.try_get(1)?;
        let firebase_token_result = row.try_get(2);
        let valid_until: Option<DateTime<Utc>> = row.try_get(3)?;

        let firebase_token: Option<&str> = if firebase_token_result.is_err() {
            None
        } else {
            Some(firebase_token_result.unwrap())
        };


        let account = Account {
            id_generated,
            account_id: AccountId::new(account_id),
            firebase_token: FirebaseToken::from_opt_str(firebase_token)?,
            valid_until
        };

        return Ok(account);
    }
}

pub async fn get_account(
    account_id: &AccountId,
    database: &Arc<Database>,
) -> anyhow::Result<Option<Account>> {
    let from_cache = {
        ACCOUNTS_CACHE.read()
            .await
            .get(account_id)
            .cloned()
    };

    if from_cache.is_some() {
        return Ok(Some(from_cache.unwrap()));
    }

    let query = r#"
        SELECT
            accounts.id_generated,
            accounts.account_id,
            accounts.firebase_token,
            accounts.valid_until
        FROM accounts
        WHERE
            accounts.account_id = $1
        AND
            accounts.deleted_on IS NULL
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let row = connection.query_opt(&statement, &[&account_id.id]).await?;
    if row.is_none() {
        return Ok(None);
    }

    let account = Account::from_row(&row.unwrap());
    if account.is_err() {
        return Err(account.err().unwrap());
    }

    let account = account.unwrap();

    {
        let mut cache = ACCOUNTS_CACHE.write().await;
        cache.insert(account.account_id.clone(), account.clone());
    };

    return Ok(Some(account));
}

pub async fn create_account(
    database: &Arc<Database>,
    account_id: &AccountId,
    valid_until: Option<DateTime<Utc>>
) -> anyhow::Result<CreateAccountResult> {
    let existing_account = get_account(account_id, database).await?;
    if existing_account.is_some() {
        warn!("create_account() account with id: {} already exists!", account_id.format_token());
        return Err(anyhow!("Account {} already exists!", account_id));
    }

    let query = r#"
        INSERT INTO accounts
        (
            account_id,
            valid_until
        )
        VALUES ($1, $2)
        RETURNING accounts.id_generated
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let id_generated: i64 = connection.query_one(
        &statement,
        &[&account_id.id, &valid_until]
    ).await?.try_get(0)?;

    {
        let mut accounts_locked = ACCOUNTS_CACHE.write().await;

        let existing_account = accounts_locked.get_mut(account_id);
        if existing_account.is_some() {
            return Err(anyhow!("Account {} already exists!", account_id));
        }

        let new_account = Account::new(
            id_generated,
            account_id.clone(),
            None,
            valid_until.clone()
        );

        accounts_locked.insert(account_id.clone(), new_account);
    }

    return Ok(CreateAccountResult::Ok);
}

pub async fn update_firebase_token(
    database: &Arc<Database>,
    account_id: &AccountId,
    firebase_token: &FirebaseToken
) -> anyhow::Result<UpdateFirebaseTokenResult> {
    let existing_account = get_account(account_id, database).await?;
    if existing_account.is_none() {
        warn!(
            "update_firebase_token() account with id: {} does not exist!",
            account_id.format_token()
        );

        return Ok(UpdateFirebaseTokenResult::AccountDoesNotExist);
    }

    let query = r#"
        UPDATE accounts
        SET
            firebase_token = $1
        WHERE
            account_id = $2
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    connection.execute(
        &statement,
        &[&firebase_token.token, &account_id.id]
    )
        .await
        .context("update_account() Failed to update firebase_token in the database")?;

    {
        let mut accounts_locked = ACCOUNTS_CACHE.write().await;

        let existing_account = accounts_locked.get_mut(account_id);
        if existing_account.is_some() {
            let mut existing_account = existing_account.unwrap();
            existing_account.firebase_token = Some(firebase_token.clone());
        } else {
            return Err(anyhow!("Account {} does not exist!", account_id));
        }
    }

    info!(
        "update_account() success. account_id: {}, firebase_token: {}",
        account_id.format_token(),
        firebase_token.format_token()
    );

    return Ok(UpdateFirebaseTokenResult::Ok);
}

pub async fn update_account_expiry_date(
    database: &Arc<Database>,
    account_id: &AccountId,
    valid_until: &DateTime<Utc>
) -> anyhow::Result<UpdateAccountExpiryDateResult> {
    let existing_account = get_account(account_id, database).await?;
    if existing_account.is_none() {
        warn!(
            "update_account_expiry_date() account with id: {} does not exist!",
            account_id.format_token()
        );

        return Ok(UpdateAccountExpiryDateResult::AccountDoesNotExist);
    }

    let query = r#"
        UPDATE accounts
        SET
            valid_until = $1
        WHERE
            account_id = $2
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    connection.execute(
        &statement,
        &[&valid_until, &account_id.id]
    )
        .await
        .context("update_account_expiry_date() Failed to update valid_until in the database")?;

    {
        let mut accounts_locked = ACCOUNTS_CACHE.write().await;

        let existing_account = accounts_locked.get_mut(account_id);
        if existing_account.is_some() {
            let mut existing_account = existing_account.unwrap();
            existing_account.valid_until = Some(valid_until.clone());
        } else {
            return Err(anyhow!("Account {} does not exist!", account_id));
        }
    }

    info!(
        "update_account_expiry_date() success. account_id: {}, valid_until: {}",
        account_id.format_token(),
        valid_until
    );

    return Ok(UpdateAccountExpiryDateResult::Ok);
}

pub async fn test_get_account_from_cache(
    account_id: &AccountId,
) -> Option<Account> {
    return ACCOUNTS_CACHE.read()
        .await
        .get(account_id)
        .cloned()
}

pub async fn test_put_account_into_cache(
    account: &Account
) {
    let mut account_cache_locked = ACCOUNTS_CACHE.write().await;
    account_cache_locked.insert(account.clone().account_id, account.clone());
}

pub async fn test_get_account_from_database(
    account_id: &AccountId,
    database: &Arc<Database>
) -> anyhow::Result<Option<Account>> {
    let query = r#"
        SELECT
            accounts.id_generated,
            accounts.account_id,
            accounts.firebase_token,
            accounts.valid_until
        FROM accounts
        WHERE
            accounts.account_id = $1
        AND
            accounts.deleted_on IS NULL
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let row = connection.query_opt(&statement, &[&account_id.id]).await?;
    if row.is_none() {
        return Ok(None);
    }

    let account = Account::from_row(&row.unwrap()).unwrap();
    return Ok(Some(account));
}

pub async fn test_put_account_into_database(
    account: &Account,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    let query = r#"
        INSERT INTO accounts
        (
            account_id,
            valid_until,
            deleted_on
        )
        VALUES ($1, $2, NULL)
        ON CONFLICT (account_id) DO UPDATE SET valid_until = $2
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    connection.execute(
        &statement,
        &[
            &account.account_id.id,
            &account.valid_until
        ]
    ).await?;

    return Ok(());
}

pub async fn test_count_accounts_in_database(database: &Arc<Database>) -> anyhow::Result<i64> {
    let query = r#"
        SELECT COUNT(accounts.id_generated)
        FROM accounts
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let accounts_count: i64 = connection.query_opt(&statement, &[]).await?.unwrap().get(0);
    return Ok(accounts_count);
}

pub async fn test_count_accounts_in_cache() -> usize {
    return ACCOUNTS_CACHE.read()
        .await
        .len();
}

pub async fn test_cleanup() {
    let mut accounts_cache_locked = ACCOUNTS_CACHE.write().await;
    accounts_cache_locked.clear();
}
