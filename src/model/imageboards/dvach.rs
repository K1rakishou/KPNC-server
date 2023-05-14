use async_trait::async_trait;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use url::Url;

use crate::helpers::string_helpers;
use crate::model::data::chan::{PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::imageboards::base_imageboard::{Imageboard, post_url_to_post_descriptor};
use crate::model::imageboards::parser::dvach_post_parser::DvachPostParser;
use crate::model::imageboards::parser::post_parser::PostParser;

lazy_static! {
    static ref POST_URL_REGEX: Regex =
        Regex::new(r"https://(\w+).\w+/(\w+)/res/(\d+).html(?:#(\d+))?").unwrap();
    static ref POST_REPLY_QUOTE_REGEX: Regex =
        Regex::new(r##">>>(\d+)\s*</a>"##).unwrap();

    static ref DVACH_POST_PARSER: Box<dyn PostParser + Sync> = Box::new(DvachPostParser {});
}


pub struct Dvach {
}

#[async_trait]
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
        return post_url_to_post_descriptor(self, post_url, &POST_URL_REGEX);
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

    fn post_quote_regex(&self) -> &'static Regex {
        return &POST_REPLY_QUOTE_REGEX;
    }

    fn post_parser(&self) -> &'static Box<dyn PostParser + Sync> {
        return &DVACH_POST_PARSER;
    }

    fn thread_json_endpoint(
        &self,
        thread_descriptor: &ThreadDescriptor,
        last_processed_post: &Option<PostDescriptor>
    ) -> Option<String> {
        if !self.matches(&thread_descriptor.catalog_descriptor.site_descriptor) {
            return None;
        }

        if last_processed_post.is_none() {
            let endpoint = format!(
                "https://2ch.hk/{}/res/{}.json",
                thread_descriptor.board_code(),
                thread_descriptor.thread_no
            );

            return Some(endpoint);
        }

        let last_processed_post = last_processed_post.as_ref().unwrap();

        let endpoint = format!(
            "https://2ch.hk/api/mobile/v2/after/{}/{}/{}",
            thread_descriptor.board_code(),
            thread_descriptor.thread_no,
            last_processed_post.post_no
        );

        return Some(endpoint);
    }

    fn supports_partial_load_head_request(&self) -> bool {
        return false;
    }

}

#[test]
fn test_url_conversion() {
    let dvach = Dvach { };

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