use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use refinery::Migration;
use tokio_postgres::{Row, Transaction};
use crate::helpers::hashers::Sha3_512_Hashable;
use crate::model::database::db::{Database, PgPooledConnection};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

struct AppliedMigration {
    version: u32,
    name: String,
    date_time: DateTime<Utc>,
    checksum: String,
}

impl AppliedMigration {
    pub fn from_row(row: &Row) -> AppliedMigration {
        let version: i32 = row.get(0);
        let name: String = row.get(1);
        let date_time: DateTime<Utc> = row.get(2);
        let checksum: String = row.get(3);

        return AppliedMigration {
            version: version as u32,
            name,
            date_time,
            checksum
        }
    }
}

pub async fn perform_migrations(database: &Arc<Database>) -> anyhow::Result<()> {
    let mut connection = database.connection().await?;
    let applied_migrations = collect_applied_migrations_as_map(&connection).await?;

    let runner = embedded::migrations::runner();
    let mut migrations = runner.get_migrations().clone();
    migrations.sort_by(|a, b| a.version().cmp(&b.version()));

    info!(
        "Found {} migrations in total, and {} already applied migrations",
        migrations.len(),
        applied_migrations.len()
    );

    let mut skipped = 0;
    let mut applied = 0;

    info!("Applying migrations...");

    let transaction = connection.transaction()
        .await
        .context("Failed to start transaction")?;

    for migration in migrations {
        if applied_migrations.contains_key(&migration.version()) {
            let migrations_match = check_migration_checksum_match(&transaction, &migration)
                .await?;

            if !migrations_match {
                panic!("Migrations do not match!");
            }

            skipped += 1;
            info!("Skipping migration {} because it's already applied", migration);
            continue;
        }

        info!("Applying migration {}...", migration);
        let migration_sql = migration.sql()
            .context(format!("Migration {} has no sql", migration))?;

        transaction.batch_execute(migration_sql)
            .await
            .context(format!("Failed to apply migration {}", migration))?;
        
        let version = migration.version() as i32;
        let name = String::from(migration.name());
        let checksum = migration_sql.sha3_512(1);

        transaction.execute(
            "INSERT INTO migrations (version, name, checksum) VALUES ($1, $2, $3)",
            &[&version, &name, &checksum]
        )
            .await
            .context("Failed to store migration")?;

        applied += 1;
        info!("Applying migration {}... success", migration);
    }

    transaction.commit()
        .await
        .context("Failed to commit transaction")?;

    info!("Applying migrations... success, skipped: {}, applied: {}", skipped, applied);
    return Ok(());
}

async fn check_migration_checksum_match(
    transaction: &Transaction<'_>,
    migration: &Migration
) -> anyhow::Result<bool> {
    let migration_sql = migration.sql();
    if migration_sql.is_none() {
        let error = anyhow!("Migration {} has no sql",migration);
        return Err(error);
    }

    let migration_sql = migration_sql.unwrap();

    let statement = transaction
        .prepare("SELECT migrations.checksum FROM migrations WHERE migrations.version = $1")
        .await
        .context("Failed to prepare statement")?;

    let row = transaction.query_opt(&statement, &[&(migration.version() as i32)])
        .await
        .context("Failed to query existing migration by id")?;

    if row.is_none() {
        let error = anyhow!(
            "Migration {} does not exist in the database (it was never applied?)",
            migration
        );

        return Err(error);
    }

    let checksum_from_db: String = row.unwrap().get(0);
    let checksum_calculated = migration_sql.sha3_512(1);
    let migrations_match = checksum_from_db == checksum_calculated;

    info!(
        "Migration {}, checksum_from_db: {}, checksum_calculated: {}, migrations_match: {}",
        migration,
        checksum_from_db,
        checksum_calculated,
        migrations_match
    );

    return Ok(migrations_match);
}

async fn check_table_exists(
    connection: &PgPooledConnection<'_>,
    table_name: &str
) -> anyhow::Result<bool> {
    let sql = r#"
SELECT
    COUNT(table_name)
FROM
    information_schema.tables
WHERE
    table_schema LIKE 'public' AND
    table_type LIKE 'BASE TABLE' AND
	table_name = $1;
"#;

    let statement = connection.prepare(sql).await?;

    let row = connection.query_opt(&statement, &[&table_name]).await?;
    if row.is_none() {
        return Ok(false);
    }

    let count: i64 = row.unwrap().get(0);
    return Ok(count > 0);
}

async fn collect_applied_migrations_as_map(
    connection: &PgPooledConnection<'_>
) -> anyhow::Result<HashMap<u32, AppliedMigration>> {
    let exists = check_table_exists(connection, "migrations")
        .await
        .context("Failed to checked whether table 'migrations' exists")?;

    if !exists {
        info!("Table 'migrations' does not exist");
        return Ok(HashMap::new());
    }

    let applied_migrations: Vec<AppliedMigration> = connection.query(
        "SELECT * from migrations",
        &[],
    )
        .await?
        .iter()
        .map(|row| AppliedMigration::from_row(row))
        .collect();

    if applied_migrations.is_empty() {
        return Ok(HashMap::new());
    }

    let mut result_map = HashMap::<u32, AppliedMigration>::with_capacity(applied_migrations.len());

    for migration in applied_migrations {
        result_map.insert(migration.version, migration);
    }

    return Ok(result_map);
}