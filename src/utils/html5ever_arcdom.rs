use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::io;
use std::mem;
use std::sync::{Arc, Mutex, Weak};

use crossbeam::atomic::AtomicCell;
use tendril::StrTendril;

use markup5ever::Attribute;
use markup5ever::ExpandedName;
use markup5ever::QualName;
use markup5ever::interface::tree_builder;
use markup5ever::interface::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use markup5ever::serialize::TraversalScope;
use markup5ever::serialize::TraversalScope::{ChildrenOnly, IncludeNode};
use markup5ever::serialize::{Serialize, Serializer};
use xml5ever::interface::ElemName;
use xml5ever::local_name;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct AtomicAttribute {
    pub name: QualName,
    pub value: String,
}

impl From<Attribute> for AtomicAttribute {
    fn from(attr: Attribute) -> Self {
        AtomicAttribute {
            name: attr.name,
            value: attr.value.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeData {
    /// The `Document` itself - the root node of a HTML document.
    Document,

    /// A `DOCTYPE` with name, public id, and system id. See
    /// [document type declaration on wikipedia][dtd wiki].
    ///
    /// [dtd wiki]: https://en.wikipedia.org/wiki/Document_type_declaration
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },

    /// A text node.
    Text { contents: Arc<Mutex<String>> },

    /// A comment.
    Comment { contents: String },

    /// An element with attributes.
    Element {
        name: QualName,
        attrs: Arc<Mutex<Vec<AtomicAttribute>>>,

        /// For HTML \<template\> elements, the [template contents].
        ///
        /// [template contents]: https://html.spec.whatwg.org/multipage/#template-contents
        template_contents: Arc<Mutex<Option<Handle>>>,

        /// Whether the node is a [HTML integration point].
        ///
        /// [HTML integration point]: https://html.spec.whatwg.org/multipage/#html-integration-point
        mathml_annotation_xml_integration_point: bool,
    },

    /// A Processing instruction.
    ProcessingInstruction { target: String, contents: String },
}

/// A DOM node.
pub struct Node {
    /// Parent node.
    pub parent: AtomicCell<Option<WeakHandle>>,
    /// Child nodes of this node.
    pub children: Mutex<Vec<Handle>>,
    /// Represents this node's data.
    pub data: NodeData,
}

impl Node {
    /// Create a new node from its contents
    pub fn new(data: NodeData) -> Arc<Self> {
        Arc::new(Node {
            data,
            parent: AtomicCell::new(None),
            children: Mutex::new(Vec::new()),
        })
    }

    /// <https://html.spec.whatwg.org/#option-element-nearest-ancestor-select>
    fn get_option_element_nearest_ancestor_select(&self) -> Option<Arc<Self>> {
        // Step 1. Let ancestorOptgroup be null.
        // NOTE: The algorithm doesn't actually need the value, so a boolean is enough.
        let mut did_see_ancestor_optgroup = false;

        // Step 2. For each ancestor of option's ancestors, in reverse tree order:
        let mut current = self.parent().and_then(|parent| parent.upgrade())?;
        loop {
            if let NodeData::Element { name, .. } = &current.data {
                // Step 2.1 If ancestor is a datalist, hr, or option element, then return null.
                if matches!(
                    name.local_name(),
                    &local_name!("datalist") | &local_name!("hr") | &local_name!("option")
                ) {
                    return None;
                }

                // Step 2.2 If ancestor is an optgroup element:
                if name.local_name() == &local_name!("optgroup") {
                    // Step 2.2.1 If ancestorOptgroup is not null, then return null.
                    if did_see_ancestor_optgroup {
                        return None;
                    }

                    // Step 2.2.2 Set ancestorOptgroup to ancestor.
                    did_see_ancestor_optgroup = true;
                }

                // Step 2.3 If ancestor is a select, then return ancestor.
                if name.local_name() == &local_name!("select") {
                    return Some(current);
                }
            };

            // Move on to the next ancestor
            let Some(next_ancestor) = current.parent().and_then(|parent| parent.upgrade()) else {
                break;
            };
            current = next_ancestor;
        }

        // Step 3. Return null.
        None
    }

    fn parent(&self) -> Option<Weak<Self>> {
        let parent = self.parent.take();
        self.parent.store(parent.clone());
        parent
    }

    /// <https://html.spec.whatwg.org/#select-enabled-selectedcontent>
    fn get_a_selects_enabled_selectedcontent(&self) -> Option<Arc<Self>> {
        // Step 1. If select has the multiple attribute, then return null.
        let NodeData::Element { name, attrs, .. } = &self.data else {
            panic!("Trying to get selectedcontent of non-element");
        };
        debug_assert_eq!(name.local_name(), &local_name!("select"));
        if attrs
            .lock()
            .unwrap()
            .iter()
            .any(|attribute| attribute.name.local == local_name!("multiple"))
        {
            return None;
        }

        // Step 2. Let selectedcontent be the first selectedcontent element descendant of select in tree order
        // if any such element exists; otherwise return null.
        // FIXME: This does not visit the nodes in tree order
        let mut remaining = VecDeque::default();
        remaining.extend(self.children.lock().unwrap().iter().cloned());
        let mut selectedcontent = None;
        while let Some(node) = remaining.pop_front() {
            remaining.extend(node.children.lock().unwrap().iter().cloned());

            let NodeData::Element { name, .. } = &self.data else {
                continue;
            };
            if name.local_name() == &local_name!("selectedcontent") {
                selectedcontent = Some(node);
                break;
            }
        }
        let selectedcontent = selectedcontent?;

        // Step 3. If selectedcontent's disabled is true, then return null.
        // FIXME: This step is unimplemented for now to reduce complexity.

        // Step 4. Return selectedcontent.
        Some(selectedcontent)
    }

    /// <https://html.spec.whatwg.org/#clone-an-option-into-a-selectedcontent>
    fn clone_an_option_into_selectedcontent(&self, selectedcontent: Arc<Self>) {
        // Step 1. Let documentFragment be a new DocumentFragment whose node document is option's node document.
        // NOTE: We just remember the children of said fragment, thats good enough.
        let mut document_fragment = Vec::new();

        // Step 2. For each child of option's children:
        for child in self.children.lock().unwrap().iter() {
            // Step 2.1 Let childClone be the result of running clone given child with subtree set to true.
            let child_clone = child.clone_with_subtree();

            // Step 2.2 Append childClone to documentFragment.
            document_fragment.push(child_clone);
        }

        // Step 3. Replace all with documentFragment within selectedcontent.
        *selectedcontent.children.lock().unwrap() = document_fragment;
    }

    /// Clones the node and all of its descendants, returning a handle to the new subtree.
    ///
    /// This function will run into infinite recursion when the DOM tree contains cycles and it makes
    /// no attempts to guard against that.
    fn clone_with_subtree(&self) -> Arc<Self> {
        let children = self
            .children
            .lock()
            .unwrap()
            .iter()
            .map(|child| child.clone_with_subtree())
            .collect();
        Arc::new(Self {
            parent: AtomicCell::new(self.parent()),
            data: self.data.clone(),
            children: Mutex::new(children),
        })
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Node")
            .field("data", &self.data)
            .field("children", &self.children)
            .finish()
    }
}

/// Reference to a DOM node.
pub type Handle = Arc<Node>;

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle = Weak<Node>;

/// Append a parentless node to another nodes' children
fn append(new_parent: &Handle, child: Handle) {
    let previous_parent = child.parent.swap(Some(Arc::downgrade(new_parent)));
    // Invariant: child cannot have existing parent
    assert!(previous_parent.is_none());
    new_parent.children.lock().unwrap().push(child);
}

/// If the node has a parent, get it and this node's position in its children
fn get_parent_and_index(target: &Handle) -> Option<(Handle, usize)> {
    if let Some(weak) = target.parent.take() {
        let parent = weak.upgrade().expect("dangling weak pointer");
        target.parent.store(Some(weak));
        let i = match parent
            .children
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .find(|&(_, child)| Arc::ptr_eq(child, target))
        {
            Some((i, _)) => i,
            None => panic!("have parent but couldn't find in parent's children!"),
        };
        Some((parent, i))
    } else {
        None
    }
}

fn append_to_existing_text(prev: &Handle, text: &str) -> bool {
    match prev.data {
        NodeData::Text { ref contents } => {
            contents.lock().unwrap().push_str(text);
            true
        }
        _ => false,
    }
}

fn remove_from_parent(target: &Handle) {
    if let Some((parent, i)) = get_parent_and_index(target) {
        parent.children.lock().unwrap().remove(i);
        target.parent.store(None);
    }
}

/// The DOM itself; the result of parsing.
pub struct ArcDom {
    /// The `Document` itself.
    pub document: Handle,

    /// Errors that occurred during parsing.
    pub errors: Mutex<Vec<Cow<'static, str>>>,

    /// The document's quirks mode.
    pub quirks_mode: AtomicCell<QuirksMode>,
}

impl TreeSink for ArcDom {
    type Output = Self;
    fn finish(self) -> Self {
        self
    }

    type Handle = Handle;

    type ElemName<'a>
        = ExpandedName<'a>
    where
        Self: 'a;

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.errors.lock().unwrap().push(msg);
    }

    fn get_document(&self) -> Handle {
        self.document.clone()
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        if let NodeData::Element {
            ref template_contents,
            ..
        } = target.data
        {
            template_contents
                .lock()
                .unwrap()
                .as_ref()
                .expect("not a template element!")
                .clone()
        } else {
            panic!("not a template element!")
        }
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.store(mode);
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Arc::ptr_eq(x, y)
    }

    fn elem_name<'a>(&self, target: &'a Handle) -> ExpandedName<'a> {
        match target.data {
            NodeData::Element { ref name, .. } => name.expanded(),
            _ => panic!("not an element!"),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        Node::new(NodeData::Element {
            name,
            attrs: Arc::new(Mutex::new(
                attrs.into_iter().map(AtomicAttribute::from).collect(),
            )),
            template_contents: Arc::new(Mutex::new(if flags.template {
                Some(Node::new(NodeData::Document))
            } else {
                None
            })),
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        })
    }

