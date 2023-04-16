use std::future::Future;
use std::pin::Pin;

use crate::init_logger;
use crate::model::repository::{account_repository, migrations_repository};
use crate::tests::shared::{database_shared, server_shared, site_repository_shared};

pub struct TestCase {
    pub name: String,
    pub function: Box<dyn Fn() -> PinFutureObj<()>>
}

pub type PinFutureObj<Output> = Pin<Box<dyn Future<Output = Output>>>;

pub async fn run_test(tests: Vec<TestCase>) {
    test_ctor().await;
    let tests_count = tests.len();

    for (index, test) in tests.iter().enumerate() {
        info!("[{}/{}] Running \'{}\'...", (index + 1), tests_count, test.name);

        database_shared::cleanup().await;
        account_repository::test_cleanup().await;
        (test.function)().await;

        info!("[{}/{}] Running \'{}\'...OK", (index + 1), tests_count, test.name);
    }

    test_dtor().await;
}

pub fn assert_none<T>(option: &Option<T>) {
    assert!(option.is_none());
}

pub fn assert_some<T>(option: &Option<T>) {
    assert!(option.is_some());
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

#[macro_export]
macro_rules! make_test {
    ($func:expr) => {
        TestCase {
            name: String::from(stringify!($func)),
            function: Box::new(|| Box::pin($func()))
        }
    };
}