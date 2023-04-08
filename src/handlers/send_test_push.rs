use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use crate::handlers::shared::{ContentType, empty_success_response, error_response};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{get_account, AccountId};
use crate::helpers::string_helpers::FormatToken;

lazy_static! {
    static ref client: fcm::Client = fcm::Client::new();
}

#[derive(Serialize, Deserialize)]
struct SendTestPushRequest {
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

    let request: SendTestPushRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into SendTestPushRequest")?;

    let firebase_api_key = std::env::var("FIREBASE_API_KEY")
        .context("Failed to read firebase api key from Environment")?;

    let account_id = AccountId::from_str(&request.email);

    let account = get_account(&database, &account_id)
        .await?;

    if account.is_none() {
        let response_json = error_response("Account not found for this account_id")?;

        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let account = account.unwrap();
    let firebase_token = account.firebase_token();

    info!(
        "send_test_push() new request, account_id=\'{}\', firebase_token=\'{}\'",
        account_id.clone(),
        firebase_token.clone().format_token()
    );

    let mut map = HashMap::new();
    map.insert("message_body", "Test push message");

    let mut builder = fcm::MessageBuilder::new(firebase_api_key.as_str(), firebase_token.token.as_str());
    builder.data(&map)?;

    let response = client.send(builder.finalize()).await?;
    let error = response.error;

    if error.is_some() {
        let response_json = error_response("Failed to send push message")?;
        error!("send_test_push() error: {:?}", error.unwrap());

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
        "send_test_push() for \'{}\' with token \'{}\' success",
        account_id,
        firebase_token.clone().format_token()
    );

    return Result::Ok(response);
}