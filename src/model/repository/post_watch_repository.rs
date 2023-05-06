use std::sync::Arc;

use crate::info;
use crate::model::database::db::Database;
use crate::model::repository::{account_repository, post_descriptor_id_repository, post_reply_repository};
use crate::model::repository::account_repository::AccountId;

pub async fn mark_post_replies_as_notified(
    account_id: &AccountId,
    reply_ids: &Vec<u64>,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    let reply_ids = reply_ids.iter()
        .map(|reply_id| *reply_id as i64)
        .collect::<Vec<i64>>();

    let retained_sent_post_reply_ids = account_repository::retain_post_db_ids_belonging_to_account(
        account_id,
        &reply_ids,
        database
    ).await?;

    if retained_sent_post_reply_ids.is_empty() {
        info!("mark_post_replies_as_notified() retain_post_db_ids_belonging_to_account() \
            returned empty vec");
        return Ok(());
    }

    post_reply_repository::mark_post_replies_as_notified(
        &retained_sent_post_reply_ids,
        database
    ).await?;

    return Ok(());
}