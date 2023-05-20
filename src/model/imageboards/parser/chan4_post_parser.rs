use std::cmp::Ordering;

use serde::Deserialize;

use crate::{error, info};
use crate::helpers::post_helpers::compare_post_descriptors;
use crate::model::data::chan::{ChanPost, ChanThread, PostDescriptor, ThreadDescriptor};
use crate::model::imageboards::parser::post_parser::PostParser;

pub enum ThreadParseResult {
    Ok(ChanThread),
    PartialParseFailed,
    FullParseFailed,
    ThreadDeletedOrClosed,
    ThreadInaccessible,
    ServerSentIncorrectData(String),
    ServerError(i32, String)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Chan4PostPartial {
    TailInfo(TailInfo),
    TailPost(TailPost),
}

#[derive(Debug, Deserialize)]
struct TailInfo {
    no: u64,
    tail_size: u16,
    tail_id: u64,
    closed: Option<i32>,
    archived: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct TailPost {
    no: u64,
    resto: u64,
    com: Option<String>
}

#[derive(Debug, Deserialize)]
struct Chan4PostFull {
    no: u64,
    resto: u64,
    com: Option<String>,
    closed: Option<i32>,
    archived: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct Chan4ThreadFull {
    posts: Vec<Chan4PostFull>
}

pub struct Chan4PostParser {}

impl PostParser for Chan4PostParser {
    fn parse(
        &self,
        thread_descriptor: &ThreadDescriptor,
        last_processed_post: &Option<PostDescriptor>,
        thread_json: &String
    ) -> anyhow::Result<ThreadParseResult> {
        if last_processed_post.is_some() {
            info!(
                "parse({}) parsing thread partially last_processed_post: {}, thread_json_len: {}",
                thread_descriptor,
                last_processed_post.clone().unwrap(),
                thread_json.len()
            );

            return parse_thread_partial(
                thread_descriptor,
                last_processed_post,
                thread_json
            );
        }

        info!(
            "parse({}) parsing thread fully thread_json_len: {}",
            thread_descriptor,
            thread_json.len()
        );

        return parse_thread_full(thread_json);
    }
}

fn parse_thread_full(thread_json: &String) -> anyhow::Result<ThreadParseResult> {
    let mut result_posts = Vec::<ChanPost>::with_capacity(32);

    let mut archived = false;
    let mut closed = false;

    let chan4_thread_full: Chan4ThreadFull = serde_json::from_str(thread_json)?;

    for (index, chan4_post_full) in chan4_thread_full.posts.iter().enumerate() {
        if index == 0 {
            archived = chan4_post_full.archived.unwrap_or(0) == 1;
            closed = chan4_post_full.closed.unwrap_or(0) == 1;
        }

        let chan_post = ChanPost {
            post_no: chan4_post_full.no,
            post_sub_no: None,
            comment_unparsed: chan4_post_full.com.clone(),
        };

        result_posts.push(chan_post);
    }

    let chan_thread = ChanThread {
        archived: archived,
        closed: closed,
        posts: result_posts
    };

    return Ok(ThreadParseResult::Ok(chan_thread));
}

fn parse_thread_partial(
    thread_descriptor: &ThreadDescriptor,
    last_processed_post: &Option<PostDescriptor>,
    thread_json: &String
) -> anyhow::Result<ThreadParseResult>  {
    let mut result_posts = Vec::<ChanPost>::with_capacity(32);

    let mut archived = false;
    let mut closed = false;
    let mut op_post_found = false;

    let last_processed_post = last_processed_post.clone().unwrap();
    let parsed_data: serde_json::Value = serde_json::from_str(thread_json)?;

    let posts = if let Some(posts) = parsed_data.get("posts") {
        posts
    } else {
        error!("parse_thread_partial({}) \'posts\' not found in json", thread_descriptor);
        return Ok(ThreadParseResult::PartialParseFailed);
    };

    let chan4_post_partial_vec: Vec<Chan4PostPartial> = serde_json::from_value(posts.clone())?;

    for chan4_post_partial in chan4_post_partial_vec {
        match chan4_post_partial {
            Chan4PostPartial::TailInfo(tail_info) => {
                op_post_found = true;

                let tail_post_descriptor = PostDescriptor::from_thread_descriptor(
                    last_processed_post.thread_descriptor.clone(),
                    tail_info.tail_id,
                    0
                );

                let ordering = compare_post_descriptors(&last_processed_post, &tail_post_descriptor);
                if ordering == Ordering::Less {
                    info!(
                        "parse_thread_partial({}) last_processed_post ({}) < tail_post_descriptor ({}). \
                        Switching to full thread load.",
                        thread_descriptor,
                        last_processed_post,
                        tail_post_descriptor
                    );
                    return Ok(ThreadParseResult::PartialParseFailed);
                }

                archived = tail_info.archived.unwrap_or(0) == 1;
                closed = tail_info.closed.unwrap_or(0) == 1;
            }
            Chan4PostPartial::TailPost(tail_post) => {
                if !op_post_found {
                    error!("parse_thread_partial({}) OP not found", thread_descriptor);
                    return Ok(ThreadParseResult::PartialParseFailed);
                }

                let chan4_post = ChanPost {
                    post_no: tail_post.no,
                    post_sub_no: None,
                    comment_unparsed: tail_post.com,
                };

                result_posts.push(chan4_post);
            }
        }
    }

    if !op_post_found {
        error!("parse_thread_partial({}) OP not found", thread_descriptor);
        return Ok(ThreadParseResult::PartialParseFailed);
    }

    let chan_thread = ChanThread {
        archived: archived,
        closed: closed,
        posts: result_posts
    };

    return Ok(ThreadParseResult::Ok(chan_thread));
}