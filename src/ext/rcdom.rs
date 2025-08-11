//! Extensions for markup5ever_rcdom crate.
use anyhow::Result;
use markup5ever::Attribute;
use markup5ever_rcdom::{Node, NodeData};
use std::cell::{Ref, RefCell};
use std::rc::{Rc, Weak};

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
    /// Sets the value of the attribute with the given name.
    ///
    /// This function creates the attribute if it does not exist.
    /// This function ignores namespaces and only checks the local name of the attribute.
    /// Returns `Ok(())` if the node is not an element.
    fn set_attr_value<S: AsRef<str> + ?Sized, V: AsRef<str> + ?Sized>(
        &self,
        name: &S,
        value: &V,
    ) -> Result<()>;
}

/// Extensions for [Rc<Node>]
pub trait RcNodeExt {
    /// Pushes a child node to the current node.
    fn push_child(&self, child: Rc<Node>) -> Result<()>;
    /// Create a deep clone
    fn deep_clone(&self, parent: Option<Weak<Node>>) -> Result<Rc<Node>>;
    /// Create a deep clone with modification of data.
    fn deep_clone_with_modify<F: Fn(&mut NodeData) -> Result<()>>(
        &self,
        parent: Option<Weak<Node>>,
        modify: F,
    ) -> Result<Rc<Node>>;
    /// Changes a child node at the given index by modifying its data.
    ///
    /// Deep clones are needed.
    fn change_child<F: Fn(&mut NodeData) -> Result<()>>(
        &self,
        index: usize,
        modify: F,
    ) -> Result<()>;
}

/// Extensions for [NodeData]
pub trait NodeDataExt {
    /// clones the node data.
    fn clone(&self) -> Result<NodeData>;
    /// Sets the content of a processing instruction.
    ///
    /// Returns `Ok(())` if the node is not a processing instruction.
    fn set_processing_instruction_content<S: AsRef<str> + ?Sized>(
        &mut self,
        content: &S,
    ) -> Result<()>;
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

    fn set_attr_value<S: AsRef<str> + ?Sized, V: AsRef<str> + ?Sized>(
        &self,
        name: &S,
        value: &V,
    ) -> Result<()> {
        match &self.data {
            NodeData::Element { attrs, .. } => {
                let mut borrowed = attrs.try_borrow_mut()?;
                if let Some(attr) = borrowed
                    .iter_mut()
                    .find(|a| a.name.local.as_ref() == name.as_ref())
                {
                    attr.value = value.as_ref().into();
                } else {
                    borrowed.push(Attribute {
                        name: markup5ever::QualName::new(
                            None,
                            markup5ever::Namespace::default(),
                            name.as_ref().into(),
                        ),
                        value: value.as_ref().into(),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl RcNodeExt for Rc<Node> {
    fn push_child(&self, child: Rc<Node>) -> Result<()> {
        child.parent.replace(Some(Rc::downgrade(self)));
        self.children.try_borrow_mut()?.push(child);
        Ok(())
    }

    fn deep_clone(&self, parent: Option<Weak<Node>>) -> Result<Rc<Node>> {
        let data = self.data.clone()?;
        let node = Node {
            data,
            children: RefCell::new(Vec::new()),
            parent: parent.into(),
        };
        let node = Rc::new(node);
        for child in self.children.try_borrow()?.iter() {
            let cloned_child = child.deep_clone(Some(Rc::downgrade(&node)))?;
            node.push_child(cloned_child)?;
        }
        Ok(node)
    }

    fn deep_clone_with_modify<F: Fn(&mut NodeData) -> Result<()>>(
        &self,
        parent: Option<Weak<Node>>,
        modify: F,
    ) -> Result<Rc<Node>> {
        let mut data = self.data.clone()?;
        modify(&mut data)?;
        let node = Node {
            data,
            children: RefCell::new(Vec::new()),
            parent: parent.into(),
        };
        let node = Rc::new(node);
        for child in self.children.try_borrow()?.iter() {
            let cloned_child = child.deep_clone(Some(Rc::downgrade(&node)))?;
            node.push_child(cloned_child)?;
        }
        Ok(node)
    }

    fn change_child<F: Fn(&mut NodeData) -> Result<()>>(
        &self,
        index: usize,
        modify: F,
    ) -> Result<()> {
        let mut children = self.children.try_borrow_mut()?;
        if index >= children.len() {
            return Err(anyhow::anyhow!("Index out of bounds"));
        }
        let child = children.remove(index);
        child.parent.take();
        let nchild = child.deep_clone_with_modify(Some(Rc::downgrade(self)), modify)?;
        children.insert(index, nchild);
        Ok(())
    }
}

impl NodeDataExt for NodeData {
    fn clone(&self) -> Result<Self> {
        Ok(match self {
            NodeData::Document => NodeData::Document,
            NodeData::Comment { contents } => NodeData::Comment {
                contents: contents.clone(),
            },
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => NodeData::Doctype {
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            },
            NodeData::Text { contents } => NodeData::Text {
                contents: contents.clone(),
            },
            NodeData::ProcessingInstruction { target, contents } => {
                NodeData::ProcessingInstruction {
                    target: target.clone(),
                    contents: contents.clone(),
                }
            }
            NodeData::Element {
                name,
                attrs,
                template_contents,
                mathml_annotation_xml_integration_point,
            } => {
                let name = name.clone();
                let mut nattrs = Vec::new();
                for attr in attrs.try_borrow()?.iter() {
                    nattrs.push(Attribute {
                        name: attr.name.clone(),
                        value: attr.value.clone(),
                    });
                }
                let attrs = RefCell::new(nattrs);
                let template = match template_contents.try_borrow()?.as_ref() {
                    Some(tc) => Some(tc.deep_clone(None)?),
                    None => None,
                };
                let template_contents = RefCell::new(template);
                NodeData::Element {
                    name,
                    attrs,
                    template_contents,
                    mathml_annotation_xml_integration_point:
                        mathml_annotation_xml_integration_point.clone(),
                }
            }
        })
    }

    fn set_processing_instruction_content<S: AsRef<str> + ?Sized>(
        &mut self,
        content: &S,
    ) -> Result<()> {
        match self {
            NodeData::ProcessingInstruction { contents, .. } => {
                *contents = content.as_ref().into();
                Ok(())
            }
            _ => Ok(()),
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
