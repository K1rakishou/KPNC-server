use std::borrow::Cow;
use crate::model::repository::account_repository::{FirebaseToken, AccountId};

pub trait FormatToken {
    fn format_token(&self) -> Cow<str>;
}

impl FormatToken for &str {
    fn format_token(&self) -> Cow<str> {
        let chars: Vec<char> = self.chars().collect();
        let string_length = chars.len();

        if string_length < 6 {
            return Cow::Borrowed(self);
        }

        let mut part_length = (string_length / 100) * 10;
        if part_length > 8 {
            part_length = 8;
        }

        let start = &chars[0..part_length];
        let end = &chars[(string_length - 1) - part_length..(string_length - 1)];

        let formatted_token = format!("{}...{}", String::from_iter(start), String::from_iter(end));
        return Cow::Owned(formatted_token);
    }
}

impl FormatToken for String {
    fn format_token(&self) -> Cow<str> {
        let chars: Vec<char> = self.chars().collect();
        let string_length = chars.len();

        if string_length < 6 {
            return Cow::Borrowed(self);
        }

        let mut part_length = (string_length / 100) * 10;
        if part_length > 8 {
            part_length = 8;
        }

        let start = &chars[0..part_length];
        let end = &chars[(string_length - 1) - part_length..(string_length - 1)];

        let formatted_token = format!("{}...{}", String::from_iter(start), String::from_iter(end));
        return Cow::Owned(formatted_token);
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