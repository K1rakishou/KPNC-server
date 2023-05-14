use std::sync::Arc;

use chrono::{DateTime, FixedOffset};

use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;

pub async fn get_last_processed_post(
    thread_descriptor: &ThreadDescriptor,
    database: &Arc<Database>
) -> anyhow::Result<Option<PostDescriptor>> {
    let query = r#"
        SELECT last_processed_post_no,
               last_processed_post_sub_no
        FROM threads
        WHERE threads.site_name = $1
          AND threads.board_code = $2
          AND threads.thread_no = $3
          AND threads.last_processed_post_no > 0
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let row_maybe = connection.query_opt(
        &statement,
        &[
            thread_descriptor.site_name(),
            thread_descriptor.board_code(),
            &(thread_descriptor.thread_no as i64)
        ]
    ).await?;

    if row_maybe.is_none() {
        return Ok(None);
    }

    let row = row_maybe.unwrap();

    let last_processed_post_no: i64 = row.try_get(0)?;
    let last_processed_post_sub_no: i64 = row.try_get(1)?;

    let last_processed_post_descriptor = PostDescriptor::from_thread_descriptor(
        thread_descriptor.clone(),
        last_processed_post_no as u64,
        last_processed_post_sub_no as u64
    );

    return Ok(Some(last_processed_post_descriptor));
}

pub async fn store_last_processed_post(
    post_descriptor: &PostDescriptor,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    let query = r#"
        INSERT INTO threads(site_name,
                            board_code,
                            thread_no,
                            last_processed_post_no,
                            last_processed_post_sub_no)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (site_name, board_code, thread_no)
            DO UPDATE SET last_processed_post_no     = $4,
                          last_processed_post_sub_no = $5
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    connection.execute(
        &statement,
        &[
            post_descriptor.site_name(),
            post_descriptor.board_code(),
            &(post_descriptor.thread_no() as i64),
            &(post_descriptor.post_no as i64),
            &(post_descriptor.post_sub_no as i64),
        ]
    ).await?;

    return Ok(());
}

pub async fn get_last_modified(
    thread_descriptor: &ThreadDescriptor,
    database: &Arc<Database>
) -> anyhow::Result<Option<DateTime<FixedOffset>>> {
    let query = r#"
        SELECT last_modified
        FROM threads
        WHERE threads.site_name = $1
          AND threads.board_code = $2
          AND threads.thread_no = $3
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let row_maybe = connection.query_opt(
        &statement,
        &[
            thread_descriptor.site_name(),
            thread_descriptor.board_code(),
            &(thread_descriptor.thread_no as i64)
        ]
    ).await?;

    if row_maybe.is_none() {
        return Ok(None);
    }

    let row = row_maybe.unwrap();
    let last_modified: Option<DateTime<FixedOffset>> = row.try_get(0)?;

    return Ok(last_modified);
}

pub async fn store_last_modified(
    last_modified: &DateTime<FixedOffset>,
    thread_descriptor: &ThreadDescriptor,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    let query = r#"
        UPDATE threads
        SET last_modified = $1
        WHERE threads.site_name = $2
          AND threads.board_code = $3
          AND threads.thread_no = $4
"#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    connection.execute(
        &statement,
        &[
            last_modified,
            thread_descriptor.site_name(),
            thread_descriptor.board_code(),
            &(thread_descriptor.thread_no as i64)
        ]
    ).await?;

    return Ok(());
}