use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use tokio::sync::{Mutex, RwLock};
use tokio_postgres::Row;

use crate::{constants, info, warn};
use crate::helpers::db_helpers;
use crate::helpers::hashers::Sha512Hashable;
use crate::helpers::string_helpers::FormatToken;
use crate::model::database::db::Database;

lazy_static! {
    static ref ACCOUNTS_CACHE: RwLock<HashMap<AccountId, Arc<Mutex<Account>>>> =
        RwLock::new(HashMap::with_capacity(1024));
}

#[derive(Clone)]
pub struct Account {
    pub id: i64,
    pub account_id: AccountId,
    pub tokens: Vec<AccountToken>,
    pub valid_until: Option<DateTime<Utc>>
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AccountToken {
    pub token: String,
    pub application_type: ApplicationType,
    pub token_type: TokenType
}

impl Display for AccountToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "AccountToken(")?;
        write!(f, "{}, ", self.token.format_token())?;
        write!(f, "{}, ", self.application_type)?;
        write!(f, "{}", self.token_type)?;
        write!(f, ")")?;
        return Ok(());
    }
}

impl AccountToken {
    pub fn from_row(row: &Row) -> anyhow::Result<AccountToken> {
        let token: String = row.try_get(0)?;
        let application_type: i64 = row.try_get(1)?;
        let token_type: i64 = row.try_get(2)?;

        let application_type = ApplicationType::from_i64(application_type);
        let token_type = TokenType::from_i64(token_type);

        let account_token = AccountToken {
            token,
            application_type,
            token_type
        };

        return Ok(account_token);
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ApplicationType {
    Unknown = -1,
    KurobaExLiteDebug = 0,
    KurobaExLiteProduction = 1,
}

impl Display for ApplicationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicationType::KurobaExLiteDebug => {
                write!(f, "KurobaExLiteDebug")?;
            }
            ApplicationType::KurobaExLiteProduction => {
                write!(f, "KurobaExLiteProduction")?;
            }
            ApplicationType::Unknown => {
                write!(f, "Unknown")?;
            }
        }

        return Ok(());
    }
}

impl ApplicationType {
    pub fn from_i64(value: i64) -> ApplicationType {
        let application_type = match value {
            0 => ApplicationType::KurobaExLiteDebug,
            1 => ApplicationType::KurobaExLiteProduction,
            _ => ApplicationType::Unknown
        };

        return application_type;
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum TokenType {
    Unknown = -1,
    Firebase = 0
}

impl Display for TokenType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenType::Firebase => {
                write!(f, "Firebase")?;
            }
            TokenType::Unknown => {
                write!(f, "Unknown")?;
            }
        }

        return Ok(());
    }
}

impl TokenType {
    pub fn from_i64(value: i64) -> TokenType {
        let token_type = match value {
            0 => TokenType::Firebase,
            _ => TokenType::Unknown
        };

        return token_type;
    }
}

impl Account {
    pub fn get_account_token(
        &self,
        application_type: &ApplicationType
    ) -> Option<&AccountToken> {
        for token in &self.tokens {
            if token.application_type == *application_type {
                return Some(token);
            }
        }

        return None;
    }

