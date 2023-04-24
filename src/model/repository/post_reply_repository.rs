use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio_postgres::Row;

use crate::{error, info};
use crate::helpers::db_helpers;
use crate::model::data::chan::PostDescriptor;
use crate::model::database::db::Database;
use crate::model::repository::post_descriptor_id_repository;
use crate::service::thread_watcher::FoundPostReply;

pub struct PostReply {
    pub owner_post_descriptor_id: i64,
    pub owner_account_id: i64,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct UnsentReply {
    pub post_reply_id_generated: i64,
    pub firebase_token: String,
    pub post_descriptor: PostDescriptor
}

impl UnsentReply {
    pub fn from_row(row: &Row) -> anyhow::Result<UnsentReply> {
        let post_reply_id_generated: i64 = row.try_get(0)?;
        let firebase_token: String = row.try_get(1)?;
        let site_name: String = row.try_get(2)?;
        let board_code: String = row.try_get(3)?;
        let thread_no: i64 = row.try_get(4)?;
        let post_no: i64 = row.try_get(5)?;
        let post_sub_no: i64 = row.try_get(6)?;

        let post_descriptor = PostDescriptor::new(
            site_name,
            board_code,
            thread_no as u64,
            post_no as u64,
            post_sub_no as u64,
        );

        let unsent_reply = UnsentReply {
            post_reply_id_generated,
            firebase_token,
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
            owner_post_descriptor_id
        )
        VALUES ($1, $2)
        ON CONFLICT (owner_account_id, owner_post_descriptor_id) DO NOTHING
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

        let post_descriptors_to_insert = post_descriptors_to_insert.unwrap();

        let post_descriptors_to_insert = post_descriptors_to_insert
            .iter()
            .map(|found_post_reply| &found_post_reply.origin)
            .collect::<Vec<&PostDescriptor>>();

        let pd_to_db_id_map = post_descriptor_id_repository::insert_descriptor_db_ids(
            &post_descriptors_to_insert,
            &transaction
        ).await?;

        let statement = transaction.prepare(query).await?;

        // TODO: whoa this is probably VERY slow!
        for (_, post_descriptor_db_id) in pd_to_db_id_map {
            transaction.execute(
                &statement,
                &[&post_reply.owner_account_id, &post_descriptor_db_id]
            ).await?;
        }
    }

    transaction.commit().await?;

    return Ok(());
}

pub async fn get_unsent_replies(
    is_dev_build: bool,
    database: &Arc<Database>
) -> anyhow::Result<HashMap<String, HashSet<UnsentReply>>> {
    let query = r#"
        SELECT
            unsent_post_reply.id_generated,
            unsent_post_reply.firebase_token,
            unsent_post_reply.site_name,
            unsent_post_reply.board_code,
            unsent_post_reply.thread_no,
            unsent_post_reply.post_no,
            unsent_post_reply.post_sub_no
        FROM
        (
            SELECT
                post_replies.id_generated,
                account.firebase_token,
                post_descriptor.site_name,
                post_descriptor.board_code,
                post_descriptor.thread_no,
                post_descriptor.post_no,
                post_descriptor.post_sub_no
            FROM post_replies
            LEFT JOIN accounts account
                ON account.id_generated = post_replies.owner_account_id
            LEFT JOIN post_descriptors post_descriptor
                ON post_replies.owner_post_descriptor_id = post_descriptor.id_generated
            WHERE
                post_replies.deleted_on IS NULL
            AND
                post_replies.notification_sent_on IS NULL
            AND
                account.firebase_token IS NOT NULL
            AND
                account.valid_until > now()
            AND
                account.deleted_on IS NULL
        ) AS unsent_post_reply
"#;

    let connection = database.connection().await?;
    let rows = connection.query(query, &[]).await?;

    if rows.is_empty() {
        info!("No unsent replies found");
        return Ok(HashMap::new());
    }

    let mut unsent_replies = HashMap::<String, HashSet<UnsentReply>>::with_capacity(rows.len());
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

        if !unsent_replies.contains_key(&unsent_reply.firebase_token) {
            unsent_replies.insert(unsent_reply.firebase_token.clone(), HashSet::with_capacity(16));
        }

        unsent_replies.get_mut(&unsent_reply.firebase_token)
            .unwrap()
            .insert(unsent_reply.clone());
    }

    return Ok(unsent_replies);
}

pub async fn mark_post_replies_as_notified(
    sent_post_reply_ids: Vec<i64>,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    info!("mark_post_replies_as_notified() Got {} sent_post_reply_ids", sent_post_reply_ids.len());

    if sent_post_reply_ids.is_empty() {
        return Ok(());
    }

    let query = r#"
        UPDATE post_replies
        SET notification_sent_on = now()
        WHERE id_generated IN ({QUERY_PARAMS})
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