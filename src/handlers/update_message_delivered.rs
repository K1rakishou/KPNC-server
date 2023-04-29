use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::{Deserialize, Serialize};

use crate::{error, info};
use crate::handlers::shared::{ContentType, empty_success_response, error_response_string, validate_post_url};
use crate::helpers::string_helpers::FormatToken;
use crate::model::database::db::Database;
use crate::model::repository::post_watch_repository;
use crate::model::repository::account_repository::AccountId;
use crate::model::repository::site_repository::SiteRepository;

const MAX_REPLY_IDS_PER_REQUEST_COUNT: usize = 8192;

#[derive(Serialize, Deserialize)]
pub struct MessageDelivered {
    pub user_id: String,
    pub reply_ids: Vec<u64>
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

    let request: MessageDelivered = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into MessageDelivered")?;

    let account_id = AccountId::from_user_id(&request.user_id)?;
    let reply_ids = request.reply_ids
        .into_iter()
        .collect::<HashSet<u64>>()
        .into_iter()
        .take(MAX_REPLY_IDS_PER_REQUEST_COUNT)
        .collect::<Vec<u64>>();

    if reply_ids.is_empty() {
        error!("update_message_delivered() reply_ids is empty");

        let response_json = empty_success_response()?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    post_watch_repository::mark_post_replies_as_notified(&account_id, &reply_ids, &database)
        .await
        .context("update_message_delivered() Failed to mark messages as sent")?;

    let response_json = empty_success_response()?;

    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(response_json)))?;

    info!(
        "update_message_delivered() Marked as delivered {} post replies for account id {}",
        reply_ids.len(),
        account_id.format_token()
    );

    return Ok(response);
}