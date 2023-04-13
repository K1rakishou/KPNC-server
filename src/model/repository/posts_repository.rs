use std::sync::Arc;
use anyhow::Context;
use tokio_postgres::Row;
use crate::helpers::db_helpers;
use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::AccountId;
use crate::model::repository::post_descriptor_id_repository;

#[derive(Debug, Eq, PartialEq)]
pub enum StartWatchingPostResult {
    Ok,
    AccountDoesNotExist,
    PostWatchAlreadyExists
}

pub async fn start_watching_post(
    database: &Arc<Database>,
    account_id: &AccountId,
    post_descriptor: &PostDescriptor
) -> anyhow::Result<StartWatchingPostResult> {
    let mut connection = database.connection().await?;
    let transaction = connection.transaction().await?;

    // TODO: should check separately whether an account exist and whether it's valid
    let query = r#"
        SELECT id_generated
        FROM accounts
        WHERE
            account_id = $1
        AND
            valid_until > now()
"#;

    let owner_account_id: Option<Row> = transaction.query_opt(
        query,
        &[&account_id.id]
    ).await?;

    if owner_account_id.is_none() {
        return Ok(StartWatchingPostResult::AccountDoesNotExist);
    }

    let owner_account_id: i64 = owner_account_id.unwrap().get(0);

    let owner_post_descriptor_id = post_descriptor_id_repository::insert_descriptor_db_id(
        post_descriptor,
        &transaction
    ).await?;

    let query = r#"
        INSERT INTO posts(
            owner_post_descriptor_id,
            is_dead
        )
        VALUES ($1, $2)
        ON CONFLICT (owner_post_descriptor_id)
        DO UPDATE SET owner_post_descriptor_id = $1
        RETURNING posts.id_generated
"#;

    let owner_post_id: i64 = transaction.query_one(
        query,
        &[
            &owner_post_descriptor_id,
            &false
        ]
    ).await?.get(0);

    let query = r#"
        INSERT INTO watches(
            owner_post_id,
            owner_account_id
        )
        VALUES ($1, $2)
        ON CONFLICT (owner_post_id, owner_account_id) DO NOTHING
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
        return Ok(StartWatchingPostResult::PostWatchAlreadyExists);
    }

    transaction.commit().await?;
    debug!("start_watching_post() Created new post watch for post {}", post_descriptor);

    return Ok(StartWatchingPostResult::Ok);
}

pub async fn get_all_watched_threads(
    database: &Arc<Database>
) -> anyhow::Result<Vec<ThreadDescriptor>> {
    let connection = database.connection().await?;

    let query = r#"
        SELECT
            posts.owner_post_descriptor_id
        FROM
            posts
        WHERE
            posts.is_dead = FALSE
        AND
            posts.deleted_on is NULL
"#;

    let rows = connection.query(query, &[]).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let owner_post_descriptor_ids = rows.iter()
        .map(|row| row.get::<usize, i64>(0))
        .collect::<Vec<i64>>();

    let post_descriptors = post_descriptor_id_repository::get_many_post_descriptors_by_db_ids(
        owner_post_descriptor_ids
    ).await;

    if post_descriptors.is_empty() {
        return Ok(vec![]);
    }

    let mut thread_descriptors = Vec::with_capacity(post_descriptors.len());

    for post_descriptor in post_descriptors {
        thread_descriptors.push(post_descriptor.thread_descriptor);
    }

    return Ok(thread_descriptors);
}

pub async fn mark_all_thread_posts_dead(
    database: &Arc<Database>,
    thread_descriptor: &ThreadDescriptor
) -> anyhow::Result<()> {
    let connection = database.connection().await?;

    let thread_post_db_ids = post_descriptor_id_repository::get_thread_post_db_ids(
        thread_descriptor
    ).await;

    let query = r#"
        UPDATE posts
        SET is_dead = TRUE
        WHERE posts.id_generated IN
"#;

    let query_with_params = db_helpers::format_query_params_string(
        query,
        "",
        thread_post_db_ids.len()
    ).string()?;

    let query_params = db_helpers::to_db_params::<i64>(&thread_post_db_ids);

    connection.execute(
        query_with_params.as_str(),
        &query_params[..]
    )
        .await
        .context(format!("Failed to update is_dead flag for thread {}", thread_descriptor))?;

    return Ok(());
}