use std::sync::Arc;
use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::AccountId;

pub async fn start_watching_post(
    database: &Arc<Database>,
    account_id: &AccountId,
    post_descriptor: &PostDescriptor
) -> anyhow::Result<()> {
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
        VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING RETURNING id_generated
        "#;

    // TODO: this fails to execute when trying to insert the same data twice.
    //  "ON CONFLICT DO NOTHING" doesn't work?
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
        "#;

    transaction.execute(
        query,
        &[
            &owner_post_id,
            &owner_account_id
        ]
    ).await?;

    transaction.commit().await?;

    return Ok(());
}

pub async fn get_all_watched_threads(
    database: &Arc<Database>
) -> anyhow::Result<Vec<ThreadDescriptor>> {
    let connection = database.connection().await?;

    let query = r#"
    SELECT posts.site_name,
       posts.board_code,
       posts.thread_no
    FROM posts
    WHERE posts.is_dead = FALSE
      AND posts.deleted_on is NULL
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