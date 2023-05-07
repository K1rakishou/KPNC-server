use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::handlers::shared::{ServerResponse, ServerSuccessResponse};
use crate::handlers::watch_post::WatchPostRequest;
use crate::model::data::chan::PostDescriptor;
use crate::model::database::db::Database;
use crate::model::repository::account_repository::{AccountId, ApplicationType};
use crate::tests::shared::http_client_shared;

pub struct TestPostWatch {
    pub account_id: AccountId,
    pub post_descriptor: PostDescriptor
}

pub async fn watch_post<'a, T : DeserializeOwned + ServerSuccessResponse>(
    user_id: &str,
    post_url: &str,
    application_type: &ApplicationType
) -> anyhow::Result<ServerResponse<T>> {
    let request = WatchPostRequest {
        user_id: user_id.to_string(),
        post_url: post_url.to_string(),
        application_type: application_type.clone()
    };

    let body = serde_json::to_string(&request).unwrap();

    let response = http_client_shared::post_request::<ServerResponse<T>>(
        "watch_post",
        &body
    ).await?;

    return Ok(response);
}

pub async fn get_post_watches_from_database(
    account_id: &AccountId,
    database: &Arc<Database>
) -> anyhow::Result<Vec<TestPostWatch>> {
    let query = r#"
        SELECT
            pd.site_name,
            pd.board_code,
            pd.thread_no,
            pd.post_no,
            pd.post_sub_no
        FROM post_watches
        INNER JOIN accounts account on account.id = post_watches.owner_account_id
        INNER JOIN posts post on post.id = post_watches.owner_post_id
        INNER JOIN post_descriptors pd on pd.id = post.owner_post_descriptor_id
        WHERE account.account_id = $1
    "#;

    let connection = database.connection().await?;
    let statement = connection.prepare(query).await?;

    let rows = connection.query(&statement, &[&account_id.id]).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }

    let mut result_vec = Vec::<TestPostWatch>::with_capacity(rows.len());

    for row in rows {
        let site_name: &str = row.get(0);
        let board_code: &str = row.get(1);
        let thread_no: i64 = row.get(2);
        let post_no: i64 = row.get(3);
        let post_sub_no: i64 = row.get(4);

        let post_descriptor = PostDescriptor::new(
            site_name.to_string(),
            board_code.to_string(),
            thread_no as u64,
            post_no as u64,
            post_sub_no as u64
        );

        let test_post_watch = TestPostWatch {
            account_id: account_id.clone(),
            post_descriptor
        };

        result_vec.push(test_post_watch);
    }

    return Ok(result_vec);
}