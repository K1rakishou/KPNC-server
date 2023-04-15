use std::sync::Arc;

use anyhow::Context;

use crate::helpers::db_helpers;
use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::repository::{account_repository, post_descriptor_id_repository};
use crate::model::repository::account_repository::AccountId;
use crate::model::repository::post_reply_repository::PostReply;

#[derive(Debug, Eq, PartialEq)]
pub enum StartWatchingPostResult {
    Ok,
    AccountDoesNotExist,
    AccountIsNotValid,
    PostWatchAlreadyExists
}

pub async fn start_watching_post(
    database: &Arc<Database>,
    account_id: &AccountId,
    post_descriptor: &PostDescriptor
) -> anyhow::Result<StartWatchingPostResult> {
    let account = account_repository::get_account(account_id, database).await?;
    if account.is_none() {
        return Ok(StartWatchingPostResult::AccountDoesNotExist);
    }

    let account = account.unwrap();
    if !account.is_valid() {
        return Ok(StartWatchingPostResult::AccountIsNotValid);
    }

    let mut connection = database.connection().await?;
    let transaction = connection.transaction().await?;

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
        INSERT INTO post_watches(
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
            &account.id_generated
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
            posts.is_dead IS NOT TRUE
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

    let connection = database.connection().await?;
    let statement = connection.prepare(query_with_params.as_str()).await?;

    connection.execute(&statement, &query_params[..])
        .await
        .context(format!("Failed to update is_dead flag for thread {}", thread_descriptor))?;

    return Ok(());
}

pub async fn find_new_replies(
    thread_descriptor: &ThreadDescriptor,
    database: &Arc<Database>,
    post_descriptor_db_ids: &Vec<i64>
) -> anyhow::Result<Vec<PostReply>> {
    let query_start = r#"
        SELECT
            posts.owner_post_descriptor_id,
            account.id_generated
        FROM posts
             LEFT JOIN post_watches watch on posts.id_generated = watch.owner_post_id
             LEFT JOIN accounts account on watch.owner_account_id = account.id_generated
             LEFT JOIN post_replies post_reply on posts.id_generated = post_reply.owner_post_descriptor_id
        WHERE
            posts.owner_post_descriptor_id IN "#;

    let query_end = r#"
        AND
            post_reply.notification_sent_on IS NULL
        AND
            post_reply.deleted_on IS NULL"#;

    let query = db_helpers::format_query_params_string(
        query_start,
        query_end,
        post_descriptor_db_ids.len()
    ).string()?;

    let connection = database.connection().await?;
    let statement = connection.prepare(query.as_str()).await?;
    let query_params = db_helpers::to_db_params::<i64>(&post_descriptor_db_ids);

    let rows = connection.query(&statement, &query_params[..]).await?;
    if rows.is_empty() {
        debug!("process_posts({}) end. No posts found related to post watchers", thread_descriptor);
        return Ok(vec![]);
    }

    let mut post_replies = Vec::<PostReply>::with_capacity(rows.len());

    for row in rows {
        let post_descriptor_id: i64 = row.get(0);
        let account_id_generated: i64 = row.get(1);

        let post_reply = PostReply {
            owner_post_descriptor_id: post_descriptor_id,
            owner_account_id: account_id_generated
        };

        post_replies.push(post_reply);
    }

    return Ok(post_replies);
}