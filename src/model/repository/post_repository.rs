use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Context;

use crate::helpers::db_helpers;
use crate::helpers::string_helpers::FormatToken;
use crate::info;
use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::repository::{account_repository, post_descriptor_id_repository};
use crate::model::repository::account_repository::AccountId;
use crate::model::repository::post_reply_repository::PostReply;

#[derive(Debug, Eq, PartialEq)]
pub enum StartWatchingPostResult {
    Ok,
    AccountDoesNotExist,
    AccountIsNotValid
}

#[derive(Debug, Eq, PartialEq)]
pub enum StopWatchingPostResult {
    Ok,
    AccountDoesNotExist,
    AccountIsNotValid
}

pub async fn start_watching_post(
    database: &Arc<Database>,
    account_id: &AccountId,
    post_descriptor: &PostDescriptor
) -> anyhow::Result<StartWatchingPostResult> {
    let account = account_repository::get_account(account_id, database).await?;
    if account.is_none() {
        info!(
            "start_watching_post() account with id \'{}\' does not exist",
            account_id.format_token()
        );

        return Ok(StartWatchingPostResult::AccountDoesNotExist);
    }

    let account = account.unwrap();
    if !account.is_valid() {
        info!(
            "start_watching_post() account with id \'{}\' is not valid (status: {})",
            account_id.format_token(),
            account.validation_status().unwrap()
        );

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

        info!("start_watching_post() Post watch {} already exists in the database", post_descriptor);
        return Ok(StartWatchingPostResult::Ok);
    }

    transaction.commit().await?;

    let firebase_token = account.firebase_token.unwrap();
    info!(
        "start_watching_post() Created new post watch for post {} for user with token {}",
        post_descriptor,
        firebase_token.format_token()
    );

    return Ok(StartWatchingPostResult::Ok);
}

pub async fn stop_watching_post(
    database: &Arc<Database>,
    account_id: &AccountId,
    post_descriptor: &PostDescriptor
) -> anyhow::Result<StopWatchingPostResult> {
    let account = account_repository::get_account(account_id, database).await?;
    if account.is_none() {
        info!(
            "stop_watching_post() account with id \'{}\' does not exist",
            account_id.format_token()
        );

        return Ok(StopWatchingPostResult::AccountDoesNotExist);
    }

    let account = account.unwrap();
    if !account.is_valid() {
        info!(
            "stop_watching_post() account with id \'{}\' is not valid (status: {})",
            account_id.format_token(),
            account.validation_status().unwrap()
        );

        return Ok(StopWatchingPostResult::AccountIsNotValid);
    }

    let connection = database.connection().await?;

    let owner_post_descriptor_id = post_descriptor_id_repository::get_post_descriptor_db_id(
        post_descriptor
    ).await;

    let query = r#"
        DELETE FROM post_watches
        WHERE id_generated IN (
            SELECT
                post_watch.id_generated
            FROM post_descriptors
                 INNER JOIN posts post on post_descriptors.id_generated = post.owner_post_descriptor_id
                 INNER JOIN post_watches post_watch on post.id_generated = post_watch.owner_post_id
                 INNER JOIN accounts a on a.id_generated = post_watch.owner_account_id
            WHERE
                post_descriptors.id_generated = $1
            AND
                a.account_id = $2
        )
    "#;

    let statement = connection.prepare(query).await?;
    let deleted = connection.execute(
        &statement,
        &[&owner_post_descriptor_id, &account.account_id.id]
    ).await?;

    let firebase_token = account.firebase_token.unwrap();
    info!(
        "stop_watching_post() Deleted {} post watches for user with token {}",
        deleted,
        firebase_token.format_token()
    );

    return Ok(StopWatchingPostResult::Ok);
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

    let mut thread_descriptors_set = HashSet::with_capacity(post_descriptors.len());

    for post_descriptor in post_descriptors {
        thread_descriptors_set.insert(post_descriptor.thread_descriptor);
    }

    let thread_descriptors = thread_descriptors_set.into_iter().collect::<Vec<ThreadDescriptor>>();
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
        WHERE posts.owner_post_descriptor_id IN ({QUERY_PARAMS})
    "#;

    let (query, query_params) = db_helpers::format_query_params(
        query,
        "{QUERY_PARAMS}",
        &thread_post_db_ids
    )?;

    let connection = database.connection().await?;
    let statement = connection.prepare(query.as_str()).await?;

    connection.execute(&statement, &query_params[..])
        .await
        .context(format!("Failed to update is_dead flag for thread {}", thread_descriptor))?;

    post_descriptor_id_repository::delete_all_thread_posts(thread_descriptor).await;

    return Ok(());
}

pub async fn find_new_replies(
    thread_descriptor: &ThreadDescriptor,
    database: &Arc<Database>,
    post_descriptor_db_ids: &Vec<i64>
) -> anyhow::Result<Vec<PostReply>> {
    let query = r#"
        SELECT
            posts.owner_post_descriptor_id,
            account.id_generated
        FROM posts
            LEFT JOIN post_watches watch on posts.id_generated = watch.owner_post_id
            LEFT JOIN accounts account on watch.owner_account_id = account.id_generated
            LEFT JOIN post_descriptors pd on pd.id_generated = posts.owner_post_descriptor_id
            LEFT JOIN post_replies post_reply on pd.id_generated = post_reply.owner_post_descriptor_id
        WHERE
            posts.owner_post_descriptor_id IN ({QUERY_PARAMS})
        AND
            post_reply.deleted_on IS NULL
        AND
            account.id_generated IS NOT NULL
    "#;

    let (query, query_params) = db_helpers::format_query_params(
        query,
        "{QUERY_PARAMS}",
        &post_descriptor_db_ids
    )?;

    let connection = database.connection().await?;
    let statement = connection.prepare(query.as_str()).await?;

    let rows = connection.query(&statement, &query_params[..]).await?;
    if rows.is_empty() {
        info!("process_posts({}) end. No posts found related to post watchers", thread_descriptor);
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