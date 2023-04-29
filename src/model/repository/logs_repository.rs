use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::info;
use crate::model::database::db::Database;

pub struct LogLine {
    pub id: i64,
    pub log_time: DateTime<Utc>,
    pub log_level: String,
    pub target: String,
    pub message: String
}

pub async fn get_logs(
    num: i64,
    last_id: i64,
    database: &Arc<Database>
) -> anyhow::Result<Vec<LogLine>> {
    info!("get_logs() num: {}, last_id: {}", num, last_id);

    let query = r#"
        SELECT *
        FROM logs
        WHERE id < $1
        ORDER BY id DESC
        LIMIT $2
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let rows = connection.query(&statement, &[&last_id, &num]).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let mut result_vec = Vec::with_capacity(rows.len());

    for row in rows {
        let id: i64 = row.try_get(0)?;
        let log_time: DateTime<Utc> = row.try_get(1)?;
        let log_level: String = row.try_get(2)?;
        let target: String = row.try_get(3)?;
        let message: String = row.try_get(4)?;

        let log_line = LogLine {
            id,
            log_time,
            log_level,
            target,
            message
        };

        result_vec.push(log_line);
    }

    return Ok(result_vec);
}