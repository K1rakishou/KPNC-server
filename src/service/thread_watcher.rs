use std::env;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::repository::{post_descriptor_id_repository, post_reply_repository, post_repository};
use crate::model::repository::site_repository::SiteRepository;
use crate::service::fcm_sender::FcmSender;

lazy_static! {
    static ref post_reply_quote_regex: Regex =
        Regex::new(r##"<a\s+href="#p(\d+)"\s+class="quotelink">&gt;&gt;\d+</a>"##).unwrap();
}

pub struct ThreadWatcher {
    num_cpus: u32,
    working: bool
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct FoundPostReply {
    pub origin: PostDescriptor,
    pub replies_to: PostDescriptor
}

#[derive(Debug, Deserialize)]
struct ChanThread {
    posts: Vec<ChanPost>
}

impl ChanThread {
    pub fn get_original_post(&self) -> Option<&ChanPost> {
        for post in &self.posts {
            if post.is_op() {
                return Some(&post);
            }
        }

        return None;
    }
}

#[derive(Debug, Deserialize)]
struct ChanPost {
    no: u64,
    resto: u64,
    closed: Option<i32>,
    archived: Option<i32>,
    com: Option<String>
}

impl ChanPost {
    pub fn is_op(&self) -> bool {
        return self.resto == 0;
    }

    pub fn is_not_active(&self) -> bool {
        let closed = self.closed.unwrap_or(0);
        let archived = self.archived.unwrap_or(0);

        return closed == 1 || archived == 1;
    }
}

impl ThreadWatcher {
    pub fn new(num_cpus: u32) -> ThreadWatcher {
        return ThreadWatcher { num_cpus, working: false };
    }

    pub async fn start(
        &mut self,
        database: &Arc<Database>,
        site_repository: &Arc<SiteRepository>,
        fcm_sender: &Arc<FcmSender>,
    ) -> anyhow::Result<()> {
        if self.working {
            panic!("ThreadWatcher already working!")
        }

        self.working = true;
        info!("ThreadWatcher started");

        let timeout_seconds = env::var("THREAD_WATCHER_TIMEOUT_SECONDS")
            .map(|value| u64::from_str(value.as_str()).unwrap())
            .unwrap_or(60 as u64);

        loop {
            if !self.working {
                break;
            }

            let result = process_watched_threads(
                self.num_cpus,
                database,
                site_repository,
                fcm_sender
            ).await;

            match result {
                Ok(_) => { info!("thread_watcher_loop() iteration success") }
                Err(error) => { error!("process_posts() iteration error: \'{}\'", error) }
            }

            info!("thread_watcher_loop() sleeping for {timeout_seconds} seconds...");
            sleep(Duration::from_secs(timeout_seconds)).await;
            info!("thread_watcher_loop() sleeping for {timeout_seconds} seconds... done");
        }

        info!("ThreadWatcher terminated");
        return Ok(());
    }

    pub async fn stop(&mut self) {
        if !self.working {
            panic!("ThreadWatcher is not working!")
        }

        self.working = false;
    }

}

async fn process_watched_threads(
    num_cpus: u32,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>,
    fcm_sender: &Arc<FcmSender>,
) -> anyhow::Result<()> {
    let all_watched_threads = post_repository::get_all_watched_threads(database)
        .await.context("process_watched_threads() Failed to get all watched threads")?;

    if all_watched_threads.is_empty() {
        info!("process_watched_threads() no watched threads to process");
        return Ok(());
    }

    let mut chunk_size: usize = (num_cpus * 4) as usize;
    if chunk_size < 8 {
        chunk_size = 8;
    }
    if chunk_size > 128 {
        chunk_size = 128;
    }

    info!(
        "process_watched_threads() found {} watched threads, processing with chunk size {}",
        all_watched_threads.len(),
        chunk_size
    );

    let process_threads_start = chrono::offset::Utc::now();

    for thread_descriptors in all_watched_threads.chunks(chunk_size) {
        let mut join_handles: Vec<JoinHandle<()>> = Vec::with_capacity(chunk_size);

        for thread_descriptor in thread_descriptors {
            let thread_descriptor_cloned = thread_descriptor.clone();
            let database_cloned = database.clone();
            let site_repository_cloned = site_repository.clone();

            let join_handle = tokio::task::spawn(async move {
                process_thread(
                    &thread_descriptor_cloned,
                    &database_cloned,
                    &site_repository_cloned,
                ).await.unwrap();
            });

            join_handles.push(join_handle);
        }

        futures::future::join_all(join_handles).await;
    }

    let delta = chrono::offset::Utc::now() - process_threads_start;
    let send_fcm_messages_start = chrono::offset::Utc::now();
    info!("process_watched_threads() processing done, took {} ms, sending out FCM messages...", delta.num_milliseconds());

    fcm_sender.send_fcm_messages(chunk_size)
        .await
        .context("Error while trying to send out FCM messages")?;

    let delta = chrono::offset::Utc::now() - send_fcm_messages_start;
    info!("process_watched_threads() sending out FCM messages done, took {} ms, success!", delta.num_milliseconds());

    return Ok(());
}

async fn process_thread(
    thread_descriptor: &ThreadDescriptor,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>
) -> anyhow::Result<()> {
    let thread_json_endpoint = site_repository.thread_json_endpoint(thread_descriptor);
    if thread_json_endpoint.is_none() {
        error!(
            "process_thread({}) marking thread as dead because the site is not supported",
            thread_descriptor
        );

        post_repository::mark_all_thread_posts_dead(database, thread_descriptor).await?;
        return Ok(());
    }

    let thread_json_endpoint = thread_json_endpoint.unwrap();
    let response = reqwest::get(thread_json_endpoint.clone())
        .await
        .with_context(|| {
            return format!(
                "process_thread({}) Failed to execute GET request to \'{}\' endpoint",
                thread_descriptor,
                thread_json_endpoint
            );
        })?;

    let status_code = response.status().as_u16();
    if status_code != 200 {
        error!("process_thread({}) bad status code {}", thread_descriptor, status_code);

        if status_code == 404 {
            error!(
                "process_thread({}) marking thread as dead because status code is 404",
                thread_descriptor
            );

            post_repository::mark_all_thread_posts_dead(database, thread_descriptor).await?;
        }

        return Ok(());
    }

    let response_text = response.text()
        .await
        .with_context(|| {
            return format!(
                "process_thread({}) Failed to extract text from response",
                thread_descriptor
            );
        })?;

    let chan_thread = serde_json::from_str::<ChanThread>(response_text.as_str());
    if chan_thread.is_err() {
        let to_print_chars_count = 512;
        let chars = response_text.chars();
        let chars_count = chars.size_hint().0;
        let text: Vec<u16> = chars.take(to_print_chars_count).map(|ch| ch as u16).collect();

        let body_text = if text.is_empty() {
            String::from("<body is empty>")
        } else {
            if chars_count < to_print_chars_count {
                String::from_utf16_lossy(text.as_slice())
            } else {
                let remaining_chars_count = chars_count - to_print_chars_count;
                format!("{} (+{} more)", String::from_utf16_lossy(text.as_slice()), remaining_chars_count)
            }
        };

        let error = chan_thread.err().unwrap();

        error!(
            "process_thread({}) Failed to convert response_text into ChanThread. \
            Error: \'{}\'. Body text: \'{}\'",
            thread_descriptor,
            error,
            body_text
        );

        return Err(anyhow!(error));
    }

    let chan_thread = chan_thread.unwrap();

    let original_post = chan_thread.get_original_post();
    if original_post.is_none() {
        let posts_count = chan_thread.posts.len();
        error!(
            "process_thread({}) thread has no original post, posts_count: {}",
            thread_descriptor,
            posts_count
        );

        return Ok(());
    }

    let original_post = original_post.unwrap();
    if original_post.is_not_active() {
        info!(
            "process_thread({}) marking thread as dead it's either archived or closed \
            (archived: {}, closed: {})",
            thread_descriptor,
            original_post.archived.unwrap_or(0) == 1,
            original_post.closed.unwrap_or(0) == 1,
        );

        post_repository::mark_all_thread_posts_dead(database, thread_descriptor).await?;

        // Fall through. We still want to send the last batch of messages if there are new replies
        // to watched posts. We won't be processing this thread on the next iteration, though,
        // because it will be filtered out during the database query.
    }

    debug!(
        "process_thread({}) got thread with {} posts",
        thread_descriptor,
        chan_thread.posts.len()
    );

    process_posts(thread_descriptor, &chan_thread, database).await?;

    return Ok(());
}

async fn process_posts(
    thread_descriptor: &ThreadDescriptor,
    chan_thread: &ChanThread,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    debug!("process_posts({}) start", thread_descriptor);

    if chan_thread.posts.is_empty() {
        info!("process_posts({}) no posts to process", thread_descriptor);
        return Ok(());
    }

    // TODO: read last processed post here and do not do anything to posts that are less than the
    //  last processed post
    let mut found_post_replies_set = HashSet::<FoundPostReply>::with_capacity(32);

    for post in &chan_thread.posts {
        let post_comment = post.com.as_ref().map(|com| com.as_str()).unwrap_or("");
        if post_comment.is_empty() {
            continue;
        }

        let captures_iter = post_reply_quote_regex.captures_iter(post_comment);
        for captures in captures_iter {
            let quote_post_no_str = captures
                .get(1)
                .map(|capture| capture.as_str())
                .unwrap_or("");

            if quote_post_no_str.is_empty() {
                continue;
            }

            let quote_post_no = u64::from_str(quote_post_no_str).unwrap_or(0);
            if quote_post_no == 0 {
                continue;
            }

            let origin = PostDescriptor::from_thread_descriptor(
                thread_descriptor.clone(),
                post.no
            );

            let replies_to = PostDescriptor::from_thread_descriptor(
                thread_descriptor.clone(),
                quote_post_no
            );

            let post_reply = FoundPostReply {
                origin,
                replies_to
            };

            found_post_replies_set.insert(post_reply);
        }
    }

    // TODO: store last processed post descriptor in the database so that we don't need to recheck
    //  the same posts again and again while a thread is alive.

    if found_post_replies_set.is_empty() {
        debug!("process_posts({}) end. No post replies found", thread_descriptor);
        return Ok(());
    }

    debug!("process_posts({}) found {} quotes", thread_descriptor, found_post_replies_set.len());
    let found_post_replies = found_post_replies_set.iter().collect::<Vec<&FoundPostReply>>();

    let post_descriptor_db_ids = post_descriptor_id_repository::get_many_post_descriptor_db_ids(
        &found_post_replies
    ).await;

    if post_descriptor_db_ids.is_empty() {
        debug!("process_posts({}) end. No reply db_ids found", thread_descriptor);
        return Ok(());
    }

    let post_replies = post_repository::find_new_replies(
        thread_descriptor,
        database,
        &post_descriptor_db_ids_to_vec_of_unique_keys(&post_descriptor_db_ids)
    ).await?;

    if post_replies.len() > 0 {
        debug!(
            "process_posts({}) storing {} post replies into the database",
            thread_descriptor,
            post_replies.len()
        );

        post_reply_repository::store(&post_replies, &post_descriptor_db_ids, database)
            .await
            .context(format!("Failed to store post {} replies", post_replies.len()))?;
    }

    debug!("process_posts({}) end. Success!", thread_descriptor);
    return Ok(());
}

fn post_descriptor_db_ids_to_vec_of_unique_keys(
    post_descriptor_db_ids: &HashMap<i64, Vec<&FoundPostReply>>
) -> Vec<i64> {
    if post_descriptor_db_ids.is_empty() {
        return vec![];
    }

    let capacity = post_descriptor_db_ids.iter().fold(0, |acc, item| acc + item.1.len());
    let mut duplicates = HashSet::<i64>::with_capacity(capacity);
    let mut result_vec = Vec::<i64>::with_capacity(capacity);

    for key in post_descriptor_db_ids.keys() {
        if !duplicates.insert(*key) {
            continue;
        }

        result_vec.push(*key);
    }

    return result_vec;
}

#[test]
fn test_regex() {
    let test_string = "<a href=\"#p251260223\" class=\"quotelink\">&gt;&gt;251260223</a>";
    let captures = post_reply_quote_regex.captures(test_string).unwrap();
    assert_eq!(2, captures.len());
    assert_eq!("251260223", captures.get(1).unwrap().as_str());

    let test_string = "<a href=\"#p425813171\" class=\"quotelink\">&gt;&gt;425813171</a>";
    let captures = post_reply_quote_regex.captures(test_string).unwrap();
    assert_eq!(2, captures.len());
    assert_eq!("425813171", captures.get(1).unwrap().as_str());
}