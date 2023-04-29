use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::Serialize;

use crate::{error, info};
use crate::handlers::shared::{ContentType, error_response_str, ServerSuccessResponse, success_response};
use crate::helpers::serde_helpers::serialize_datetime;
use crate::model::database::db::Database;
use crate::model::repository::logs_repository;

#[derive(Serialize)]
struct GetLogsResponse {
    log_lines: Vec<LogLineResponse>
}

#[derive(Serialize)]
struct LogLineResponse {
    id: i64,
    #[serde(serialize_with = "serialize_datetime")]
    log_time: DateTime<Utc>,
    log_level: String,
    target: String,
    message: String
}

impl ServerSuccessResponse for GetLogsResponse {

}

pub async fn handle(
    query: &str,
    _: Incoming,
    database: &Arc<Database>
) -> anyhow::Result<Response<Full<Bytes>>> {
    let params = query
        .split('&')
        .take(2)
        .filter_map(|parameter| {
            let key_value = parameter.split('=').take(2).collect::<Vec<&str>>();

            let key = *key_value.get(0).unwrap_or(&"");
            let value = *key_value.get(1).unwrap_or(&"");

            if key.is_empty() || value.is_empty() {
                return None;
            }

            return Some((key, value));
        })
        .collect::<HashMap<&str, &str>>();

    let num_str = params.get("num").unwrap_or(&"");
    let last_id_str = params.get("last_id").unwrap_or(&"");

    if num_str.is_empty() {
        error!("get_logs() Num parameter not found");

        let response_json = error_response_str("Num parameter not found")?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let num = i64::from_str(num_str);
    if num.is_err() {
        let error_message = format!("Failed to convert num \'{}\' to number", num_str);
        error!("get_logs() {}", error_message);

        let response_json = error_response_str(&error_message)?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let num = num.unwrap();
    let last_id = i64::from_str(last_id_str).unwrap_or(i64::MAX);

    let log_lines = logs_repository::get_logs(num, last_id, database).await?;

    let log_lines_response = log_lines.iter().map(|log_line| {
        return LogLineResponse {
            id: log_line.id,
            log_time: log_line.log_time.clone(),
            log_level: log_line.log_level.clone(),
            target: log_line.target.clone(),
            message: log_line.message.clone(),
        }
    }).collect::<Vec<LogLineResponse>>();

    let get_logs_response = GetLogsResponse {
        log_lines: log_lines_response
    };

    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(success_response(get_logs_response)?)))?;

    info!("get_logs() Success");
    return Ok(response);
}