#[cfg(test)]
mod tests {
    use crate::handlers::shared::EmptyResponse;
    use crate::make_test;
    use crate::tests::shared::{account_repository_shared, database_shared};
    use crate::tests::shared::shared::{assert_none, run_test, TestCase};

    #[tokio::test]
    async fn run_tests() {
        let tests: Vec<TestCase> = vec![
            make_test!(should_not_create_account_when_user_id_is_too_short),
            make_test!(should_not_create_account_when_user_id_is_too_long),
        ];

        run_test(tests).await;
    }

    async fn should_not_create_account_when_user_id_is_too_short() {
        let user_id = &account_repository_shared::TEST_BAD_USER_ID1;

        let database = database_shared::database();
        database_shared::cleanup().await;

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
        database_shared::cleanup().await;

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

}