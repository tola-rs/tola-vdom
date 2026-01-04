//! HTML Rendering for VDOM
//!
//! Renders VDOM documents and patches to HTML strings.

use crate::algo::{Patch, PatchOp};
use crate::attr::Attrs;
use crate::core::{HasStableId, PhaseExt};
use crate::node::{Document, Element, Node};

// =============================================================================
// RenderConfig
// =============================================================================

/// Default attribute name for stable IDs used in hot reload.
pub const DEFAULT_ID_ATTR: &str = "data-tola-id";

/// Configuration for HTML rendering.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Whether to emit stable ID attributes for hot reload.
    pub emit_ids: bool,
    /// Whether to minify output (remove unnecessary whitespace).
    pub minify: bool,
    /// Attribute name for stable IDs (default: "data-tola-id").
    ///
    /// This allows users to customize the attribute name if needed
    /// to avoid conflicts with existing attributes.
    pub id_attr_name: String,
}

impl RenderConfig {
    /// Development config (emit IDs, no minify).
    pub const DEV: Self = Self {
        emit_ids: true,
        minify: false,
        id_attr_name: String::new(), // Will use DEFAULT_ID_ATTR
    };

    /// Production config (no IDs, minify).
    pub const PROD: Self = Self {
        emit_ids: false,
        minify: true,
        id_attr_name: String::new(),
    };

    /// Create a new config.
    pub fn new(emit_ids: bool, minify: bool) -> Self {
        Self {
            emit_ids,
            minify,
            id_attr_name: DEFAULT_ID_ATTR.to_string(),
        }
    }

    /// Set custom attribute name for stable IDs.
    pub fn with_id_attr(mut self, attr_name: impl Into<String>) -> Self {
        self.id_attr_name = attr_name.into();
        self
    }

    /// Get the attribute name for stable IDs.
    pub fn id_attr(&self) -> &str {
        if self.id_attr_name.is_empty() {
            DEFAULT_ID_ATTR
        } else {
            &self.id_attr_name
        }
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self::new(true, false)
    }
}

// =============================================================================
// Document Rendering
// =============================================================================

/// Render a document to HTML bytes.
pub fn render_document_bytes<P>(doc: &Document<P>, config: &RenderConfig) -> Vec<u8>
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    render_document(doc, config).into_bytes()
}

/// Render a document to HTML string.
pub fn render_document<P>(doc: &Document<P>, config: &RenderConfig) -> String
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    let mut output = String::new();
    render_element(&doc.root, config, &mut output);
    output
}

/// Render an element to HTML.
fn render_element<P>(elem: &Element<P>, config: &RenderConfig, output: &mut String)
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    output.push('<');
    output.push_str(&elem.tag);

    // Render attributes
    render_attrs(&elem.attrs, output);

    // Emit stable ID if configured
    if config.emit_ids {
        let id = elem.ext.stable_id();
        output.push(' ');
        output.push_str(config.id_attr());
        output.push_str("=\"");
        output.push_str(&id.to_attr_value());
        output.push('"');
    }

    // Void elements
    if is_void_element(&elem.tag) {
        output.push_str(" />");
        return;
    }

    output.push('>');

    // Render children
    for child in &elem.children {
        render_node(child, config, output);
    }

    output.push_str("</");
    output.push_str(&elem.tag);
    output.push('>');
}

/// Render a node to HTML.
fn render_node<P>(node: &Node<P>, config: &RenderConfig, output: &mut String)
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    match node {
        Node::Element(elem) => render_element(elem, config, output),
        Node::Text(text) => {
            if text.is_raw() {
                // Raw text: output as-is without escaping
                output.push_str(&text.content);
            } else {
                // Normal text: escape HTML special characters
                output.push_str(&escape_html(&text.content));
            }
        }
    }
}

/// Render attributes to HTML.
fn render_attrs(attrs: &Attrs, output: &mut String) {
    for (name, value) in attrs.iter() {
        output.push(' ');
        output.push_str(name);
        output.push_str("=\"");
        output.push_str(&escape_attr(value));
        output.push('"');
    }
}

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(c),
        }
    }
    result
}

/// Escape attribute value special characters.
fn escape_attr(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '"' => result.push_str("&quot;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(c),
        }
    }
    result
}

