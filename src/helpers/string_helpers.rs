use std::borrow::Cow;
use std::collections::HashMap;

use crate::model::repository::account_repository::{AccountId, FirebaseToken};

pub trait FormatToken {
    fn format_token(&self) -> Cow<str>;
}

impl FormatToken for &str {
    fn format_token(&self) -> Cow<str> {
        let chars: Vec<char> = self.chars().collect();
        return format_token_internal(self, &chars);
    }
}

impl FormatToken for String {
    fn format_token(&self) -> Cow<str> {
        let chars: Vec<char> = self.chars().collect();
        return format_token_internal(self, &chars);
    }
}

impl FormatToken for AccountId {
    fn format_token(&self) -> Cow<str> {
        return self.id.format_token();
    }
}

impl FormatToken for FirebaseToken {
    fn format_token(&self) -> Cow<str> {
        return self.token.format_token();
    }
}

fn format_token_internal<'a>(token: &'a str, chars: &Vec<char>) -> Cow<'a, str> {
    const THREEDOT_LENGTH: usize = 3;
    const PART_LENGTH: usize = 10;

    let string_length = chars.len();
    let mut current_part_length = PART_LENGTH as i32;

    loop {
        if current_part_length < 3 {
            return Cow::Borrowed(token);
        }

        let remaining_length = (string_length as i32) -
            (THREEDOT_LENGTH as i32) -
            ((current_part_length as i32) * 2);

        if remaining_length <= 0 {
            current_part_length -= 1;
            continue;
        }

        break;
    }

    let part_length = current_part_length as usize;

    let start = &chars[..part_length];
    let end = &chars[(string_length - part_length)..];

    let formatted_token = format!("{}...{}", String::from_iter(start), String::from_iter(end));
    return Cow::Owned(formatted_token);
}

pub fn extract_site_name_from_domain(domain: &str) -> &str {
    let last_index = domain.rfind('.');
    if last_index.is_none() {
        return domain;
    }
    let last_index = last_index.unwrap();

    let domain = &domain[0..last_index];

    let last_index = domain.rfind('.');
    if last_index.is_none() {
        return domain;
    }
    let last_index = last_index.unwrap();

    return &domain[last_index + 1..];
}

pub fn query_to_params(query: &str) -> HashMap<String, String> {
    let mut result_map = HashMap::<String, String>::new();

    query
        .split('&')
        .for_each(|kv| {
            let split = kv.split('=').collect::<Vec<&str>>();

            let key = split.get(0).unwrap_or(&"");
            if key.is_empty() {
                return;
            }

            let value = split.get(1).unwrap_or(&"");

            result_map.insert(key.to_string(), value.to_string());
        });

    return result_map;
}

pub fn insert_after_every_nth(input: &str, insert: &str, n: usize) -> String {
    return input
        .chars()
        .enumerate()
        .map(|(i, c)| {
            return if (i + 1) % n == 0 {
                format!("{}{}", c, insert)
            } else {
                c.to_string()
            };
        })
        .collect();
}

#[test]
fn test_format_token_internal() {
    let token = "";
    assert_eq!("", token.format_token());

    let token = "1";
    assert_eq!("1", token.format_token());

    let token = "123456";
    assert_eq!("123456", token.format_token());

    let token = "1234567";
    assert_eq!("1234567", token.format_token());

    let token = "12345678";
    assert_eq!("12345678", token.format_token());

    let token = "123456789";
    assert_eq!("123456789", token.format_token());

    let token = "1234567890";
    assert_eq!("123...890", token.format_token());

    let token = "1234567890ABCDEF";
    assert_eq!("123456...ABCDEF", token.format_token());

    let token = "61b976821ad4a7545054a2e45367e3af53522477d39b28fdca26b36fed95f8b1a2005e3188b682a74f9e772aa3cb7201fcb6d01ce6cb2cdf720690fd26d5bb1e";
    assert_eq!("61b976821a...fd26d5bb1e", token.format_token());
}

#[test]
fn test_extract_site_name_from_domain() {
    assert_eq!("2ch", extract_site_name_from_domain("2ch.hk"));
    assert_eq!("4chan", extract_site_name_from_domain("boards.4chan.org"));
}