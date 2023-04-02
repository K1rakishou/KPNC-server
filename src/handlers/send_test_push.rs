use std::collections::HashMap;
use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use tokio::sync::Mutex;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use crate::data::account_storage::get_firebase_token;

lazy_static! {
    static ref client: fcm::Client = fcm::Client::new();
}

#[derive(Serialize, Deserialize)]
struct SendTestPushRequest {
    email: String
}

pub async fn handle(query: &str, body: Incoming) -> anyhow::Result<Response<Full<Bytes>>> {
    let body_bytes = body.collect()
        .await
        .context("Failed to collect body")?
        .to_bytes();

    let body_as_string = String::from_utf8(body_bytes.to_vec())
        .context("Failed to convert body into a string")?;

    let request: SendTestPushRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into SendTestPushRequest")?;

    let firebase_api_key = std::env::var("FIREBASE_API_KEY")
        .context("Failed to read firebase api key")?;

    let email = request.email;
    info!("send_test_push() new request, email={}", email.clone());

    let firebase_token = get_firebase_token(&email).await;
    if firebase_token.is_none() {
        let response = Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Account not found for email")))?;

        return Result::Ok(response);
    }

    let firebase_token = firebase_token.unwrap();

    let mut map = HashMap::new();
    map.insert("message_body", "Test push message");

    let mut builder = fcm::MessageBuilder::new(firebase_api_key.as_str(), firebase_token.as_str());
    builder.data(&map)?;

    let response = client.send(builder.finalize()).await?;
    let error = response.error;

    if error.is_some() {
        error!("send_test_push() error: {:?}", error.unwrap());

        let response = Response::builder()
            .status(500)
            .body(Full::new(Bytes::from("Failed to send push message")))?;

        return Result::Ok(response);
    }

    info!("send_test_push() success");

    let response = Response::builder()
        .status(200)
        .body(Full::new(Bytes::from("Successfully sent push message")))?;

    return Result::Ok(response);
}