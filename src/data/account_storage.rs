use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use lazy_static::lazy_static;

lazy_static! {
    static ref accounts: RwLock<HashMap<String, Account>> = RwLock::new(HashMap::with_capacity(1024));
}

struct Account {
    email: String,
    firebase_token: String,
    valid_until: Option<DateTime<Utc>>
}

impl Account {
    pub fn new_with_token(email: String, firebase_token: String) -> Account {
        return Account {
            email: email,
            firebase_token,
            valid_until: Option::None
        }
    }
}

pub async fn get_firebase_token(email: &String) -> Option<String> {
    return accounts.read()
        .await
        .get(email)
        .map(|account| account.firebase_token.clone());
}

pub async fn update_account_token(email: &String, token: String) -> anyhow::Result<()> {
    let mut accounts_locked = accounts.write().await;

    let existing_account = accounts_locked.get_mut(email);
    if existing_account.is_some() {
        let mut existing_account = existing_account.unwrap();
        existing_account.firebase_token = token;
    } else {
        accounts_locked.insert(email.clone(), Account::new_with_token(email.clone(), token));
    }

    return Ok(());
}