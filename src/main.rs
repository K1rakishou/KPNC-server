use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::Context;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use log::LevelFilter;
use tokio::net::TcpListener;
use crate::model::database::db::Database;
use crate::model::repository::migrations_repository::perform_migrations;
use crate::router::router;

#[macro_use]
extern crate log;

mod constants;
mod model;
mod service;
mod router;
mod handlers;
mod helpers;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let is_dev_build = i32::from_str(&env::var("DEVELOPMENT_BUILD")?)? == 1;
    init_logger(is_dev_build);

    info!("main() initializing the server");

    let num_cpus = num_cpus::get() as u32;
    info!("main() detected cpu cores: {}", num_cpus);

    info!("main() initializing database...");
    let connection_string = env::var("DATABASE_CONNECTION_STRING")
        .context("Failed to read database connection string from Environment")?;
    let database = Database::new(connection_string, num_cpus).await?;
    let database = Arc::new(database);
    info!("main() initializing database... done");

    info!("main() processing migrations...");
    perform_migrations(&database).await?;
    info!("main() processing migrations... done");

    info!("main() starting up server...");
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    info!("main() starting up server... done");

    info!("main() waiting for connections...");

    loop {
        let (stream, sock_addr) = listener.accept().await?;
        let database_cloned = database.clone();

        tokio::task::spawn(async move {
            let result = http1::Builder::new()
                .serve_connection(stream, service_fn(|request| { router(request, &database_cloned) }))
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

fn init_logger(is_dev_build: bool) {
    let level_filter = if is_dev_build {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    env_logger::builder()
        .filter_level(level_filter)
        .init();
}