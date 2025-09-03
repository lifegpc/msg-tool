//!Extensions for emote_psb crate.
use emote_psb::VirtualPsb;
use emote_psb::header::PsbHeader;
use emote_psb::types::collection::*;
use emote_psb::types::number::*;
use emote_psb::types::reference::*;
use emote_psb::types::string::*;
use emote_psb::types::*;
#[cfg(feature = "json")]
use json::JsonValue;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::ops::{Index, IndexMut};

const NONE: PsbValueFixed = PsbValueFixed::None;

#[derive(Debug, Serialize, Deserialize)]
/// Represents of a PSB value.
pub enum PsbValueFixed {
    /// No value.
    None,
    /// Represents a null value.
    Null,
    /// Represents a boolean value.
    Bool(bool),
    /// Represents a number value.
    Number(PsbNumber),
    /// Represents an array of integers.
    IntArray(PsbUintArray),
    /// Represents a string value.
    String(PsbString),
    /// Represents a list of PSB values.
    List(PsbListFixed),
    /// Represents an object with key-value pairs.
    Object(PsbObjectFixed),
    /// Represents a resource reference.
    Resource(PsbResourceRef),
    /// Represents an extra resource reference.
    ExtraResource(PsbExtraRef),
    /// Represents a compiler number.
    CompilerNumber,
    /// Represents a compiler string.
    CompilerString,
    /// Represents a compiler resource.
    CompilerResource,
    /// Represents a compiler decimal.
    CompilerDecimal,
    /// Represents a compiler array.
    CompilerArray,
    /// Represents a compiler boolean.
    CompilerBool,
    /// Represents a compiler binary tree.
    CompilerBinaryTree,
}

impl PsbValueFixed {
    /// Converts this value to original PSB value type.
    pub fn to_psb(self, warn_on_none: bool) -> PsbValue {
        match self {
            PsbValueFixed::None => {
                if warn_on_none {
                    eprintln!("Warning: PSB value is None, output script may broken.");
                    crate::COUNTER.inc_warning();
                }
                PsbValue::None
            }
            PsbValueFixed::Null => PsbValue::Null,
            PsbValueFixed::Bool(b) => PsbValue::Bool(b),
            PsbValueFixed::Number(n) => PsbValue::Number(n),
            PsbValueFixed::IntArray(arr) => PsbValue::IntArray(arr),
            PsbValueFixed::String(s) => PsbValue::String(s),
            PsbValueFixed::List(l) => PsbValue::List(l.to_psb(warn_on_none)),
            PsbValueFixed::Object(o) => PsbValue::Object(o.to_psb(warn_on_none)),
            PsbValueFixed::Resource(r) => PsbValue::Resource(r),
            PsbValueFixed::ExtraResource(er) => PsbValue::ExtraResource(er),
            PsbValueFixed::CompilerNumber => PsbValue::CompilerNumber,
            PsbValueFixed::CompilerString => PsbValue::CompilerString,
            PsbValueFixed::CompilerResource => PsbValue::CompilerResource,
            PsbValueFixed::CompilerDecimal => PsbValue::CompilerDecimal,
            PsbValueFixed::CompilerArray => PsbValue::CompilerArray,
            PsbValueFixed::CompilerBool => PsbValue::CompilerBool,
            PsbValueFixed::CompilerBinaryTree => PsbValue::CompilerBinaryTree,
        }
    }

    /// Returns true if this value is a list.
    pub fn is_list(&self) -> bool {
        matches!(self, PsbValueFixed::List(_))
    }

    /// Returns true if this value is an object.
    pub fn is_object(&self) -> bool {
        matches!(self, PsbValueFixed::Object(_))
    }

    /// Returns true if this value is a string or null.
    pub fn is_string_or_null(&self) -> bool {
        self.is_string() || self.is_null()
    }

    /// Returns true if this value is a string.
    pub fn is_string(&self) -> bool {
        matches!(self, PsbValueFixed::String(_))
    }

