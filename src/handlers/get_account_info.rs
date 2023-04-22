use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Utc};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::{Deserialize, Serialize};

use crate::handlers::shared::{ContentType, error_response_str, ServerSuccessResponse, success_response};
use crate::helpers::serde_helpers::{deserialize_datetime, serialize_datetime};
use crate::model::database::db::Database;
use crate::model::repository::account_repository;
use crate::model::repository::account_repository::AccountId;

#[derive(Serialize, Deserialize)]
pub struct AccountInfoRequest {
    pub user_id: String
}

#[derive(Serialize, Deserialize)]
pub struct AccountInfoResponse {
    pub is_valid: bool,
    #[serde(serialize_with = "serialize_datetime", deserialize_with = "deserialize_datetime")]
    pub valid_until: Option<DateTime<Utc>>
}

impl ServerSuccessResponse for AccountInfoResponse {
    
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

    let request: AccountInfoRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into AccountInfoRequest")?;

    let account_id = AccountId::from_user_id(&request.user_id)?;

    let account = account_repository::get_account(&account_id, database)
        .await
        .context(format!("Failed to get account from repository with account_id \'{}\'", account_id))?;

    if account.is_none() {
        let response_json = error_response_str("Account does not exist")?;
        error!("get_account_info() Account with id \'{}\' does not exist", account_id);

        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let account = account.unwrap();

    let account_info_response = AccountInfoResponse {
        is_valid: is_account_valid(&account.valid_until),
        valid_until: account.valid_until
    };

    let response_json = success_response(account_info_response)?;
    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(response_json)))?;

    return Ok(response);
}

fn is_account_valid(valid_until: &Option<DateTime<Utc>>) -> bool {
    if valid_until.is_none() {
        return false;
    }

    let valid_until = valid_until.unwrap();
    return valid_until > chrono::offset::Utc::now();
}