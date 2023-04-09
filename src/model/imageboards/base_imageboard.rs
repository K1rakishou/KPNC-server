use crate::model::data::chan::{PostDescriptor, SiteDescriptor, ThreadDescriptor};

pub trait Imageboard {
    fn name(&self) -> &'static str;
    fn matches(&self, site_descriptor: &SiteDescriptor) -> bool;
    fn url_matches(&self, url: &str) -> bool;
    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor>;
    fn thread_json_endpoint(&self, thread_descriptor: &ThreadDescriptor) -> Option<String>;
}