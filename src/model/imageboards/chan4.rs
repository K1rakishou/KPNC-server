use async_trait::async_trait;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use url::Url;

use crate::helpers::string_helpers;
use crate::model::data::chan::{PostDescriptor, SiteDescriptor, ThreadDescriptor};
use crate::model::imageboards::base_imageboard::{
    Imageboard,
    post_url_to_post_descriptor
};
use crate::model::imageboards::parser::chan4_post_parser::Chan4PostParser;
use crate::model::imageboards::parser::post_parser::PostParser;

lazy_static! {
    static ref POST_URL_REGEX: Regex =
        Regex::new(r"https://boards.(\w+).org/(\w+)/thread/(\d+)(?:#p(\d+))?").unwrap();
    static ref POST_REPLY_QUOTE_REGEX: Regex =
        Regex::new(r#"class="quotelink">&gt;&gt;(\d+)</a>"#).unwrap();

    static ref CHAN4_POST_PARSER: Box<dyn PostParser + Sync> = Box::new(Chan4PostParser {});
}

pub struct Chan4 {
}

#[async_trait]
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
        // TODO: check top-level domain as well
        return site_name == "4chan" || site_name == "4channel";
    }

    fn post_url_to_post_descriptor(&self, post_url: &str) -> Option<PostDescriptor> {
        return post_url_to_post_descriptor(self, post_url, &POST_URL_REGEX);
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

    fn post_quote_regex(&self) -> &'static Regex {
        return &POST_REPLY_QUOTE_REGEX;
    }

    fn post_parser(&self) -> &'static Box<dyn PostParser + Sync> {
        return &CHAN4_POST_PARSER;
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
                "https://a.4cdn.org/{}/thread/{}.json",
                thread_descriptor.board_code(),
                thread_descriptor.thread_no
            );

            return Some(endpoint);
        }

        let endpoint = format!(
            "https://a.4cdn.org/{}/thread/{}-tail.json",
            thread_descriptor.board_code(),
            thread_descriptor.thread_no
        );

        return Some(endpoint);
    }

    fn supports_partial_load_head_request(&self) -> bool {
        return true;
    }

}

#[test]
fn test_url_conversion() {
    let chan4 = Chan4 { };

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

#[test]
fn test_post_quote_regex() {
    let test_string = "<a href=\"#p251260223\" class=\"quotelink\">&gt;&gt;251260223</a>";
    let captures = POST_REPLY_QUOTE_REGEX.captures(test_string).unwrap();
    assert_eq!(2, captures.len());
    assert_eq!("251260223", captures.get(1).unwrap().as_str());

    let test_string = "<a href=\"#p92933496\" class=\"quotelink\">&gt;&gt;92933496</a><br>\
    <a href=\"#p92933523\" class=\"quotelink\">&gt;&gt;92933523</a><br>\
    Will look into them, upon first look, it shouldn&#039;t be much work";
    let captures = POST_REPLY_QUOTE_REGEX.captures_iter(test_string).collect::<Vec<Captures>>();
    assert_eq!(2, captures.len());
    assert_eq!("92933496", captures.get(0).unwrap().get(1).unwrap().as_str());
    assert_eq!("92933523", captures.get(1).unwrap().get(1).unwrap().as_str());
}