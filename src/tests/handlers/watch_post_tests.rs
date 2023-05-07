#[cfg(test)]
mod tests {
    use crate::handlers::shared::EmptyResponse;
    use crate::model::repository::account_repository::{AccountId, ApplicationType};
    use crate::test_case;
    use crate::tests::shared::{account_repository_shared, database_shared, watch_post_repository_shared};
    use crate::tests::shared::server_shared::TEST_MASTER_PASSWORD;
    use crate::tests::shared::shared::{run_test, TestCase};

    #[tokio::test]
    async fn run_tests() {
        let tests: Vec<TestCase> = vec![
            test_case!(should_not_watch_post_if_account_does_not_exist),
            test_case!(should_not_watch_post_if_account_is_expired),
            test_case!(should_not_watch_post_if_site_is_not_supported),
            test_case!(should_not_watch_post_if_link_is_unparseable),
            test_case!(should_not_watch_post_if_link_is_too_short),
            test_case!(should_not_watch_post_if_link_is_too_long),
            test_case!(should_start_watching_post_if_params_are_good),
            test_case!(should_not_create_duplicates_when_one_post_is_watched_multiple_times),
        ];

        run_test(tests).await;
    }

    async fn should_not_watch_post_if_account_does_not_exist() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
            user_id1,
            "https://boards.4channel.org/vg/thread/426895061#p426901491",
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("Account does not exist", server_response.error.unwrap());
    }

    async fn should_not_watch_post_if_account_is_expired() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        account_repository_shared::create_expired_account::<EmptyResponse>(
            TEST_MASTER_PASSWORD,
            user_id1,
            1
        ).await.unwrap();

        let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
            user_id1,
            "https://boards.4channel.org/vg/thread/426895061#p426901491",
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!(
            "Account has no token",
            server_response.error.unwrap()
        );
    }

    async fn should_not_watch_post_if_site_is_not_supported() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
            user_id1,
            "https://imageboard.com/vg/thread/426895061#p426901491",
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!(
            "Site for url 'https://imageboard.com/vg/thread/426895061#p426901491' is not supported",
            server_response.error.unwrap()
        );
    }

    async fn should_not_watch_post_if_link_is_unparseable() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
            user_id1,
            "https://boards.4channel.org/vg/thread/4268<BAM>95061#p426901491",
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!(
            "Failed to parse \'https://boards.4channel.org/vg/thread/4268<BAM>95061#p426901491\' url as post url",
            server_response.error.unwrap()
        );
    }

    async fn should_not_watch_post_if_link_is_too_short() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
            user_id1,
            "",
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!(
            "post_url is empty",
            server_response.error.unwrap()
        );
    }

    async fn should_not_watch_post_if_link_is_too_long() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
            user_id1,
            "https://boards.4channel.org/vg/11111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111\
            11111111111111111111111111111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111",
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!(
            "post_url is too long",
            server_response.error.unwrap()
        );
    }

    async fn should_start_watching_post_if_params_are_good() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;
        let user_id2 = &account_repository_shared::TEST_GOOD_USER_ID2;

        let account_id1 = AccountId::test_unsafe(user_id1).unwrap();
        let account_id2 = AccountId::test_unsafe(user_id2).unwrap();

        account_repository_shared::create_account_actual(
            TEST_MASTER_PASSWORD,
            user_id1
        ).await;

        account_repository_shared::update_firebase_token::<EmptyResponse>(
            TEST_MASTER_PASSWORD,
            user_id1,
            &account_repository_shared::TEST_GOOD_FIREBASE_TOKEN1,
            &application_type
        ).await.unwrap();

        account_repository_shared::create_account_actual(
            TEST_MASTER_PASSWORD,
            user_id2
        ).await;

        account_repository_shared::update_firebase_token::<EmptyResponse>(
            TEST_MASTER_PASSWORD,
            user_id2,
            &account_repository_shared::TEST_GOOD_FIREBASE_TOKEN2,
            &application_type
        ).await.unwrap();

        let database = database_shared::database();

        {
            let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
                user_id1,
                "https://boards.4channel.org/vg/thread/426895061#p426901491",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let test_post_watches = watch_post_repository_shared::get_post_watches_from_database(
                &account_id1,
                database
            )
                .await
                .unwrap();

            assert_eq!(1, test_post_watches.len());

            let test_post_watch = test_post_watches.first().unwrap();
            assert_eq!(account_id1.id, test_post_watch.account_id.id);
            assert_eq!(426895061, test_post_watch.post_descriptor.thread_no());
            assert_eq!(426901491, test_post_watch.post_descriptor.post_no);
        }

        {
            let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
                user_id2,
                "https://boards.4channel.org/vg/thread/426895061#p426901492",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let test_post_watches = watch_post_repository_shared::get_post_watches_from_database(
                &account_id2,
                database
            )
                .await
                .unwrap();

            assert_eq!(1, test_post_watches.len());

            let test_post_watch = test_post_watches.first().unwrap();
            assert_eq!(account_id2.id, test_post_watch.account_id.id);
            assert_eq!(426895061, test_post_watch.post_descriptor.thread_no());
            assert_eq!(426901492, test_post_watch.post_descriptor.post_no);
        }
    }

    async fn should_not_create_duplicates_when_one_post_is_watched_multiple_times() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;
        let user_id2 = &account_repository_shared::TEST_GOOD_USER_ID2;

        let account_id1 = AccountId::test_unsafe(user_id1).unwrap();
        let account_id2 = AccountId::test_unsafe(user_id2).unwrap();

        account_repository_shared::create_account_actual(
            TEST_MASTER_PASSWORD,
            user_id1
        ).await;

        account_repository_shared::update_firebase_token::<EmptyResponse>(
            TEST_MASTER_PASSWORD,
            user_id1,
            &account_repository_shared::TEST_GOOD_FIREBASE_TOKEN1,
            &application_type
        ).await.unwrap();

        account_repository_shared::create_account_actual(
            TEST_MASTER_PASSWORD,
            user_id2
        ).await;

        account_repository_shared::update_firebase_token::<EmptyResponse>(
            TEST_MASTER_PASSWORD,
            user_id2,
            &account_repository_shared::TEST_GOOD_FIREBASE_TOKEN2,
            &application_type
        ).await.unwrap();

        let database = database_shared::database();

        {
            let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
                user_id1,
                "https://boards.4channel.org/vg/thread/426895061#p426901491",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
                user_id1,
                "https://boards.4channel.org/vg/thread/426895061#p426901491",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let test_post_watches = watch_post_repository_shared::get_post_watches_from_database(
                &account_id1,
                database
            )
                .await
                .unwrap();

            assert_eq!(1, test_post_watches.len());

            let test_post_watch = test_post_watches.first().unwrap();
            assert_eq!(account_id1.id, test_post_watch.account_id.id);
            assert_eq!(426895061, test_post_watch.post_descriptor.thread_no());
            assert_eq!(426901491, test_post_watch.post_descriptor.post_no);
        }

        {
            let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
                user_id2,
                "https://boards.4channel.org/vg/thread/426895061#p426901492",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let server_response = watch_post_repository_shared::watch_post::<EmptyResponse>(
                user_id2,
                "https://boards.4channel.org/vg/thread/426895061#p426901492",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let test_post_watches = watch_post_repository_shared::get_post_watches_from_database(
                &account_id2,
                database
            )
                .await
                .unwrap();

            assert_eq!(1, test_post_watches.len());

            let test_post_watch = test_post_watches.first().unwrap();
            assert_eq!(account_id2.id, test_post_watch.account_id.id);
            assert_eq!(426895061, test_post_watch.post_descriptor.thread_no());
            assert_eq!(426901492, test_post_watch.post_descriptor.post_no);
        }
    }

}