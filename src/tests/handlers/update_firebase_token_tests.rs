#[cfg(test)]
mod tests {
    use crate::handlers::shared::EmptyResponse;
    use crate::model::repository::account_repository::{AccountId, ApplicationType};
    use crate::test_case;
    use crate::tests::shared::{account_repository_shared, database_shared};
    use crate::tests::shared::shared::{run_test, TestCase};

    #[tokio::test]
    async fn run_tests() {
        let tests: Vec<TestCase> = vec![
            test_case!(should_not_update_firebase_token_if_account_does_not_exist),
            test_case!(should_not_update_firebase_token_if_token_is_too_short),
            test_case!(should_not_update_firebase_token_if_token_is_too_long),
            test_case!(should_update_token_if_params_are_good),
        ];

        run_test(tests).await;
    }

    async fn should_not_update_firebase_token_if_account_does_not_exist() {
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;
        let application_type = ApplicationType::KurobaExLiteDebug;

        let server_response = account_repository_shared::update_firebase_token::<EmptyResponse>(
            user_id1,
            "test123",
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("Account does not exist", server_response.error.unwrap());
    }

    async fn should_not_update_firebase_token_if_token_is_too_short() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        account_repository_shared::create_account_actual(
            user_id1
        ).await;

        let server_response = account_repository_shared::update_firebase_token::<EmptyResponse>(
            user_id1,
            &account_repository_shared::TEST_VERY_SHORT_FIREBASE_TOKEN,
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("Bad token length 0 must be within 1..1024", server_response.error.unwrap());
    }

    async fn should_not_update_firebase_token_if_token_is_too_long() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        account_repository_shared::create_account_actual(
            user_id1
        ).await;

        let server_response = account_repository_shared::update_firebase_token::<EmptyResponse>(
            user_id1,
            &account_repository_shared::TEST_VERY_LONG_FIREBASE_TOKEN,
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("Bad token length 1610 must be within 1..1024", server_response.error.unwrap());
    }

    async fn should_update_token_if_params_are_good() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;
        let user_id2 = &account_repository_shared::TEST_GOOD_USER_ID2;
        let account_id1 = AccountId::from_user_id(user_id1).unwrap();
        let account_id2 = AccountId::from_user_id(user_id2).unwrap();
        let database = database_shared::database();

        account_repository_shared::create_account_actual(
            user_id1
        ).await;

        account_repository_shared::create_account_actual(
            user_id2
        ).await;

        {
            let server_response = account_repository_shared::update_firebase_token::<EmptyResponse>(
                user_id1,
                "good token 1",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let from_cache = account_repository_shared::get_account_from_cache(user_id1)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(1, from_cache.id);
            assert_eq!(account_id1.id, from_cache.account_id.id);
            assert_eq!("good token 1", &from_cache.account_token(&application_type).unwrap().token);
            assert!(&from_cache.valid_until.is_some());

            let from_database = account_repository_shared::get_account_from_database(user_id1, database)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(1, from_database.id);
            assert_eq!(account_id1.id, from_database.account_id.id);
            assert_eq!("good token 1", &from_database.account_token(&application_type).unwrap().token);
            assert!(&from_database.valid_until.is_some());
        }

        {
            let server_response = account_repository_shared::update_firebase_token::<EmptyResponse>(
                user_id2,
                "good token 2",
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());


            let from_cache = account_repository_shared::get_account_from_cache(user_id2)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(2, from_cache.id);
            assert_eq!(account_id2.id, from_cache.account_id.id);
            assert_eq!("good token 2", &from_cache.account_token(&application_type).unwrap().token);
            assert!(&from_cache.valid_until.is_some());

            let from_database = account_repository_shared::get_account_from_database(user_id2, database)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(2, from_database.id);
            assert_eq!(account_id2.id, from_database.account_id.id);
            assert_eq!("good token 2", &from_database.account_token(&application_type).unwrap().token);
            assert!(&from_database.valid_until.is_some());
        }
    }
}