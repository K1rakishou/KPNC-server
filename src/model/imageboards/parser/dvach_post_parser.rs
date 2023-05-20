use serde::Deserialize;

use crate::{error, info};
use crate::model::data::chan::{ChanPost, ChanThread, PostDescriptor, ThreadDescriptor};
use crate::model::imageboards::parser::chan4_post_parser::ThreadParseResult;
use crate::model::imageboards::parser::post_parser::PostParser;

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

pub struct DvachPostParser {}

impl PostParser for DvachPostParser {
    fn parse(
        &self,
        thread_descriptor: &ThreadDescriptor,
        last_processed_post: &Option<PostDescriptor>,
        thread_json: &String
    ) -> anyhow::Result<ThreadParseResult> {
        // TODO: '{"error":{"code":-3,"message":"Тред не существует."},"result":0}'
        if last_processed_post.is_some() {
            info!(
                "parse({}) parsing thread partially last_processed_post: {}, thread_json_len: {}",
                thread_descriptor,
                last_processed_post.clone().unwrap(),
                thread_json.len()
            );

            return parse_thread_partial(
                thread_descriptor,
                thread_json
            );
        }

        info!(
            "parse({}) parsing thread fully thread_json_len: {}",
            thread_descriptor,
            thread_json.len()
        );

        return parse_thread_full(
            thread_descriptor,
            thread_json
        );
    }
}

fn parse_thread_partial(
    thread_descriptor: &ThreadDescriptor,
    thread_json: &String
) -> anyhow::Result<ThreadParseResult> {
    let dvach_thread = serde_json::from_str::<DvachThread>(thread_json)?;
    return parse_shared(thread_descriptor, &dvach_thread);
}

fn parse_thread_full(
    thread_descriptor: &ThreadDescriptor,
    thread_json: &String
) -> anyhow::Result<ThreadParseResult> {
    let dvach_threads = serde_json::from_str::<DvachThreads>(thread_json)?;
    if dvach_threads.threads.is_empty() {
        error!("parse_thread_full({}) DvachThreads has no threads", thread_descriptor);
        return Ok(ThreadParseResult::FullParseFailed);
    }

    let dvach_thread = dvach_threads.threads.first().unwrap();
    return parse_shared(thread_descriptor, &dvach_thread);
}

fn parse_shared(
    thread_descriptor: &ThreadDescriptor,
    dvach_thread: &DvachThread
) -> anyhow::Result<ThreadParseResult> {
    let original_post = dvach_thread.posts.first();
    if original_post.is_none() {
        error!("parse_shared({}) DvachThread has no posts", thread_descriptor);
        return Ok(ThreadParseResult::FullParseFailed);
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

    return Ok(ThreadParseResult::Ok(chan_thread));
}