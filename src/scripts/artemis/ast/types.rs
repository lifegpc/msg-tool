use std::cmp::{PartialEq, PartialOrd};
use std::convert::From;
use std::ops::{Deref, Index, IndexMut};

#[derive(Clone, Debug, PartialEq)]
/// Represents a value in LUA table
pub enum Value {
    /// Float number
    Float(f64),
    /// Integer number
    Int(i64),
    /// String value
    Str(String),
    /// Key value pair
    KeyVal((Box<Value>, Box<Value>)),
    /// Array of values
    Array(Vec<Value>),
    /// Null(nli) value
    Null,
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl<'a> From<&'a str> for Value {
    fn from(s: &'a str) -> Self {
        Value::Str(s.to_string())
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

/// Reprsents a key in nested arrays.
/// For example, in the array `{"save", text="test"}`, the key is `"save"`.
pub struct Key<'a>(pub &'a str);

/// Represents a key in key value pairs.
/// For example, in the key value pair `[1] = "test"`, the key is `1`.
#[derive(Clone, Copy)]
pub struct NumKey<T: Clone + Copy>(pub T);

impl<'a> Deref for Key<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

const NULL: Value = Value::Null;

impl Value {
    /// Returns a reference to the string if the value is a string, otherwise returns None.
    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// Returns a string if the value is a string, otherwise returns None.
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

    /// Find a nested array by key (first value of nested array).
    /// If the key is not found, it creates a new array with the key and returns a mutable reference to it.
    ///
    /// # Example
    /// ```lua
    /// {
    ///    {"save", text="test"},
    /// }
    /// ```
    /// for above array, calling `find_array_mut("save")` will return a mutable reference to the array `{"save", text="test"}`.
    pub fn find_array_mut(&mut self, key: &str) -> &mut Value {
        match &self {
            Value::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    if &item[0] == key {
                        return &mut self[i];
                    }
                }
                self.push_member(Value::Array(vec![Value::Str(key.to_string())]));
                self.last_member_mut()
            }
            _ => {
                *self = Value::Array(vec![Value::Str(key.to_string())]);
                self.last_member_mut()
            }
        }
    }

    /// Returns true if the value is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    /// Returns true if the value is a string.
    pub fn is_str(&self) -> bool {
        matches!(self, Value::Str(_))
    }

    /// Returns true if the value is a key-value pair.
    pub fn is_kv(&self) -> bool {
        matches!(self, Value::KeyVal(_))
    }

    /// Returns true if the value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Returns the key of a key-value pair if it exists, otherwise returns None.
    pub fn kv_key(&self) -> Option<&Value> {
        if let Value::KeyVal((k, _)) = self {
            Some(&k)
        } else {
            None
        }
    }

