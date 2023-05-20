use std::sync::Arc;

use rand::distributions::Alphanumeric;
use rand::Rng;
use tokio_postgres::Transaction;

use crate::info;
use crate::model::database::db::Database;
use crate::model::repository::account_repository;
use crate::model::repository::account_repository::{AccountId, CreateAccountResult};

pub const NEW_ACCOUNT_TRIAL_PERIOD_DAYS: usize = 7;

pub async fn cleanup(database: &Arc<Database>) -> anyhow::Result<u64> {
    let query = r#"
        DELETE
        FROM invites
        WHERE
            accepted_on IS NULL
        AND
            now() > expires_on
    "#;

    let connection = database.connection().await?;
    let deleted = connection.execute(query, &[]).await?;

    return Ok(deleted);
}

pub async fn generate_invites(
    database: &Arc<Database>,
    amount_to_generate: u8
) -> anyhow::Result<Vec<String>> {
    let mut new_invites = Vec::<String>::with_capacity(amount_to_generate as usize);

    let mut connection = database.connection().await?;
    let transaction = connection.transaction().await?;

    for _ in 0..amount_to_generate {
        let invite_id = generate_invite_id(&transaction).await?;
        create_invite(&invite_id, &transaction).await?;

        new_invites.push(invite_id);
    }

    transaction.commit().await?;
    return Ok(new_invites);
}

pub async fn accept_invite(
    invite: &String,
    database: &Arc<Database>,
) -> anyhow::Result<Option<String>> {
    let mut connection = database.connection().await?;
    let transaction = connection.transaction().await?;

    let exists_and_valid = invite_exists_and_valid(invite, &transaction).await?;
    if !exists_and_valid {
        info!("accept_invite() invite does not exist or not valid, invite: {}", invite);
        return Ok(None);
    }

    mark_invite_as_accepted(invite, &transaction).await?;
    transaction.commit().await?;

    let (user_id, account_id) = generate_account_id(&database).await?;

    let valid_until = chrono::offset::Utc::now() +
        chrono::Duration::days(NEW_ACCOUNT_TRIAL_PERIOD_DAYS as i64);

    let create_account_result = account_repository::create_account(
        database,
        &account_id,
        Some(valid_until)
    ).await?;

    return match create_account_result {
        CreateAccountResult::Ok => {
            info!("accept_invite() success");
            Ok(Some(user_id))
        }
        CreateAccountResult::AccountAlreadyExists => {
            info!("accept_invite() Account already exists, invite: {}", invite);
            Ok(None)
        }
    }
}

async fn mark_invite_as_accepted(
    invite: &String,
    transaction: &Transaction<'_>,
) -> anyhow::Result<()> {
    let query = r#"
        UPDATE invites
        SET accepted_on = now()
        WHERE invite_id = $1
    "#;

    let statement = transaction.prepare(query).await?;
    transaction.execute(&statement, &[&invite]).await?;

    return Ok(());
}

async fn invite_exists_and_valid(
    invite: &String,
    transaction: &Transaction<'_>
) -> anyhow::Result<bool> {
    let query = r#"
        SELECT invite_id
        FROM invites
        WHERE
            invite_id = $1
        AND
            accepted_on IS NULL
        AND
            now() < expires_on
    "#;

    let statement = transaction.prepare(query).await?;
    let exists_and_valid = transaction.query_opt(&statement, &[&invite]).await?.is_some();

    return Ok(exists_and_valid);
}

async fn create_invite(
    invite_id: &String,
    transaction: &Transaction<'_>
) -> anyhow::Result<()> {
    let query = r#"
        INSERT INTO invites
        (
            invite_id,
            expires_on
        )
        VALUES ($1, (now() + interval '1 days'))
    "#;

    transaction.execute(
        query,
        &[
            &invite_id
        ]
    ).await?;

    return Ok(());
}

async fn generate_account_id(
    database: &Arc<Database>
) -> anyhow::Result<(String, AccountId)> {
    let mut user_id: String;

    loop {
        user_id = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(128)
            .map(char::from)
            .collect();

        let account_id = AccountId::from_user_id(&user_id)?;

        let account_does_not_exist = account_repository::get_account_from_database(
            &account_id,
            database
        ).await?.is_none();

        if account_does_not_exist {
            break;
        }
    }

    let account_id = AccountId::from_user_id(&user_id)?;
    return Ok((user_id, account_id));
}

async fn generate_invite_id(transaction: &Transaction<'_>) -> anyhow::Result<String> {
    let mut invite_id: String;

    loop {
        invite_id = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(256)
            .map(char::from)
            .collect();

        let query = r#"
            SELECT invite_id
            FROM invites
            WHERE invite_id = $1
        "#;

        let does_not_exist = transaction.query_opt(
            query,
            &[&invite_id]
        ).await?.is_none();

        if does_not_exist {
            break;
        }
    }

    return Ok(invite_id);
}