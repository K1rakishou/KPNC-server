use crate::model::data::chan::{ChanThread, PostDescriptor, SiteDescriptor, ThreadDescriptor};

pub trait Imageboard {
    fn name(&self) -> &'static str;
    fn matches(&self, site_descriptor: &SiteDescriptor) -> bool;
    fn url_matches(&self, url: &str) -> bool;
    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor>;
    fn post_descriptor_to_url(&self, post_descriptor: &PostDescriptor) -> Option<String>;
    fn thread_json_endpoint(&self, thread_descriptor: &ThreadDescriptor) -> Option<String>;
    
    fn read_thread_json(&self, json: &String) -> anyhow::Result<Option<ChanThread>>;
}