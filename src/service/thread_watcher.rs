use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use lazy_static::lazy_static;
use regex::Regex;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use crate::{error, info};
use crate::helpers::post_helpers;
use crate::model::data::chan::{ChanThread, PostDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::imageboards::base_imageboard::ThreadLoadResult;
use crate::model::repository::{post_descriptor_id_repository, post_reply_repository, post_repository, thread_repository};
use crate::model::repository::site_repository::SiteRepository;
use crate::service::fcm_sender::FcmSender;

lazy_static! {
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::new();
}

pub struct ThreadWatcher {
    num_cpus: u32,
    timeout_seconds: u64,
    is_dev_build: bool,
    working: bool
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct FoundPostReply {
    pub origin: PostDescriptor,
    pub replies_to: PostDescriptor
}

impl ThreadWatcher {
    pub fn new(num_cpus: u32, timeout_seconds: u64, is_dev_build: bool) -> ThreadWatcher {
        return ThreadWatcher {
            num_cpus,
            timeout_seconds,
            is_dev_build,
            working: false
        };
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
        let default_timeout_seconds = self.timeout_seconds;

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

            if self.is_dev_build && result.is_err() {
                result.unwrap();
                unreachable!();
            }

            let processed_threads = match result {
                Ok(processed_threads) => {
                    info!(
                        "thread_watcher_loop() iteration success, processed_threads: {}",
                        processed_threads
                    );

                    processed_threads
                }
                Err(error) => {
                    error!("process_posts() iteration error: \'{}\'", error);

                    0
                }
            };

            let timeout_seconds = match processed_threads {
                0..=255 => default_timeout_seconds,
                256..=1023 => default_timeout_seconds * 2,
                1024..=4096 => default_timeout_seconds * 3,
                _ => default_timeout_seconds * 5,
            };

            info!("thread_watcher_loop() sleeping for {timeout_seconds} seconds...");
            sleep(Duration::from_secs(timeout_seconds)).await;
            info!("thread_watcher_loop() sleeping for {timeout_seconds} seconds... done");
        }

        info!("ThreadWatcher terminated");
        return Ok(());
    }

}

async fn process_watched_threads(
    num_cpus: u32,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>,
    fcm_sender: &Arc<FcmSender>,
) -> anyhow::Result<usize> {
    let all_watched_threads = post_repository::get_all_watched_threads(database)
        .await
        .context("process_watched_threads() Failed to get all watched threads")?;

    if all_watched_threads.is_empty() {
        info!("process_watched_threads() no watched threads to process");
        return Ok(0);
    }

    let mut chunk_size: usize = (num_cpus * 4) as usize;
    if chunk_size < 16 {
        chunk_size = 16;
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
    info!(
        "process_watched_threads() processing done, took {} ms, sending out FCM messages...",
        delta.num_milliseconds()
    );

    let sent_fcm_messages = fcm_sender.send_fcm_messages(chunk_size)
        .await
        .context("Error while trying to send out FCM messages")?;

    let delta = chrono::offset::Utc::now() - send_fcm_messages_start;
    info!(
        "process_watched_threads() sending out FCM messages done ({} messages sent), \
        took {} ms, success!",
        sent_fcm_messages,
        delta.num_milliseconds()
    );

    return Ok(all_watched_threads.len());
}

async fn process_thread(
    thread_descriptor: &ThreadDescriptor,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>
) -> anyhow::Result<()> {
    let last_processed_post = thread_repository::get_last_processed_post(
        thread_descriptor,
        database
    ).await?;

    if last_processed_post.is_some() {
        info!(
            "process_thread({}) last_processed_post: {}",
            thread_descriptor,
            last_processed_post.clone().unwrap()
        );
    } else {
        info!(
            "process_thread({}) last_processed_post: None",
            thread_descriptor,
        );
    }

    let thread_load_result = site_repository.load_thread(
        &HTTP_CLIENT,
        database,
        &last_processed_post,
        thread_descriptor,
    ).await?;

    let (chan_thread, last_modified) = match thread_load_result {
        ThreadLoadResult::Success(chan_thread, last_modified) => { (chan_thread, last_modified) }
        ThreadLoadResult::SiteNotSupported => {
            error!(
                "process_thread({}) marking thread as dead because the site is not supported",
                thread_descriptor
            );

            post_repository::mark_thread_as_dead(database, thread_descriptor, true).await?;
            return Ok(());
        }
        ThreadLoadResult::HeadRequestBadStatusCode(status_code) => {
            error!("process_thread({}) (HEAD) bad status code {}", thread_descriptor, status_code);

            if status_code == 404 {
                error!(
                    "process_thread({}) (HEAD) marking thread as dead because status code is 404",
                    thread_descriptor
                );

                post_repository::mark_thread_as_dead(database, thread_descriptor, true).await?;
            }

            return Ok(());
        }
        ThreadLoadResult::GetRequestBadStatusCode(status_code) => {
            error!("process_thread({}) bad status code {}", thread_descriptor, status_code);

            if status_code == 404 {
                error!(
                    "process_thread({}) marking thread as dead because status code is 404",
                    thread_descriptor
                );

                post_repository::mark_thread_as_dead(database, thread_descriptor, true).await?;
            }

            return Ok(());
        }
        ThreadLoadResult::ThreadDeletedOrClosed => {
            error!("process_thread({}) thread is deleted or closed", thread_descriptor);

            post_repository::mark_thread_as_dead(database, thread_descriptor, true).await?;
            return Ok(());
        }
        ThreadLoadResult::ThreadInaccessible => {
            error!("process_thread({}) thread is inaccessible", thread_descriptor);
            return Ok(());
        }
        ThreadLoadResult::ServerSentIncorrectData(message) => {
            error!(
                "process_thread({}) server sent incorrect data, reason: {}",
                thread_descriptor,
                message
            );

            return Ok(());
        }
        ThreadLoadResult::ThreadWasNotModifiedSinceLastCheck => {
            info!(
                "process_thread({}) content wasn't modified since last check, exiting",
                thread_descriptor
            );

            return Ok(())
        }
        ThreadLoadResult::FailedToReadChanThread(body_text_part) => {
            error!(
                "process_thread({}) Failed to convert response_text into ChanThread. Body text: \'{}\'",
                thread_descriptor,
                body_text_part
            );

            return Err(anyhow!("Failed to read ChanThread"));
        }
        ThreadLoadResult::ServerError(code, message) => {
            let message = format!("ServerError code: {}, message: \'{}\'", code, message);

            error!(
                "process_thread({}) Server returned error: \'{}\'",
                thread_descriptor,
                message
            );

            return Err(anyhow!("Server returned an error: {}", message));
        }
    };

    if chan_thread.is_not_active() {
        info!(
            "process_thread({}) marking thread as dead it's either archived or closed \
            (archived: {}, closed: {})",
            thread_descriptor,
            chan_thread.archived,
            chan_thread.closed,
        );

        // Do not delete the cached posts here, we still want to process them.
        // Only mark the threads as dead
        post_repository::mark_thread_as_dead(database, thread_descriptor, false).await?;

        // Fall through. We still want to send the last batch of messages if there are new replies
        // to watched posts. We won't be processing this thread on the next iteration, though,
        // because it will be filtered out during the database query.
    }

    info!(
        "process_thread({}) got thread with {} posts",
        thread_descriptor,
        chan_thread.posts.len()
    );

    process_posts(
        site_repository,
        &last_processed_post,
        thread_descriptor,
        &chan_thread,
        database
    ).await?;

    if last_modified.is_some() {
        let last_modified = last_modified.unwrap();

        info!(
            "process_thread({}) updating last_modified: {}",
            thread_descriptor,
            last_modified
        );

        thread_repository::store_last_modified(
            &last_modified,
            thread_descriptor,
            database
        ).await?;
    }

    return Ok(());
}

async fn process_posts(
    site_repository: &Arc<SiteRepository>,
    last_processed_post: &Option<PostDescriptor>,
    thread_descriptor: &ThreadDescriptor,
    chan_thread: &ChanThread,
    database: &Arc<Database>
) -> anyhow::Result<()> {
    info!("process_posts({}) start", thread_descriptor);

    if chan_thread.posts.is_empty() {
        info!("process_posts({}) no posts to process", thread_descriptor);
        return Ok(());
    }

    let imageboard = site_repository.by_site_descriptor(thread_descriptor.site_descriptor());
    if imageboard.is_none() {
        info!("process_posts({}) no site found", thread_descriptor);
        return Ok(());
    }

    let imageboard = imageboard.unwrap();

    let mut found_post_replies_set =
        HashSet::<FoundPostReply>::with_capacity(chan_thread.posts.len());
    let mut new_posts_count = 0;
    let post_quote_regex = imageboard.post_quote_regex();

    find_post_replies(
        thread_descriptor,
        &chan_thread,
        last_processed_post,
        &mut found_post_replies_set,
        &mut new_posts_count,
        post_quote_regex
    );

    info!("process_posts({}) new_posts_count: {}", thread_descriptor, new_posts_count);

    let last_post = chan_thread.posts.last();
    if last_post.is_none() {
        return Ok(());
    }

    let last_post = last_post.unwrap();

    let last_post_descriptor = PostDescriptor::from_thread_descriptor(
        thread_descriptor.clone(),
        last_post.post_no,
        last_post.post_sub_no.unwrap_or(0)
    );

    info!(
        "process_posts({}) storing {} as last_processed_post",
        thread_descriptor,
        last_post_descriptor
    );

    thread_repository::store_last_processed_post(
        &last_post_descriptor,
        database
    ).await?;

    if found_post_replies_set.is_empty() {
        info!("process_posts({}) end. No post replies found", thread_descriptor);
        return Ok(());
    }

    info!("process_posts({}) found {} quotes", thread_descriptor, found_post_replies_set.len());

    find_and_store_new_post_replies(
        thread_descriptor,
        &mut found_post_replies_set,
        database,
    ).await?;

    info!("process_posts({}) end. Success!", thread_descriptor);
    return Ok(());
}

pub async fn find_and_store_new_post_replies(
    thread_descriptor: &ThreadDescriptor,
    found_post_replies_set: &mut HashSet<FoundPostReply>,
    database: &Arc<Database>,
) -> anyhow::Result<()> {
    let found_post_replies = found_post_replies_set.iter().collect::<Vec<&FoundPostReply>>();

    let post_descriptor_db_ids = post_descriptor_id_repository::get_many_found_post_reply_db_ids(
        &found_post_replies
    ).await;

    if post_descriptor_db_ids.is_empty() {
        info!("process_posts({}) end. No reply db_ids found", thread_descriptor);
        return Ok(());
    }

    let post_replies = post_repository::find_new_replies(
        thread_descriptor,
        database,
        &post_descriptor_db_ids_to_vec_of_unique_keys(&post_descriptor_db_ids)
    ).await?;

    if post_replies.len() > 0 {
        info!(
            "process_posts({}) storing {} post replies into the database",
            thread_descriptor,
            post_replies.len()
        );

        post_reply_repository::store(&post_replies, &post_descriptor_db_ids, database)
            .await
            .context(format!("Failed to store post {} replies", post_replies.len()))?;
    }

    return Ok(());
}

fn find_post_replies(
    thread_descriptor: &ThreadDescriptor,
    chan_thread: &ChanThread,
    last_processed_post: &Option<PostDescriptor>,
    found_post_replies_set: &mut HashSet<FoundPostReply>,
    new_posts_count: &mut i32,
    post_quote_regex: &Regex
) {
    for post in &chan_thread.posts {
        let origin = PostDescriptor::from_thread_descriptor(
            thread_descriptor.clone(),
            post.post_no,
            post.post_sub_no.unwrap_or(0)
        );

        if last_processed_post.is_some() {
            let last_processed_post = last_processed_post.clone().unwrap();
            let comparison_result = post_helpers::compare_post_descriptors(
                &origin,
                &last_processed_post
            );

            if comparison_result == Ordering::Less || comparison_result == Ordering::Equal {
                continue;
            }
        }

        *new_posts_count += 1;

        let post_comment = post.comment_unparsed.as_ref().map(|com| com.as_str()).unwrap_or("");
        if post_comment.is_empty() {
            continue;
        }

        let captures_iter = post_quote_regex.captures_iter(post_comment);
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

            let replies_to = PostDescriptor::from_thread_descriptor(
                thread_descriptor.clone(),
                quote_post_no,
                0
            );

            let post_reply = FoundPostReply {
                origin: origin.clone(),
                replies_to
            };

            found_post_replies_set.insert(post_reply);
        }
    }
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
        let post_descriptor_db_id = *key;

        if !duplicates.insert(post_descriptor_db_id) {
            continue;
        }

        result_vec.push(post_descriptor_db_id);
    }

    return result_vec;
}