use std::sync::Arc;

use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::{Deserialize, Serialize};

use crate::{error, info};
use crate::handlers::shared::{ContentType, empty_success_response, error_response_str};
use crate::helpers::string_helpers::FormatToken;
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{AccountId, CreateAccountResult};
use crate::model::repository::account_repository;

#[derive(Serialize, Deserialize)]
pub struct CreateNewAccountRequest {
    pub user_id: String,
    pub valid_for_days: u64
}

pub async fn handle(
    _query: &str,
    body: Incoming,
    database: &Arc<Database>
) -> anyhow::Result<Response<Full<Bytes>>> {
    let body_bytes = body.collect()
        .await
        .context("Failed to collect body")?
        .to_bytes();

    let body_as_string = String::from_utf8(body_bytes.to_vec())
        .context("Failed to convert body into a string")?;

    let request: CreateNewAccountRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into CreateNewAccountRequest")?;

    let account_id = AccountId::from_user_id(&request.user_id)?;
    let valid_for_days = request.valid_for_days as i64;

    if valid_for_days <= 0 || valid_for_days > 365 {
        error!("create_account() bad valid_for_days: {}", valid_for_days);

        let response_json = error_response_str("valid_for_days must be in range 0..365")?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let valid_until = chrono::offset::Utc::now() + chrono::Duration::days(valid_for_days);

    let result = account_repository::create_account(database, &account_id, Some(valid_until))
        .await
        .context(format!("Failed to created account for account with account_id: \'{}\'", account_id))?;

    if result != CreateAccountResult::Ok {
        let error_message = match result {
            CreateAccountResult::Ok => unreachable!(),
            CreateAccountResult::AccountAlreadyExists => "Account already exists"
        };

        let full_error_message = format!(
            "Failed to create a new account for account_id \'{}\': \"{}\"",
            account_id,
            error_message
        );

        error!("create_account() {}", full_error_message);

        let response_json = error_response_str("Account already exists")?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let response_json = empty_success_response()?;

    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(response_json)))?;

    info!(
        "create_account() Successfully created new account. account_id: \'{}\', valid_until: {:?}",
        account_id.format_token(),
        valid_until
    );

    return Ok(response);
}