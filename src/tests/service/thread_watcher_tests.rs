#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
    use crate::model::database::db::Database;
    use crate::model::repository::{account_repository, post_descriptor_id_repository, post_repository};
    use crate::model::repository::account_repository::{AccountId, FirebaseToken};
    use crate::service::thread_watcher;
    use crate::service::thread_watcher::FoundPostReply;
    use crate::test_case;
    use crate::tests::shared::database_shared;
    use crate::tests::shared::shared::{run_test, TestCase};

    struct PostReply {
        account_id: AccountId,
        post_descriptor: PostDescriptor
    }

    #[tokio::test]
    async fn run_tests() {
        let tests: Vec<TestCase> = vec![
            test_case!(test_one_account_watches_one_post),
            test_case!(test_two_accounts_watch_two_posts),
            test_case!(test_two_accounts_watch_the_same_post),
        ];

        run_test(tests).await;
    }

    async fn test_one_account_watches_one_post() {
        let database = database_shared::database();

        let account_id = AccountId::from_user_id("111111111111111111111111111111111111").unwrap();
        let firebase_token = FirebaseToken::from_str("1234567890").unwrap();
        let thread_descriptor = ThreadDescriptor::new("test".to_string(), "test".to_string(), 1);
        let watched_post = PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 1, 0);

        let mut found_post_replies_set = HashSet::from(
            [
                FoundPostReply {
                    origin: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 2, 0),
                    replies_to: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 1, 0),
                }
            ]
        );

        {
            let valid_until = chrono::offset::Utc::now() + chrono::Duration::days(1);

            account_repository::create_account(
                database,
                &account_id,
                Some(valid_until)
            ).await.unwrap();

            account_repository::update_firebase_token(
                database,
                &account_id,
                &firebase_token
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id,
                &watched_post
            ).await.unwrap();
        }

        thread_watcher::find_and_store_new_post_replies(
            &thread_descriptor,
            &mut found_post_replies_set,
            database,
        ).await.unwrap();

        let post_replies = load_post_replies(database).await.unwrap();

        assert_eq!(1, post_replies.len());
        let post_reply = post_replies.first().unwrap();

        assert_eq!(account_id.id, post_reply.account_id.id);
        assert_eq!(thread_descriptor, post_reply.post_descriptor.thread_descriptor);
        assert_eq!(2, post_reply.post_descriptor.post_no);
    }

    async fn test_two_accounts_watch_two_posts() {
        let database = database_shared::database();

        let account_id1 = AccountId::from_user_id("111111111111111111111111111111111111").unwrap();
        let account_id2 = AccountId::from_user_id("222222222222222222222222222222222222").unwrap();
        let firebase_token1 = FirebaseToken::from_str("1234567890").unwrap();
        let firebase_token2 = FirebaseToken::from_str("0987654321").unwrap();
        let thread_descriptor = ThreadDescriptor::new("test".to_string(), "test".to_string(), 1);
        let watched_post1 = PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 1, 0);
        let watched_post2 = PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 2, 0);

        let mut found_post_replies_set = HashSet::from(
            [
                FoundPostReply {
                    origin: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 3, 0),
                    replies_to: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 1, 0),
                },
                FoundPostReply {
                    origin: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 4, 0),
                    replies_to: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 2, 0),
                }
            ]
        );

        {
            let valid_until = chrono::offset::Utc::now() + chrono::Duration::days(1);

            account_repository::create_account(
                database,
                &account_id1,
                Some(valid_until)
            ).await.unwrap();

            account_repository::create_account(
                database,
                &account_id2,
                Some(valid_until)
            ).await.unwrap();

            account_repository::update_firebase_token(
                database,
                &account_id1,
                &firebase_token1
            ).await.unwrap();

            account_repository::update_firebase_token(
                database,
                &account_id2,
                &firebase_token2
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id1,
                &watched_post1
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id2,
                &watched_post2
            ).await.unwrap();
        }

        thread_watcher::find_and_store_new_post_replies(
            &thread_descriptor,
            &mut found_post_replies_set,
            database,
        ).await.unwrap();

        let post_replies = load_post_replies(database).await.unwrap();

        assert_eq!(2, post_replies.len());
        let post_reply1 = post_replies.first().unwrap();
        let post_reply2 = post_replies.last().unwrap();

        assert_eq!(account_id1.id, post_reply1.account_id.id);
        assert_eq!(thread_descriptor, post_reply1.post_descriptor.thread_descriptor);
        assert_eq!(3, post_reply1.post_descriptor.post_no);

        assert_eq!(account_id2.id, post_reply2.account_id.id);
        assert_eq!(thread_descriptor, post_reply2.post_descriptor.thread_descriptor);
        assert_eq!(4, post_reply2.post_descriptor.post_no);
    }

    async fn test_two_accounts_watch_the_same_post() {
        let database = database_shared::database();

        let account_id1 = AccountId::from_user_id("111111111111111111111111111111111111").unwrap();
        let account_id2 = AccountId::from_user_id("222222222222222222222222222222222222").unwrap();
        let firebase_token1 = FirebaseToken::from_str("1234567890").unwrap();
        let firebase_token2 = FirebaseToken::from_str("0987654321").unwrap();
        let thread_descriptor = ThreadDescriptor::new("test".to_string(), "test".to_string(), 1);
        let watched_post = PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 1, 0);

        let mut found_post_replies_set = HashSet::from(
            [
                FoundPostReply {
                    origin: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 2, 0),
                    replies_to: PostDescriptor::from_thread_descriptor(thread_descriptor.clone(), 1, 0),
                }
            ]
        );

        {
            let valid_until = chrono::offset::Utc::now() + chrono::Duration::days(1);

            account_repository::create_account(
                database,
                &account_id1,
                Some(valid_until)
            ).await.unwrap();

            account_repository::create_account(
                database,
                &account_id2,
                Some(valid_until)
            ).await.unwrap();

            account_repository::update_firebase_token(
                database,
                &account_id1,
                &firebase_token1
            ).await.unwrap();

            account_repository::update_firebase_token(
                database,
                &account_id2,
                &firebase_token2
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id1,
                &watched_post
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id2,
                &watched_post
            ).await.unwrap();
        }

        thread_watcher::find_and_store_new_post_replies(
            &thread_descriptor,
            &mut found_post_replies_set,
            database,
        ).await.unwrap();

        let post_replies = load_post_replies(database).await.unwrap();

        assert_eq!(2, post_replies.len());
        let post_reply1 = post_replies.first().unwrap();
        let post_reply2 = post_replies.last().unwrap();

        assert_eq!(account_id1.id, post_reply1.account_id.id);
        assert_eq!(thread_descriptor, post_reply1.post_descriptor.thread_descriptor);
        assert_eq!(2, post_reply1.post_descriptor.post_no);

        assert_eq!(account_id2.id, post_reply2.account_id.id);
        assert_eq!(thread_descriptor, post_reply2.post_descriptor.thread_descriptor);
        assert_eq!(2, post_reply2.post_descriptor.post_no);
    }

    async fn load_post_replies(database: &Arc<Database>) -> anyhow::Result<Vec<PostReply>> {
        let query = r#"
            SELECT owner_account_id, owner_post_descriptor_id
            FROM post_replies
        "#;

        let connection = database.connection().await?;
        let statement = connection.prepare(query).await?;

        let rows = connection.query(&statement, &[]).await?;
        if rows.is_empty() {
            return Ok(vec![]);
        }

        let mut result_vec = Vec::<PostReply>::with_capacity(rows.len());

        for row in rows {
            let owner_account_id: i64 = row.get(0);
            let owner_post_descriptor_id: i64 = row.get(1);

            let query = r#"
                SELECT account_id
                FROM accounts
                WHERE accounts.id_generated = $1
            "#;

            let statement = connection.prepare(query).await?;

            let account_id_row = connection.query_one(&statement, &[&owner_account_id]).await?;
            let account_id_string: String = account_id_row.get(0);
            let account_id = AccountId::new(account_id_string);

            let query = r#"
                SELECT site_name, board_code, thread_no, post_no, post_sub_no
                FROM post_descriptors
                WHERE post_descriptors.id_generated = $1
            "#;

            let statement = connection.prepare(query).await?;

            let rows = connection.query(&statement, &[&owner_post_descriptor_id]).await?;

            let post_descriptors = rows.iter().map(|pd_row| {
                let site_name: String = pd_row.get(0);
                let board_code: String = pd_row.get(1);
                let thread_no: i64 = pd_row.get(2);
                let post_no: i64 = pd_row.get(3);
                let post_sub_no: i64 = pd_row.get(4);

                let post_descriptor = PostDescriptor::new(
                    site_name,
                    board_code,
                    thread_no as u64,
                    post_no as u64,
                    post_sub_no as u64
                );

                return post_descriptor;
            }).collect::<Vec<PostDescriptor>>();

            post_descriptors.iter().for_each(|pd| {
                let post_reply = PostReply {
                    account_id: account_id.clone(),
                    post_descriptor: pd.clone(),
                };

                result_vec.push(post_reply);
            });
        }

        return Ok(result_vec);
    }
}