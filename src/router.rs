use http_body_util::Full;
use hyper::{Request, Response};
use hyper::body::Bytes;
use crate::handlers;

pub async fn router(request: Request<hyper::body::Incoming>) -> anyhow::Result<Response<Full<Bytes>>> {
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

    let query = path_and_query.query().unwrap_or("");

    let handler_result = match path {
        "update_firebase_token" => handlers::update_firebase_token::handle(query, body).await,
        "send_test_push" => handlers::send_test_push::handle(query, body).await,
        _ => handlers::index::handle(query, body).await
    };

    if handler_result.is_err() {
        let handler_error = handler_result
            .as_ref()
            .err();

        let handler_error_message = handler_error
            .map(|err| err.to_string())
            .unwrap_or(String::from("Unknown error"));

        log::error!("Error: {:?}", handler_error);

        let error_message = format!("Failed to process request, error: '{}'", handler_error_message);
        let response = Response::new(Full::new(Bytes::from(error_message)));
        return Ok(response);
    }

    return handler_result
}