    pub fn is_valid(&self, application_type: &ApplicationType) -> bool {
        let token = &self.get_account_token(application_type);
        if token.is_none() {
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

    pub fn validation_status(&self, application_type: &ApplicationType) -> Option<String> {
        let token = &self.get_account_token(application_type);
        if token.is_none() {
            return Some(format!("token for app_type \'{}\' is not set", application_type));
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

    pub fn add_or_update_token(&mut self, new_token: AccountToken) {
        for (index, old_token) in self.tokens.iter().enumerate() {
            if old_token.token == new_token.token {
                let mut updated_token = self.tokens[index].clone();
                updated_token.token_type = new_token.token_type;
                updated_token.application_type = new_token.application_type;
                return;
            }
        }

        self.tokens.push(new_token)
    }

    pub fn account_token(&self, application_type: &ApplicationType) -> Option<&AccountToken> {
        return self.get_account_token(application_type);
    }

    pub fn new(
        id: i64,
        account_id: AccountId,
        tokens: Vec<AccountToken>,
        valid_until: Option<DateTime<Utc>>
    ) -> Account {
        return Account {
            id,
            account_id,
            tokens,
            valid_until
        }
    }

    pub fn from_row(row: &Row) -> anyhow::Result<Account> {
        let id: i64 = row.try_get(0)?;
        let account_id: String = row.try_get(1)?;
        let valid_until: Option<DateTime<Utc>> = row.try_get(2)?;

        let account = Account {
            id,
            account_id: AccountId::new(account_id),
            tokens: Vec::with_capacity(4),
            valid_until
        };

        return Ok(account);
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
        write!(f, "{}, ", self.account_id)?;
        write!(f, "{}, ", self.tokens.len())?;
        write!(f, "{:?}, ", self.valid_until)?;
        write!(f, ")")?;
        return Ok(());
    }
}

pub async fn get_account(
    account_id: &AccountId,
    database: &Arc<Database>,
) -> anyhow::Result<Option<Arc<Mutex<Account>>>> {
    let from_cache = {
        ACCOUNTS_CACHE.read()
            .await
            .get(account_id)
            .cloned()
    };

    if from_cache.is_some() {
        return Ok(Some(from_cache.unwrap()));
    }

    let account = get_account_from_database(&account_id, database).await?;
    if account.is_none() {
        return Ok(None);
    }

    let account_tokens = get_account_tokens_from_database(&account_id, database).await?;

    let mut account = account.unwrap();
    for account_token in account_tokens {
        account.add_or_update_token(account_token);
    }

    let account_id = account.account_id.clone();
    let account = Arc::new(Mutex::new(account));

    {
        let mut cache = ACCOUNTS_CACHE.write().await;
        cache.insert(account_id, account.clone());
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
        RETURNING accounts.id
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let id: i64 = connection.query_one(
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
            id,
            account_id.clone(),
            Vec::with_capacity(4),
            valid_until.clone()
        );

        let new_account = Arc::new(Mutex::new(new_account));
        accounts_locked.insert(account_id.clone(), new_account);
    }

    return Ok(CreateAccountResult::Ok);
}

pub async fn update_firebase_token(
    database: &Arc<Database>,
    account_id: &AccountId,
    application_type: &ApplicationType,
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

    let account_id_generated = { existing_account.unwrap().lock().await.id };

    let query = r#"
        INSERT INTO account_tokens (
            owner_account_id,
            token,
            application_type,
            token_type
        )
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (token, application_type, token_type) DO NOTHING
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    connection.execute(
        &statement,
        &[
            &account_id_generated,
            &firebase_token.token,
            &(application_type.clone() as i64),
            &(TokenType::Firebase as i64)
        ]
    )
        .await
        .context("update_firebase_token() Failed to update firebase_token in the database")?;

    {
        let mut accounts_locked = ACCOUNTS_CACHE.write().await;

        let existing_account = accounts_locked.get_mut(account_id);
        if existing_account.is_some() {
            let mut existing_account = existing_account.unwrap().lock().await;

            let account_token = AccountToken {
                token: firebase_token.token.clone(),
                application_type: application_type.clone(),
                token_type: TokenType::Firebase
            };

            existing_account.add_or_update_token(account_token);
        } else {
            return Err(anyhow!("Account {} does not exist!", account_id));
        }
    }

    info!(
        "update_firebase_token() success. account_id: {}, firebase_token: {}",
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
            let mut existing_account = existing_account.unwrap().lock().await;
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

pub async fn retain_post_db_ids_belonging_to_account(
    account_id: &AccountId,
    reply_ids: &Vec<i64>,
    database: &Arc<Database>
) -> anyhow::Result<Vec<i64>> {
    let query = r#"
        SELECT
            post_replies.id
        FROM post_replies
        INNER JOIN accounts account on account.id = post_replies.owner_account_id
        WHERE
            account.account_id = $1
        AND
            post_replies.id IN ({QUERY_PARAMS})
    "#;

    let connection = database.connection().await?;

    let (query, mut db_params) = db_helpers::format_query_params_with_start_index(
        query,
        "{QUERY_PARAMS}",
        1,
        reply_ids
    )?;


    db_params.insert(0, &account_id.id);


    info!("TTTAAA query: {}", query);
    info!("TTTAAA db_params: {:?}", db_params);

    let statement = connection.prepare(&query).await?;

    let rows = connection.query(&statement, &db_params[..]).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let mut result_vec = Vec::<i64>::with_capacity(rows.len());

    for row in rows {
        let post_descriptor_id: i64 = row.try_get(0)?;
        result_vec.push(post_descriptor_id);
    }

    return Ok(result_vec);
}

async fn get_account_from_database(
    account_id: &AccountId,
    database: &Arc<Database>
) -> anyhow::Result<Option<Account>> {
    let query = r#"
        SELECT
            accounts.id,
            accounts.account_id,
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

    return Ok(Some(account.unwrap()));
}

async fn get_account_tokens_from_database(
    account_id: &AccountId,
    database: &Arc<Database>
) -> anyhow::Result<Vec<AccountToken>> {
    let query = r#"
        SELECT
            token,
            application_type,
            token_type
        FROM accounts
        INNER JOIN
            account_tokens account_token on accounts.id = account_token.owner_account_id
        WHERE account_id = $1
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let rows = connection.query(&statement, &[&account_id.id]).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let mut result_vec = Vec::<AccountToken>::with_capacity(rows.len());

    for row in rows {
        let account_token = AccountToken::from_row(&row)?;
        result_vec.push(account_token);
    }

    return Ok(result_vec);
}

pub async fn test_get_account_from_cache(
    account_id: &AccountId,
) -> Option<Arc<Mutex<Account>>> {
    return ACCOUNTS_CACHE.read()
        .await
        .get(account_id)
        .cloned()
}

pub async fn test_put_account_into_cache(
    account: &Account
) {
    let mut account_cache_locked = ACCOUNTS_CACHE.write().await;

    let account_id = account.account_id.clone();
    let account = Arc::new(Mutex::new(account.clone()));

    account_cache_locked.insert(account_id, account);
}

pub async fn test_get_account_from_database(
    account_id: &AccountId,
    database: &Arc<Database>
) -> anyhow::Result<Option<Account>> {
    let query = r#"
        SELECT
            accounts.id,
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
        SELECT COUNT(accounts.id)
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
