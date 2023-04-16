use std::env;
use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::model::database::db::Database;

static DATABASE: OnceCell<Arc<Database>> = OnceCell::new();

pub fn database() -> &'static Arc<Database> {
    return DATABASE.get().unwrap();
}

pub async fn ctor() {
    let connection_string = env::var("DATABASE_CONNECTION_STRING").unwrap();
    let database = Database::new(connection_string, 4).await.unwrap();
    let _ = DATABASE.set(Arc::new(database));

    {
        let database = DATABASE.get().unwrap();
        let connection = database.connection().await.unwrap();

        let query = r#"
            DROP TABLE IF EXISTS public.migrations;
            DROP TABLE IF EXISTS public.post_watches;
            DROP TABLE IF EXISTS public.posts;
            DROP TABLE IF EXISTS public.post_replies;
            DROP TABLE IF EXISTS public.accounts;
            DROP TABLE IF EXISTS public.post_descriptors;
            DROP TABLE IF EXISTS public.threads;
        "#;

        connection.batch_execute(query).await.unwrap();
    }
}

pub async fn cleanup() {
    let database = DATABASE.get().unwrap();
    let connection = database.connection().await.unwrap();

    let query = r#"
        DELETE FROM public.migrations;
        DELETE FROM public.post_watches;
        DELETE FROM public.posts;
        DELETE FROM public.post_replies;
        DELETE FROM public.accounts;
        DELETE FROM public.post_descriptors;
        DELETE FROM public.threads;

        ALTER SEQUENCE accounts_id_generated_seq RESTART;
        ALTER SEQUENCE post_descriptors_id_generated_seq RESTART;
        ALTER SEQUENCE post_replies_id_generated_seq RESTART;
        ALTER SEQUENCE post_watches_id_generated_seq RESTART;
        ALTER SEQUENCE posts_id_generated_seq RESTART;
        ALTER SEQUENCE threads_id_generated_seq RESTART;
    "#;

    connection.batch_execute(query).await.unwrap();
}

pub async fn dtor() {
    let database = DATABASE.get().unwrap();
    let connection = database.connection().await.unwrap();

    let query = r#"
        DROP TABLE IF EXISTS public.migrations;
        DROP TABLE IF EXISTS public.post_watches;
        DROP TABLE IF EXISTS public.posts;
        DROP TABLE IF EXISTS public.post_replies;
        DROP TABLE IF EXISTS public.accounts;
        DROP TABLE IF EXISTS public.post_descriptors;
        DROP TABLE IF EXISTS public.threads;
    "#;

    connection.batch_execute(query).await.unwrap();
}