    /// Returns true if this value is none.
    pub fn is_none(&self) -> bool {
        matches!(self, PsbValueFixed::None)
    }

    /// Returns true if this value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, PsbValueFixed::Null)
    }

    /// Sets the value of this PSB value to a new string.
    pub fn set_str(&mut self, value: &str) {
        match self {
            PsbValueFixed::String(s) => {
                let s = s.string_mut();
                s.clear();
                s.push_str(value);
            }
            _ => {
                *self = PsbValueFixed::String(PsbString::from(value.to_owned()));
            }
        }
    }

    /// Sets the value of this PSB value to a new string.
    pub fn set_string(&mut self, value: String) {
        self.set_str(&value);
    }

    /// Returns the value as a boolean, if it is a boolean.
    pub fn as_u8(&self) -> Option<u8> {
        self.as_i64().map(|n| n.try_into().ok()).flatten()
    }

    /// Returns the value as a [u32], if it is a number.
    pub fn as_u32(&self) -> Option<u32> {
        self.as_i64().map(|n| n as u32)
    }

    /// Returns the value as a [i64], if it is a number.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            PsbValueFixed::Number(n) => match n {
                PsbNumber::Integer(n) => Some(*n),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns the value as a string, if it is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PsbValueFixed::String(s) => Some(s.string()),
            _ => None,
        }
    }

    /// Returns the lengtho of a list or object.
    pub fn len(&self) -> usize {
        match self {
            PsbValueFixed::List(l) => l.len(),
            PsbValueFixed::Object(o) => o.values.len(),
            _ => 0,
        }
    }

    /// Returns a iterator over the entries of an object.
    pub fn entries(&self) -> ObjectIter<'_> {
        match self {
            PsbValueFixed::Object(o) => o.iter(),
            _ => ObjectIter::empty(),
        }
    }

    /// Returns a mutable iterator over the entries of an object.
    pub fn entries_mut(&mut self) -> ObjectIterMut<'_> {
        match self {
            PsbValueFixed::Object(o) => o.iter_mut(),
            _ => ObjectIterMut::empty(),
        }
    }

    /// Returns a iterator over the members of a list.
    pub fn members(&self) -> ListIter<'_> {
        match self {
            PsbValueFixed::List(l) => l.iter(),
            _ => ListIter::empty(),
        }
    }

    /// Returns a mutable iterator over the members of a list.
    pub fn members_mut(&mut self) -> ListIterMut<'_> {
        match self {
            PsbValueFixed::List(l) => l.iter_mut(),
            _ => ListIterMut::empty(),
        }
    }

    /// Returns the resource ID if this value is a resource reference.
    pub fn resource_id(&self) -> Option<u64> {
        match self {
            PsbValueFixed::Resource(r) => Some(r.resource_ref),
            _ => None,
        }
    }

    /// Converts this value to a JSON value, if possible.
    #[cfg(feature = "json")]
    pub fn to_json(&self) -> Option<JsonValue> {
        match self {
            PsbValueFixed::Null => Some(JsonValue::Null),
            PsbValueFixed::Bool(b) => Some(JsonValue::Boolean(*b)),
            PsbValueFixed::Number(n) => match n {
                PsbNumber::Integer(i) => Some(JsonValue::Number((*i).into())),
                PsbNumber::Float(f) => Some(JsonValue::Number((*f).into())),
                PsbNumber::Double(d) => Some(JsonValue::Number((*d).into())),
            },
            PsbValueFixed::String(s) => Some(JsonValue::String(s.string().to_owned())),
            PsbValueFixed::Resource(s) => {
                Some(JsonValue::String(format!("resource#{}", s.resource_ref)))
            }
            PsbValueFixed::ExtraResource(s) => Some(JsonValue::String(format!(
                "extra_resource#{}",
                s.extra_resource_ref
            ))),
            PsbValueFixed::IntArray(arr) => Some(JsonValue::Array(
                arr.iter().map(|n| JsonValue::Number((*n).into())).collect(),
            )),
            PsbValueFixed::List(l) => Some(l.to_json()),
            PsbValueFixed::Object(o) => Some(o.to_json()),
            _ => None,
        }
    }

    /// Converts a JSON value to a PSB value.
    #[cfg(feature = "json")]
    pub fn from_json(obj: &JsonValue) -> Self {
        match obj {
            JsonValue::Null => PsbValueFixed::Null,
            JsonValue::Boolean(b) => PsbValueFixed::Bool(*b),
            JsonValue::Number(n) => {
                let data: f64 = (*n).into();
                if data.fract() == 0.0 {
                    PsbValueFixed::Number(PsbNumber::Integer(data as i64))
                } else {
                    PsbValueFixed::Number(PsbNumber::Float(data as f32))
                }
            }
            JsonValue::String(s) => {
                if s.starts_with("resource#") {
                    if let Ok(id) = s[9..].parse::<u64>() {
                        return PsbValueFixed::Resource(PsbResourceRef { resource_ref: id });
                    }
                } else if s.starts_with("extra_resource#") {
                    if let Ok(id) = s[16..].parse::<u64>() {
                        return PsbValueFixed::ExtraResource(PsbExtraRef {
                            extra_resource_ref: id,
                        });
                    }
                }
                PsbValueFixed::String(PsbString::from(s.clone()))
            }
            JsonValue::Array(arr) => {
                let values: Vec<PsbValueFixed> = arr.iter().map(PsbValueFixed::from_json).collect();
                PsbValueFixed::List(PsbListFixed { values })
            }
            JsonValue::Object(obj) => {
                let mut values = HashMap::new();
                for (key, value) in obj.iter() {
                    values.insert(key.to_owned(), PsbValueFixed::from_json(value));
                }
                PsbValueFixed::Object(PsbObjectFixed { values })
            }
            JsonValue::Short(n) => {
                let s = n.as_str();
                if s.starts_with("resource#") {
                    if let Ok(id) = s[9..].parse::<u64>() {
                        return PsbValueFixed::Resource(PsbResourceRef { resource_ref: id });
                    }
                } else if s.starts_with("extra_resource#") {
                    if let Ok(id) = s[16..].parse::<u64>() {
                        return PsbValueFixed::ExtraResource(PsbExtraRef {
                            extra_resource_ref: id,
                        });
                    }
                }
                PsbValueFixed::String(PsbString::from(s.to_owned()))
            }
        }
    }
}

