use std::sync::Arc;
use anyhow::Context;
use chrono::{DateTime, Utc};
use http_body_util::{BodyExt, Full};
use hyper::{Response};
use hyper::body::{Bytes, Incoming};
use serde::{Deserialize, Serialize};
use crate::handlers::shared::{ContentType, error_response, success_response};
use crate::model::database::db::Database;
use crate::model::repository::account_repository;
use crate::model::repository::account_repository::AccountId;
use crate::helpers::serde_helpers::serialize_datetime;

#[derive(Deserialize)]
struct AccountInfoRequest {
    email: String
}

#[derive(Serialize)]
struct AccountInfoResponse {
    is_valid: bool,
    #[serde(serialize_with = "serialize_datetime")]
    valid_until: Option<DateTime<Utc>>
}

pub async fn handle(
    query: &str,
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

    let account_id = AccountId::from_str(&request.email)?;

    let account = account_repository::get_account(database, &account_id)
        .await
        .context(format!("Failed to get account from repository with account_id \'{}\'", account_id))?;

    if account.is_none() {
        let response_json = error_response("Account does not exist")?;
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