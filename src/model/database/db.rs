use std::sync::Arc;

use anyhow::{anyhow, Context};
use bb8::{Pool, PooledConnection};
use bb8_postgres::PostgresConnectionManager;
use tokio_postgres::NoTls;

pub struct Database {
    pool: Arc<Pool<PostgresConnectionManager<NoTls>>>
}

pub type PgPooledConnection<'a> = PooledConnection<'a, PostgresConnectionManager<NoTls>>;

impl Database {
    pub async fn new(connection_string: String, cpu_cores_count: u32) -> anyhow::Result<Database> {
        let manager = PostgresConnectionManager::new_from_stringlike(
            connection_string,
            NoTls
        ).context("Failed to connect to the database")?;

        let pool = Pool::builder()
            .min_idle(Some(cpu_cores_count))
            .max_size(cpu_cores_count * 2)
            .build(manager)
            .await
            .context("Failed to create connection pool")?;

        let database = Database {
            pool: Arc::new(pool)
        };

        return Ok(database);
    }

    pub async fn connection(&self) -> anyhow::Result<PgPooledConnection<'_>> {
        return match self.pool.get().await {
            Ok(connection) => { Ok(connection) },
            Err(error) => { Err(anyhow!(error.to_string())) }
        }
    }

}