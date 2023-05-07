use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lazy_static::lazy_static;
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio_postgres::Transaction;

use crate::info;
use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::service::thread_watcher::FoundPostReply;

lazy_static! {
    static ref PD_TO_TD_CACHE: RwLock<HashMap<ThreadDescriptor, HashSet<PostDescriptor>>> =
        RwLock::new(HashMap::with_capacity(1024));
    static ref DBID_TO_PD_CACHE: RwLock<HashMap<i64, PostDescriptor>> =
        RwLock::new(HashMap::with_capacity(4096));
    static ref PD_TO_DBID_CACHE: RwLock<HashMap<PostDescriptor, i64>> =
        RwLock::new(HashMap::with_capacity(4096));

    static ref DBID_TO_TD_CACHE: RwLock<HashMap<i64, ThreadDescriptor>> =
        RwLock::new(HashMap::with_capacity(1024));
    static ref TD_TO_DBID_CACHE: RwLock<HashMap<ThreadDescriptor, i64>> =
        RwLock::new(HashMap::with_capacity(1024));
}

pub async fn init(database: &Arc<Database>) -> anyhow::Result<()> {
    info!("init() start");

    cache_thread_descriptors(database).await?;
    cache_post_descriptors(database).await?;

    info!("init() end");
    return Ok(());
}

async fn cache_thread_descriptors(database: &Arc<Database>) -> anyhow::Result<()> {
    let query = r#"
        SELECT
            thread.id,
            thread.site_name,
            thread.board_code,
            thread.thread_no
        FROM threads as thread
        LEFT JOIN post_descriptors post_descriptor
            ON thread.id = post_descriptor.owner_thread_id
        WHERE
            thread.is_dead = FALSE
        AND
            thread.deleted_on IS NULL
    "#;

    let connection = database.connection().await?;
    let rows = connection.query(query, &[]).await?;

    let mut loaded_thread_descriptors = 0;
    info!("cache_thread_descriptors() found {} rows", rows.len());

    {
        let mut dbid_to_td_cache_locked = DBID_TO_TD_CACHE.write().await;
        let mut td_to_dbid_cache_locked = TD_TO_DBID_CACHE.write().await;

        for row in rows {
            let id: i64 = row.get(0);
            let site_name: String = row.get(1);
            let board_code: String = row.get(2);
            let thread_no: i64 = row.get(3);

            let thread_descriptor = ThreadDescriptor::new(
                site_name,
                board_code,
                thread_no as u64
            );

            td_to_dbid_cache_locked.insert(thread_descriptor.clone(), id);
            dbid_to_td_cache_locked.insert(id, thread_descriptor);

            loaded_thread_descriptors += 1;
        }
    }

    info!("cache_thread_descriptors() end, loaded_thread_descriptors: {}", loaded_thread_descriptors);
    return Ok(());
}

async fn cache_post_descriptors(database: &Arc<Database>) -> anyhow::Result<()> {
    let query = r#"
        WITH alive_threads AS (
            SELECT
                thread.site_name,
                thread.board_code,
                thread.thread_no
            FROM threads as thread
            LEFT JOIN post_descriptors post_descriptor
                ON thread.id = post_descriptor.owner_thread_id
            WHERE
                thread.is_dead = FALSE
            AND
                thread.deleted_on IS NULL
        )

        SELECT
            post_descriptor.id,
            thread.site_name,
            thread.board_code,
            thread.thread_no,
            post_descriptor.post_no,
            post_descriptor.post_sub_no
        FROM threads AS thread
        FULL OUTER JOIN post_descriptors post_descriptor
            ON thread.id = post_descriptor.owner_thread_id
        WHERE
            thread.site_name IN (SELECT site_name FROM alive_threads)
        AND
            thread.board_code IN (SELECT board_code FROM alive_threads)
        AND
            thread.thread_no IN (SELECT thread_no FROM alive_threads)
    "#;

    let connection = database.connection().await?;
    let rows = connection.query(query, &[]).await?;

    let mut loaded_post_descriptors = 0;
    info!("cache_post_descriptors() found {} rows", rows.len());

    {
        let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
        let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
        let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

        for row in rows {
            let id: i64 = row.get(0);
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
            pd_to_dbid_cache_locked.insert(post_descriptor.clone(), id);
            dbid_to_pd_cache_locked.insert(id, post_descriptor);

            loaded_post_descriptors += 1;
        }
    }

    info!("cache_post_descriptors() end, loaded_post_descriptors: {}", loaded_post_descriptors);
    return Ok(());
}

