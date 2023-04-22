use std::sync::Arc;

use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::Deserialize;
use serde::Serialize;

use crate::handlers::shared::{ContentType, empty_success_response, error_response_str, error_response_string};
use crate::helpers::string_helpers::FormatToken;
use crate::model::database::db::Database;
use crate::model::repository::account_repository;
use crate::model::repository::account_repository::{AccountId, FirebaseToken, UpdateFirebaseTokenResult};

#[derive(Serialize, Deserialize)]
pub struct UpdateFirebaseTokenRequest {
    pub user_id: String,
    pub firebase_token: String
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

    let request: UpdateFirebaseTokenRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into UpdateFirebaseTokenRequest")?;

    let account_id = AccountId::from_user_id(&request.user_id)?;
    let firebase_token = FirebaseToken::from_str(&request.firebase_token)?;

    let result = account_repository::update_firebase_token(database, &account_id, &firebase_token)
        .await
        .context(format!("Failed to update firebase token for account with id \'{}\'", account_id))?;

    if result != UpdateFirebaseTokenResult::Ok {
        let error_message = match result {
            UpdateFirebaseTokenResult::Ok => unreachable!(),
            UpdateFirebaseTokenResult::AccountDoesNotExist => "Account does not exist"
        };

        let full_error_message = format!(
            "Failed to update firebase token for account for account_id \'{}\': \"{}\"",
            account_id,
            error_message
        );

        error!("update_firebase_token() {}", full_error_message);

        let response_json = error_response_str(error_message)?;
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
        "update_firebase_token() Successfully updated firebase_token. account_id: \'{}\', firebase_token: \'{}\'",
        account_id.format_token(),
        firebase_token.format_token()
    );

    return Ok(response);
}