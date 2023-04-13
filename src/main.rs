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
use crate::model::repository::post_descriptor_id_repository;
use crate::model::repository::site_repository::SiteRepository;
use crate::router::router;
use crate::service::thread_watcher::ThreadWatcher;

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
    let database = Database::new(num_cpus).await?;
    let database = Arc::new(database);
    info!("main() initializing database... done");

    info!("main() processing migrations...");
    perform_migrations(&database).await?;
    info!("main() processing migrations... done");

    info!("main() starting up server...");
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    let site_repository = Arc::new(SiteRepository::new());
    let database_cloned_for_watcher = database.clone();
    let site_repository_for_watcher = site_repository.clone();

    post_descriptor_id_repository::init(&database)
        .await
        .context("Failed to init post_descriptor_id_repository")?;

    tokio::task::spawn(async move {
        let mut thread_watcher = ThreadWatcher::new(num_cpus);

        thread_watcher.start(
            &database_cloned_for_watcher,
            &site_repository_for_watcher
        ).await.unwrap();
    });

    info!("main() starting up server... done, waiting for connections...");

    loop {
        let (stream, sock_addr) = listener.accept().await?;
        let database_cloned_for_router = database.clone();
        let site_repository_cloned = site_repository.clone();

        tokio::task::spawn(async move {
            http1::Builder::new()
                .serve_connection(
                    stream,
                    service_fn(|request| {
                        return router(
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