pub async fn delete_all_thread_posts(thread_descriptor: &ThreadDescriptor) {
    let mut dbid_to_td_cache_locked = DBID_TO_TD_CACHE.write().await;
    let mut td_to_dbid_cache_locked = TD_TO_DBID_CACHE.write().await;

    let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
    let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
    let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

    let thread_db_id = td_to_dbid_cache_locked.remove(thread_descriptor);
    if thread_db_id.is_some() {
        dbid_to_td_cache_locked.remove(&thread_db_id.unwrap());
    }

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

pub async fn get_post_descriptor_db_id(post_descriptor: &PostDescriptor) -> Option<i64> {
    let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;
    return pd_to_dbid_cache_locked.get(post_descriptor).cloned();
}

pub async fn get_many_post_descriptor_db_ids(post_descriptors: &Vec<PostDescriptor>) -> Vec<i64> {
    let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;
    
    return post_descriptors.iter()
        .filter_map(|post_descriptor| pd_to_dbid_cache_locked.get(post_descriptor).cloned())
        .collect::<Vec<i64>>()
}

pub async fn get_many_found_post_reply_db_ids<'a>(
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

pub async fn get_many_post_descriptors_by_db_ids(db_ids: &Vec<i64>) -> Vec<PostDescriptor> {
    if db_ids.is_empty() {
        return vec![];
    }

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

pub async fn insert_post_descriptor_db_id(
    post_descriptor: &PostDescriptor,
    transaction: &Transaction<'_>
) -> anyhow::Result<i64> {
    {
        let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;

        let id = pd_to_dbid_cache_locked.get(post_descriptor);
        if id.is_some() {
            return Ok(*id.unwrap());
        }
    }

    let thread_db_id = insert_thread_descriptor_db_id(
        &post_descriptor.thread_descriptor,
        transaction
    ).await?;

    let query = r#"
        INSERT INTO post_descriptors
        (
            owner_thread_id,
            post_no,
            post_sub_no
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (owner_thread_id, post_no, post_sub_no)
            DO UPDATE SET post_no = post_descriptors.post_no
        RETURNING id
    "#;

    let id: i64 = transaction.query_one(
        query,
        &[
            &thread_db_id,
            &(post_descriptor.post_no as i64),
            &(post_descriptor.post_sub_no as i64)
        ],
    ).await?.get(0);

    insert_post_descriptor_into_cache(
        post_descriptor,
        id
    ).await;

    return Ok(id);
}

pub async fn insert_descriptor_db_ids<'a>(
    post_descriptors: &Vec<&'a PostDescriptor>,
    transaction: &Transaction<'_>
) -> anyhow::Result<HashMap<&'a PostDescriptor, i64>> {
    if post_descriptors.is_empty() {
        return Ok(HashMap::new());
    }

    let thread_descriptors = post_descriptors.iter()
        .map(|pd| &pd.thread_descriptor)
        .collect::<HashSet<&ThreadDescriptor>>();

    let thread_db_ids = insert_thread_descriptor_db_ids(
        &thread_descriptors,
        transaction
    ).await?;

    let mut result_map = HashMap::<&PostDescriptor, i64>::with_capacity(post_descriptors.len());

    let mut post_descriptors_to_insert =
        Vec::<&PostDescriptor>::with_capacity(post_descriptors.len() / 2);

    {
        let pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.read().await;

        for post_descriptor in post_descriptors {
            let id = pd_to_dbid_cache_locked.get(post_descriptor);
            if id.is_some() {
                result_map.insert(post_descriptor, *id.unwrap());
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
            owner_thread_id,
            post_no,
            post_sub_no
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (owner_thread_id, post_no, post_sub_no)
            DO UPDATE SET post_no = post_descriptors.post_no
        RETURNING id
    "#;

    // TODO: this might be slow
    for post_descriptor in post_descriptors_to_insert {
        let thread_db_id = thread_db_ids.get(&post_descriptor.thread_descriptor);
        if thread_db_id.is_none() {
            continue;
        }

        let thread_db_id = thread_db_id.unwrap();

        let id: i64 = transaction.query_one(
            query,
            &[
                &thread_db_id,
                &(post_descriptor.post_no as i64),
                &(post_descriptor.post_sub_no as i64)
            ],
        ).await?.get(0);

        insert_post_descriptor_into_cache(
            post_descriptor,
            id
        ).await;

        result_map.insert(post_descriptor, id);
    }

    return Ok(result_map);
}

async fn insert_thread_descriptor_db_ids(
    thread_descriptors: &HashSet<&ThreadDescriptor>,
    transaction: &Transaction<'_>
) -> anyhow::Result<HashMap<ThreadDescriptor, i64>> {
    if thread_descriptors.is_empty() {
        return Ok(HashMap::new());
    }

    let thread_descriptors_to_insert = {
        let td_to_dbid_cache_locked = TD_TO_DBID_CACHE.read().await;
        let mut thread_descriptors_to_insert =
            Vec::<&ThreadDescriptor>::with_capacity(thread_descriptors.len() / 2);

        for thread_descriptor in thread_descriptors {
            let id = td_to_dbid_cache_locked.get(thread_descriptor);
            if id.is_some() {
                thread_descriptors_to_insert.push(thread_descriptor);
            }
        }

        thread_descriptors_to_insert
    };

    if thread_descriptors_to_insert.is_empty() {
        return Ok(HashMap::new());
    }

    let mut result_map =
        HashMap::<ThreadDescriptor, i64>::with_capacity(thread_descriptors_to_insert.len());

    // TODO: slow!!!
    for thread_descriptor in thread_descriptors_to_insert {
        let query = r#"
            INSERT INTO threads
            (
                site_name,
                board_code,
                thread_no
            )
            VALUES ($1, $2, $3)
            ON CONFLICT (site_name, board_code, thread_no)
                DO UPDATE SET board_code = threads.board_code
            RETURNING id
        "#;

        let id: i64 = transaction.query_one(
            query,
            &[
                &thread_descriptor.site_name(),
                &thread_descriptor.board_code(),
                &(thread_descriptor.thread_no as i64)
            ],
        ).await?.get(0);

        insert_thread_descriptor_into_cache(
            thread_descriptor,
            id
        ).await;

        result_map.insert(thread_descriptor.clone(), id);
    }

    return Ok(result_map);
}

async fn insert_thread_descriptor_db_id(
    thread_descriptor: &ThreadDescriptor,
    transaction: &Transaction<'_>
) -> anyhow::Result<i64> {
    {
        let td_to_dbid_cache_locked = TD_TO_DBID_CACHE.read().await;

        let id = td_to_dbid_cache_locked.get(thread_descriptor);
        if id.is_some() {
            return Ok(*id.unwrap());
        }
    }

    let query = r#"
        INSERT INTO threads
        (
            site_name,
            board_code,
            thread_no
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (site_name, board_code, thread_no)
            DO UPDATE SET board_code = threads.board_code
        RETURNING id
    "#;

    let id: i64 = transaction.query_one(
        query,
        &[
            &thread_descriptor.site_name(),
            &thread_descriptor.board_code(),
            &(thread_descriptor.thread_no as i64)
        ],
    ).await?.get(0);

    insert_thread_descriptor_into_cache(
        thread_descriptor,
        id
    ).await;

    return Ok(id);
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

    pd_to_td_cache_locked
        .get_mut(&post_descriptor.thread_descriptor)
        .unwrap()
        .insert(post_descriptor.clone());
}

async fn insert_thread_descriptor_into_cache(thread_descriptor: &ThreadDescriptor, id: i64) {
    let mut dbid_to_td_cache_locked = DBID_TO_TD_CACHE.write().await;
    let mut td_to_td_cache_locked = TD_TO_DBID_CACHE.write().await;

    td_to_td_cache_locked.insert(thread_descriptor.clone(), id);
    dbid_to_td_cache_locked.insert(id, thread_descriptor.clone());
}

async fn insert_post_descriptor_into_cache(post_descriptor: &PostDescriptor, id: i64) {
    let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
    let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
    let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

    insert_pd_for_td(&post_descriptor, &mut pd_to_td_cache_locked);
    pd_to_dbid_cache_locked.insert(post_descriptor.clone(), id);
    dbid_to_pd_cache_locked.insert(id, post_descriptor.clone());
}

pub async fn test_cleanup() {
    let mut pd_to_dbid_cache_locked = PD_TO_DBID_CACHE.write().await;
    let mut dbid_to_pd_cache_locked = DBID_TO_PD_CACHE.write().await;
    let mut pd_to_td_cache_locked = PD_TO_TD_CACHE.write().await;

    pd_to_dbid_cache_locked.clear();
    dbid_to_pd_cache_locked.clear();
    pd_to_td_cache_locked.clear();
}