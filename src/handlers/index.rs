use http_body_util::Full;
use hyper::Response;
use hyper::body::{Bytes, Incoming};

pub async fn handle(_query: &str, _: Incoming) -> anyhow::Result<Response<Full<Bytes>>> {
    let response = format!("This is the index page!");

    let response = Response::builder()
        .status(200)
        .body(Full::new(Bytes::from(response)))?;

    return Ok(response)
}