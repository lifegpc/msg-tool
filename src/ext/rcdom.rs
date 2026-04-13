//! Extensions for markup5ever_rcdom crate.
use crate::utils::html5ever_arcdom::{AtomicAttribute, Node, NodeData};
use anyhow::Result;
use std::sync::{Arc, Mutex, Weak};

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
    fn push_child(&self, child: Arc<Node>) -> Result<()>;
    /// Create a deep clone
    fn deep_clone(&self, parent: Option<Weak<Node>>) -> Result<Arc<Node>>;
    /// Create a deep clone with modification of data.
    fn deep_clone_with_modify<F: Fn(&mut NodeData) -> Result<()>>(
        &self,
        parent: Option<Weak<Node>>,
        modify: F,
    ) -> Result<Arc<Node>>;
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
    fn clone2(&self) -> Result<NodeData>;
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
            NodeData::ProcessingInstruction { target, .. } => target == name.as_ref(),
            _ => false,
        }
    }

    fn element_attr_keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = String> + 'a>> {
        match &self.data {
            NodeData::Element { attrs, .. } => {
                let attrs = attrs.lock().unwrap();
                Ok(Box::new(KeyIter { attrs, index: 0 }))
            }
            _ => Ok(Box::new(std::iter::empty())),
        }
    }

    fn get_attr_value<S: AsRef<str> + ?Sized>(&self, name: &S) -> Result<Option<String>> {
        match &self.data {
            NodeData::Element { attrs, .. } => {
                let attrs = attrs.lock().unwrap();
                Ok(attrs
                    .iter()
                    .find(|a| a.name.local.as_ref() == name.as_ref())
                    .map(|a| a.value.to_string()))
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
                let mut borrowed = attrs.lock().unwrap();
                if let Some(attr) = borrowed
                    .iter_mut()
                    .find(|a| a.name.local.as_ref() == name.as_ref())
                {
                    attr.value = value.as_ref().into();
                } else {
                    borrowed.push(AtomicAttribute {
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

impl RcNodeExt for Arc<Node> {
    fn push_child(&self, child: Arc<Node>) -> Result<()> {
        child.parent.store(Some(Arc::downgrade(self)));
        self.children.lock().unwrap().push(child);
        Ok(())
    }

    fn deep_clone(&self, parent: Option<Weak<Node>>) -> Result<Arc<Node>> {
        let data = self.data.clone2()?;
        let node = Node {
            data,
            children: Mutex::new(Vec::new()),
            parent: parent.into(),
        };
        let node = Arc::new(node);
        for child in self.children.lock().unwrap().iter() {
            let cloned_child = child.deep_clone(Some(Arc::downgrade(&node)))?;
            node.push_child(cloned_child)?;
        }
        Ok(node)
    }

    fn deep_clone_with_modify<F: Fn(&mut NodeData) -> Result<()>>(
        &self,
        parent: Option<Weak<Node>>,
        modify: F,
    ) -> Result<Arc<Node>> {
        let mut data = self.data.clone2()?;
        modify(&mut data)?;
        let node = Node {
            data,
            children: Mutex::new(Vec::new()),
            parent: parent.into(),
        };
        let node = Arc::new(node);
        for child in self.children.lock().unwrap().iter() {
            let cloned_child = child.deep_clone(Some(Arc::downgrade(&node)))?;
            node.push_child(cloned_child)?;
        }
        Ok(node)
    }

    fn change_child<F: Fn(&mut NodeData) -> Result<()>>(
        &self,
        index: usize,
        modify: F,
    ) -> Result<()> {
        let mut children = self.children.lock().unwrap();
        if index >= children.len() {
            return Err(anyhow::anyhow!("Index out of bounds"));
        }
        let child = children.remove(index);
        child.parent.take();
        let nchild = child.deep_clone_with_modify(Some(Arc::downgrade(self)), modify)?;
        children.insert(index, nchild);
        Ok(())
    }
}

impl NodeDataExt for NodeData {
    fn clone2(&self) -> Result<Self> {
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
                contents: Arc::new(Mutex::new(contents.lock().unwrap().clone())),
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
                for attr in attrs.lock().unwrap().iter() {
                    nattrs.push(AtomicAttribute {
                        name: attr.name.clone(),
                        value: attr.value.clone(),
                    });
                }
                let attrs = Mutex::new(nattrs);
                let template = match template_contents.lock().unwrap().as_ref() {
                    Some(tc) => Some(tc.deep_clone(None)?),
                    None => None,
                };
                let template_contents = Mutex::new(template);
                NodeData::Element {
                    name,
                    attrs: Arc::new(attrs),
                    template_contents: Arc::new(template_contents),
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

struct KeyIter<'a> {
    attrs: std::sync::MutexGuard<'a, Vec<AtomicAttribute>>,
    index: usize,
}

impl<'a> Iterator for KeyIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.attrs.len() {
            None
        } else {
            let key = self.attrs[self.index].name.local.as_ref();
            self.index += 1;
            Some(key.to_string())
        }
    }
}