    fn create_comment(&self, text: StrTendril) -> Handle {
        Node::new(NodeData::Comment {
            contents: text.to_string(),
        })
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        Node::new(NodeData::ProcessingInstruction {
            target: target.to_string(),
            contents: data.to_string(),
        })
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        // Append to an existing Text node if we have one.
        if let NodeOrText::AppendText(text) = &child {
            if let Some(h) = parent.children.lock().unwrap().last() {
                if append_to_existing_text(h, text) {
                    return;
                }
            }
        }

        append(
            parent,
            match child {
                NodeOrText::AppendText(text) => Node::new(NodeData::Text {
                    contents: Arc::new(Mutex::new(text.to_string())),
                }),
                NodeOrText::AppendNode(node) => node,
            },
        );
    }

    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        let (parent, i) = get_parent_and_index(sibling)
            .expect("append_before_sibling called on node without parent");

        let child = match (child, i) {
            // No previous node.
            (NodeOrText::AppendText(text), 0) => Node::new(NodeData::Text {
                contents: Arc::new(Mutex::new(text.to_string())),
            }),

            // Look for a text node before the insertion point.
            (NodeOrText::AppendText(text), i) => {
                let children = parent.children.lock().unwrap();
                let prev = &children[i - 1];
                if append_to_existing_text(prev, &text) {
                    return;
                }
                Node::new(NodeData::Text {
                    contents: Arc::new(Mutex::new(text.to_string())),
                })
            }

            // The tree builder promises we won't have a text node after
            // the insertion point.

            // Any other kind of node.
            (NodeOrText::AppendNode(node), _) => node,
        };

