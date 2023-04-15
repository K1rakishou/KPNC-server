use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lazy_static::lazy_static;
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio_postgres::Transaction;

use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::service::thread_watcher::FoundPostReply;

lazy_static! {
    static ref PD_TO_TD_CACHE: RwLock<HashMap<ThreadDescriptor, HashSet<PostDescriptor>>> =
        RwLock::new(HashMap::with_capacity(256));
    static ref DBID_TO_PD_CACHE: RwLock<HashMap<i64, PostDescriptor>> =
        RwLock::new(HashMap::with_capacity(4096));
    static ref PD_TO_DBID_CACHE: RwLock<HashMap<PostDescriptor, i64>> =
        RwLock::new(HashMap::with_capacity(4096));
}

pub async fn init(database: &Arc<Database>) -> anyhow::Result<()> {
    info!("init() start");

    // Select all posts that belong to the same threads as active post watches (not dead, not deleted)
    let query = r#"
        WITH alive_threads AS (
            SELECT
                pd.site_name,
                pd.board_code,
                pd.thread_no
            FROM post_descriptors as pd
            LEFT JOIN posts post ON pd.id_generated = post.owner_post_descriptor_id
            WHERE post.is_dead = FALSE
            AND post.deleted_on IS NULL
        )

        SELECT
            pd.id_generated,
            pd.site_name,
            pd.board_code,
            pd.thread_no,
            pd.post_no,
            pd.post_sub_no
        FROM posts
        FULL OUTER JOIN post_descriptors pd on pd.id_generated = posts.owner_post_descriptor_id
        WHERE
            pd.site_name IN (SELECT site_name FROM alive_threads)
        AND
            pd.board_code IN (SELECT board_code FROM alive_threads)
        AND
            pd.thread_no IN (SELECT thread_no FROM alive_threads)
"#;

    let connection = database.connection().await?;
    let rows = connection.query(query, &[]).await?;

    let mut loaded_post_descriptors = 0;
    info!("init() found {} rows", rows.len());

    {
        let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
        let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
        let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

        for row in rows {
            let id_generated: i64 = row.get(0);
            let site_name: String = row.get(1);
            let board_code: String = row.get(2);
            let thread_no: i64 = row.get(3);
            let post_no: i64 = row.get(4);
            let post_sub_no: i64 = row.get(5);

            let post_descriptor = PostDescriptor::new(
                site_name,
                board_code,
                thread_no as u64,
                post_no as u64,
                post_sub_no as u64
            );

            insert_pd_for_td(&post_descriptor, &mut pd_to_td_cache_locked);
            pd_to_dbid_cache_locked.insert(post_descriptor.clone(), id_generated);
            dbid_to_pd_cache_locked.insert(id_generated, post_descriptor);

            loaded_post_descriptors += 1;
        }
    }

    info!("init() end, loaded_post_descriptors: {}", loaded_post_descriptors);
    return Ok(());
}

pub async fn delete_all_thread_posts(thread_descriptor: &ThreadDescriptor) {
    let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
    let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
    let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

    let thread_posts = pd_to_td_cache_locked.remove(thread_descriptor);
    if thread_posts.is_none() {
        return;
    }

    let thread_posts = thread_posts.unwrap();
    if thread_posts.is_empty() {
        return;
    }

    for thread_post in &thread_posts {
        pd_to_dbid_cache_locked.remove(thread_post);
    }

    let mut to_remove = Vec::<i64>::with_capacity(thread_posts.len());

    for (db_id, post_descriptor) in dbid_to_pd_cache_locked.iter() {
        if thread_posts.contains(post_descriptor) {
            to_remove.push(*db_id);
        }
    }

    for db_id in to_remove {
        dbid_to_pd_cache_locked.remove(&db_id);
    }
}

pub async fn get_post_descriptor_db_id(post_descriptor: &PostDescriptor) -> i64 {
    let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;
    return *pd_to_dbid_cache_locked.get(post_descriptor).unwrap();
}

pub async fn get_many_post_descriptor_db_ids<'a>(
    post_replies: &Vec<&'a FoundPostReply>
) -> HashMap<i64, Vec<&'a FoundPostReply>> {
    let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;
    let mut result_map = HashMap::<i64, Vec<&'a FoundPostReply>>::with_capacity(post_replies.len());

    for post_reply in post_replies {
        let post_descriptor_db_id = pd_to_dbid_cache_locked.get(&post_reply.replies_to);
        if post_descriptor_db_id.is_some() {
            let post_descriptor_db_id = *post_descriptor_db_id.unwrap();

            let posts_vec = result_map.entry(post_descriptor_db_id).or_insert(Vec::new());
            posts_vec.push(post_reply);
        }
    }

    return result_map;
}

pub async fn get_many_post_descriptors_by_db_ids(db_ids: Vec<i64>) -> Vec<PostDescriptor> {
    let dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.read().await;
    let mut result_vec = Vec::<PostDescriptor>::with_capacity(db_ids.len());

    for db_id in db_ids {
        let post_descriptor = dbid_to_pd_cache_locked.get(&db_id);
        if post_descriptor.is_some() {
            result_vec.push(post_descriptor.unwrap().clone());
        }
    }

    return result_vec;
}

