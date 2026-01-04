//! Node types using the unified `core::PhaseExt` trait.
//!
//! This module provides `Element`, `Node`, `Text`, and `Document` types
//! that work with the Family system where all families (built-in and
//! user-defined) are treated equally.
//!
//! # Key Features
//!
//! - Uses `P::Ext` (single enum) for element extensions
//! - Works with macro-generated phases from `#[vdom::families]`
//! - GAT-based type-safe family data access via `ExtractFamily`

mod element;
mod text;
mod document;

pub use element::Element;
pub use text::{Text, TextKind};
pub use document::Document;

use smallvec::SmallVec;
use crate::core::PhaseExt;

/// Node in a VDOM tree - either Element or Text.
#[derive(Debug, Clone)]
pub enum Node<P: PhaseExt> {
    Element(Box<Element<P>>),
    Text(Text<P>),
}

impl<P: PhaseExt> Node<P> {
    /// Check if this is an element node.
    #[inline]
    pub fn is_element(&self) -> bool {
        matches!(self, Node::Element(_))
    }

    /// Check if this is a text node.
    #[inline]
    pub fn is_text(&self) -> bool {
        matches!(self, Node::Text(_))
    }

    /// Get as element reference.
    #[inline]
    pub fn as_element(&self) -> Option<&Element<P>> {
        match self {
            Node::Element(e) => Some(e),
            _ => None,
        }
    }

    /// Get as mutable element reference.
    #[inline]
    pub fn as_element_mut(&mut self) -> Option<&mut Element<P>> {
        match self {
            Node::Element(e) => Some(e),
            _ => None,
        }
    }

    /// Get as text reference.
    #[inline]
    pub fn as_text(&self) -> Option<&Text<P>> {
        match self {
            Node::Text(t) => Some(t),
            _ => None,
        }
    }

