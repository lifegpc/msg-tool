//! Extensions for markup5ever_rcdom crate.
use anyhow::Result;
use markup5ever::Attribute;
use markup5ever_rcdom::{Node, NodeData};
use std::cell::Ref;

/// Extensions for [Node]
pub trait NodeExt {
    /// Checks if the node is an element with the given name.
    ///
    /// This function ignore namespaces.
    fn is_element<S: AsRef<str> + ?Sized>(&self, name: &S) -> bool;
    /// Checks if the node is a processing instruction with the given name.
    fn is_processing_instruction<S: AsRef<str> + ?Sized>(&self, name: &S) -> bool;
    /// Returns an iterator over the attribute keys of the element node.
    ///
    /// This function returns an empty iterator if the node is not an element.
    /// Only the local names of the attributes are returned, ignoring namespaces.
    fn element_attr_keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = String> + 'a>>;
    /// Gets the value of the attribute with the given name.
    ///
    /// This function returns `Ok(None)` if the node is not an element or if the attribute does not exist.
    /// This function ignores namespaces and only checks the local name of the attribute.
    fn get_attr_value<S: AsRef<str> + ?Sized>(&self, name: &S) -> Result<Option<String>>;
}

impl NodeExt for Node {
    fn is_element<S: AsRef<str> + ?Sized>(&self, name: &S) -> bool {
        match &self.data {
            NodeData::Element { name: ename, .. } => ename.local.as_ref() == name.as_ref(),
            _ => false,
        }
    }

    fn is_processing_instruction<S: AsRef<str> + ?Sized>(&self, name: &S) -> bool {
        match &self.data {
            NodeData::ProcessingInstruction { target, .. } => target.as_ref() == name.as_ref(),
            _ => false,
        }
    }

    fn element_attr_keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = String> + 'a>> {
        match &self.data {
            NodeData::Element { attrs, .. } => {
                let borrowed = attrs.try_borrow()?;
                let iter = AttrKeyIter { borrowed, pos: 0 };
                Ok(Box::new(iter))
            }
            _ => Ok(Box::new(std::iter::empty())),
        }
    }

    fn get_attr_value<S: AsRef<str> + ?Sized>(&self, name: &S) -> Result<Option<String>> {
        match &self.data {
            NodeData::Element { attrs, .. } => {
                let borrowed = attrs.try_borrow()?;
                if let Some(attr) = borrowed
                    .iter()
                    .find(|a| a.name.local.as_ref() == name.as_ref())
                {
                    Ok(Some(attr.value.to_string()))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}

struct AttrKeyIter<'a> {
    borrowed: Ref<'a, Vec<Attribute>>,
    pos: usize,
}

impl<'a> Iterator for AttrKeyIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.borrowed.len() {
            let attr = &self.borrowed[self.pos];
            self.pos += 1;
            Some(attr.name.local.to_string())
        } else {
            None
        }
    }
}
