use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::Full;
use hyper::{Request, Response};
use hyper::body::Bytes;

use crate::{error, handlers, info};
use crate::handlers::shared::ContentType;
use crate::helpers::throttler;
use crate::model::database::db::Database;
use crate::model::repository::site_repository::SiteRepository;

pub struct TestContext {
    pub enable_throttler: bool
}

pub async fn router(
    test_context: Option<TestContext>,
    master_password: &String,
    sock_addr: &SocketAddr,
    request: Request<hyper::body::Incoming>,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>,
) -> anyhow::Result<Response<Full<Bytes>>> {
    let remote_address = sock_addr.to_string();
    let (parts, body) = request.into_parts();

    let master_password_from_request = parts.headers.get("X-Master-Password")
        .map(|header_value| header_value.to_str().unwrap_or(""))
        .unwrap_or("");

    let path_and_query = parts.uri.path_and_query();
    if path_and_query.is_none() {
        error!("router() path_and_query not found");

        let error_message = "path_and_query not found";
        let response_json = handlers::shared::error_response_str(error_message)?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let path_and_query = path_and_query.unwrap();
    let mut path = path_and_query.path();

    info!("router() New request to \'{}\' from \'{}\'", path, remote_address);

    let can_proceed = throttler::can_proceed(test_context, path.to_string(), &remote_address).await?;
    if !can_proceed {
        info!("router() Client {} has been throttled", remote_address);

        let error_message = "You are making too many requests, please wait a little bit.";
        let response_json = handlers::shared::error_response_str(error_message)?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    }

    let start = chrono::offset::Utc::now();
    let query = path_and_query.query().unwrap_or("");

    match path {
        "/get_logs" |
        "/create_account" |
        "/update_account_expiry_date" => {
            if master_password != master_password_from_request {
                info!(
                    "router() Client {} sent incorrect master password: \'{}\'",
                    remote_address,
                    master_password_from_request
                );

                let error_message = "Incorrect master password";
                let response_json = handlers::shared::error_response_str(error_message)?;
                let response = Response::builder()
                    .json()
                    .status(403)
                    .body(Full::new(Bytes::from(response_json)))?;

                return Ok(response);
            }
        },
        _ => {
            // no-op
        }
    };

    // Do not forget to update throttler as well when changing paths here.
    let handler_result = match path {
        "/create_account" => {
            handlers::create_account::handle(query, body, database).await
        },
        "/update_account_expiry_date" => {
            handlers::update_account_expiry_date::handle(query, body, database).await
        },
        "/update_firebase_token" => {
            handlers::update_firebase_token::handle(query, body, database).await
        },
        "/update_message_delivered" => {
            handlers::update_message_delivered::handle(query, body, database, site_repository).await
        }
        "/get_account_info" => {
            handlers::get_account_info::handle(query, body, database).await
        },
        "/get_logs" => {
            handlers::get_logs::handle(query, body, database).await
        }
        "/watch_post" => {
            handlers::watch_post::handle(query, body, database, site_repository).await
        },
        "/unwatch_post" => {
            handlers::unwatch_post::handle(query, body, database, site_repository).await
        },
        _ => {
            handlers::index::handle(query, body).await
        }
    };

    let delta = chrono::offset::Utc::now() - start;

    if handler_result.is_err() {
        let handler_error = handler_result
            .as_ref()
            .err();

        let handler_error_message = handler_error
            .map(|err| err.to_string())
            .unwrap_or(String::from("Unknown error"));

        error!("router() Request to {} error: {:?}", path, handler_error);

        let response_json = handlers::shared::error_response_string(&handler_error_message)?;
        let response = Response::builder()
            .json()
            .status(200)
            .body(Full::new(Bytes::from(response_json)))?;

        return Ok(response);
    } else {
        info!(
            "router() Request to \'{}\' from \'{}\' success, took {} ms",
            path,
            remote_address,
            delta.num_milliseconds()
        );
    }

    return handler_result
}
