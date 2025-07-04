use emote_psb::VirtualPsb;
use emote_psb::header::PsbHeader;
use emote_psb::types::collection::*;
use emote_psb::types::number::*;
use emote_psb::types::reference::*;
use emote_psb::types::string::*;
use emote_psb::types::*;
#[cfg(feature = "json")]
use json::JsonValue;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::ops::{Index, IndexMut};

const NONE: PsbValueFixed = PsbValueFixed::None;

#[derive(Debug)]
pub enum PsbValueFixed {
    None,
    Null,
    Bool(bool),
    Number(PsbNumber),
    IntArray(PsbUintArray),
    String(PsbString),
    List(PsbListFixed),
    Object(PsbObjectFixed),
    Resource(PsbResourceRef),
    ExtraResource(PsbExtraRef),
    CompilerNumber,
    CompilerString,
    CompilerResource,
    CompilerDecimal,
    CompilerArray,
    CompilerBool,
    CompilerBinaryTree,
}

impl PsbValueFixed {
    pub fn to_psb(self) -> PsbValue {
        match self {
            PsbValueFixed::None => PsbValue::None,
            PsbValueFixed::Null => PsbValue::Null,
            PsbValueFixed::Bool(b) => PsbValue::Bool(b),
            PsbValueFixed::Number(n) => PsbValue::Number(n),
            PsbValueFixed::IntArray(arr) => PsbValue::IntArray(arr),
            PsbValueFixed::String(s) => PsbValue::String(s),
            PsbValueFixed::List(l) => PsbValue::List(l.to_psb()),
            PsbValueFixed::Object(o) => PsbValue::Object(o.to_psb()),
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

    pub fn is_list(&self) -> bool {
        matches!(self, PsbValueFixed::List(_))
    }

    pub fn is_object(&self) -> bool {
        matches!(self, PsbValueFixed::Object(_))
    }

    pub fn is_string_or_null(&self) -> bool {
        self.is_string() || self.is_null()
    }

    pub fn is_string(&self) -> bool {
        matches!(self, PsbValueFixed::String(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, PsbValueFixed::None)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, PsbValueFixed::Null)
    }

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

    pub fn set_string(&mut self, value: String) {
        self.set_str(&value);
    }

    pub fn as_u8(&self) -> Option<u8> {
        self.as_i64().map(|n| n.try_into().ok()).flatten()
    }

    pub fn as_u32(&self) -> Option<u32> {
        self.as_i64().map(|n| n as u32)
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            PsbValueFixed::Number(n) => match n {
                PsbNumber::Integer(n) => Some(*n),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PsbValueFixed::String(s) => Some(s.string()),
            _ => None,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            PsbValueFixed::List(l) => l.len(),
            PsbValueFixed::Object(o) => o.values.len(),
            _ => 0,
        }
    }

    pub fn entries(&self) -> ObjectIter<'_> {
        match self {
            PsbValueFixed::Object(o) => o.iter(),
            _ => ObjectIter::empty(),
        }
    }

    pub fn entries_mut(&mut self) -> ObjectIterMut<'_> {
        match self {
            PsbValueFixed::Object(o) => o.iter_mut(),
            _ => ObjectIterMut::empty(),
        }
    }

    pub fn members(&self) -> ListIter<'_> {
        match self {
            PsbValueFixed::List(l) => l.iter(),
            _ => ListIter::empty(),
        }
    }

    pub fn members_mut(&mut self) -> ListIterMut<'_> {
        match self {
            PsbValueFixed::List(l) => l.iter_mut(),
            _ => ListIterMut::empty(),
        }
    }

    pub fn resource_id(&self) -> Option<u64> {
        match self {
            PsbValueFixed::Resource(r) => Some(r.resource_ref),
            _ => None,
        }
    }

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
            PsbValueFixed::List(l) => &l.values[index],
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