    /// Returns the keys in a lua table.
    pub fn kv_keys<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Value> + 'a> {
        match self {
            Value::KeyVal((k, _)) => Box::new(std::iter::once(&**k)),
            Value::Array(arr) => Box::new(arr.iter().filter_map(|v| v.kv_key())),
            _ => Box::new(std::iter::empty()),
        }
    }

    /// Returns the last member of the array if it exists, otherwise returns a reference to `NULL`.
    pub fn last_member(&self) -> &Value {
        match self {
            Value::Array(arr) => arr.last().unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    /// Returns a mutable reference to the last member of the array.
    ///
    /// If the array is empty, it creates a new member with `NULL` and returns it.
    /// If the value is not an array, it converts it to an array with a single `NULL` member.
    pub fn last_member_mut(&mut self) -> &mut Value {
        match self {
            Value::Array(arr) => {
                if arr.is_empty() {
                    arr.push(NULL);
                }
                arr.last_mut().unwrap()
            }
            _ => {
                *self = Value::Array(vec![NULL]);
                self.last_member_mut()
            }
        }
    }

    /// Returns the length of the array.
    pub fn len(&self) -> usize {
        match self {
            Value::Array(arr) => arr.len(),
            _ => 0,
        }
    }

    /// Inserts a member at the specified index in the array.
    ///
    /// If the index is out of bounds, it appends the value to the end of the array.
    /// If the value is not an array, it converts it to an array with a single member.
    pub fn insert_member(&mut self, index: usize, value: Value) {
        match self {
            Value::Array(arr) => {
                if index < arr.len() {
                    arr.insert(index, value);
                } else {
                    arr.push(value);
                }
            }
            _ => {
                *self = Value::Array(vec![value]);
            }
        }
    }

    /// Returns an iterator over the members of the array.
    pub fn members<'a>(&'a self) -> Iter<'a> {
        match self {
            Value::Array(arr) => Iter { iter: arr.iter() },
            _ => Iter::default(),
        }
    }

    /// Returns a mutable iterator over the members of the array.
    pub fn members_mut<'a>(&'a mut self) -> IterMut<'a> {
        match self {
            Value::Array(arr) => IterMut {
                iter: arr.iter_mut(),
            },
            _ => IterMut::default(),
        }
    }

    /// Creates a new empty array.
    pub fn new_array() -> Self {
        Value::Array(Vec::new())
    }

    /// Creates a new key-value pair.
    pub fn new_kv<K: Into<Value>, V: Into<Value>>(key: K, value: V) -> Self {
        Value::KeyVal((Box::new(key.into()), Box::new(value.into())))
    }

    /// Pushes a member to the end of the array.
    pub fn push_member(&mut self, value: Value) {
        match self {
            Value::Array(arr) => arr.push(value),
            _ => {
                *self = Value::Array(vec![value]);
            }
        }
    }

    /// Sets the value to a string.
    pub fn set_str<S: AsRef<str> + ?Sized>(&mut self, value: &S) {
        *self = Value::Str(value.as_ref().to_string());
    }

    /// Sets the value to a string.
    pub fn set_string<S: Into<String>>(&mut self, value: S) {
        *self = Value::Str(value.into());
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
                    *self = Value::KeyVal((Box::new(index.to_string().into()), Box::new(NULL)));
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
                    arr.push(Value::KeyVal((
                        Box::new(index.to_string().into()),
                        Box::new(NULL),
                    )));
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
                *self = Value::Array(vec![Value::KeyVal((
                    Box::new(index.to_string().into()),
                    Box::new(NULL),
                ))]);
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

impl<'a> Index<&'a Value> for Value {
    type Output = Value;

    fn index(&self, key: &'a Value) -> &Self::Output {
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

impl<'a> IndexMut<&'a Value> for Value {
    fn index_mut(&mut self, index: &'a Value) -> &mut Self::Output {
        match &self {
            Value::KeyVal((k, _)) => {
                if k == index {
                    if let Value::KeyVal((_, v)) = self {
                        v
                    } else {
                        unreachable!()
                    }
                } else {
                    *self = Value::KeyVal((Box::new(index.clone()), Box::new(NULL)));
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
                    arr.push(Value::KeyVal((Box::new(index.clone()), Box::new(NULL))));
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
                *self = Value::Array(vec![Value::KeyVal((
                    Box::new(index.clone()),
                    Box::new(NULL),
                ))]);
                self.index_mut(index)
            }
        }
    }
}

impl<'a> Index<&'a Box<Value>> for Value {
    type Output = Value;

    #[inline(always)]
    fn index(&self, key: &'a Box<Value>) -> &Self::Output {
        self.index(&**key)
    }
}

impl Index<NumKey<i64>> for Value {
    type Output = Value;

    fn index(&self, key: NumKey<i64>) -> &Self::Output {
        match self {
            Value::KeyVal((k, v)) if k == key.0 => v,
            Value::Array(arr) => {
                for item in arr.iter().rev() {
                    if let Value::KeyVal((k, v)) = item {
                        if k == key.0 {
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

impl IndexMut<NumKey<i64>> for Value {
    fn index_mut(&mut self, key: NumKey<i64>) -> &mut Self::Output {
        match &self {
            Value::KeyVal((k, _)) => {
                if k == key.0 {
                    if let Value::KeyVal((_, v)) = self {
                        v
                    } else {
                        unreachable!()
                    }
                } else {
                    *self = Value::KeyVal((Box::new(key.0.into()), Box::new(NULL)));
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
                        if k == key.0 {
                            if let Value::KeyVal((_, v)) = &mut self[i] {
                                return v;
                            } else {
                                unreachable!()
                            }
                        }
                    }
                }
                if let Value::Array(arr) = self {
                    arr.push(Value::KeyVal((Box::new(key.0.into()), Box::new(NULL))));
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
                *self = Value::Array(vec![Value::KeyVal((
                    Box::new(key.0.into()),
                    Box::new(NULL),
                ))]);
                self.index_mut(key)
            }
        }
    }
}

impl Index<NumKey<f64>> for Value {
    type Output = Value;

    fn index(&self, key: NumKey<f64>) -> &Self::Output {
        match self {
            Value::KeyVal((k, v)) if k == key.0 => v,
            Value::Array(arr) => {
                for item in arr.iter().rev() {
                    if let Value::KeyVal((k, v)) = item {
                        if k == key.0 {
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

impl IndexMut<NumKey<f64>> for Value {
    fn index_mut(&mut self, key: NumKey<f64>) -> &mut Self::Output {
        match &self {
            Value::KeyVal((k, _)) => {
                if k == key.0 {
                    if let Value::KeyVal((_, v)) = self {
                        v
                    } else {
                        unreachable!()
                    }
                } else {
                    *self = Value::KeyVal((Box::new(key.0.into()), Box::new(NULL)));
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
                        if k == key.0 {
                            if let Value::KeyVal((_, v)) = &mut self[i] {
                                return v;
                            } else {
                                unreachable!()
                            }
                        }
                    }
                }
                if let Value::Array(arr) = self {
                    arr.push(Value::KeyVal((Box::new(key.0.into()), Box::new(NULL))));
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
                *self = Value::Array(vec![Value::KeyVal((
                    Box::new(key.0.into()),
                    Box::new(NULL),
                ))]);
                self.index_mut(key)
            }
        }
    }
}

impl<'a, 'b> Index<&'b Key<'a>> for Value {
    type Output = Value;

    #[inline(always)]
    fn index(&self, key: &'b Key<'a>) -> &Self::Output {
        self.find_array(&key.0)
    }
}

impl<'a, 'b> IndexMut<&'b Key<'a>> for Value {
    #[inline(always)]
    fn index_mut(&mut self, key: &'b Key<'a>) -> &mut Self::Output {
        self.find_array_mut(&key.0)
    }
}

impl<'a> Index<Key<'a>> for Value {
    type Output = Value;

    #[inline(always)]
    fn index(&self, key: Key<'a>) -> &Self::Output {
        self.find_array(&key.0)
    }
}

impl<'a> IndexMut<Key<'a>> for Value {
    #[inline(always)]
    fn index_mut(&mut self, key: Key<'a>) -> &mut Self::Output {
        self.find_array_mut(&key.0)
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

impl PartialEq<str> for Box<Value> {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        **self == *other
    }
}

impl PartialEq<String> for Box<Value> {
    #[inline(always)]
    fn eq(&self, other: &String) -> bool {
        **self == *other
    }
}

impl PartialEq<i64> for Box<Value> {
    #[inline(always)]
    fn eq(&self, other: &i64) -> bool {
        **self == *other
    }
}

impl PartialEq<f64> for Box<Value> {
    #[inline(always)]
    fn eq(&self, other: &f64) -> bool {
        **self == *other
    }
}

impl PartialEq<Value> for Box<Value> {
    #[inline(always)]
    fn eq(&self, other: &Value) -> bool {
        **self == *other
    }
}

impl<'a> PartialEq<i64> for &'a Box<Value> {
    #[inline(always)]
    fn eq(&self, other: &i64) -> bool {
        **self == *other
    }
}

impl<'a> PartialEq<f64> for &'a Box<Value> {
    #[inline(always)]
    fn eq(&self, other: &f64) -> bool {
        **self == *other
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

impl PartialOrd<i64> for Box<Value> {
    #[inline(always)]
    fn partial_cmp(&self, other: &i64) -> Option<std::cmp::Ordering> {
        (**self).partial_cmp(other)
    }
}

impl PartialOrd<f64> for Box<Value> {
    #[inline(always)]
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        (**self).partial_cmp(other)
    }
}

#[derive(Default)]
/// An iterator over the members of an array.
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
/// A mutable iterator over the members of an array.
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
/// Represents an AST file.
pub struct AstFile {
    /// The version of the AST file.
    pub astver: Option<f64>,
    /// The name of the AST file.
    pub astname: Option<String>,
    /// The data of the AST file.
    pub ast: Value,
}
