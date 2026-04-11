use serde::Deserialize;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
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

impl std::fmt::Display for CaseInsensitiveString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Hash for CaseInsensitiveString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_ascii_lowercase().hash(state);
    }
}

impl Borrow<CaseInsensitiveStr> for CaseInsensitiveString {
    fn borrow(&self) -> &CaseInsensitiveStr {
        CaseInsensitiveStr::from_str(&self.0)
    }
}

#[repr(transparent)]
pub struct CaseInsensitiveStr(str);

impl CaseInsensitiveStr {
    pub fn from_str(s: &str) -> &Self {
        // SAFETY: CaseInsensitiveStr has the same memory layout as str, so this transmute is safe.
        unsafe { &*(s as *const str as *const Self) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq for CaseInsensitiveStr {
    fn eq(&self, other: &Self) -> bool {
        self.eq_ignore_ascii_case(&other.0)
    }
}

impl Eq for CaseInsensitiveStr {}

impl PartialOrd for CaseInsensitiveStr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CaseInsensitiveStr {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .to_ascii_lowercase()
            .cmp(&other.0.to_ascii_lowercase())
    }
}

impl Deref for CaseInsensitiveStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for CaseInsensitiveStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Hash for CaseInsensitiveStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_ascii_lowercase().hash(state);
    }
}

#[test]
fn test_btree_map() {
    let mut map = std::collections::BTreeMap::new();
    map.insert(CaseInsensitiveString("hella".to_string()), 0);
    map.insert(CaseInsensitiveString("Hello".to_string()), 1);
    map.insert(CaseInsensitiveString("world".to_string()), 2);
    assert_eq!(map.get(CaseInsensitiveStr::from_str("hello")), Some(&1));
    assert_eq!(map.get(CaseInsensitiveStr::from_str("WORLD")), Some(&2));
    assert_eq!(map.get(CaseInsensitiveStr::from_str("hella")), Some(&0));
}

#[test]
fn test_hash_map() {
    let mut map = std::collections::HashMap::new();
    map.insert(CaseInsensitiveString("hells".to_string()), 0);
    map.insert(CaseInsensitiveString("Hello".to_string()), 1);
    map.insert(CaseInsensitiveString("world".to_string()), 2);
    assert_eq!(map.get(CaseInsensitiveStr::from_str("hello")), Some(&1));
    assert_eq!(map.get(CaseInsensitiveStr::from_str("WORLD")), Some(&2));
    assert_eq!(map.get(CaseInsensitiveStr::from_str("hells")), Some(&0));
}
