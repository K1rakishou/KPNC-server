use std::net::SocketAddr;
use std::sync::{Arc};
use http_body_util::Full;
use hyper::{Request, Response};
use hyper::body::Bytes;
use crate::model::database::db::Database;
use crate::handlers;
use crate::model::repository::site_repository::SiteRepository;

pub async fn router(
    sock_addr: &SocketAddr,
    request: Request<hyper::body::Incoming>,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>,
) -> anyhow::Result<Response<Full<Bytes>>> {
    let remote_address = sock_addr.to_string();
    let (parts, body) = request.into_parts();

    let path_and_query = parts.uri.path_and_query();
    if path_and_query.is_none() {
        return Ok(Response::new(Full::new(Bytes::from("path_and_query not found"))))
    }

    let path_and_query = path_and_query.unwrap();
    let mut path = path_and_query.path();
    if path.starts_with('/') {
        path = &path[1..];
    }

    let start = chrono::offset::Utc::now();

    info!("New request to \'{}\' from \'{}\'", path, remote_address);
    let query = path_and_query.query().unwrap_or("");

    let handler_result = match path {
        "create_account" => handlers::create_account::handle(query, body, database).await,
        "update_firebase_token" => handlers::update_firebase_token::handle(query, body, database).await,
        "get_account_info" => handlers::get_account_info::handle(query, body, database).await,
        "watch_post" => handlers::watch_post::handle(query, body, database, site_repository).await,
        _ => handlers::index::handle(query, body).await
    };

    let delta = chrono::offset::Utc::now() - start;

    if handler_result.is_err() {
        let handler_error = handler_result
            .as_ref()
            .err();

        let handler_error_message = handler_error
            .map(|err| err.to_string())
            .unwrap_or(String::from("Unknown error"));

        log::error!("Request to {} error: {:?}", path, handler_error);

        let error_message = format!("Failed to process request, error: '{}'", handler_error_message);
        let response = Response::new(Full::new(Bytes::from(error_message)));
        return Ok(response);
    } else {
        info!(
            "Request to \'{}\' from \'{}\' success, took {} ms",
            path,
            remote_address,
            delta.num_milliseconds()
        );
    }

    return handler_result
}