impl Index<usize> for PsbValueFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            PsbValueFixed::List(l) => &l[index],
            _ => &NONE,
        }
    }
}

impl IndexMut<usize> for PsbValueFixed {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match self {
            PsbValueFixed::List(l) => {
                if index < l.values.len() {
                    &mut l.values[index]
                } else {
                    l.values.push(NONE);
                    l.values.last_mut().unwrap()
                }
            }
            _ => {
                *self = PsbValueFixed::List(PsbListFixed { values: vec![NONE] });
                self.index_mut(0)
            }
        }
    }
}

impl<'a> Index<&'a str> for PsbValueFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: &'a str) -> &Self::Output {
        match self {
            PsbValueFixed::Object(o) => &o[index],
            _ => &NONE,
        }
    }
}

impl<'a> Index<&'a String> for PsbValueFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: &'a String) -> &Self::Output {
        self.index(index.as_str())
    }
}

impl Index<String> for PsbValueFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: String) -> &Self::Output {
        self.index(index.as_str())
    }
}

impl IndexMut<&str> for PsbValueFixed {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        match self {
            PsbValueFixed::Object(o) => o.index_mut(index),
            _ => {
                *self = PsbValueFixed::Object(PsbObjectFixed {
                    values: HashMap::new(),
                });
                self.index_mut(index)
            }
        }
    }
}

impl IndexMut<&String> for PsbValueFixed {
    fn index_mut(&mut self, index: &String) -> &mut Self::Output {
        self.index_mut(index.as_str())
    }
}

impl IndexMut<String> for PsbValueFixed {
    fn index_mut(&mut self, index: String) -> &mut Self::Output {
        self.index_mut(index.as_str())
    }
}

