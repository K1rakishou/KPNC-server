#[cfg(test)]
mod tests {
    use crate::handlers::shared::EmptyResponse;
    use crate::make_test;
    use crate::model::repository::account_repository;
    use crate::model::repository::account_repository::AccountId;
    use crate::tests::shared::{account_repository_shared, database_shared};
    use crate::tests::shared::shared::{assert_none, assert_some, run_test, TestCase};

    #[tokio::test]
    async fn run_tests() {
        let tests: Vec<TestCase> = vec![
            make_test!(should_not_create_account_when_user_id_is_too_short),
            make_test!(should_not_create_account_when_user_id_is_too_long),
            make_test!(should_not_create_account_when_valid_for_days_is_zero),
            make_test!(should_not_create_account_when_valid_for_days_is_too_big),
            make_test!(should_not_create_account_with_the_same_id_more_than_once),
            make_test!(should_create_account_when_parameters_are_good),
            make_test!(should_create_multiple_accounts_when_parameters_are_good),
        ];

        run_test(tests).await;
    }

    async fn should_not_create_account_when_user_id_is_too_short() {
        let user_id = &account_repository_shared::TEST_BAD_USER_ID1;
        let database = database_shared::database();

        let server_response = account_repository_shared::create_account::<EmptyResponse>(
            user_id,
            1
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("Bad user_id length 31 must be within 32..128 symbols", server_response.error.unwrap());

        let from_cache = account_repository_shared::get_account_from_cache(user_id)
            .await
            .unwrap();
        assert_none(&from_cache);

        let from_database = account_repository_shared::get_account_from_database(user_id, database)
            .await
            .unwrap();
        assert_none(&from_database);
    }

    async fn should_not_create_account_when_user_id_is_too_long() {
        let user_id = &account_repository_shared::TEST_BAD_USER_ID2;
        let database = database_shared::database();

        let server_response = account_repository_shared::create_account::<EmptyResponse>(
            user_id,
            1
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("Bad user_id length 129 must be within 32..128 symbols", server_response.error.unwrap());

        let from_cache = account_repository_shared::get_account_from_cache(user_id)
            .await
            .unwrap();
        assert_none(&from_cache);

        let from_database = account_repository_shared::get_account_from_database(user_id, database)
            .await
            .unwrap();
        assert_none(&from_database);
    }

    async fn should_not_create_account_when_valid_for_days_is_zero() {
        let user_id = &account_repository_shared::TEST_GOOD_USER_ID1;
        let database = database_shared::database();

        let server_response = account_repository_shared::create_account::<EmptyResponse>(
            user_id,
            0
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("valid_for_days must be in range 0..365", server_response.error.unwrap());

        let from_cache = account_repository_shared::get_account_from_cache(user_id)
            .await
            .unwrap();
        assert_none(&from_cache);

        let from_database = account_repository_shared::get_account_from_database(user_id, database)
            .await
            .unwrap();
        assert_none(&from_database);
    }

    async fn should_not_create_account_when_valid_for_days_is_too_big() {
        let user_id = &account_repository_shared::TEST_GOOD_USER_ID1;
        let database = database_shared::database();

        let server_response = account_repository_shared::create_account::<EmptyResponse>(
            user_id,
            1000
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("valid_for_days must be in range 0..365", server_response.error.unwrap());

        let from_cache = account_repository_shared::get_account_from_cache(user_id)
            .await
            .unwrap();
        assert_none(&from_cache);

        let from_database = account_repository_shared::get_account_from_database(user_id, database)
            .await
            .unwrap();
        assert_none(&from_database);
    }

    async fn should_not_create_account_with_the_same_id_more_than_once() {
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;
        let database = database_shared::database();

        {
            let server_response = account_repository_shared::create_account::<EmptyResponse>(
                user_id1,
                1,
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let server_response = account_repository_shared::create_account::<EmptyResponse>(
                user_id1,
                1,
            ).await.unwrap();

            assert!(server_response.data.is_none());
            assert!(server_response.error.is_some());

            assert_eq!("Account already exists", server_response.error.unwrap());
        }

        let accounts_count_in_db = account_repository::test_count_accounts_in_database(database).await.unwrap();
        assert_eq!(1, accounts_count_in_db);

        let accounts_count_in_cache = account_repository::test_count_accounts_in_cache().await;
        assert_eq!(1, accounts_count_in_cache);
    }

    async fn should_create_account_when_parameters_are_good() {
        let user_id = &account_repository_shared::TEST_GOOD_USER_ID1;
        let account_id = AccountId::from_user_id(user_id).unwrap();
        let database = database_shared::database();

        let server_response = account_repository_shared::create_account::<EmptyResponse>(
            user_id,
            1
        ).await.unwrap();

        assert!(server_response.data.is_some());
        assert!(server_response.error.is_none());

        let from_cache = account_repository_shared::get_account_from_cache(user_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(1, from_cache.id_generated);
        assert_eq!(account_id.id, from_cache.account_id.id);
        assert_none(&from_cache.firebase_token());
        assert_some(&from_cache.valid_until);

        let from_database = account_repository_shared::get_account_from_database(user_id, database)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(1, from_database.id_generated);
        assert_eq!(account_id.id, from_database.account_id.id);
        assert_none(&from_database.firebase_token());
        assert_some(&from_database.valid_until);
    }

    async fn should_create_multiple_accounts_when_parameters_are_good() {
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;
        let user_id2 = &account_repository_shared::TEST_GOOD_USER_ID2;
        let account_id1 = AccountId::from_user_id(user_id1).unwrap();
        let account_id2 = AccountId::from_user_id(user_id2).unwrap();
        let database = database_shared::database();

        {
            let server_response = account_repository_shared::create_account::<EmptyResponse>(
                user_id1,
                1,
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let from_cache = account_repository_shared::get_account_from_cache(user_id1)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(1, from_cache.id_generated);
            assert_eq!(account_id1.id, from_cache.account_id.id);
            assert_none(&from_cache.firebase_token());
            assert_some(&from_cache.valid_until);

            let from_database = account_repository_shared::get_account_from_database(user_id1, database)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(1, from_database.id_generated);
            assert_eq!(account_id1.id, from_database.account_id.id);
            assert_none(&from_database.firebase_token());
            assert_some(&from_database.valid_until);
        }

        {
            let server_response = account_repository_shared::create_account::<EmptyResponse>(
                user_id2,
                1,
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let from_cache = account_repository_shared::get_account_from_cache(user_id2)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(2, from_cache.id_generated);
            assert_eq!(account_id2.id, from_cache.account_id.id);
            assert_none(&from_cache.firebase_token());
            assert_some(&from_cache.valid_until);

            let from_database = account_repository_shared::get_account_from_database(user_id2, database)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(2, from_database.id_generated);
            assert_eq!(account_id2.id, from_database.account_id.id);
            assert_none(&from_database.firebase_token());
            assert_some(&from_database.valid_until);
        }
    }

}