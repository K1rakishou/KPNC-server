use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::Response;

pub async fn handle(_query: &str, _: Incoming) -> anyhow::Result<Response<Full<Bytes>>> {
    let response = format!("Yep, this is the index page!");

    let response = Response::builder()
        .status(200)
        .body(Full::new(Bytes::from(response)))?;

    return Ok(response)
}