impl Clone for PsbValueFixed {
    fn clone(&self) -> Self {
        match self {
            PsbValueFixed::None => PsbValueFixed::None,
            PsbValueFixed::Null => PsbValueFixed::Null,
            PsbValueFixed::Bool(b) => PsbValueFixed::Bool(*b),
            PsbValueFixed::Number(n) => PsbValueFixed::Number(n.clone()),
            PsbValueFixed::IntArray(arr) => PsbValueFixed::IntArray(arr.clone()),
            PsbValueFixed::String(s) => PsbValueFixed::String(PsbString::from(s.string().clone())),
            PsbValueFixed::List(l) => PsbValueFixed::List(l.clone()),
            PsbValueFixed::Object(o) => PsbValueFixed::Object(o.clone()),
            PsbValueFixed::Resource(r) => PsbValueFixed::Resource(r.clone()),
            PsbValueFixed::ExtraResource(er) => PsbValueFixed::ExtraResource(er.clone()),
            PsbValueFixed::CompilerNumber => PsbValueFixed::CompilerNumber,
            PsbValueFixed::CompilerString => PsbValueFixed::CompilerString,
            PsbValueFixed::CompilerResource => PsbValueFixed::CompilerResource,
            PsbValueFixed::CompilerDecimal => PsbValueFixed::CompilerDecimal,
            PsbValueFixed::CompilerArray => PsbValueFixed::CompilerArray,
            PsbValueFixed::CompilerBool => PsbValueFixed::CompilerBool,
            PsbValueFixed::CompilerBinaryTree => PsbValueFixed::CompilerBinaryTree,
        }
    }
}

impl PartialEq<String> for PsbValueFixed {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<str> for PsbValueFixed {
    fn eq(&self, other: &str) -> bool {
        match self {
            PsbValueFixed::String(s) => s.string() == other,
            _ => false,
        }
    }
}

impl<'a> PartialEq<&'a str> for PsbValueFixed {
    fn eq(&self, other: &&'a str) -> bool {
        self == *other
    }
}

/// Trait to convert a PSB value to a fixed PSB value.
pub trait PsbValueExt {
    /// Converts this PSB value to a fixed PSB value.
    fn to_psb_fixed(self) -> PsbValueFixed;
}

impl PsbValueExt for PsbValue {
    fn to_psb_fixed(self) -> PsbValueFixed {
        match self {
            PsbValue::None => PsbValueFixed::None,
            PsbValue::Null => PsbValueFixed::Null,
            PsbValue::Bool(b) => PsbValueFixed::Bool(b),
            PsbValue::Number(n) => PsbValueFixed::Number(n),
            PsbValue::IntArray(arr) => PsbValueFixed::IntArray(arr),
            PsbValue::String(s) => PsbValueFixed::String(s),
            PsbValue::List(l) => PsbValueFixed::List(PsbList::to_psb_fixed(l)),
            PsbValue::Object(o) => PsbValueFixed::Object(PsbObject::to_psb_fixed(o)),
            PsbValue::Resource(r) => PsbValueFixed::Resource(r),
            PsbValue::ExtraResource(er) => PsbValueFixed::ExtraResource(er),
            PsbValue::CompilerNumber => PsbValueFixed::CompilerNumber,
            PsbValue::CompilerString => PsbValueFixed::CompilerString,
            PsbValue::CompilerResource => PsbValueFixed::CompilerResource,
            PsbValue::CompilerDecimal => PsbValueFixed::CompilerDecimal,
            PsbValue::CompilerArray => PsbValueFixed::CompilerArray,
            PsbValue::CompilerBool => PsbValueFixed::CompilerBool,
            PsbValue::CompilerBinaryTree => PsbValueFixed::CompilerBinaryTree,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
/// Represents a PSB list of PSB values.
pub struct PsbListFixed {
    /// The values in the list.
    pub values: Vec<PsbValueFixed>,
}

impl PsbListFixed {
    /// Converts this PSB list to a original PSB list.
    pub fn to_psb(self, warn_on_none: bool) -> PsbList {
        let v: Vec<_> = self
            .values
            .into_iter()
            .map(|v| v.to_psb(warn_on_none))
            .collect();
        PsbList::from(v)
    }

    /// Returns a iterator over the values in the list.
    pub fn iter(&self) -> ListIter<'_> {
        ListIter {
            inner: self.values.iter(),
        }
    }

    /// Returns a mutable iterator over the values in the list.
    pub fn iter_mut(&mut self) -> ListIterMut<'_> {
        ListIterMut {
            inner: self.values.iter_mut(),
        }
    }

    /// Returns a reference to the values in the list.
    pub fn values(&self) -> &Vec<PsbValueFixed> {
        &self.values
    }

    /// Returns the length of the list.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Converts this PSB list to a JSON value.
    #[cfg(feature = "json")]
    pub fn to_json(&self) -> JsonValue {
        let data: Vec<_> = self.values.iter().filter_map(|v| v.to_json()).collect();
        JsonValue::Array(data)
    }
}

impl Index<usize> for PsbListFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: usize) -> &Self::Output {
        self.values.get(index).unwrap_or(&NONE)
    }
}

