use std::str::FromStr;

use lazy_static::lazy_static;
use regex::{Captures, Regex};
use serde::Deserialize;
use url::Url;

use crate::helpers::string_helpers;
use crate::model::data::chan::{ChanPost, ChanThread, PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::imageboards::base_imageboard::Imageboard;

lazy_static! {
    static ref POST_URL_REGEX: Regex =
        Regex::new(r"https://(\w+).\w+/(\w+)/res/(\d+).html(?:#(\d+))?").unwrap();
    static ref POST_REPLY_QUOTE_REGEX: Regex =
        Regex::new(r##">>>(\d+)\s*</a>"##).unwrap();
}


pub struct Dvach {

}

#[derive(Debug, Deserialize)]
struct DvachPost {
    num: u64,
    op: u64,
    closed: Option<i32>,
    comment: Option<String>
}

#[derive(Debug, Deserialize)]
struct DvachThread {
    posts: Vec<DvachPost>
}

#[derive(Debug, Deserialize)]
struct DvachThreads {
    threads: Vec<DvachThread>
}

impl Imageboard for Dvach {
    fn name(&self) -> &'static str {
        return "2ch"
    }

    fn matches(&self, site_descriptor: &SiteDescriptor) -> bool {
        return site_descriptor.site_name_str() == "2ch";
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
        return site_name == "2ch";
    }

    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor> {
        if !self.url_matches(post_url) {
            return None;
        }

        let captures = POST_URL_REGEX.captures(post_url);
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

        let post_no = captures.get(4).map(|post_no| post_no.as_str()).unwrap_or("");
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

    fn post_descriptor_to_url(&self, post_descriptor: &PostDescriptor) -> Option<String> {
        let mut string_builder = string_builder::Builder::new(72);

        string_builder.append("https://");
        string_builder.append(post_descriptor.site_name().as_str());
        string_builder.append(".hk");
        string_builder.append("/");
        string_builder.append(post_descriptor.board_code().as_str());
        string_builder.append("/");
        string_builder.append("res");
        string_builder.append("/");
        string_builder.append(post_descriptor.thread_no().to_string());
        string_builder.append(".html");
        string_builder.append("#");
        string_builder.append(post_descriptor.post_no.to_string());

        let string = string_builder.string();
        if string.is_err() {
            return None;
        }

        return Some(string.unwrap());
    }

    fn thread_json_endpoint(&self, thread_descriptor: &ThreadDescriptor) -> Option<String> {
        if !self.matches(&thread_descriptor.catalog_descriptor.site_descriptor) {
            return None;
        }

        let endpoint = format!(
            "https://2ch.hk/{}/res/{}.json",
            thread_descriptor.board_code(),
            thread_descriptor.thread_no
        );

        return Some(endpoint);
    }

    fn post_quote_regex(&self) -> &'static Regex {
        return &POST_REPLY_QUOTE_REGEX;
    }

    fn read_thread_json(&self, json: &String) -> anyhow::Result<Option<ChanThread>> {
        let dvach_threads = serde_json::from_str::<DvachThreads>(json)?;
        if dvach_threads.threads.is_empty() {
            return Ok(None);
        }

        let dvach_thread = dvach_threads.threads.first();
        if dvach_thread.is_none() {
            return Ok(None);
        }

        let dvach_thread = dvach_thread.unwrap();

        let original_post = dvach_thread.posts.first();
        if original_post.is_none() {
            return Ok(None);
        }

        let original_post = original_post.unwrap();
        let mut chan_posts = Vec::<ChanPost>::with_capacity(dvach_thread.posts.len());

        for chan4_post in &dvach_thread.posts {
            let chan_post = ChanPost {
                post_no: chan4_post.num,
                post_sub_no: None,
                comment_unparsed: chan4_post.comment.clone()
            };

            chan_posts.push(chan_post);
        }

        let chan_thread = ChanThread {
            posts: chan_posts,
            closed: original_post.closed.unwrap_or(0) == 1,
            archived: false,
        };

        return Ok(Some(chan_thread));
    }
}

#[test]
fn test_url_conversion() {
    let dvach = Dvach {};

    let pd1 = dvach.post_url_to_post_descriptor(
        "https://2ch.hk/test/res/197273.html#197871"
    ).unwrap();

    assert_eq!("2ch", pd1.site_name().as_str());
    assert_eq!(197273, pd1.thread_no());
    assert_eq!(197871, pd1.post_no);

    let td1 = dvach.post_url_to_post_descriptor(
        "https://2ch.hk/test/res/197273.html"
    );

    assert!(td1.is_none());
}

#[test]
fn test_post_quote_regex() {
    let test_string = "<a href=\"/test/res/197273.html#197895\" class=\"post-reply-link\" \
    data-thread=\"197273\" data-num=\"197895\">>>197895</a><br><a href=\"/test/res/197273.html#197896\" \
    class=\"post-reply-link\" data-thread=\"197273\" data-num=\"197896\">>>197896</a><br>test reply 1";

    let captures = POST_REPLY_QUOTE_REGEX.captures_iter(test_string).collect::<Vec<Captures>>();
    assert_eq!(2, captures.len());
    assert_eq!("197895", captures.get(0).unwrap().get(1).unwrap().as_str());
    assert_eq!("197896", captures.get(1).unwrap().get(1).unwrap().as_str());
}