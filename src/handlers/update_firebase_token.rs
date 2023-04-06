use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::{Response};
use hyper::body::{Bytes, Incoming};
use serde::{Deserialize, Serialize};
use crate::model::repository::account_repository::update_account_token;

#[derive(Serialize, Deserialize)]
struct UpdateFirebaseTokenRequest {
    email: String,
    token: String
}

pub async fn handle(query: &str, body: Incoming) -> anyhow::Result<Response<Full<Bytes>>> {
    let body_bytes = body.collect()
        .await
        .context("Failed to collect body")?
        .to_bytes();

    let body_as_string = String::from_utf8(body_bytes.to_vec())
        .context("Failed to convert body into a string")?;

    let request: UpdateFirebaseTokenRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into UpdateFirebaseTokenRequest")?;

    let email = request.email;
    let token = request.token;

    update_account_token(&email, token.clone())
        .await
        .context(format!("Failed to update account token for account with email: {}", email))?;

    log::debug!("Updated token account '{}'", email.clone());

    let response_text = format!("email='{}', token='{}'", email, token);

    let response = Response::builder()
        .status(200)
        .body(Full::new(Bytes::from(response_text)))?;

    return Ok(response);
}