impl IndexMut<usize> for PsbListFixed {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index < self.values.len() {
            &mut self.values[index]
        } else {
            self.values.push(NONE);
            self.values.last_mut().unwrap()
        }
    }
}

/// Iterator for a slice of PSB values in a list.
pub struct ListIter<'a> {
    inner: std::slice::Iter<'a, PsbValueFixed>,
}

impl<'a> ListIter<'a> {
    /// Creates an empty iterator.
    pub fn empty() -> Self {
        ListIter {
            inner: Default::default(),
        }
    }
}

impl<'a> Iterator for ListIter<'a> {
    type Item = &'a PsbValueFixed;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a> ExactSizeIterator for ListIter<'a> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> DoubleEndedIterator for ListIter<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

/// Mutable iterator for a slice of PSB values in a list.
pub struct ListIterMut<'a> {
    inner: std::slice::IterMut<'a, PsbValueFixed>,
}

impl<'a> ListIterMut<'a> {
    /// Creates an empty mutable iterator.
    pub fn empty() -> Self {
        ListIterMut {
            inner: Default::default(),
        }
    }
}

impl<'a> Iterator for ListIterMut<'a> {
    type Item = &'a mut PsbValueFixed;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a> ExactSizeIterator for ListIterMut<'a> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> DoubleEndedIterator for ListIterMut<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

/// Trait to convert a PSB list to a fixed PSB list.
pub trait PsbListExt {
    /// Converts this PSB list to a fixed PSB list.
    fn to_psb_fixed(self) -> PsbListFixed;
}

impl PsbListExt for PsbList {
    fn to_psb_fixed(self) -> PsbListFixed {
        let values: Vec<_> = self
            .unwrap()
            .into_iter()
            .map(PsbValue::to_psb_fixed)
            .collect();
        PsbListFixed { values }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
/// Represents a PSB object with key-value pairs.
pub struct PsbObjectFixed {
    /// The key-value pairs in the object.
    pub values: HashMap<String, PsbValueFixed>,
}

impl PsbObjectFixed {
    /// Creates a new empty PSB object.
    pub fn to_psb(self, warn_on_none: bool) -> PsbObject {
        let mut hash_map = HashMap::new();
        for (key, value) in self.values {
            hash_map.insert(key, value.to_psb(warn_on_none));
        }
        PsbObject::from(hash_map)
    }

    /// Gets a reference of value in the object by key.
    pub fn get_value(&self, key: &str) -> Option<&PsbValueFixed> {
        self.values.get(key)
    }

    /// Returns a iterator over the entries of the object.
    pub fn iter(&self) -> ObjectIter<'_> {
        ObjectIter {
            inner: self.values.iter(),
        }
    }

    /// Returns a mutable iterator over the entries of the object.
    pub fn iter_mut(&mut self) -> ObjectIterMut<'_> {
        ObjectIterMut {
            inner: self.values.iter_mut(),
        }
    }

