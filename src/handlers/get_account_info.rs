use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Utc};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::{Deserialize, Serialize};

use crate::{error, info};
use crate::handlers::shared::{ContentType, error_response_str, error_response_string, ServerSuccessResponse, success_response};
use crate::helpers::serde_helpers::{deserialize_datetime, serialize_datetime_option};
use crate::helpers::serde_helpers::{deserialize_application_type, serialize_application_type};
use crate::helpers::string_helpers::FormatToken;
use crate::model::database::db::Database;
use crate::model::repository::account_repository;
use crate::model::repository::account_repository::{AccountId, ApplicationType};

#[derive(Serialize, Deserialize)]
pub struct AccountInfoRequest {
    pub user_id: String,
    #[serde(
        serialize_with = "serialize_application_type",
        deserialize_with = "deserialize_application_type"
    )]
    pub application_type: ApplicationType,
}

#[derive(Serialize, Deserialize)]
pub struct AccountInfoResponse {
    pub account_id: String,
    pub is_valid: bool,
    #[serde(
        serialize_with = "serialize_datetime_option",
        deserialize_with = "deserialize_datetime"
    )]
    pub valid_until: Option<DateTime<Utc>>
}

impl ServerSuccessResponse for AccountInfoResponse {
    
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

    let request: AccountInfoRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into AccountInfoRequest")?;

    let application_type = request.application_type;
    if application_type == ApplicationType::Unknown {
        let error_message = format!(
            "Unsupported \'application_type\' parameter value: {}",
            application_type as isize
        );

        error!("get_account_info() {}", error_message);

        let response_json = error_response_string(&error_message)?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let account_id = AccountId::from_user_id(&request.user_id)?;

    let account = account_repository::get_account(&account_id, database)
        .await
        .with_context(|| {
            return format!(
                "get_account_info() Failed to get account from repository with account_id \'{}\'",
                account_id.format_token()
            );
        })?;

    if account.is_none() {
        error!(
            "get_account_info() Account with id \'{}\' does not exist",
            account_id.format_token()
        );

        let response_json = error_response_str("Account does not exist")?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let account = account.unwrap();

    let account_info_response = {
        let acc = account.lock().await;

        AccountInfoResponse {
            account_id: acc.account_id.id.clone(),
            is_valid: acc.is_valid(&application_type),
            valid_until: acc.valid_until
        }
    };

    let response_json = success_response(account_info_response)?;
    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(response_json)))?;

    info!("get_account_info() Success \'{}\'", account_id.format_token());
    return Ok(response);
}