use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;
use url::Url;

use crate::helpers::string_helpers;
use crate::model::data::chan::{PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::imageboards::base_imageboard::Imageboard;

lazy_static! {
    static ref post_url_regex: Regex =
        Regex::new(r"https://boards.(\w+).org/(\w+)/thread/(\d+)(?:#p(\d+))?").unwrap();
}

pub struct Chan4 {

}

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
        return site_name == "4chan" || site_name == "4channel";
    }

    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor> {
        if !self.url_matches(post_url) {
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

        let mut post_no = captures.get(4).map(|post_no| post_no.as_str()).unwrap_or("");
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
}

#[test]
fn test() {
    let chan4 = Chan4 {};

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