    /// Converts this PSB object to a JSON value.
    #[cfg(feature = "json")]
    pub fn to_json(&self) -> JsonValue {
        let mut obj = json::object::Object::new();
        for (key, value) in &self.values {
            if let Some(json_value) = value.to_json() {
                obj.insert(key, json_value);
            }
        }
        JsonValue::Object(obj)
    }

    /// Converts a JSON object to a PSB object.
    #[cfg(feature = "json")]
    pub fn from_json(obj: &JsonValue) -> Self {
        let mut values = HashMap::new();
        for (key, value) in obj.entries() {
            values.insert(key.to_owned(), PsbValueFixed::from_json(value));
        }
        PsbObjectFixed { values }
    }
}

impl<'a> Index<&'a str> for PsbObjectFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: &'a str) -> &Self::Output {
        self.values.get(index).unwrap_or(&NONE)
    }
}

impl<'a> Index<&'a String> for PsbObjectFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: &'a String) -> &Self::Output {
        self.index(index.as_str())
    }
}

impl Index<String> for PsbObjectFixed {
    type Output = PsbValueFixed;

    fn index(&self, index: String) -> &Self::Output {
        self.index(index.as_str())
    }
}

impl<'a> IndexMut<&'a str> for PsbObjectFixed {
    fn index_mut(&mut self, index: &'a str) -> &mut Self::Output {
        self.values.entry(index.to_string()).or_insert(NONE)
    }
}

impl<'a> IndexMut<&'a String> for PsbObjectFixed {
    fn index_mut(&mut self, index: &'a String) -> &mut Self::Output {
        self.index_mut(index.as_str())
    }
}

impl IndexMut<String> for PsbObjectFixed {
    fn index_mut(&mut self, index: String) -> &mut Self::Output {
        self.values.entry(index).or_insert(NONE)
    }
}

/// Trait to convert a PSB object to a fixed PSB object.
pub trait PsbObjectExt {
    /// Converts this PSB object to a fixed PSB object.
    fn to_psb_fixed(self) -> PsbObjectFixed;
}

impl PsbObjectExt for PsbObject {
    fn to_psb_fixed(self) -> PsbObjectFixed {
        let mut hash_map = HashMap::new();
        for (key, value) in self.unwrap() {
            hash_map.insert(key, PsbValue::to_psb_fixed(value));
        }
        PsbObjectFixed { values: hash_map }
    }
}

/// Iterator for a slice of PSB values in an object.
pub struct ObjectIter<'a> {
    inner: std::collections::hash_map::Iter<'a, String, PsbValueFixed>,
}

impl<'a> ObjectIter<'a> {
    /// Creates an empty iterator.
    pub fn empty() -> Self {
        ObjectIter {
            inner: Default::default(),
        }
    }
}

impl<'a> Iterator for ObjectIter<'a> {
    type Item = (&'a String, &'a PsbValueFixed);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
impl<'a> ExactSizeIterator for ObjectIter<'a> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// Mutable iterator for a slice of PSB values in an object.
pub struct ObjectIterMut<'a> {
    inner: std::collections::hash_map::IterMut<'a, String, PsbValueFixed>,
}

impl<'a> ObjectIterMut<'a> {
    /// Creates an empty mutable iterator.
    pub fn empty() -> Self {
        ObjectIterMut {
            inner: Default::default(),
        }
    }
}

impl<'a> Iterator for ObjectIterMut<'a> {
    type Item = (&'a String, &'a mut PsbValueFixed);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a> ExactSizeIterator for ObjectIterMut<'a> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// Represents a fixed version of a virtual PSB.
#[derive(Clone, Debug)]
pub struct VirtualPsbFixed {
    header: PsbHeader,
    resources: Vec<Vec<u8>>,
    extra: Vec<Vec<u8>>,
    root: PsbObjectFixed,
}

impl Serialize for VirtualPsbFixed {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("VirtualPsbFixed", 3)?;
        state.serialize_field("version", &self.header.version)?;
        state.serialize_field("encryption", &self.header.encryption)?;
        state.serialize_field("data", &self.root)?;
        state.end()
    }
}