pub trait PsbValueExt {
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

#[derive(Clone, Debug)]
pub struct PsbListFixed {
    pub values: Vec<PsbValueFixed>,
}

impl PsbListFixed {
    pub fn to_psb(self) -> PsbList {
        let v: Vec<_> = self.values.into_iter().map(|v| v.to_psb()).collect();
        PsbList::from(v)
    }

    pub fn iter(&self) -> ListIter<'_> {
        ListIter {
            inner: self.values.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> ListIterMut<'_> {
        ListIterMut {
            inner: self.values.iter_mut(),
        }
    }

    pub fn values(&self) -> &Vec<PsbValueFixed> {
        &self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

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

pub struct ListIter<'a> {
    inner: std::slice::Iter<'a, PsbValueFixed>,
}

impl<'a> ListIter<'a> {
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

pub struct ListIterMut<'a> {
    inner: std::slice::IterMut<'a, PsbValueFixed>,
}

impl<'a> ListIterMut<'a> {
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

pub trait PsbListExt {
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

#[derive(Clone, Debug)]
pub struct PsbObjectFixed {
    pub values: HashMap<String, PsbValueFixed>,
}

impl PsbObjectFixed {
    pub fn to_psb(self) -> PsbObject {
        let mut hash_map = HashMap::new();
        for (key, value) in self.values {
            hash_map.insert(key, value.to_psb());
        }
        PsbObject::from(hash_map)
    }

    pub fn get_value(&self, key: &str) -> Option<&PsbValueFixed> {
        self.values.get(key)
    }

    pub fn iter(&self) -> ObjectIter<'_> {
        ObjectIter {
            inner: self.values.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> ObjectIterMut<'_> {
        ObjectIterMut {
            inner: self.values.iter_mut(),
        }
    }

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

pub trait PsbObjectExt {
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

pub struct ObjectIter<'a> {
    inner: std::collections::hash_map::Iter<'a, String, PsbValueFixed>,
}

impl<'a> ObjectIter<'a> {
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

pub struct ObjectIterMut<'a> {
    inner: std::collections::hash_map::IterMut<'a, String, PsbValueFixed>,
}

impl<'a> ObjectIterMut<'a> {
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

#[derive(Clone, Debug)]
pub struct VirtualPsbFixed {
    header: PsbHeader,
    resources: Vec<Vec<u8>>,
    extra: Vec<Vec<u8>>,
    root: PsbObjectFixed,
}

impl VirtualPsbFixed {
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

    pub fn header(&self) -> PsbHeader {
        self.header
    }

    pub fn resources(&self) -> &Vec<Vec<u8>> {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut Vec<Vec<u8>> {
        &mut self.resources
    }

    pub fn extra(&self) -> &Vec<Vec<u8>> {
        &self.extra
    }

    pub fn extra_mut(&mut self) -> &mut Vec<Vec<u8>> {
        &mut self.extra
    }

    pub fn root(&self) -> &PsbObjectFixed {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut PsbObjectFixed {
        &mut self.root
    }

    pub fn set_root(&mut self, root: PsbObjectFixed) {
        self.root = root;
    }

    pub fn unwrap(self) -> (PsbHeader, Vec<Vec<u8>>, Vec<Vec<u8>>, PsbObjectFixed) {
        (self.header, self.resources, self.extra, self.root)
    }

    pub fn to_psb(self) -> VirtualPsb {
        let (header, resources, extra, root) = self.unwrap();
        VirtualPsb::new(header, resources, extra, root.to_psb())
    }

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

    #[cfg(feature = "json")]
    pub fn to_json(&self) -> JsonValue {
        json::object! {
            "version": self.header.version,
            "encryption": self.header.encryption,
            "data": self.root.to_json(),
        }
    }
}

pub trait VirtualPsbExt {
    fn to_psb_fixed(self) -> VirtualPsbFixed;
}

impl VirtualPsbExt for VirtualPsb {
    fn to_psb_fixed(self) -> VirtualPsbFixed {
        let (header, resources, extra, root) = self.unwrap();
        VirtualPsbFixed::new(header, resources, extra, root.to_psb_fixed())
    }
}
