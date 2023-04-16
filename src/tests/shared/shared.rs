use std::future::Future;

use crate::init_logger;
use crate::model::repository::migrations_repository;
use crate::tests::shared::{database_shared, server_shared, site_repository_shared};

pub async fn run_test<Fut>(tests: impl FnOnce() -> Fut) -> ()
    where Fut: Future<Output = ()>,
{
    test_ctor().await;
    tests().await;
    test_dtor().await;
}

pub fn assert_none<T>(option: &Option<T>) {
    assert!(option.is_none());
}

async fn test_ctor() {
    init_logger(false);
    info!("test_ctor start");

    database_shared::ctor().await;
    let database = database_shared::database();
    migrations_repository::perform_migrations(database).await.unwrap();

    site_repository_shared::ctor().await;
    let site_repository = site_repository_shared::site_repository();

    server_shared::ctor(site_repository, database).await;

    info!("test_ctor end");
}

async fn test_dtor() {
    info!("test_dtor start");

    server_shared::dtor().await;
    site_repository_shared::dtor().await;
    database_shared::dtor().await;

    info!("test_dtor end");
}