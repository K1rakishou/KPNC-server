use std::collections::HashMap;
use std::sync::Arc;

use crate::model::data::chan::{PostDescriptor, ThreadDescriptor};
use crate::model::imageboards::base_imageboard::Imageboard;
use crate::model::imageboards::chan4::Chan4;

type ImageboardSynced = Arc<dyn Imageboard + Sync + Send>;

pub struct SiteRepository {
    sites: HashMap<String, ImageboardSynced>
}

impl SiteRepository {
    pub fn new() -> SiteRepository {
        let mut sites = HashMap::<String, ImageboardSynced>::new();

        let chan4 = Chan4 {};
        sites.insert(chan4.name().to_string(), Arc::new(chan4));

        return SiteRepository { sites };
    }

    pub fn by_url(&self, post_url: &str) -> Option<&ImageboardSynced> {
        for (_, imageboard) in &self.sites {
            let matches = imageboard.url_matches(post_url);
            if matches {
                return Some(&imageboard)
            }
        }

        return None;
    }

    pub fn thread_json_endpoint(&self, thread_descriptor: &ThreadDescriptor) -> Option<String> {
        for (_, imageboard) in &self.sites {
            let matches = imageboard.matches(&thread_descriptor.site_descriptor());
            if matches {
                return imageboard.thread_json_endpoint(thread_descriptor);
            }
        }

        return None;
    }

    pub fn to_url(&self, post_descriptor: &PostDescriptor) -> Option<String> {
        for (_, imageboard) in &self.sites {
            let matches = imageboard.matches(&post_descriptor.site_descriptor());
            if matches {
                return imageboard.post_descriptor_to_url(post_descriptor);
            }
        }

        return None;
    }

}