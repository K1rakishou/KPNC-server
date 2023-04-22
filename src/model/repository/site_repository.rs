use std::collections::HashMap;
use std::sync::Arc;

use crate::model::data::chan::{ChanThread, PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::imageboards::base_imageboard::Imageboard;
use crate::model::imageboards::chan4::Chan4;
use crate::model::imageboards::dvach::Dvach;

type ImageboardSynced = Arc<dyn Imageboard + Sync + Send>;

pub struct SiteRepository {
    sites: HashMap<String, ImageboardSynced>
}

impl SiteRepository {
    pub fn new() -> SiteRepository {
        let mut sites = HashMap::<String, ImageboardSynced>::new();

        let chan4 = Chan4 {};
        sites.insert(chan4.name().to_string(), Arc::new(chan4));

        let dvach = Dvach {};
        sites.insert(dvach.name().to_string(), Arc::new(dvach));

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

    pub fn by_site_descriptor(&self, site_descriptor: &SiteDescriptor) -> Option<&ImageboardSynced> {
        return self.sites.get(site_descriptor.site_name());
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

    pub fn read_thread_json(
        &self,
        site_descriptor: &SiteDescriptor,
        json: &String
    ) -> anyhow::Result<Option<ChanThread>> {
        let imageboard = self.sites.get(site_descriptor.site_name());
        if imageboard.is_none() {
            return Ok(None);
        }

        let imageboard = imageboard.unwrap();
        return imageboard.read_thread_json(json)
    }

}