//!Extensions for fancy_regex crate.
use anyhow::Result;
use fancy_regex::Regex;

/// Extension trait for [fancy_regex::Regex] to provide more convenient methods.
pub trait FancyRegexExt {
    /// Splits the input string by the regex pattern.
    /// Like python's `re.split()`, but returns an iterator.
    fn py_split<'a>(&'a self, input: &'a str) -> Result<PySplit<'a>>;
}

/// An iterator that splits a string by a regex pattern, similar to Python's `re.split()`.
pub struct PySplit<'a> {
    str: &'a str,
    pos: Vec<(usize, usize)>,
    start: usize,
}

impl<'a> Iterator for PySplit<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.str.len() {
            return None;
        }
        match self.pos.first().cloned() {
            Some((start, end)) => {
                if self.start < start {
                    let result = &self.str[self.start..start];
                    self.start = start;
                    if start == end {
                        self.pos.remove(0);
                    }
                    Some(result)
                } else if self.start < end {
                    let result = &self.str[self.start..end];
                    self.start = end;
                    self.pos.remove(0);
                    Some(result)
                } else {
                    unreachable!();
                }
            }
            None => {
                if self.start < self.str.len() {
                    let result = &self.str[self.start..];
                    self.start = self.str.len();
                    Some(result)
                } else {
                    None
                }
            }
        }
    }
}

impl FancyRegexExt for Regex {
    fn py_split<'a>(&'a self, input: &'a str) -> Result<PySplit<'a>> {
        let mut poss = Vec::new();
        for pos in self.find_iter(input) {
            let pos = pos?;
            poss.push((pos.start(), pos.end()));
        }
        Ok(PySplit {
            str: input,
            pos: poss,
            start: 0,
        })
    }
}
