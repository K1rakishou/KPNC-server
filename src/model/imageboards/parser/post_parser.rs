use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::imageboards::parser::chan4_post_parser::ThreadParseResult;

pub trait PostParser {
    fn parse(
        &self, 
        thread_descriptor: &ThreadDescriptor,
        last_processed_post: &Option<PostDescriptor>,
        thread_json: &String
    ) -> anyhow::Result<ThreadParseResult>;
}