#![feature(once_cell)]
#![feature(async_closure)]
#![feature(thread_id_value)]

use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use lazy_static::lazy_static;
use tokio::net::TcpListener;

use crate::helpers::{logger, throttler};
use crate::model::database::db::Database;
use crate::model::repository::migrations_repository::perform_migrations;
use crate::model::repository::post_descriptor_id_repository;
use crate::model::repository::site_repository::SiteRepository;
use crate::router::{router, TestContext};
use crate::service::fcm_sender::FcmSender;
use crate::service::thread_watcher::ThreadWatcher;

mod constants;
mod model;
mod service;
mod router;
mod handlers;
mod helpers;

#[cfg(test)]
mod tests;

lazy_static! {
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::new();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let is_dev_build = i32::from_str(
        &env::var("DEVELOPMENT_BUILD")
            .context("Failed to read DEVELOPMENT_BUILD from Environment")?
    )? == 1;
    let timeout_seconds = env::var("THREAD_WATCHER_TIMEOUT_SECONDS")
        .map(|value| u64::from_str(value.as_str()).unwrap())
        .context("Failed to read THREAD_WATCHER_TIMEOUT_SECONDS")?;
    let connection_string = env::var("DATABASE_CONNECTION_STRING")
        .context("Failed to read DATABASE_CONNECTION_STRING")?;
    let firebase_api_key = env::var("FIREBASE_API_KEY")
        .context("Failed to read FIREBASE_API_KEY from Environment")?;
    let master_password = env::var("MASTER_PASSWORD")
        .context("Failed to read MASTER_PASSWORD from Environment")?;

    let num_cpus = num_cpus::get() as u32;
    let database = Database::new(connection_string, num_cpus).await?;
    let database = Arc::new(database);
    init_logger(is_dev_build, Some(database.clone()));

    info!("main() initializing the server");
    info!("main() detected cpu cores: {}", num_cpus);

    info!("main() processing migrations...");
    perform_migrations(&database).await?;
    info!("main() processing migrations... done");

    info!("main() starting up server...");
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;

    let site_repository = Arc::new(SiteRepository::new(&HTTP_CLIENT));
    let database_cloned_for_watcher = database.clone();
    let site_repository_for_watcher = site_repository.clone();

    let fcm_sender = FcmSender::new(
        is_dev_build,
        firebase_api_key,
        &database.clone(),
        &site_repository.clone()
    );
    let fcm_sender = Arc::new(fcm_sender);

    post_descriptor_id_repository::init(&database)
        .await
        .context("Failed to init post_descriptor_id_repository")?;

    tokio::task::spawn(async move {
        let mut thread_watcher = ThreadWatcher::new(num_cpus, timeout_seconds, is_dev_build);

        thread_watcher.start(
            &database_cloned_for_watcher,
            &site_repository_for_watcher,
            &fcm_sender
        ).await.unwrap();
    });

    tokio::task::spawn(async move {
        throttler::cleanup_task().await;
    });

    info!("main() starting up server... done, waiting for connections...");

    loop {
        let (stream, sock_addr) = listener.accept().await?;
        let database_cloned_for_router = database.clone();
        let site_repository_cloned = site_repository.clone();
        let master_password_cloned = master_password.clone();

        tokio::task::spawn(async move {
            http1::Builder::new()
                .serve_connection(
                    stream,
                    service_fn(|request| {
                        let test_context: Option<TestContext> = None;

                        return router(
                            test_context,
                            &master_password_cloned,
                            &sock_addr,
                            request,
                            &database_cloned_for_router,
                            &site_repository_cloned
                        );
                    }),
                )
                .await
                .unwrap();
        });
    }
}

pub fn init_logger(is_dev_build: bool, database: Option<Arc<Database>>) {
    logger::init_logger(is_dev_build, database);
}