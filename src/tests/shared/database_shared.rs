use std::env;
use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::model::database::db::Database;

static DATABASE: OnceCell<Arc<Database>> = OnceCell::new();

pub fn database() -> &'static Arc<Database> {
    return DATABASE.get().unwrap();
}

pub async fn ctor() {
    let connection_string = "postgresql://localhost/test?user=postgres&password=test123".to_string();
    let database = Database::new(connection_string, 4).await.unwrap();
    let _ = DATABASE.set(Arc::new(database));

    {
        let database = DATABASE.get().unwrap();
        let connection = database.connection().await.unwrap();

        let query = r#"
            DROP TABLE IF EXISTS public.account_tokens CASCADE;
            DROP TABLE IF EXISTS public.accounts CASCADE;
            DROP TABLE IF EXISTS public.logs CASCADE;
            DROP TABLE IF EXISTS public.migrations CASCADE;
            DROP TABLE IF EXISTS public.post_descriptors CASCADE;
            DROP TABLE IF EXISTS public.post_replies CASCADE;
            DROP TABLE IF EXISTS public.post_watches CASCADE;
            DROP TABLE IF EXISTS public.threads CASCADE;
        "#;

        connection.batch_execute(query).await.unwrap();
    }
}

pub async fn cleanup() {
    let database = DATABASE.get().unwrap();
    let connection = database.connection().await.unwrap();

    let query = r#"
        DELETE FROM public.account_tokens;
        DELETE FROM public.accounts;
        DELETE FROM public.logs;
        DELETE FROM public.migrations;
        DELETE FROM public.post_descriptors;
        DELETE FROM public.post_replies;
        DELETE FROM public.post_watches;
        DELETE FROM public.threads;

        ALTER SEQUENCE account_tokens_id_seq RESTART;
        ALTER SEQUENCE accounts_id_seq RESTART;
        ALTER SEQUENCE logs_id_seq RESTART;
        ALTER SEQUENCE post_descriptors_id_seq RESTART;
        ALTER SEQUENCE post_replies_id_seq RESTART;
        ALTER SEQUENCE post_watches_id_seq RESTART;
        ALTER SEQUENCE threads_id_seq RESTART;
    "#;

    connection.batch_execute(query).await.unwrap();
}

pub async fn dtor() {
    let database = DATABASE.get().unwrap();
    let connection = database.connection().await.unwrap();

    let query = r#"
        DROP TABLE IF EXISTS public.account_tokens CASCADE;
        DROP TABLE IF EXISTS public.accounts CASCADE;
        DROP TABLE IF EXISTS public.logs CASCADE;
        DROP TABLE IF EXISTS public.migrations CASCADE;
        DROP TABLE IF EXISTS public.post_descriptors CASCADE;
        DROP TABLE IF EXISTS public.post_replies CASCADE;
        DROP TABLE IF EXISTS public.post_watches CASCADE;
        DROP TABLE IF EXISTS public.threads CASCADE;
    "#;

    connection.batch_execute(query).await.unwrap();
}