#[derive(Deserialize)]
pub struct VirtualPsbFixedData {
    version: u16,
    encryption: u16,
    data: PsbObjectFixed,
}

impl VirtualPsbFixed {
    /// Creates a new fixed virtual PSB.
    pub fn new(
        header: PsbHeader,
        resources: Vec<Vec<u8>>,
        extra: Vec<Vec<u8>>,
        root: PsbObjectFixed,
    ) -> Self {
        Self {
            header,
            resources,
            extra,
            root,
        }
    }

    /// Returns the header of the PSB.
    pub fn header(&self) -> PsbHeader {
        self.header
    }

    /// Returns a reference to the resources of the PSB.
    pub fn resources(&self) -> &Vec<Vec<u8>> {
        &self.resources
    }

    /// Returns a mutable reference to the resources of the PSB.
    pub fn resources_mut(&mut self) -> &mut Vec<Vec<u8>> {
        &mut self.resources
    }

    /// Returns a reference to the extra resources of the PSB.
    pub fn extra(&self) -> &Vec<Vec<u8>> {
        &self.extra
    }

    /// Returns a mutable reference to the extra resources of the PSB.
    pub fn extra_mut(&mut self) -> &mut Vec<Vec<u8>> {
        &mut self.extra
    }

    /// Returns a reference to the root object of the PSB.
    pub fn root(&self) -> &PsbObjectFixed {
        &self.root
    }

    /// Returns a mutable reference to the root object of the PSB.
    pub fn root_mut(&mut self) -> &mut PsbObjectFixed {
        &mut self.root
    }

    /// Sets the root of the PSB.
    pub fn set_root(&mut self, root: PsbObjectFixed) {
        self.root = root;
    }

    /// Unwraps the PSB into its components.
    pub fn unwrap(self) -> (PsbHeader, Vec<Vec<u8>>, Vec<Vec<u8>>, PsbObjectFixed) {
        (self.header, self.resources, self.extra, self.root)
    }

    /// Converts this fixed PSB to a virtual PSB.
    pub fn to_psb(self, warn_on_none: bool) -> VirtualPsb {
        let (header, resources, extra, root) = self.unwrap();
        VirtualPsb::new(header, resources, extra, root.to_psb(warn_on_none))
    }

    /// Converts json object to a fixed PSB.
    #[cfg(feature = "json")]
    pub fn from_json(&mut self, obj: &JsonValue) -> Result<(), anyhow::Error> {
        let version = obj["version"]
            .as_u16()
            .ok_or_else(|| anyhow::anyhow!("Invalid PSB version"))?;
        let encryption = obj["encryption"]
            .as_u16()
            .ok_or_else(|| anyhow::anyhow!("Invalid PSB encryption"))?;
        self.header.version = version;
        self.header.encryption = encryption;
        self.root = PsbObjectFixed::from_json(&obj["data"]);
        Ok(())
    }

    pub fn set_data(&mut self, data: VirtualPsbFixedData) {
        self.header.version = data.version;
        self.header.encryption = data.encryption;
        self.root = data.data;
    }

    /// Converts this fixed PSB to a JSON object.
    #[cfg(feature = "json")]
    pub fn to_json(&self) -> JsonValue {
        json::object! {
            "version": self.header.version,
            "encryption": self.header.encryption,
            "data": self.root.to_json(),
        }
    }
}

/// Trait to convert a virtual PSB to a fixed PSB.
pub trait VirtualPsbExt {
    /// Converts this virtual PSB to a fixed PSB.
    fn to_psb_fixed(self) -> VirtualPsbFixed;
}

impl VirtualPsbExt for VirtualPsb {
    fn to_psb_fixed(self) -> VirtualPsbFixed {
        let (header, resources, extra, root) = self.unwrap();
        VirtualPsbFixed::new(header, resources, extra, root.to_psb_fixed())
    }
}
