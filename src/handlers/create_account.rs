use std::sync::Arc;
use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::{Response};
use hyper::body::{Bytes, Incoming};
use serde::{Deserialize};
use crate::handlers::shared::{ContentType, empty_success_response, error_response, error_response_string};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{CreateAccountResult, FirebaseToken, AccountId};
use crate::model::repository::account_repository;

#[derive(Deserialize)]
struct CreateNewAccountRequest {
    email: String
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

    let request: CreateNewAccountRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into CreateNewAccountRequest")?;

    let account_id = AccountId::from_email(&request.email)?;
    let valid_until = chrono::offset::Utc::now() + chrono::Duration::days(180);

    // TODO: only allow creating new accounts for requests with special header
    let result = account_repository::create_account(database, &account_id, Some(&valid_until))
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

        let response_json = error_response("Account already exists")?;
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
        "Successfully created new account. account_id: \'{}\', valid_until: {:?}",
        account_id,
        valid_until
    );

    return Ok(response);
}