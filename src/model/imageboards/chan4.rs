use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use serde::Deserialize;
use url::Url;

use crate::error;
use crate::helpers::string_helpers;
use crate::model::data::chan::{ChanPost, ChanThread, PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::database::db::Database;
use crate::model::imageboards::base_imageboard::{Imageboard, post_url_to_post_descriptor, ThreadLoadResult, was_content_modified_since_last_check};

lazy_static! {
    static ref POST_URL_REGEX: Regex =
        Regex::new(r"https://boards.(\w+).org/(\w+)/thread/(\d+)(?:#p(\d+))?").unwrap();
    static ref POST_REPLY_QUOTE_REGEX: Regex =
        Regex::new(r#"class="quotelink">&gt;&gt;(\d+)</a>"#).unwrap();
}

pub struct Chan4 {
    pub http_client: &'static reqwest::Client
}

#[derive(Debug, Deserialize)]
struct Chan4Post {
    no: u64,
    resto: u64,
    closed: Option<i32>,
    archived: Option<i32>,
    com: Option<String>
}

#[derive(Debug, Deserialize)]
struct Chan4Thread {
    posts: Vec<Chan4Post>
}

#[async_trait]
impl Imageboard for Chan4 {
    fn name(&self) -> &'static str {
        return "4chan";
    }

    fn matches(&self, site_descriptor: &SiteDescriptor) -> bool {
        return site_descriptor.site_name_str() == "4chan";
    }

    fn url_matches(&self, url: &str) -> bool {
        let url = Url::parse(url);
        if url.is_err() {
            return false;
        }

        let url = url.unwrap();

        let domain = url.domain();
        if domain.is_none() {
            return false;
        }

        let site_name = string_helpers::extract_site_name_from_domain(domain.unwrap());
        if site_name.is_empty() {
            return false
        }

        let site_name = site_name.to_string().to_lowercase();
        // TODO: check top-level domain as well
        return site_name == "4chan" || site_name == "4channel";
    }

    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor> {
        return post_url_to_post_descriptor(self, post_url, &POST_URL_REGEX);
    }

    fn post_descriptor_to_url(&self, post_descriptor: &PostDescriptor) -> Option<String> {
        let mut string_builder = string_builder::Builder::new(72);

        string_builder.append("https://boards.");
        string_builder.append(post_descriptor.site_name().as_str());
        string_builder.append(".org");
        string_builder.append("/");
        string_builder.append(post_descriptor.board_code().as_str());
        string_builder.append("/");
        string_builder.append("thread");
        string_builder.append("/");
        string_builder.append(post_descriptor.thread_no().to_string());
        string_builder.append("#p");
        string_builder.append(post_descriptor.post_no.to_string());

        let string = string_builder.string();
        if string.is_err() {
            return None;
        }

        return Some(string.unwrap());
    }

    fn thread_json_endpoint(
        &self,
        thread_descriptor: &ThreadDescriptor
    ) -> Option<String> {
        if !self.matches(&thread_descriptor.catalog_descriptor.site_descriptor) {
            return None;
        }

        let endpoint = format!(
            "https://a.4cdn.org/{}/thread/{}.json",
            thread_descriptor.board_code(),
            thread_descriptor.thread_no
        );

        return Some(endpoint);
    }

    fn post_quote_regex(&self) -> &'static Regex {
        return &POST_REPLY_QUOTE_REGEX;
    }

    async fn load_thread(
        &self,
        database: &Arc<Database>,
        thread_descriptor: &ThreadDescriptor,
        last_processed_post: &Option<PostDescriptor>,
        thread_json_endpoint: &String
    ) -> anyhow::Result<ThreadLoadResult> {
        let head_request = self.http_client.head(thread_json_endpoint.clone()).build()?;
        let head_response = self.http_client.execute(head_request).await?;

        let head_request_status_code = head_response.status().as_u16();
        if head_request_status_code != 200 {
            return Ok(ThreadLoadResult::HeadRequestBadStatusCode(head_request_status_code));
        }

        let last_modified_str = head_response.headers()
            .get("Last-Modified")
            .map(|header_value| header_value.to_str().unwrap_or(""))
            .unwrap_or("");

        let last_modified = DateTime::parse_from_rfc2822(last_modified_str);
        let last_modified: Option<DateTime<FixedOffset>> = if last_modified.is_err() {
            error!(
                "process_thread({}) Failed to parse \'{}\' as DateTime (last_modified)",
                thread_descriptor,
                last_modified_str
            );

            None
        } else {
            Some(last_modified.unwrap())
        };

        let thread_updated_since_last_check = was_content_modified_since_last_check(
            thread_descriptor,
            &last_modified,
            database
        ).await?;

        if !thread_updated_since_last_check {
            return Ok(ThreadLoadResult::ThreadWasNotModifiedSinceLastCheck);
        }

        let request = self.http_client.get(thread_json_endpoint.clone()).build()?;
        let response = self.http_client.execute(request)
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
            return Ok(ThreadLoadResult::GetRequestBadStatusCode(status_code));
        }

        let response_text = response.text()
            .await
            .with_context(|| {
                return format!(
                    "process_thread({}) Failed to extract text from response",
                    thread_descriptor
                );
            })?;

        let chan_thread = read_thread_json(
            &response_text
        )?;

        if chan_thread.is_none() {
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
                    format!("{} (+{} more)", String::from_utf16_lossy(chars.as_slice()), remaining_chars_count)
                }
            };

            return Ok(ThreadLoadResult::FailedToReadChanThread(body_text));
        }

        return Ok(ThreadLoadResult::Success(chan_thread.unwrap(), last_modified));
    }
}

