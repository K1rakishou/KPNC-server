use std::sync::Arc;
use anyhow::Context;
use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::AccountId;

pub async fn start_watching_post(
    database: &Arc<Database>,
    account_id: &AccountId,
    post_descriptor: &PostDescriptor
) -> anyhow::Result<bool> {
    let mut connection = database.connection().await?;
    let transaction = connection.transaction().await?;

    // TODO: check account is valid before inserting new post watch

    let owner_account_id: i64 = transaction.query_one(
        "SELECT id_generated FROM accounts WHERE account_id = $1",
        &[&account_id.id]
    ).await?.get(0);

    let query = r#"
        INSERT INTO posts(
            site_name,
            board_code,
            thread_no,
            post_no,
            post_sub_no,
            is_dead
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (site_name, board_code, thread_no, post_no, post_sub_no)
        DO UPDATE SET site_name = $1
        RETURNING id_generated
"#;

    let owner_post_id: i64 = transaction.query_one(
        query,
        &[
            post_descriptor.site_name(),
            post_descriptor.board_code(),
            &(post_descriptor.thread_no() as i64),
            &(post_descriptor.post_no as i64),
            &(post_descriptor.post_sub_no as i64),
            &false
        ]
    ).await?.get(0);

    let query = r#"
        INSERT INTO watches(
            owner_post_id,
            owner_account_id
        )
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        RETURNING id_generated
"#;

    let new_watch_inserted = transaction.query_opt(
        query,
        &[
            &owner_post_id,
            &owner_account_id
        ]
    ).await?.is_some();

    if !new_watch_inserted {
        transaction.rollback().await?;

        debug!("start_watching_post() Post watch {} already exists in the database", post_descriptor);
        return Ok(false);
    }

    transaction.commit().await?;
    debug!("start_watching_post() Created new post watch for post {}", post_descriptor);

    return Ok(true);
}

pub async fn get_all_watched_threads(
    database: &Arc<Database>
) -> anyhow::Result<Vec<ThreadDescriptor>> {
    let connection = database.connection().await?;

    let query = r#"
        SELECT
            posts.site_name,
            posts.board_code,
            posts.thread_no
        FROM
            posts
        WHERE
            posts.is_dead = FALSE
        AND
            posts.deleted_on is NULL
        GROUP BY posts.site_name, posts.board_code, posts.thread_no
"#;

    let rows = connection.query(query, &[]).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let mut thread_descriptors = Vec::with_capacity(rows.len());

    for row in rows {
        thread_descriptors.push(ThreadDescriptor::from_row(&row));
    }

    return Ok(thread_descriptors);
}

pub async fn mark_all_thread_posts_dead(
    database: &Arc<Database>,
    thread_descriptor: &ThreadDescriptor
) -> anyhow::Result<()> {
    let connection = database.connection().await?;

    let query = r#"
        UPDATE posts
        SET is_dead = TRUE
        WHERE
            posts.site_name = $1
        AND
            posts.board_code = $2
        AND
            posts.thread_no = $3
"#;

    connection.execute(
        query,
        &[
            thread_descriptor.site_name(),
            thread_descriptor.board_code(),
            &(thread_descriptor.thread_no as i64)
        ]
    )
        .await
        .context(format!("Failed to update is_dead flag for thread {}", thread_descriptor))?;

    return Ok(());
}