use std::cmp::{PartialEq, PartialOrd};
use std::ops::{Deref, Index, IndexMut};

#[derive(Clone, Debug)]
pub enum Value {
    Float(f64),
    Int(i64),
    Str(String),
    KeyVal((String, Box<Value>)),
    Array(Vec<Value>),
    Null,
}

/// Reprsents a key in nested arrays.
/// For example, in the array `{"save", text="test"}`, the key is `"save"`.
pub struct Key<'a>(pub &'a str);

impl<'a> Deref for Key<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

const NULL: Value = Value::Null;

#[allow(dead_code)]
impl Value {
    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<String> {
        if let Value::Str(s) = self {
            Some(s.clone())
        } else {
            None
        }
    }

    /// Find a nested array by key (first value of nested array).
    /// If the key is not found, it returns a reference to `NULL`.
    ///
    /// # Example
    /// ```lua
    /// {
    ///    {"save", text="test"},
    /// }
    /// ```
    /// for above array, calling `find_array("save")` will return the entire array `{"save", text="test"}`.
    pub fn find_array(&self, key: &str) -> &Value {
        match self {
            Value::Array(arr) => {
                for item in arr {
                    if &item[0] == key {
                        return item;
                    }
                }
                &NULL
            }
            _ => &NULL,
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    pub fn is_kv(&self) -> bool {
        matches!(self, Value::KeyVal(_))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn kv_key(&self) -> Option<&str> {
        if let Value::KeyVal((k, _)) = self {
            Some(k)
        } else {
            None
        }
    }

    pub fn kv_keys<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        match self {
            Value::KeyVal((k, _)) => Box::new(std::iter::once(k.as_str())),
            Value::Array(arr) => Box::new(arr.iter().filter_map(|v| v.kv_key())),
            _ => Box::new(std::iter::empty()),
        }
    }

    pub fn members<'a>(&'a self) -> Iter<'a> {
        match self {
            Value::Array(arr) => Iter { iter: arr.iter() },
            _ => Iter::default(),
        }
    }

    pub fn members_mut<'a>(&'a mut self) -> IterMut<'a> {
        match self {
            Value::Array(arr) => IterMut {
                iter: arr.iter_mut(),
            },
            _ => IterMut::default(),
        }
    }

    pub fn last_member(&self) -> &Value {
        match self {
            Value::Array(arr) => arr.last().unwrap_or(&NULL),
            _ => &NULL,
        }
    }
}

impl Index<usize> for Value {
    type Output = Value;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            Value::Array(arr) => {
                if index < arr.len() {
                    &arr[index]
                } else {
                    &NULL
                }
            }
            _ => &NULL,
        }
    }
}

impl IndexMut<usize> for Value {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match self {
            Value::Array(arr) => {
                if index < arr.len() {
                    &mut arr[index]
                } else {
                    arr.push(NULL);
                    arr.last_mut().unwrap()
                }
            }
            _ => {
                *self = Value::Array(vec![NULL]);
                self.index_mut(0)
            }
        }
    }
}

impl<'a> Index<&'a str> for Value {
    type Output = Value;

    fn index(&self, key: &'a str) -> &Self::Output {
        match self {
            Value::KeyVal((k, v)) if k == key => v,
            Value::Array(arr) => {
                for item in arr.iter().rev() {
                    if let Value::KeyVal((k, v)) = item {
                        if k == key {
                            return v;
                        }
                    }
                }
                &NULL
            }
            _ => &NULL,
        }
    }
}

impl<'a> IndexMut<&'a str> for Value {
    fn index_mut(&mut self, index: &'a str) -> &mut Self::Output {
        match &self {
            Value::KeyVal((k, _)) => {
                if k == index {
                    if let Value::KeyVal((_, v)) = self {
                        v
                    } else {
                        unreachable!()
                    }
                } else {
                    *self = Value::KeyVal((index.to_string(), Box::new(NULL)));
                    if let Value::KeyVal((_, v)) = self {
                        v
                    } else {
                        unreachable!()
                    }
                }
            }
            Value::Array(arr) => {
                for (i, item) in arr.iter().enumerate().rev() {
                    if let Value::KeyVal((k, _)) = item {
                        if k == index {
                            if let Value::KeyVal((_, v)) = &mut self[i] {
                                return v;
                            } else {
                                unreachable!()
                            }
                        }
                    }
                }
                if let Value::Array(arr) = self {
                    arr.push(Value::KeyVal((index.to_string(), Box::new(NULL))));
                    if let Value::KeyVal((_, v)) = arr.last_mut().unwrap() {
                        v
                    } else {
                        unreachable!()
                    }
                } else {
                    unreachable!()
                }
            }
            _ => {
                *self = Value::Array(vec![Value::KeyVal((index.to_string(), Box::new(NULL)))]);
                self.index_mut(index)
            }
        }
    }
}

impl<'a> Index<&'a String> for Value {
    type Output = Value;

    #[inline(always)]
    fn index(&self, key: &'a String) -> &Self::Output {
        self.index(key.as_str())
    }
}

impl<'a> IndexMut<&'a String> for Value {
    #[inline(always)]
    fn index_mut(&mut self, index: &'a String) -> &mut Self::Output {
        self.index_mut(index.as_str())
    }
}

impl Index<String> for Value {
    type Output = Value;

    #[inline(always)]
    fn index(&self, key: String) -> &Self::Output {
        self.index(key.as_str())
    }
}

impl IndexMut<String> for Value {
    #[inline(always)]
    fn index_mut(&mut self, index: String) -> &mut Self::Output {
        self.index_mut(index.as_str())
    }
}

impl<'a, 'b> Index<&'b Key<'a>> for Value {
    type Output = Value;

    #[inline(always)]
    fn index(&self, key: &'b Key<'a>) -> &Self::Output {
        self.find_array(&key.0)
    }
}

impl<'a> Index<Key<'a>> for Value {
    type Output = Value;

    #[inline(always)]
    fn index(&self, key: Key<'a>) -> &Self::Output {
        self.find_array(&key.0)
    }
}

impl PartialEq<str> for Value {
    fn eq(&self, other: &str) -> bool {
        match self {
            Value::Str(s) => s == other,
            _ => false,
        }
    }
}

impl PartialEq<String> for Value {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<i64> for Value {
    fn eq(&self, other: &i64) -> bool {
        match self {
            Value::Int(i) => i == other,
            _ => false,
        }
    }
}

impl PartialEq<f64> for Value {
    fn eq(&self, other: &f64) -> bool {
        match self {
            Value::Float(f) => f == other,
            _ => false,
        }
    }
}

impl PartialOrd<i64> for Value {
    fn partial_cmp(&self, other: &i64) -> Option<std::cmp::Ordering> {
        match self {
            Value::Int(i) => i.partial_cmp(other),
            _ => None,
        }
    }
}

impl PartialOrd<f64> for Value {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        match self {
            Value::Float(f) => f.partial_cmp(other),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Iter<'a> {
    iter: std::slice::Iter<'a, Value>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

#[derive(Default)]
pub struct IterMut<'a> {
    iter: std::slice::IterMut<'a, Value>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut Value;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a> ExactSizeIterator for IterMut<'a> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> DoubleEndedIterator for IterMut<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

#[derive(Clone, Debug)]
pub struct AstFile {
    pub astver: f64,
    pub astname: Option<String>,
    pub ast: Value,
}
