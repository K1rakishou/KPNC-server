use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use lazy_static::lazy_static;
use tokio_postgres::Row;
use crate::helpers::hashers::Sha3_512_Hashable;
use crate::model::database::db::Database;
use crate::helpers::string_helpers::FormatToken;

lazy_static! {
    static ref accounts_cache: RwLock<HashMap<UserId, Account>> = RwLock::new(HashMap::with_capacity(1024));
}

#[derive(Clone)]
pub struct Account {
    user_id: UserId,
    firebase_token: FirebaseToken,
    valid_until: Option<DateTime<Utc>>
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct UserId {
    pub id: String
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct FirebaseToken {
    pub token: String
}

impl UserId {
    pub fn from_str(value: &str) -> UserId {
        return UserId { id: value.sha3_512() }
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "UserId(id: {})", self.id);
    }
}

impl FirebaseToken {
    pub fn from_str(value: &str) -> FirebaseToken {
        return FirebaseToken { token: String::from(value) }
    }
}

impl Display for FirebaseToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "FirebaseToken(token: {})", self.token);
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Account(")?;
        write!(f, "user_id: {}, ", self.user_id)?;
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

    pub fn new_with_token(user_id: UserId, firebase_token: FirebaseToken) -> Account {
        return Account {
            user_id,
            firebase_token,
            valid_until: None
        }
    }

    pub fn from_row(row: &Row) -> anyhow::Result<Account> {
        let user_id: String = row.try_get(0)?;
        let firebase_token: String = row.try_get(1)?;
        let valid_until: Option<DateTime<Utc>> = row.try_get(2)?;

        let account = Account {
            user_id: UserId::from_str(&user_id),
            firebase_token: FirebaseToken::from_str(&firebase_token),
            valid_until
        };

        return Ok(account);
    }
}

pub async fn get_account(
    database: &Arc<Database>,
    user_id: &UserId
) -> anyhow::Result<Option<Account>> {
    let from_cache = {
        accounts_cache.read()
            .await
            .get(user_id)
            .cloned()
    };

    if from_cache.is_some() {
        return Ok(from_cache);
    }

    let connection = database.connection().await?;
    let statement = connection.prepare("SELECT * FROM users WHERE users.user_id = $1").await?;

    let row = connection.query_opt(&statement, &[&user_id.id]).await?;
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

        cache.insert(account.user_id.clone(), account.clone());
        cache.get(&account.user_id).cloned()
    };

    return Ok(account);
}

pub async fn create_account(
    database: &Arc<Database>,
    user_id: &UserId,
    firebase_token: &FirebaseToken,
    valid_until: Option<&DateTime<Utc>>
) -> anyhow::Result<bool> {
    let existing_account = { accounts_cache.read().await.get(user_id).cloned() };
    if existing_account.is_some() {
        warn!("create_account() account with id: {} already exists!", user_id);
        return Ok(false);
    }

    let new_account = Account::new_with_token(user_id.clone(), firebase_token.clone());
    let connection = database.connection().await?;

    let statement = connection
        .prepare("INSERT INTO users(user_id, firebase_token, valid_until) VALUES ($1, $2, $3)")
        .await?;

    connection.execute(
        &statement,
        &[&new_account.user_id.id, &new_account.firebase_token.token, &valid_until]
    ).await?;

    {
        let mut accounts_locked = accounts_cache.write().await;

        let existing_account = accounts_locked.get_mut(user_id);
        if existing_account.is_some() {
            let mut existing_account = existing_account.unwrap();
            existing_account.firebase_token = new_account.firebase_token;
        } else {
            accounts_locked.insert(user_id.clone(), new_account);
        }
    }

    return Ok(true);
}

pub async fn update_account(
    database: &Arc<Database>,
    user_id: &UserId,
    firebase_token: &FirebaseToken,
    valid_until: Option<&DateTime<Utc>>
) -> anyhow::Result<bool> {
    let existing_account = { accounts_cache.read().await.get(user_id).cloned() };
    if existing_account.is_none() {
        warn!("update_account() account with id: {} already exists!", user_id);
        return Ok(false);
    }

    let updated_account = Account::new_with_token(user_id.clone(), firebase_token.clone());
    let connection = database.connection().await?;

    if existing_account.is_none() {
        let statement = connection
            .prepare("INSERT INTO users(user_id, firebase_token, valid_until) VALUES ($1, $2, $3)")
            .await?;

        connection.execute(
            &statement,
            &[&updated_account.user_id.id, &updated_account.firebase_token.token, &valid_until]
        ).await?;
    } else {
        let statement = connection
            .prepare("UPDATED users(user_id, firebase_token, valid_until) VALUES ($1, $2, $3)")
            .await?;

        connection.execute(
            &statement,
            &[&updated_account.user_id.id, &updated_account.firebase_token.token, &valid_until]
        ).await?;
    }

    {
        let mut accounts_locked = accounts_cache.write().await;

        let existing_account = accounts_locked.get_mut(user_id);
        if existing_account.is_some() {
            let mut existing_account = existing_account.unwrap();
            existing_account.firebase_token = updated_account.firebase_token;
        } else {
            accounts_locked.insert(user_id.clone(), updated_account);
        }
    }

    info!(
        "update_account() success. user_id: {}, firebase_token: {}, valid_until: {:?}",
        user_id,
        firebase_token.format_token(),
        valid_until
    );

    return Ok(true);
}