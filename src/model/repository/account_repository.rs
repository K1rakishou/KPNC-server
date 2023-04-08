use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use anyhow::Context;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use lazy_static::lazy_static;
use tokio_postgres::Row;
use crate::constants;
use crate::helpers::hashers::Sha3_512_Hashable;
use crate::model::database::db::Database;
use crate::helpers::string_helpers::FormatToken;

lazy_static! {
    static ref accounts_cache: RwLock<HashMap<AccountId, Account>> = RwLock::new(HashMap::with_capacity(1024));
}

#[derive(Clone)]
pub struct Account {
    account_id: AccountId,
    firebase_token: FirebaseToken,
    pub valid_until: Option<DateTime<Utc>>
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
pub enum UpdateFirebaseTokenResult {
    Ok,
    AccountDoesNotExist
}

impl AccountId {
    pub fn from_str(value: &str) -> AccountId {
        return AccountId { id: value.sha3_512(constants::USER_ID_HASH_ITERATIONS) }
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.id);
    }
}

impl FirebaseToken {
    pub fn from_str(value: &str) -> FirebaseToken {
        return FirebaseToken { token: String::from(value) }
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
        write!(f, "firebase_token: {}, ", self.firebase_token)?;
        write!(f, "valid_until: {:?}, ", self.valid_until)?;
        write!(f, ")")?;
        return Ok(());
    }
}

impl Account {
    pub fn firebase_token(&self) -> &FirebaseToken {
        return &self.firebase_token
    }

    pub fn new(
        account_id: AccountId,
        firebase_token: FirebaseToken,
        valid_until: Option<DateTime<Utc>>
    ) -> Account {
        return Account {
            account_id,
            firebase_token,
            valid_until
        }
    }

    pub fn new_with_token(
        account_id: AccountId,
        firebase_token: FirebaseToken
    ) -> Account {
        return Account {
            account_id,
            firebase_token,
            valid_until: Option::None
        }
    }

    pub fn from_row(row: &Row) -> anyhow::Result<Account> {
        let account_id: String = row.try_get(0)?;
        let firebase_token: String = row.try_get(1)?;
        let valid_until: Option<DateTime<Utc>> = row.try_get(2)?;

        let account = Account {
            account_id: AccountId::from_str(&account_id),
            firebase_token: FirebaseToken::from_str(&firebase_token),
            valid_until
        };

        return Ok(account);
    }
}

pub async fn get_account(
    database: &Arc<Database>,
    account_id: &AccountId
) -> anyhow::Result<Option<Account>> {
    let from_cache = {
        accounts_cache.read()
            .await
            .get(account_id)
            .cloned()
    };

    if from_cache.is_some() {
        return Ok(from_cache);
    }

    let connection = database.connection().await?;
    let statement = connection.prepare("SELECT * FROM accounts WHERE accounts.account_id = $1").await?;

    let row = connection.query_opt(&statement, &[&account_id.id]).await?;
    if row.is_none() {
        return Ok(None);
    }

    let account = Account::from_row(&row.unwrap());
    if account.is_err() {
        return Err(account.err().unwrap());
    }

    let account = {
        let account = account.unwrap();
        let mut cache = accounts_cache.write().await;

        cache.insert(account.account_id.clone(), account.clone());
        cache.get(&account.account_id).cloned()
    };

    return Ok(account);
}

pub async fn create_account(
    database: &Arc<Database>,
    account_id: &AccountId,
    firebase_token: &FirebaseToken,
    valid_until: Option<&DateTime<Utc>>
) -> anyhow::Result<CreateAccountResult> {
    let existing_account = get_account(database, account_id).await?;
    if existing_account.is_some() {
        warn!("create_account() account with id: {} already exists!", account_id);
        return Ok(CreateAccountResult::AccountAlreadyExists);
    }

    let connection = database.connection().await?;

    let statement = connection
        .prepare("INSERT INTO accounts(account_id, firebase_token, valid_until) VALUES ($1, $2, $3)")
        .await?;

    connection.execute(
        &statement,
        &[&account_id.id, &firebase_token.token, &valid_until]
    ).await?;

    {
        let mut accounts_locked = accounts_cache.write().await;

        let existing_account = accounts_locked.get_mut(account_id);
        if existing_account.is_some() {
            let mut existing_account = existing_account.unwrap();
            existing_account.firebase_token = firebase_token.clone();
            existing_account.valid_until = valid_until.cloned();
        } else {
            let new_account = Account::new(
                account_id.clone(),
                firebase_token.clone(),
                valid_until.cloned()
            );

            accounts_locked.insert(account_id.clone(), new_account);
        }
    }

    return Ok(CreateAccountResult::Ok);
}

pub async fn update_firebase_token(
    database: &Arc<Database>,
    account_id: &AccountId,
    firebase_token: &FirebaseToken
) -> anyhow::Result<UpdateFirebaseTokenResult> {
    let existing_account = get_account(database, account_id).await?;
    if existing_account.is_none() {
        return Ok(UpdateFirebaseTokenResult::AccountDoesNotExist);
    }

    let connection = database.connection().await?;

    let statement = connection
        .prepare("UPDATE accounts SET account_id = $1, firebase_token = $2")
        .await?;

    connection.execute(
        &statement,
        &[&account_id.id, &firebase_token.token]
    ).await.context("update_account() Failed to update firebase_token in the database")?;

    {
        let mut accounts_locked = accounts_cache.write().await;

        let existing_account = accounts_locked.get_mut(account_id);
        if existing_account.is_some() {
            let mut existing_account = existing_account.unwrap();
            existing_account.firebase_token = firebase_token.clone();
        } else {
            let updated_account = Account::new_with_token(
                account_id.clone(),
                firebase_token.clone()
            );

            accounts_locked.insert(account_id.clone(), updated_account);
        }
    }

    info!(
        "update_account() success. account_id: {}, firebase_token: {}",
        account_id,
        firebase_token.format_token()
    );

    return Ok(UpdateFirebaseTokenResult::Ok);
}