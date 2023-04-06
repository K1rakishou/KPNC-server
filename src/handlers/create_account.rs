use std::sync::Arc;
use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::{Response};
use hyper::body::{Bytes, Incoming};
use serde::{Deserialize, Serialize};
use crate::handlers::shared::{empty_success_response, error_response};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{create_account, FirebaseToken, UserId};
use crate::helpers::string_helpers::FormatToken;

#[derive(Serialize, Deserialize)]
struct UpdateFirebaseTokenRequest {
    user_id: String,
    firebase_token: String
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

    let request: UpdateFirebaseTokenRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into UpdateFirebaseTokenRequest")?;

    let user_id = UserId::from_str(&request.user_id);
    let firebase_token = FirebaseToken::from_str(&request.firebase_token);
    let valid_until = chrono::offset::Utc::now() + chrono::Duration::days(180);

    let created = create_account(database, &user_id, &firebase_token, Some(&valid_until))
        .await
        .context(format!("Failed to created account for account with user_id: {}", user_id))?;

    if !created {
        let response_json = error_response("Account with this user id already exists")
            .context("Failed to serialize UpdateFirebaseTokenResponse to json")?;

        let response = Response::builder()
            .status(400)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let response_json = empty_success_response()?;

    let response = Response::builder()
        .status(200)
        .body(Full::new(Bytes::from(response_json)))?;

    info!(
        "Successfully created new account. user_id: {}, firebase_token: {}, valid_until: {:?}",
        user_id,
        firebase_token.format_token(),
        valid_until
    );

    return Ok(response);
}