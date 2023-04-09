use std::{env};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{anyhow, Context};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use serde::{Deserialize};
use crate::model::data::chan::ThreadDescriptor;
use crate::model::database::db::Database;
use crate::model::repository::posts_repository;
use crate::model::repository::site_repository::SiteRepository;

pub struct ThreadWatcher {
    num_cpus: u32,
    working: bool
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
        site_repository: &Arc<SiteRepository>
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

            let result = process_posts(self.num_cpus, database, site_repository).await;
            match result {
                Ok(_) => { info!("process_posts() success") }
                Err(error) => { error!("process_posts() error: {}", error) }
            }

            info!("process_posts() sleeping for {timeout_seconds} seconds...");
            sleep(Duration::from_secs(timeout_seconds)).await;
            info!("process_posts() sleeping for {timeout_seconds} seconds... done");
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

async fn process_posts(
    num_cpus: u32,
    database: &Arc<Database>,
    site_repository: &Arc<SiteRepository>
) -> anyhow::Result<()> {
    let all_watched_threads = posts_repository::get_all_watched_threads(database)
        .await.context("process_posts() Failed to get all watched threads")?;

    if all_watched_threads.is_empty() {
        info!("process_posts() no watched threads to process");
        return Ok(());
    }

    info!("process_posts() found {} watched threads", all_watched_threads.len());

    let mut chunk_size: usize = (num_cpus * 4) as usize;
    if chunk_size < 8 {
        chunk_size = 8;
    }
    if chunk_size > 128 {
        chunk_size = 128;
    }

    info!("process_posts() using chunk size {}", chunk_size);

    for thread_descriptors in all_watched_threads.chunks(chunk_size) {
        let mut join_handles: Vec<JoinHandle<()>> = Vec::with_capacity(chunk_size);

        for thread_descriptor in thread_descriptors {
            let thread_descriptor_cloned = thread_descriptor.clone();
            let database_cloned = database.clone();
            let site_repository_cloned = site_repository.clone();

            let join_handle = tokio::task::spawn(async move {
                let process_thread_result = process_thread(
                    &thread_descriptor_cloned,
                    &database_cloned,
                    &site_repository_cloned,
                ).await;

                if process_thread_result.is_err() {
                    let error = process_thread_result.err().unwrap();

                    error!(
                        "process_posts() Error \'{}\' while processing thread {}",
                        error,
                        thread_descriptor_cloned
                    );
                }
            });

            join_handles.push(join_handle);
        }

        futures::future::join_all(join_handles).await;
    }

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

        posts_repository::mark_all_thread_posts_dead(database, thread_descriptor).await?;
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

            posts_repository::mark_all_thread_posts_dead(database, thread_descriptor).await?;
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
        let chars = response_text.chars();
        let chars_count = chars.size_hint().0;
        let text: Vec<u16> = chars.take(128).map(|ch| ch as u16).collect();

        let body_text = if text.is_empty() {
            String::from("<body is empty>")
        } else {
            if chars_count < 128 {
                String::from_utf16_lossy(text.as_slice())
            } else {
                let remaining_chars_count = chars_count - 128;
                format!("{} +{} more", String::from_utf16_lossy(text.as_slice()), remaining_chars_count)
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

        posts_repository::mark_all_thread_posts_dead(database, thread_descriptor).await?;
        return Ok(());
    }

    debug!("process_thread({}) got thread with {} posts", thread_descriptor, chan_thread.posts.len());
    debug!("process_thread({}) OP: {:?}", thread_descriptor, original_post);

    return Ok(());
}