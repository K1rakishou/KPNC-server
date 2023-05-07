#[cfg(test)]
mod tests {
    use crate::handlers::get_account_info::AccountInfoResponse;
    use crate::handlers::shared::EmptyResponse;
    use crate::model::repository::account_repository::{AccountId, ApplicationType};
    use crate::test_case;
    use crate::tests::shared::{account_repository_shared, database_shared};
    use crate::tests::shared::shared::{run_test, TestCase};

    #[tokio::test]
    async fn run_tests() {
        let tests: Vec<TestCase> = vec![
            test_case!(should_return_nothing_if_account_does_not_exist),
            test_case!(should_return_account_info_if_account_exists),
        ];

        run_test(tests).await;
    }

    async fn should_return_nothing_if_account_does_not_exist() {
        let application_type = ApplicationType::KurobaExLiteDebug;
        let user_id1 = &account_repository_shared::TEST_GOOD_USER_ID1;

        let server_response = account_repository_shared::get_account_info::<EmptyResponse>(
            user_id1,
            &application_type
        ).await.unwrap();

        assert!(server_response.data.is_none());
        assert!(server_response.error.is_some());
        assert_eq!("Account does not exist", server_response.error.unwrap());
    }

    async fn should_return_account_info_if_account_exists() {
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
            let server_response = account_repository_shared::get_account_info::<AccountInfoResponse>(
                user_id1,
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let account_info_response = server_response.data.unwrap();
            assert_eq!(true, account_info_response.is_valid);
            assert_eq!(false, account_info_response.valid_until.is_none());

            let from_cache = account_repository_shared::get_account_from_cache(user_id1)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(1, from_cache.id);
            assert_eq!(account_id1.id, from_cache.account_id.id);
            assert!(&from_cache.account_token(&application_type).is_none());
            assert!(&from_cache.valid_until.is_some());

            let from_database = account_repository_shared::get_account_from_database(user_id1, database)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(1, from_database.id);
            assert_eq!(account_id1.id, from_database.account_id.id);
            assert!(&from_database.account_token(&application_type).is_none());
            assert!(&from_database.valid_until.is_some());
        }

        {
            let server_response = account_repository_shared::get_account_info::<AccountInfoResponse>(
                user_id2,
                &application_type
            ).await.unwrap();

            assert!(server_response.data.is_some());
            assert!(server_response.error.is_none());

            let account_info_response = server_response.data.unwrap();
            assert_eq!(true, account_info_response.is_valid);
            assert_eq!(false, account_info_response.valid_until.is_none());

            let from_cache = account_repository_shared::get_account_from_cache(user_id2)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(2, from_cache.id);
            assert_eq!(account_id2.id, from_cache.account_id.id);
            assert!(&from_cache.account_token(&application_type).is_none());
            assert!(&from_cache.valid_until.is_some());

            let from_database = account_repository_shared::get_account_from_database(user_id2, database)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(2, from_database.id);
            assert_eq!(account_id2.id, from_database.account_id.id);
            assert!(&from_database.account_token(&application_type).is_none());
            assert!(&from_database.valid_until.is_some());
        }
    }

}