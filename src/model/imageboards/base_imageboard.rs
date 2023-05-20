use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use async_recursion::async_recursion;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use regex::Regex;
use reqwest::Response;

use crate::{error, info};
use crate::model::data::chan::{ChanThread, PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::imageboards::parser::chan4_post_parser::ThreadParseResult;
use crate::model::imageboards::parser::post_parser::PostParser;
use crate::model::repository::site_repository::ImageboardSynced;
use crate::model::repository::thread_repository;

#[async_trait]
pub trait Imageboard {
    fn name(&self) -> &'static str;
    fn matches(&self, site_descriptor: &SiteDescriptor) -> bool;
    fn url_matches(&self, url: &str) -> bool;
    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor>;
    fn post_descriptor_to_url(&self, post_descriptor: &PostDescriptor) -> Option<String>;
    fn post_quote_regex(&self) -> &'static Regex;
    fn post_parser(&self) -> &'static Box<dyn PostParser + Sync>;
    fn thread_json_endpoint(
        &self,
        thread_descriptor: &ThreadDescriptor,
        last_processed_post: &Option<PostDescriptor>
    ) -> Option<String>;
    fn supports_partial_load_head_request(&self) -> bool;
}

pub enum ThreadLoadResult {
    Success(ChanThread, Option<DateTime<FixedOffset>>),
    ThreadWasNotModifiedSinceLastCheck,
    SiteNotSupported,
    HeadRequestBadStatusCode(u16),
    GetRequestBadStatusCode(u16),
    ThreadDeletedOrClosed,
    ThreadInaccessible,
    FailedToReadChanThread(String),
    ServerSentIncorrectData(String),
    ServerError(i32, String)
}

#[async_recursion]
pub async fn load_thread(
    imageboard: &ImageboardSynced,
    http_client: &'static reqwest::Client,
    database: &Arc<Database>,
    thread_descriptor: &ThreadDescriptor,
    last_processed_post: &Option<PostDescriptor>
) -> anyhow::Result<ThreadLoadResult> {
    info!(
        "load_thread({}) using partial load: {}",
        thread_descriptor,
        last_processed_post.is_some()
    );

    let thread_json_endpoint = imageboard.thread_json_endpoint(thread_descriptor, last_processed_post);
    if thread_json_endpoint.is_none() {
        info!("load_thread({}) site is not supported", thread_descriptor);
        return Ok(ThreadLoadResult::SiteNotSupported);
    }

    let thread_json_endpoint = thread_json_endpoint.unwrap();

    let head_request = http_client.head(thread_json_endpoint.clone()).build()?;
    let head_response = http_client.execute(head_request).await?;

    let status_code = head_response.status().as_u16();
    if status_code != 200 {
        // 2ch.hk will return 404 when sending a HEAD request to v2 API that supports partial thread
        // loading so we don't want to switch to full thread load in the case, just ignore this 404.
        if status_code != 404 || imageboard.supports_partial_load_head_request() {
            if last_processed_post.is_some() && status_code == 404 {
                info!(
                    "load_thread({}) HEAD status_code == 404, switching to full load",
                    thread_descriptor
                );

                return load_thread(
                    imageboard,
                    http_client,
                    database,
                    thread_descriptor,
                    &None,
                ).await;
            }

            error!("load_thread({}) HEAD status_code == 404", thread_descriptor);
            return Ok(ThreadLoadResult::HeadRequestBadStatusCode(status_code));
        }
    }

    let last_modified = parse_last_modified_header(
        thread_descriptor,
        head_response
    ).await;

    if last_modified.is_some() {
        let thread_updated_since_last_check = was_content_modified_since_last_check(
            thread_descriptor,
            &last_modified,
            database
        ).await?;

        if !thread_updated_since_last_check {
            info!("load_thread({}) Thread was not updated since last check", thread_descriptor);
            return Ok(ThreadLoadResult::ThreadWasNotModifiedSinceLastCheck);
        }
    }

    let request = http_client.get(thread_json_endpoint.clone()).build()?;
    let response = http_client.execute(request)
        .await
        .with_context(|| {
            return format!(
                "load_thread({}) Failed to execute GET request to \'{}\' endpoint",
                thread_descriptor,
                thread_json_endpoint
            );
        })?;

    let status_code = response.status().as_u16();
    if status_code != 200 {
        if last_processed_post.is_some() && status_code == 404 {
            info!("load_thread({}) GET status_code == 404, switching to full load", thread_descriptor);
            return load_thread(
                imageboard,
                http_client,
                database,
                thread_descriptor,
                &None
            ).await;
        }

        error!("load_thread({}) GET status_code == 404", thread_descriptor);
        return Ok(ThreadLoadResult::GetRequestBadStatusCode(status_code));
    }

    let response_text = response.text()
        .await
        .with_context(|| {
            return format!(
                "load_thread({}) Failed to extract text from response",
                thread_descriptor
            );
        })?;

    let thread_parse_result = imageboard.post_parser().parse(
        thread_descriptor,
        last_processed_post,
        &response_text
    );

    let thread_parse_result = if thread_parse_result.is_err() {
        let to_print_chars_count = 512;
        let chars = response_text.chars();
        let chars_count = chars.size_hint().0;
        let chars: Vec<u16> = chars.take(to_print_chars_count).map(|ch| ch as u16).collect();

        let body_text = if chars.is_empty() {
            String::from("<body is empty>")
        } else {
            if chars_count < to_print_chars_count {
                String::from_utf16_lossy(chars.as_slice())
            } else {
                let remaining_chars_count = chars_count - to_print_chars_count;
                format!(
                    "{} (+{} more)",
                    String::from_utf16_lossy(chars.as_slice()),
                    remaining_chars_count
                )
            }
        };

        error!(
            "load_thread({}) imageboard.post_parser().parse error: {}",
            thread_descriptor,
            thread_parse_result.err().unwrap()
        );

        return Ok(ThreadLoadResult::FailedToReadChanThread(body_text));
    } else {
        thread_parse_result.unwrap()
    };

    let chan_thread = match thread_parse_result {
        ThreadParseResult::Ok(chan_thread) => { chan_thread }
        ThreadParseResult::PartialParseFailed => {
            info!(
                "load_thread({}) Failed to parse thread partially, switching to full load",
                thread_descriptor
            );

            return load_thread(
                imageboard,
                http_client,
                database,
                thread_descriptor,
                &None
            ).await;
        }
        ThreadParseResult::FullParseFailed => {
            let error_text = format!("Failed to parse thread {} fully", thread_descriptor);
            return Ok(ThreadLoadResult::FailedToReadChanThread(error_text));
        }
        ThreadParseResult::ThreadDeletedOrClosed => {
            return Ok(ThreadLoadResult::ThreadDeletedOrClosed);
        }
        ThreadParseResult::ThreadInaccessible => {
            return Ok(ThreadLoadResult::ThreadInaccessible);
        }
        ThreadParseResult::ServerSentIncorrectData(reason) => {
            return Ok(ThreadLoadResult::ServerSentIncorrectData(reason));
        }
        ThreadParseResult::ServerError(code, message) => {
            return Ok(ThreadLoadResult::ServerError(code, message));
        }
    };

    if chan_thread.posts.is_empty() {
        info!(
            "load_thread({}) thread has no posts, is partial load: {}",
            thread_descriptor,
            last_processed_post.is_some()
        );

        return Ok(ThreadLoadResult::FailedToReadChanThread("Thread has no posts".to_string()));
    }

    info!(
        "load_thread({}) success, is partial load: {}",
        thread_descriptor,
        last_processed_post.is_some()
    );

    return Ok(ThreadLoadResult::Success(chan_thread, last_modified));
}

async fn parse_last_modified_header(
    thread_descriptor: &ThreadDescriptor,
    head_response: Response
) -> Option<DateTime<FixedOffset>> {
    let last_modified_str = head_response.headers()
        .get("Last-Modified")
        .map(|header_value| header_value.to_str().unwrap_or(""))
        .unwrap_or("");

    if last_modified_str.is_empty() {
        info!("load_thread({}) Last-Modified not found in headers", thread_descriptor);
        return None;
    }

    let last_modified = DateTime::parse_from_rfc2822(last_modified_str);
    if last_modified.is_err() {
        error!(
            "load_thread({}) Failed to parse \'{}\' as DateTime (last_modified)",
            thread_descriptor,
            last_modified_str
        );

        return None;
    }

    return Some(last_modified.unwrap());
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