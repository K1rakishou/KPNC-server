use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hyper::server::conn::http1;
use hyper::service::service_fn;
use lazy_static::lazy_static;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::model::database::db::Database;
use crate::model::repository::site_repository::SiteRepository;
use crate::router::{router, TestContext};

static SERVER_WORKING_FLAG: AtomicBool = AtomicBool::new(false);

lazy_static! {
    static ref SERVER_HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
}

pub async fn ctor(
    site_repository: &Arc<SiteRepository>,
    database: &Arc<Database>
) {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    SERVER_WORKING_FLAG.store(true, Ordering::SeqCst);

    let database_cloned_for_router = database.clone();
    let site_repository_cloned = site_repository.clone();

    let join_handle: JoinHandle<()> = tokio::task::spawn(async move {
        loop {
            if !SERVER_WORKING_FLAG.load(Ordering::SeqCst) {
                break;
            }

            let (stream, sock_addr) = listener.accept().await.unwrap();
            let database_cloned_for_router = database_cloned_for_router.clone();
            let site_repository_cloned = site_repository_cloned.clone();

            tokio::task::spawn(async move {
                http1::Builder::new()
                    .serve_connection(
                        stream,
                        service_fn(|request| {
                            let test_context = TestContext { enable_throttler: false };
                            let test_context = Some(test_context);

                            return router(
                                test_context,
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

        return ();
    });

    {
        let mut server_handle_locked = SERVER_HANDLE.lock().await;
        *server_handle_locked = Some(join_handle);
    }
}

pub async fn dtor() {
    SERVER_WORKING_FLAG.store(false, Ordering::SeqCst);

    let mut server_handle_locked = SERVER_HANDLE.lock().await;
    let server_handle = server_handle_locked.take().unwrap();
    server_handle.abort();
    let _ = server_handle.await;
}