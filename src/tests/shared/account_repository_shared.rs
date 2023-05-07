use std::sync::Arc;

use fcm::Duration;
use lazy_static::lazy_static;
use serde::de::DeserializeOwned;

use crate::handlers::create_account::CreateNewAccountRequest;
use crate::handlers::get_account_info::AccountInfoRequest;
use crate::handlers::shared::{EmptyResponse, ServerResponse, ServerSuccessResponse};
use crate::handlers::update_firebase_token::UpdateFirebaseTokenRequest;
use crate::model::database::db::Database;
use crate::model::repository::account_repository;
use crate::model::repository::account_repository::{Account, AccountId, ApplicationType};
use crate::tests::shared::{account_repository_shared, database_shared, http_client_shared};

lazy_static! {
    pub static ref TEST_BAD_USER_ID1: String = String::from("1111111111111111111111111111111");
    pub static ref TEST_BAD_USER_ID2: String = String::from("111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111");

    pub static ref TEST_GOOD_USER_ID1: String = String::from("11111111111111111111111111111111111");
    pub static ref TEST_GOOD_USER_ID2: String = String::from("22222222222222222222222222222222222");

    pub static ref TEST_VERY_SHORT_FIREBASE_TOKEN: String = String::from("");
    pub static ref TEST_VERY_LONG_FIREBASE_TOKEN: String = String::from("22222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222");
}

pub async fn create_account<'a, T : DeserializeOwned + ServerSuccessResponse>(
    user_id: &str,
    valid_for_days: u64
) -> anyhow::Result<ServerResponse<T>> {
    let request = CreateNewAccountRequest {
        user_id: user_id.to_string(),
        valid_for_days
    };

    let body = serde_json::to_string(&request).unwrap();

    let response = http_client_shared::post_request::<ServerResponse<T>>(
        "create_account",
        &body
    ).await?;

    return Ok(response);
}

pub async fn create_expired_account<'a, T : DeserializeOwned + ServerSuccessResponse>(
    user_id: &str,
    valid_for_days: u64
) -> anyhow::Result<()> {
    let _ = create_account::<T>(user_id, valid_for_days).await?;
    let account_id = AccountId::test_unsafe(user_id)?;

    let database = database_shared::database();

    let account_mutex = account_repository::test_get_account_from_cache(&account_id)
        .await
        .unwrap();
    let mut account = account_mutex.lock().await;

    let new_valid_until = account.valid_until.unwrap() - Duration::days(3);
    account.valid_until = Some(new_valid_until);

    {
        // Update valid_until in the cache
        account_repository::test_put_account_into_cache(&account).await;
    }

    {
        // Update valid_until in the database
        account_repository::test_put_account_into_database(&account, database).await.unwrap();
    }

    return Ok(());
}

pub async fn get_account_info<'a, T : DeserializeOwned + ServerSuccessResponse>(
    user_id: &str,
    application_type: &ApplicationType
) -> anyhow::Result<ServerResponse<T>> {
    let request = AccountInfoRequest {
        user_id: user_id.to_string(),
        application_type: application_type.clone()
    };

    let body = serde_json::to_string(&request).unwrap();

    let response = http_client_shared::post_request::<ServerResponse<T>>(
        "get_account_info",
        &body
    ).await?;

    return Ok(response);
}

pub async fn update_firebase_token<'a, T : DeserializeOwned + ServerSuccessResponse>(
    user_id: &str,
    firebase_token: &str,
    application_type: &ApplicationType
) -> anyhow::Result<ServerResponse<T>> {
    let request = UpdateFirebaseTokenRequest {
        user_id: user_id.to_string(),
        firebase_token: firebase_token.to_string(),
        application_type: application_type.clone()
    };

    let body = serde_json::to_string(&request).unwrap();

    let response = http_client_shared::post_request::<ServerResponse<T>>(
        "update_firebase_token",
        &body
    ).await?;

    return Ok(response);
}

pub async fn get_account_from_cache(user_id: &str) -> anyhow::Result<Option<Account>> {
    let account_id = AccountId::test_unsafe(user_id)?;

    let account = account_repository::test_get_account_from_cache(&account_id).await;
    if account.is_none() {
        return Ok(None);
    }

    let account = account.unwrap().lock().await.clone();
    return Ok(Some(account));
}

pub async fn get_account_from_database(
    user_id: &str,
    database: &Arc<Database>
) -> anyhow::Result<Option<Account>> {
    let account_id = AccountId::test_unsafe(user_id)?;
    let account = account_repository::test_get_account_from_database(&account_id, database).await?;
    return Ok(account)
}

pub async fn create_account_actual(user_id: &String) {
    let server_response = account_repository_shared::create_account::<EmptyResponse>(
        user_id,
        1
    ).await.unwrap();

    assert!(server_response.data.is_some());
    assert!(server_response.error.is_none());
}