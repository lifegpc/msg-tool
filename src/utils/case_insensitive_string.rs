use serde::Deserialize;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Deserialize)]
#[serde(transparent)]
/// A case-insensitive string wrapper. Can be used as a key in BTreeMap with a case-insensitive ordering.
///
/// WARN: Borrowing a &str/&String from this struct will not be case-insensitive. So getting a value from a BTreeMap with a &str/&String key is not ignore case. It just use it's original case.
pub struct CaseInsensitiveString(String);

impl PartialEq for CaseInsensitiveString {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl PartialEq<String> for CaseInsensitiveString {
    fn eq(&self, other: &String) -> bool {
        self.0.eq_ignore_ascii_case(other)
    }
}

impl PartialEq<&str> for CaseInsensitiveString {
    fn eq(&self, other: &&str) -> bool {
        self.0.eq_ignore_ascii_case(other)
    }
}

impl Eq for CaseInsensitiveString {}

impl PartialOrd for CaseInsensitiveString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CaseInsensitiveString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .to_ascii_lowercase()
            .cmp(&other.0.to_ascii_lowercase())
    }
}

impl Deref for CaseInsensitiveString {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CaseInsensitiveString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Borrow<str> for CaseInsensitiveString {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Borrow<String> for CaseInsensitiveString {
    fn borrow(&self) -> &String {
        &self.0
    }
}

impl std::fmt::Display for CaseInsensitiveString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