        remove_from_parent(&child);

        child.parent.store(Some(Arc::downgrade(&parent)));
        parent.children.lock().unwrap().insert(i, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let parent = element.parent.take();
        let has_parent = parent.is_some();
        element.parent.store(parent);

        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        append(
            &self.document,
            Node::new(NodeData::Doctype {
                name: name.to_string(),
                public_id: public_id.to_string(),
                system_id: system_id.to_string(),
            }),
        );
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        let mut existing = if let NodeData::Element { ref attrs, .. } = target.data {
            attrs.lock().unwrap()
        } else {
            panic!("not an element")
        };

        let existing_names = existing
            .iter()
            .map(|e| e.name.clone())
            .collect::<HashSet<_>>();
        existing.extend(
            attrs
                .into_iter()
                .filter(|attr| !existing_names.contains(&attr.name))
                .map(AtomicAttribute::from),
        );
    }

    fn remove_from_parent(&self, target: &Handle) {
        remove_from_parent(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let mut children = node.children.lock().unwrap();
        let mut new_children = new_parent.children.lock().unwrap();
        for child in children.iter() {
            let previous_parent = child.parent.swap(Some(Arc::downgrade(new_parent)));
            assert!(Arc::ptr_eq(
                node,
                &previous_parent.unwrap().upgrade().expect("dangling weak")
            ))
        }
        new_children.extend(mem::take(&mut *children));
    }

    fn is_mathml_annotation_xml_integration_point(&self, target: &Handle) -> bool {
        if let NodeData::Element {
            mathml_annotation_xml_integration_point,
            ..
        } = target.data
        {
            mathml_annotation_xml_integration_point
        } else {
            panic!("not an element!")
        }
    }

    fn maybe_clone_an_option_into_selectedcontent(&self, option: &Self::Handle) {
        let NodeData::Element { name, attrs, .. } = &option.data else {
            panic!("\"maybe clone an option into selectedcontent\" called with non-element node");
        };
        debug_assert_eq!(name.local_name(), &local_name!("option"));

        // Step 1. Let select be option's option element nearest ancestor select.
        let select = option.get_option_element_nearest_ancestor_select();

        // Step 2. If all of the following conditions are true:
        // * select is not null;
        // * option's selectedness is true; and
        // * select's enabled selectedcontent is not null,
        // then run clone an option into a selectedcontent given option and select's enabled selectedcontent.
        if let Some(selectedcontent) =
            select.and_then(|select| select.get_a_selects_enabled_selectedcontent())
        {
            if attrs
                .lock()
                .unwrap()
                .iter()
                .any(|attribute| attribute.name.local == local_name!("selected"))
            {
                option.clone_an_option_into_selectedcontent(selectedcontent);
            }
        }
    }
}

impl Default for ArcDom {
    fn default() -> ArcDom {
        ArcDom {
            document: Node::new(NodeData::Document),
            errors: Default::default(),
            quirks_mode: AtomicCell::new(tree_builder::NoQuirks),
        }
    }
}

enum SerializeOp {
    Open(Handle),
    Close(QualName),
}

pub struct SerializableHandle(Handle);

impl From<Handle> for SerializableHandle {
    fn from(h: Handle) -> SerializableHandle {
        SerializableHandle(h)
    }
}

impl Serialize for SerializableHandle {
    fn serialize<S>(&self, serializer: &mut S, traversal_scope: TraversalScope) -> io::Result<()>
    where
        S: Serializer,
    {
        let mut ops = VecDeque::new();
        match traversal_scope {
            IncludeNode => ops.push_back(SerializeOp::Open(self.0.clone())),
            ChildrenOnly(_) => ops.extend(
                self.0
                    .children
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|h| SerializeOp::Open(h.clone())),
            ),
        }

