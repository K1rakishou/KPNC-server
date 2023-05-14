use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use regex::Regex;

use crate::info;
use crate::model::data::chan::{ChanThread, PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::repository::thread_repository;

#[async_trait]
pub trait Imageboard {
    fn name(&self) -> &'static str;
    fn matches(&self, site_descriptor: &SiteDescriptor) -> bool;
    fn url_matches(&self, url: &str) -> bool;
    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor>;
    fn post_descriptor_to_url(&self, post_descriptor: &PostDescriptor) -> Option<String>;
    fn post_quote_regex(&self) -> &'static Regex;

    async fn load_thread(
        &self,
        http_client: &'static reqwest::Client,
        database: &Arc<Database>,
        thread_descriptor: &ThreadDescriptor,
        last_processed_post: &Option<PostDescriptor>
    ) -> anyhow::Result<ThreadLoadResult>;
}

pub enum ThreadLoadResult {
    Success(ChanThread, Option<DateTime<FixedOffset>>),
    ThreadWasNotModifiedSinceLastCheck,
    SiteNotSupported,
    HeadRequestBadStatusCode(u16),
    GetRequestBadStatusCode(u16),
    FailedToReadChanThread(String)
}

pub async fn was_content_modified_since_last_check(
    thread_descriptor: &ThreadDescriptor,
    last_modified_remote: &Option<DateTime<FixedOffset>>,
    database: &Arc<Database>
) -> anyhow::Result<bool> {
    if last_modified_remote.is_none() {
        return Ok(true)
    }

    let last_modified_local = thread_repository::get_last_modified(
        thread_descriptor,
        database
    ).await?;

    if last_modified_local.is_none() {
        return Ok(true);
    }

    let last_modified_remote = last_modified_remote.unwrap();
    let last_modified_local = last_modified_local.unwrap();
    let content_was_modified = last_modified_remote > last_modified_local;

    info!(
        "was_content_modified_since_last_check({}) \
        last_modified_remote: {}, \
        last_modified_local: {}, \
        content_was_modified: {}",
        thread_descriptor,
        last_modified_remote,
        last_modified_local,
        content_was_modified
    );

    return Ok(content_was_modified);
}

pub fn post_url_to_post_descriptor(
    imageboard: &dyn Imageboard,
    post_url: &str,
    post_url_regex: &Regex
) -> Option<PostDescriptor> {
    if !imageboard.url_matches(post_url) {
        return None;
    }

    let captures = post_url_regex.captures(post_url);
    if captures.is_none() {
        return None;
    }

    let captures = captures.unwrap();

    let site_name = captures.get(1)?.as_str();
    if site_name.is_empty() {
        return None;
    }

    let board_code = captures.get(2)?.as_str();
    if board_code.is_empty() {
        return None
    }

    let thread_no_raw = captures.get(3)?.as_str();
    let thread_no = u64::from_str(thread_no_raw);
    if thread_no.is_err() {
        return None;
    }
    let thread_no = thread_no.unwrap();

    let post_no = captures.get(4)
        .map(|post_no| post_no.as_str())
        .unwrap_or("");

    if post_no.is_empty() {
        return None;
    }

    let post_no = u64::from_str(post_no);
    if post_no.is_err() {
        return None;
    }
    let post_no = post_no.unwrap();

    let post_descriptor = PostDescriptor::new(
        String::from(site_name),
        String::from(board_code),
        thread_no,
        post_no,
        0
    );

    return Some(post_descriptor);
}