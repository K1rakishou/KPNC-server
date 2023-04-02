use http_body_util::Full;
use hyper::{Request, Response};
use hyper::body::{Bytes, Incoming};

pub async fn handle(query: &str, _: Incoming) -> anyhow::Result<Response<Full<Bytes>>> {
    let firebase_api_key = std::env::var("FIREBASE_API_KEY")?;
    let response = format!("query='{}', firebase_api_key='{}'", query, firebase_api_key);

    return Ok(Response::new(Full::new(Bytes::from(response))))
}