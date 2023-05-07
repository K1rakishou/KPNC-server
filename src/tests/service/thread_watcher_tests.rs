#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
    use crate::model::repository::{account_repository, post_reply_repository, post_repository};
    use crate::model::repository::account_repository::{AccountId, AccountToken, ApplicationType, FirebaseToken, TokenType};
    use crate::service::thread_watcher;
    use crate::service::thread_watcher::FoundPostReply;
    use crate::test_case;
    use crate::tests::shared::database_shared;
    use crate::tests::shared::shared::{run_test, TestCase};

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
        let application_type = ApplicationType::KurobaExLiteDebug;
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
                &application_type,
                &firebase_token
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id,
                &application_type,
                &watched_post
            ).await.unwrap();
        }

        thread_watcher::find_and_store_new_post_replies(
            &thread_descriptor,
            &mut found_post_replies_set,
            database,
        ).await.unwrap();

        let unsent_replies = post_reply_repository::get_unsent_replies(
            true,
            database
        ).await.unwrap();

        assert_eq!(1, unsent_replies.len());

        let replies = unsent_replies.iter()
            .take(1)
            .collect::<Vec<_>>();
        let (account_token, unsent_replies_set) = replies.first().unwrap();

        assert_eq!(firebase_token.token, account_token.token);
        assert_eq!(application_type, account_token.application_type);
        assert_eq!(TokenType::Firebase, account_token.token_type);

        assert_eq!(1, unsent_replies_set.len());
        let unsent_reply = unsent_replies_set.iter().next().unwrap();

        assert_eq!(1, unsent_reply.post_reply_id);
        assert_eq!(2, unsent_reply.post_descriptor.post_no);
    }

    async fn test_two_accounts_watch_two_posts() {
        let application_type = ApplicationType::KurobaExLiteDebug;
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

            account_repository::update_firebase_token(
                database,
                &account_id1,
                &application_type,
                &firebase_token1
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id1,
                &application_type,
                &watched_post1
            ).await.unwrap();

            account_repository::create_account(
                database,
                &account_id2,
                Some(valid_until)
            ).await.unwrap();

            account_repository::update_firebase_token(
                database,
                &account_id2,
                &application_type,
                &firebase_token2
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id2,
                &application_type,
                &watched_post2
            ).await.unwrap();
        }

        thread_watcher::find_and_store_new_post_replies(
            &thread_descriptor,
            &mut found_post_replies_set,
            database,
        ).await.unwrap();

        let unsent_replies = post_reply_repository::get_unsent_replies(
            true,
            database
        ).await.unwrap();

        assert_eq!(2, unsent_replies.len());

        {
            let (account_token, unsent_replies_set) = unsent_replies
                .iter()
                .find(|(token, _)| token.token == firebase_token1.token)
                .unwrap();

            assert_eq!(firebase_token1.token, account_token.token);
            assert_eq!(application_type, account_token.application_type);
            assert_eq!(TokenType::Firebase, account_token.token_type);

            let unsent_reply = unsent_replies_set
                .iter()
                .find(|unsent_reply| unsent_reply.post_reply_id == 2)
                .unwrap();

            assert_eq!(2, unsent_reply.post_reply_id);
            assert_eq!(3, unsent_reply.post_descriptor.post_no);
        }

        {
            let (account_token, unsent_replies_set) = unsent_replies
                .iter()
                .find(|(token, _)| token.token == firebase_token2.token)
                .unwrap();

            assert_eq!(firebase_token2.token, account_token.token);
            assert_eq!(application_type, account_token.application_type);
            assert_eq!(TokenType::Firebase, account_token.token_type);

            let unsent_reply = unsent_replies_set
                .iter()
                .find(|unsent_reply| unsent_reply.post_reply_id == 1)
                .unwrap();

            assert_eq!(1, unsent_reply.post_reply_id);
            assert_eq!(4, unsent_reply.post_descriptor.post_no);
        }
    }

    async fn test_two_accounts_watch_the_same_post() {
        let application_type = ApplicationType::KurobaExLiteDebug;
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
                &application_type,
                &firebase_token1
            ).await.unwrap();

            account_repository::update_firebase_token(
                database,
                &account_id2,
                &application_type,
                &firebase_token2
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id1,
                &application_type,
                &watched_post
            ).await.unwrap();

            post_repository::start_watching_post(
                database,
                &account_id2,
                &application_type,
                &watched_post
            ).await.unwrap();
        }

        thread_watcher::find_and_store_new_post_replies(
            &thread_descriptor,
            &mut found_post_replies_set,
            database,
        ).await.unwrap();

        let unsent_replies = post_reply_repository::get_unsent_replies(
            true,
            database
        ).await.unwrap();

        assert_eq!(2, unsent_replies.len());

        {
            let (account_token, unsent_replies_set) = unsent_replies
                .iter()
                .find(|(token, _)| token.token == firebase_token1.token)
                .unwrap();

            assert_eq!(firebase_token1.token, account_token.token);
            assert_eq!(application_type, account_token.application_type);
            assert_eq!(TokenType::Firebase, account_token.token_type);

            let unsent_reply = unsent_replies_set
                .iter()
                .find(|unsent_reply| unsent_reply.post_reply_id == 1)
                .unwrap();

            assert_eq!(1, unsent_reply.post_reply_id);
            assert_eq!(2, unsent_reply.post_descriptor.post_no);
        }

        {
            let (account_token, unsent_replies_set) = unsent_replies
                .iter()
                .find(|(token, _)| token.token == firebase_token2.token)
                .unwrap();

            assert_eq!(firebase_token2.token, account_token.token);
            assert_eq!(application_type, account_token.application_type);
            assert_eq!(TokenType::Firebase, account_token.token_type);

            let unsent_reply = unsent_replies_set
                .iter()
                .find(|unsent_reply| unsent_reply.post_reply_id == 2)
                .unwrap();

            assert_eq!(2, unsent_reply.post_reply_id);
            assert_eq!(2, unsent_reply.post_descriptor.post_no);
        }
    }

}