fn read_thread_json(json: &String) -> anyhow::Result<Option<ChanThread>> {
    let chan4_thread = serde_json::from_str::<Chan4Thread>(json)?;
    if chan4_thread.posts.is_empty() {
        return Ok(None);
    }

    let original_post = chan4_thread.posts.first();
    if original_post.is_none() {
        return Ok(None);
    }

    let original_post = original_post.unwrap();
    let mut chan_posts = Vec::<ChanPost>::with_capacity(chan4_thread.posts.len());

    for chan4_post in &chan4_thread.posts {
        let chan_post = ChanPost {
            post_no: chan4_post.no,
            post_sub_no: None,
            comment_unparsed: chan4_post.com.clone()
        };

        chan_posts.push(chan_post);
    }

    let chan_thread = ChanThread {
        posts: chan_posts,
        closed: original_post.closed.unwrap_or(0) == 1,
        archived: original_post.archived.unwrap_or(0) == 1,
    };

    return Ok(Some(chan_thread));
}


#[test]
fn test_url_conversion() {
    let chan4 = Chan4 { http_client: &reqwest::Client::new() };

    let pd1 = chan4.post_url_to_post_descriptor(
        "https://boards.4chan.org/a/thread/1234567890#p1234567891"
    ).unwrap();

    assert_eq!("4chan", pd1.site_name().as_str());
    assert_eq!(1234567890, pd1.thread_no());
    assert_eq!(1234567891, pd1.post_no);

    let td1 = chan4.post_url_to_post_descriptor(
        "https://boards.4chan.org/a/thread/1234567890"
    );

    assert!(td1.is_none());
}

#[test]
fn test_post_quote_regex() {
    let test_string = "<a href=\"#p251260223\" class=\"quotelink\">&gt;&gt;251260223</a>";
    let captures = POST_REPLY_QUOTE_REGEX.captures(test_string).unwrap();
    assert_eq!(2, captures.len());
    assert_eq!("251260223", captures.get(1).unwrap().as_str());

    let test_string = "<a href=\"#p92933496\" class=\"quotelink\">&gt;&gt;92933496</a><br>\
    <a href=\"#p92933523\" class=\"quotelink\">&gt;&gt;92933523</a><br>\
    Will look into them, upon first look, it shouldn&#039;t be much work";
    let captures = POST_REPLY_QUOTE_REGEX.captures_iter(test_string).collect::<Vec<Captures>>();
    assert_eq!(2, captures.len());
    assert_eq!("92933496", captures.get(0).unwrap().get(1).unwrap().as_str());
    assert_eq!("92933523", captures.get(1).unwrap().get(1).unwrap().as_str());
}