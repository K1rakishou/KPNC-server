use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio_postgres::Row;

use crate::{error, info};
use crate::helpers::db_helpers;
use crate::model::data::chan::PostDescriptor;
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{AccountToken, ApplicationType, TokenType};
use crate::model::repository::post_descriptor_id_repository;
use crate::service::thread_watcher::FoundPostReply;

const MAX_NOTIFICATION_DELIVERY_ATTEMPTS: i16 = 25;

#[derive(Debug)]
pub struct PostReply {
    pub owner_post_descriptor_id: i64,
    pub owner_account_id: i64,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct UnsentReply {
    pub post_reply_id: i64,
    pub token: AccountToken,
    pub post_descriptor: PostDescriptor
}

impl UnsentReply {
    pub fn from_row(row: &Row) -> anyhow::Result<UnsentReply> {
        let post_reply_id: i64 = row.try_get(0)?;
        let site_name: String = row.try_get(2)?;
        let board_code: String = row.try_get(3)?;
        let thread_no: i64 = row.try_get(4)?;
        let post_no: i64 = row.try_get(5)?;
        let post_sub_no: i64 = row.try_get(6)?;
        let token: String = row.try_get(7)?;
        let application_type: i64 = row.try_get(8)?;
        let token_type: i64 = row.try_get(9)?;

        let post_descriptor = PostDescriptor::new(
            site_name,
            board_code,
            thread_no as u64,
            post_no as u64,
            post_sub_no as u64,
        );

        let application_type = ApplicationType::from_i64(application_type);
        let token_type = TokenType::from_i64(token_type);

        let account_token = AccountToken {
            token,
            application_type,
            token_type
        };

        let unsent_reply = UnsentReply {
            post_reply_id,
            token: account_token,
            post_descriptor
        };

        return Ok(unsent_reply);
    }
}

pub async fn store(
    post_replies: &Vec<PostReply>,
    post_descriptor_db_ids: &HashMap<i64, Vec<&FoundPostReply>>,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    if post_replies.is_empty() {
        return Ok(());
    }

    // TODO: this might not perform well. Maybe I should do like they suggest here:
    //  https://stackoverflow.com/questions/71684651/multiple-value-inserts-to-postgres-using-tokio-postgres-in-rust
    let query = r#"
        INSERT INTO post_replies
        (
            owner_account_id,
            owner_post_descriptor_id,
            reply_to_post_descriptor_id
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (
            owner_account_id,
            owner_post_descriptor_id,
            reply_to_post_descriptor_id
        ) DO NOTHING
    "#;

    let mut connection = database.connection().await?;
    let transaction = connection.transaction().await?;

    for post_reply in post_replies {
        let post_descriptors_to_insert = post_descriptor_db_ids.get(
            &post_reply.owner_post_descriptor_id
        );

        if post_descriptors_to_insert.is_none() {
            continue;
        }

        let found_post_replies = post_descriptors_to_insert.unwrap();

        let origin_post_db_ids = post_descriptor_id_repository::insert_descriptor_db_ids(
            &found_post_replies.iter().map(|fpr| &fpr.origin).collect::<Vec<&PostDescriptor>>(),
            &transaction
        ).await?;

        let reply_to_post_db_ids = post_descriptor_id_repository::insert_descriptor_db_ids(
            &found_post_replies.iter().map(|fpr| &fpr.replies_to).collect::<Vec<&PostDescriptor>>(),
            &transaction
        ).await?;

        let statement = transaction.prepare(query).await?;

        for found_post_reply in found_post_replies {
            let origin_post_db_id = origin_post_db_ids.get(&found_post_reply.origin);
            let reply_to_post_db_id = reply_to_post_db_ids.get(&found_post_reply.replies_to);

            transaction.execute(
                &statement,
                &[&post_reply.owner_account_id, &origin_post_db_id, &reply_to_post_db_id]
            ).await?;
        }
    }

    transaction.commit().await?;

    return Ok(());
}

pub async fn get_unsent_replies(
    is_dev_build: bool,
    database: &Arc<Database>
) -> anyhow::Result<HashMap<AccountToken, HashSet<UnsentReply>>> {
    // Damn, this motherfucker is kinda too complex but I have no idea how to simplify it.
    // The idea here is to extract post_replies.id, account_token.token, thread.site_name,
    // thread.board_code, thread.thread_no, post_descriptor.post_no, post_descriptor.post_sub_no
    // but only for post_replies that match post_watches' account_token.application_type.
    // In other words, accounts can have multiple tokens with different application types
    // (for example for KurobaExLite there are two application types: Debug and Production, since
    // the user can have both applications installed on their phone). When we start watching a post
    // we send what application was it the created this post watch. So when a reply to this watch
    // comes we only send the reply to the token that is associated with the original post watch.
    let query = r#"
        WITH
            -- Associate post_reply with account_token.application_type
            post_reply_application_type AS (
                SELECT
                    post_replies.id,
                    post_replies.owner_account_id,
                    account_token.application_type
                FROM post_replies
                         INNER JOIN accounts account
                                    ON account.id = post_replies.owner_account_id
                         INNER JOIN account_tokens account_token
                                    ON account.id = account_token.owner_account_id
            ),
            -- Associate post_replies with post_watch.application_type
            post_watch_application_type AS (
                SELECT
                    post_watch.id,
                    post_watch.owner_account_id,
                    post_watch.application_type
                FROM post_replies
                         INNER JOIN post_descriptors post_descriptor
                                    ON post_replies.reply_to_post_descriptor_id = post_descriptor.id
                         INNER JOIN post_watches post_watch
                                    ON post_descriptor.id = post_watch.owner_post_descriptor_id
            )

        SELECT
            post_replies.id,
            account_token.token,
            thread.site_name,
            thread.board_code,
            thread.thread_no,
            post_descriptor.post_no,
            post_descriptor.post_sub_no,
            account_token.token,
            account_token.application_type,
            account_token.token_type
        FROM post_replies
            INNER JOIN accounts account
                ON post_replies.owner_account_id = account.id
            INNER JOIN account_tokens account_token
                ON account_token.owner_account_id = account.id
            INNER JOIN post_descriptors post_descriptor
                ON post_replies.owner_post_descriptor_id = post_descriptor.id
            INNER JOIN threads thread
                ON post_descriptor.owner_thread_id = thread.id
            INNER JOIN post_watches post_watch
                ON post_watch.owner_post_descriptor_id = post_replies.reply_to_post_descriptor_id
            INNER JOIN post_reply_application_type prat
                ON post_replies.id = prat.id
            INNER JOIN post_watch_application_type pwat
                ON post_watch.id = pwat.id
        WHERE
            prat.owner_account_id = pwat.owner_account_id
        AND
            -- Select only post replies that have the same application_type as post watches they reply to
            prat.application_type = pwat.application_type
        AND
            post_replies.deleted_on IS NULL
        AND
            post_replies.notification_delivery_attempt < $1
        AND
            post_replies.notification_delivered_on IS NULL
        AND
            account.valid_until > now()
        AND
            account.deleted_on IS NULL
    "#;

    let connection = database.connection().await?;
    let rows = connection.query(query, &[&MAX_NOTIFICATION_DELIVERY_ATTEMPTS]).await?;

    if rows.is_empty() {
        info!("No unsent replies found");
        return Ok(HashMap::new());
    }

    let mut unsent_replies = HashMap::<AccountToken, HashSet<UnsentReply>>::with_capacity(rows.len());
    let mut error_logged = false;

    for row in rows {
        let unsent_reply = UnsentReply::from_row(&row);
        if unsent_reply.is_err() {
            if is_dev_build {
                unsent_reply.unwrap();
            } else if !error_logged {
                error_logged = true;

                let error = unsent_reply.err().unwrap();
                error!("Failed to map row to UnsentReply, error: {}", error);
            }

            continue;
        }

        let unsent_reply = unsent_reply.unwrap();

        if !unsent_replies.contains_key(&unsent_reply.token) {
            unsent_replies.insert(unsent_reply.token.clone(), HashSet::with_capacity(16));
        }

        unsent_replies.get_mut(&unsent_reply.token)
            .unwrap()
            .insert(unsent_reply.clone());
    }

    return Ok(unsent_replies);
}

pub async fn increment_notification_delivery_attempt(
    sent_post_reply_ids: &Vec<i64>,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    info!("increment_notification_delivery_attempt() Got {} sent_post_reply_ids", sent_post_reply_ids.len());

    if sent_post_reply_ids.is_empty() {
        return Ok(());
    }

    let query = r#"
        UPDATE post_replies
        SET notification_delivery_attempt = notification_delivery_attempt + 1
        WHERE id IN ({QUERY_PARAMS})
    "#;

    let (query, db_params) = db_helpers::format_query_params(
        query,
        "{QUERY_PARAMS}",
        &sent_post_reply_ids
    )?;

    let connection = database.connection().await?;
    let statement = connection.prepare(&query).await?;
    connection.execute(&statement, &db_params[..]).await?;

    return Ok(());
}

pub async fn mark_post_replies_as_notified(
    sent_post_reply_ids: &Vec<i64>,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    info!("mark_post_replies_as_notified() Got {} sent_post_reply_ids", sent_post_reply_ids.len());

    let query = r#"
        UPDATE post_replies
        SET notification_delivered_on = now()
        WHERE id IN ({QUERY_PARAMS})
    "#;

    let (query, db_params) = db_helpers::format_query_params(
        query,
        "{QUERY_PARAMS}",
        &sent_post_reply_ids
    )?;

    let connection = database.connection().await?;
    let statement = connection.prepare(&query).await?;
    connection.execute(&statement, &db_params[..]).await?;

    return Ok(());
}