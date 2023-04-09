use std::sync::Arc;
use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::{Response};
use hyper::body::{Bytes, Incoming};
use serde::{Deserialize};
use crate::handlers::shared::{ContentType, empty_success_response, error_response_string, validate_post_url};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{AccountId};
use crate::model::repository::posts_repository;
use crate::model::repository::site_repository::SiteRepository;

#[derive(Deserialize)]
struct WatchPostRequest {
    email: String,
    post_url: String
}

pub async fn handle(
    query: &str,
    body: Incoming,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>
) -> anyhow::Result<Response<Full<Bytes>>> {
    let body_bytes = body.collect()
        .await
        .context("Failed to collect body")?
        .to_bytes();

    let body_as_string = String::from_utf8(body_bytes.to_vec())
        .context("Failed to convert body into a string")?;

    let request: WatchPostRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into WatchPostRequest")?;

    let account_id = AccountId::from_str(&request.email)?;
    let post_url = validate_post_url(&request.post_url)?;

    let imageboard = site_repository.by_url(post_url);
    if imageboard.is_none() {
        let full_error_message = format!("Site for url \'{}\' is not supported", post_url);

        let response_json = error_response_string(&full_error_message)?;
        error!("watch_post() {}", full_error_message);

        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let imageboard = imageboard.unwrap();

    let post_descriptor = imageboard.post_url_to_post_descriptor(post_url);
    if post_descriptor.is_none() {
        let full_error_message = format!("Failed to parse \'{}\' url", post_url);

        let response_json = error_response_string(&full_error_message)?;
        error!("watch_post() {}", full_error_message);

        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let post_descriptor = post_descriptor.unwrap();

    let post_watch_created = posts_repository::start_watching_post(
        database,
        &account_id,
        &post_descriptor
    ).await.context(format!("Failed to start watching post {}", post_descriptor))?;

    let response_json = empty_success_response()?;

    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(response_json)))?;

    if post_watch_created {
        info!(
            "Successfully started watching post {} for account {}",
            post_descriptor,
            account_id
        );
    } else {
        info!(
            "Post watch for post {} and account id {} was not created because it already exists",
            post_descriptor,
            account_id
        );
    }

    return Ok(response);
}