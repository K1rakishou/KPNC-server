use std::convert::Infallible;
use std::net::SocketAddr;
use std::ops::Deref;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use log::LevelFilter;
use tokio::net::TcpListener;
use crate::router::router;

#[macro_use]
extern crate log;

mod router;
mod handlers;
mod data;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::init();

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, sock_addr) = listener.accept().await?;

        tokio::task::spawn(async move {
            let result = http1::Builder::new()
                .serve_connection(stream, service_fn(router))
                .await;

            if result.is_err() {
                let error = result.unwrap();
                error!(
                    "Failed to process request from {}, error: {:?}",
                    sock_addr.ip().to_string(),
                    error
                )
            }
        });
    }
}