use std::sync::Arc;
use std::time::Duration;

use crate::{error, info};
use crate::model::database::db::Database;
use crate::model::repository::invites_repository;

pub async fn invites_cleanup_task(database: &Arc<Database>) {
    info!("invites_cleanup_task() start");

    loop {
        info!("invites_cleanup_task() cleaning up...");

        let result = invites_repository::cleanup(database).await;
        let deleted = if result.is_err() {
            error!("invites_cleanup_task::cleanup() error: {}", anyhow::anyhow!(result.err().unwrap()));
            0
        } else {
            result.unwrap()
        };

        info!("invites_cleanup_task() cleaning up... done, deleted: {}, waiting...", deleted);
        tokio::time::sleep(Duration::from_secs(30 * 60)).await;
        info!("invites_cleanup_task() waiting... done");
    }

    info!("invites_cleanup_task() end");
}