    /// Get as mutable text reference.
    #[inline]
    pub fn as_text_mut(&mut self) -> Option<&mut Text<P>> {
        match self {
            Node::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// Type alias for children collection.
pub type Children<P> = SmallVec<[Node<P>; 8]>;

#[cfg(all(test, feature = "macros"))]
mod tests {
    use super::*;
    use crate::core::ExtractFamily;
    use crate::families::{LinkFamily, HeadingFamily, SvgFamily, MediaFamily};
    use crate::families::link::LinkRaw;
    use crate::families::heading::HeadingRaw;
    use crate::vdom;

    // Define test site with Family system
    #[vdom::families]
    pub struct TestSite {
        link: LinkFamily,
        heading: HeadingFamily,
        svg: SvgFamily,
        media: MediaFamily,
    }

    #[test]
    fn test_element_new() {
        let elem: Element<TestSite::Raw> = Element::new("div");
        assert_eq!(&*elem.tag, "div");
        assert!(elem.is_empty());
        assert_eq!(elem.family_name(), "none"); // default
    }

    #[test]
    fn test_element_with_ext() {
        let raw_ext = TestSite::RawExt::Link(LinkRaw::new("https://example.com"));
        let elem: Element<TestSite::Raw> = Element::with_ext("a", raw_ext);

        assert_eq!(&*elem.tag, "a");
        assert_eq!(elem.family_name(), "link");

        // GAT-based access
        let link_data = ExtractFamily::<LinkFamily>::get(&elem.ext).unwrap();
        assert_eq!(link_data.href.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_element_attrs() {
        let mut elem: Element<TestSite::Raw> = Element::new("div");
        elem.set_attr("class", "container");
        elem.set_attr("id", "main");

        assert_eq!(elem.get_attr("class"), Some("container"));
        assert_eq!(elem.get_attr("id"), Some("main"));
        assert_eq!(elem.id(), Some("main"));
        assert_eq!(elem.class(), Some("container"));
        assert!(elem.has_attr("class"));
        assert!(!elem.has_attr("style"));
    }

    #[test]
    fn test_element_children() {
        let mut parent: Element<TestSite::Raw> = Element::new("div");
        parent.push_elem(Element::new("span"));
        parent.push_text("Hello");

        assert_eq!(parent.len(), 2);
        assert!(!parent.is_empty());
        assert_eq!(parent.text_content(), "Hello");

        let span = parent.first_child().unwrap();
        assert_eq!(&*span.tag, "span");
    }

    #[test]
    fn test_element_builder() {
        let elem: Element<TestSite::Raw> = Element::new("div")
            .with_id("main")
            .with_class("container")
            .attr("data-foo", "bar")
            .child(Element::new("span"))
            .text("Hello");

        assert_eq!(elem.id(), Some("main"));
        assert_eq!(elem.class(), Some("container"));
        assert_eq!(elem.get_attr("data-foo"), Some("bar"));
        assert_eq!(elem.len(), 2);
    }

    #[test]
    fn test_document_basic() {
        let root: Element<TestSite::Raw> = Element::new("html")
            .child(Element::new("head"))
            .child(Element::new("body"));

        let doc = Document::new(root);
        assert_eq!(doc.phase_name(), "TestSite::Raw");
        assert_eq!(doc.element_count(), 3);
    }

    #[test]
    fn test_document_find() {
        let root: Element<TestSite::Raw> = Element::new("div")
            .child(Element::new("span").with_class("highlight"))
            .child(Element::new("p"));

        let doc = Document::new(root);

        let span = doc.find(|e| e.tag == "span").unwrap();
        assert_eq!(span.class(), Some("highlight"));

        assert!(doc.find(|e| e.tag == "missing").is_none());
        assert!(doc.any(|e| e.tag == "p"));
    }

    #[test]
    fn test_document_find_all() {
        let root: Element<TestSite::Raw> = Element::new("div")
            .child(Element::new("span"))
            .child(Element::new("span"))
            .child(Element::new("p"));

        let doc = Document::new(root);
        let spans = doc.find_all(|e| e.tag == "span");
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn test_document_elements_iterator() {
        let root: Element<TestSite::Raw> = Element::new("div")
            .child(Element::new("span"))
            .child(Element::new("p"));

        let doc = Document::new(root);
        let tags: Vec<_> = doc.elements().map(|e| &*e.tag).collect();
        assert_eq!(tags, vec!["div", "span", "p"]);
    }

    #[test]
    fn test_indexed_phase_stable_id() {
        use crate::id::StableId;

        // Create indexed extension
        let raw_ext = TestSite::RawExt::Heading(HeadingRaw::new(2));
        let indexed_ext = TestSite::index_ext(raw_ext, StableId::from_raw(12345));

        // Create element with indexed extension
        let elem: Element<TestSite::Indexed> = Element::with_ext("h2", indexed_ext);

        // stable_id() is available because IndexedExt implements HasStableId
        assert_eq!(elem.stable_id().as_raw(), 12345);
        assert_eq!(elem.family_name(), "heading");
    }

    #[test]
    fn test_full_phase_cycle() {
        use crate::id::StableId;

        // Raw phase
        let raw_ext = TestSite::RawExt::Link(LinkRaw::new("/about"));
        let raw_elem: Element<TestSite::Raw> = Element::with_ext("a", raw_ext)
            .text("About Us");

        assert_eq!(raw_elem.family_name(), "link");
        assert_eq!(raw_elem.text_content(), "About Us");

        // Index: Raw → Indexed
        let indexed_ext = TestSite::index_ext(raw_elem.ext.clone(), StableId::from_raw(999));
        let indexed_elem: Element<TestSite::Indexed> = Element::with_ext("a", indexed_ext);

        assert_eq!(indexed_elem.stable_id().as_raw(), 999);

        // Type-safe access to family data
        let link_data = ExtractFamily::<LinkFamily>::get(&indexed_elem.ext).unwrap();
        assert_eq!(link_data.href.as_deref(), Some("/about"));

        // Process: Indexed → Processed
        let processed_ext = TestSite::process_ext(&indexed_elem.ext);
        let processed_elem: Element<TestSite::Processed> = Element::with_ext("a", processed_ext);

        assert_eq!(processed_elem.stable_id().as_raw(), 999);

        // Processed family data
        let link_processed = ExtractFamily::<LinkFamily>::get(&processed_elem.ext).unwrap();
        assert!(!link_processed.is_external); // /about is internal
    }

    #[test]
    fn test_find_by_family() {
        let raw_link = TestSite::RawExt::Link(LinkRaw::new("https://example.com"));
        let raw_heading = TestSite::RawExt::Heading(HeadingRaw::new(1));

        let root: Element<TestSite::Raw> = Element::new("article")
            .child(Element::with_ext("h1", raw_heading).text("Title"))
            .child(Element::with_ext("a", raw_link).text("Link"));

        let doc = Document::new(root);

        let links = doc.find_by::<TestSite::FamilyKind::Link>();
        assert_eq!(links.len(), 1);
        assert_eq!(&*links[0].tag, "a");

        let headings = doc.find_by::<TestSite::FamilyKind::Heading>();
        assert_eq!(headings.len(), 1);
        assert_eq!(&*headings[0].tag, "h1");
    }
}
