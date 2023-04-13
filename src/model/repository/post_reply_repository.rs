use std::sync::Arc;
use tokio_postgres::types::ToSql;
use crate::model::database::db::Database;

pub struct PostReply {
    pub owner_post_descriptor_id: i64,
    pub owner_account_id: i64,
}

pub async fn store(
    post_replies: &Vec<PostReply>,
    database: &Arc<Database>
) -> anyhow::Result<()> {
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
        let statement = transaction.prepare(query).await?;

        transaction.execute(
            &statement,
            &[&post_reply.owner_account_id, &post_reply.owner_post_descriptor_id]
        ).await?;
    }

    transaction.commit().await?;

    return Ok(());
}