        while let Some(op) = ops.pop_front() {
            match op {
                SerializeOp::Open(handle) => match handle.data {
                    NodeData::Element {
                        ref name,
                        ref attrs,
                        ..
                    } => {
                        serializer.start_elem(
                            name.clone(),
                            attrs
                                .lock()
                                .unwrap()
                                .iter()
                                .map(|at| (&at.name, &at.value[..])),
                        )?;

                        ops.reserve(1 + handle.children.lock().unwrap().len());
                        ops.push_front(SerializeOp::Close(name.clone()));

                        for child in handle.children.lock().unwrap().iter().rev() {
                            ops.push_front(SerializeOp::Open(child.clone()));
                        }
                    }

                    NodeData::Doctype { ref name, .. } => serializer.write_doctype(name)?,

                    NodeData::Text { ref contents } => {
                        serializer.write_text(&contents.lock().unwrap())?
                    }

                    NodeData::Comment { ref contents } => serializer.write_comment(contents)?,

                    NodeData::ProcessingInstruction {
                        ref target,
                        ref contents,
                    } => serializer.write_processing_instruction(target, contents)?,

                    NodeData::Document => panic!("Can't serialize Document node itself"),
                },

                SerializeOp::Close(name) => {
                    serializer.end_elem(name)?;
                }
            }
        }

        Ok(())
    }
}