/// Check if element is a void element (self-closing).
fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

// =============================================================================
// Patch Rendering
// =============================================================================

/// Render PatchOps to Patches with HTML.
pub fn render_patches<P>(ops: &[PatchOp<P>], config: &RenderConfig) -> Vec<Patch>
where
    P: PhaseExt,
    P::Ext: HasStableId + Clone,
{
    ops.iter()
        .map(|op| render_patch_op(op, config))
        .collect()
}

/// Render a single PatchOp to a Patch.
fn render_patch_op<P>(op: &PatchOp<P>, config: &RenderConfig) -> Patch
where
    P: PhaseExt,
    P::Ext: HasStableId + Clone,
{
    match op {
        PatchOp::Replace { target, element } => Patch::Replace {
            target: *target,
            html: render_element_to_string(element, config),
        },
        PatchOp::UpdateText { target, text } => Patch::UpdateText {
            target: *target,
            text: text.clone(),
        },
        PatchOp::ReplaceChildren {
            target,
            children,
            is_svg,
        } => Patch::ReplaceChildren {
            target: *target,
            html: render_children_to_string(children, config),
            is_svg: *is_svg,
        },
        PatchOp::Remove { target } => Patch::Remove { target: *target },
        PatchOp::Insert { anchor, node } => Patch::Insert {
            anchor: *anchor,
            html: render_node_to_string(node, config),
        },
        PatchOp::Move { target, to } => Patch::Move {
            target: *target,
            to: *to,
        },
        PatchOp::UpdateAttrs { target, changes } => Patch::UpdateAttrs {
            target: *target,
            attrs: changes.clone(),
        },
    }
}

fn render_element_to_string<P>(elem: &Element<P>, config: &RenderConfig) -> String
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    let mut output = String::new();
    render_element(elem, config, &mut output);
    output
}

fn render_node_to_string<P>(node: &Node<P>, config: &RenderConfig) -> String
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    let mut output = String::new();
    render_node(node, config, &mut output);
    output
}

fn render_children_to_string<P>(children: &[Node<P>], config: &RenderConfig) -> String
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    let mut output = String::new();
    for child in children {
        render_node(child, config, &mut output);
    }
    output
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(all(test, feature = "macros"))]
mod tests {
    use super::*;
    use crate::families::{HeadingFamily, LinkFamily, MediaFamily, SvgFamily};
    use crate::id::StableId;
    use crate::vdom;

    #[vdom::families]
    pub struct RenderTestSite {
        link: LinkFamily,
        heading: HeadingFamily,
        svg: SvgFamily,
        media: MediaFamily,
    }

    #[test]
    fn test_render_simple_element() {
        let elem: Element<RenderTestSite::Indexed> = Element::with_ext(
            "div",
            RenderTestSite::IndexedExt::None(crate::core::NoneIndexed {
                stable_id: StableId::from_raw(123),
            }),
        );
        let doc = Document::new(elem);

        let html = render_document(&doc, &RenderConfig::default());
        assert!(html.contains("<div"));
        assert!(html.contains("data-tola-id=\"7b\"")); // 123 in hex
        assert!(html.contains("</div>"));
    }

    #[test]
    fn test_render_without_ids() {
        let elem: Element<RenderTestSite::Indexed> = Element::with_ext(
            "p",
            RenderTestSite::IndexedExt::None(crate::core::NoneIndexed {
                stable_id: StableId::from_raw(456),
            }),
        );
        let doc = Document::new(elem);

        let html = render_document(&doc, &RenderConfig::PROD);
        assert!(!html.contains("data-tola-id"));
    }

    #[test]
    fn test_custom_id_attr() {
        let elem: Element<RenderTestSite::Indexed> = Element::with_ext(
            "span",
            RenderTestSite::IndexedExt::None(crate::core::NoneIndexed {
                stable_id: StableId::from_raw(789),
            }),
        );
        let doc = Document::new(elem);

        let config = RenderConfig::new(true, false).with_id_attr("data-my-id");
        let html = render_document(&doc, &config);
        assert!(html.contains("data-my-id=\"315\"")); // 789 in hex
        assert!(!html.contains("data-tola-id"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }
}
