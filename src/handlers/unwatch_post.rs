use std::sync::Arc;

use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::{Deserialize, Serialize};

use crate::{error, info};
use crate::handlers::shared::{ContentType, empty_success_response, error_response_str, error_response_string, validate_post_url};
use crate::helpers::serde_helpers::{deserialize_application_type, serialize_application_type};
use crate::helpers::string_helpers::FormatToken;
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{AccountId, ApplicationType};
use crate::model::repository::post_repository;
use crate::model::repository::post_repository::StopWatchingPostResult;
use crate::model::repository::site_repository::SiteRepository;

#[derive(Serialize, Deserialize)]
pub struct UnwatchPostRequest {
    pub user_id: String,
    pub post_url: String,
    #[serde(
        serialize_with = "serialize_application_type",
        deserialize_with = "deserialize_application_type"
    )]
    pub application_type: ApplicationType,
}

pub async fn handle(
    _query: &str,
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

    let request: UnwatchPostRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into UnwatchPostRequest")?;

    let application_type = request.application_type;
    if application_type == ApplicationType::Unknown {
        let error_message = format!(
            "Unsupported \'application_type\' parameter value: {}",
            application_type as isize
        );

        error!("unwatch_post() {}", error_message);

        let response_json = error_response_string(&error_message)?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let account_id = AccountId::from_user_id(&request.user_id)?;
    let post_url = validate_post_url(&request.post_url)?;

    let imageboard = site_repository.by_url(post_url);
    if imageboard.is_none() {
        let full_error_message = format!("Site for url \'{}\' is not supported", post_url);

        let response_json = error_response_string(&full_error_message)?;
        error!("unwatch_post() {}", full_error_message);

        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let imageboard = imageboard.unwrap();

    let post_descriptor = imageboard.post_url_to_post_descriptor(post_url);
    if post_descriptor.is_none() {
        let full_error_message = format!("Failed to parse \'{}\' url as post url", post_url);

        let response_json = error_response_string(&full_error_message)?;
        error!("unwatch_post() {}", full_error_message);

        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let post_descriptor = post_descriptor.unwrap();
    info!("unwatch_post() post_descriptor: {}", post_descriptor);

    let post_watch_deleted_result = post_repository::stop_watching_post(
        database,
        &account_id,
        &application_type,
        &post_descriptor
    ).await.context(format!("Failed to unwatch post {}", post_descriptor))?;

    if post_watch_deleted_result != StopWatchingPostResult::Ok {
        let error_message = match post_watch_deleted_result {
            StopWatchingPostResult::Ok => unreachable!(),
            StopWatchingPostResult::AccountDoesNotExist => "Account does not exist",
            StopWatchingPostResult::AccountIsNotValid => "Account already expired",
        };

        let response_json = error_response_str(error_message)?;

        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        info!(
            "Failed to unwatch post {} for account {}, result: {:?}",
            post_descriptor,
            account_id,
            post_watch_deleted_result
        );

        return Ok(response);
    }

    let response_json = empty_success_response()?;

    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(response_json)))?;

    info!(
        "Post watch for post {} and account id {} was successfully deleted",
        post_descriptor,
        account_id.format_token()
    );

    return Ok(response);
}