pub async fn get_thread_post_descriptors(thread_descriptor: &ThreadDescriptor) -> Vec<PostDescriptor> {
    let pd_to_td_cache_locked = PD_TO_TD_CACHE.read().await;

    let post_descriptor_set = pd_to_td_cache_locked.get(thread_descriptor);
    if post_descriptor_set.is_none() {
        return vec![];
    }

    let post_descriptor_set = post_descriptor_set.unwrap();
    if post_descriptor_set.is_empty() {
        return vec![];
    }

    let mut result_vec = Vec::<PostDescriptor>::with_capacity(post_descriptor_set.len());
    for post_descriptor in post_descriptor_set {
        result_vec.push(post_descriptor.clone());
    }

    return result_vec;
}

pub async fn get_thread_post_db_ids(thread_descriptor: &ThreadDescriptor) -> Vec<i64> {
    let pd_to_td_cache_locked = PD_TO_TD_CACHE.read().await;

    let post_descriptor_set = pd_to_td_cache_locked.get(thread_descriptor);
    if post_descriptor_set.is_none() {
        return vec![];
    }

    let post_descriptor_set = post_descriptor_set.unwrap();
    if post_descriptor_set.is_empty() {
        return vec![];
    }

    let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;
    let mut result_vec = Vec::<i64>::with_capacity(post_descriptor_set.len());

    for post_descriptor in post_descriptor_set {
        let db_id = pd_to_dbid_cache_locked.get(post_descriptor);

        if db_id.is_some() {
            result_vec.push(*db_id.unwrap());
        }
    }

    return result_vec;
}

pub async fn insert_descriptor_db_id(
    post_descriptor: &PostDescriptor,
    transaction: &Transaction<'_>
) -> anyhow::Result<i64> {
    {
        let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;

        let id_generated = pd_to_dbid_cache_locked.get(post_descriptor);
        if id_generated.is_some() {
            return Ok(*id_generated.unwrap());
        }
    }

    let query = r#"
        INSERT INTO post_descriptors
        (
            site_name,
            board_code,
            thread_no,
            post_no,
            post_sub_no
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id_generated
"#;

    let id_generated: i64 = transaction.query_one(
        query,
        &[
            post_descriptor.site_name(),
            post_descriptor.board_code(),
            &(post_descriptor.thread_no() as i64),
            &(post_descriptor.post_no as i64),
            &(post_descriptor.post_sub_no as i64)
        ],
    ).await?.get(0);

    let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
    let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
    let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

    insert_pd_for_td(&post_descriptor, &mut pd_to_td_cache_locked);
    pd_to_dbid_cache_locked.insert(post_descriptor.clone(), id_generated);
    dbid_to_pd_cache_locked.insert(id_generated, post_descriptor.clone());

    return Ok(id_generated);
}

pub async fn insert_descriptor_db_ids<'a>(
    post_descriptors: &Vec<&'a PostDescriptor>,
    transaction: &Transaction<'_>
) -> anyhow::Result<HashMap<&'a PostDescriptor, i64>> {
    if post_descriptors.is_empty() {
        return Ok(HashMap::new());
    }

    let mut result_map = HashMap::<&PostDescriptor, i64>::with_capacity(post_descriptors.len());

    let mut post_descriptors_to_insert =
        Vec::<&PostDescriptor>::with_capacity(post_descriptors.len() / 2);

    {
        let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;

        for post_descriptor in post_descriptors {
            let id_generated = pd_to_dbid_cache_locked.get(post_descriptor);
            if id_generated.is_some() {
                result_map.insert(post_descriptor, *id_generated.unwrap());
            } else {
                post_descriptors_to_insert.push(post_descriptor);
            }
        }
    }

    if post_descriptors_to_insert.is_empty() {
        // All post descriptors were already cached
        return Ok(result_map);
    }

    let query = r#"
        INSERT INTO post_descriptors
        (
            site_name,
            board_code,
            thread_no,
            post_no,
            post_sub_no
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id_generated
"#;

    // TODO: this might be slow
    for post_descriptor in post_descriptors_to_insert {
        let id_generated: i64 = transaction.query_one(
            query,
            &[
                post_descriptor.site_name(),
                post_descriptor.board_code(),
                &(post_descriptor.thread_no() as i64),
                &(post_descriptor.post_no as i64),
                &(post_descriptor.post_sub_no as i64)
            ],
        ).await?.get(0);

        let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
        let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
        let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

        insert_pd_for_td(&post_descriptor, &mut pd_to_td_cache_locked);
        pd_to_dbid_cache_locked.insert(post_descriptor.clone(), id_generated);
        dbid_to_pd_cache_locked.insert(id_generated, post_descriptor.clone());

        result_map.insert(post_descriptor, id_generated);
    }

    return Ok(result_map);
}

fn insert_pd_for_td(
    post_descriptor: &PostDescriptor,
    pd_to_td_cache_locked: &mut RwLockWriteGuard<HashMap<ThreadDescriptor, HashSet<PostDescriptor>>>
) {
    if !pd_to_td_cache_locked.contains_key(&post_descriptor.thread_descriptor) {
        pd_to_td_cache_locked.insert(
            post_descriptor.clone().thread_descriptor,
            HashSet::<PostDescriptor>::with_capacity(64)
        );
    }

    pd_to_td_cache_locked.get_mut(&post_descriptor.thread_descriptor).unwrap()
        .insert(post_descriptor.clone());
}