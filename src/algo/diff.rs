//! VDOM Diff Algorithm
//!
//! Computes minimal patch operations between two VDOM trees.
//! This is a **pure algorithm module** with **no render dependencies**.
//!
//! # Architecture: Diff/Render Separation
//!
//! ```text
//! diff(old, new) -> DiffResult<PatchOp>  // Pure data, no HTML
//!       |
//!       v
//! render_patches(ops, emit_ids) -> Vec<Patch>  // HTML rendering
//! ```
//!
//! `PatchOp` stores node references, `Patch` stores rendered HTML.
//! This separation enables:
//! - Testing diff logic without render
//! - Different render strategies (minified, pretty-printed)
//! - Deferred rendering for optimization
//!
//! # Algorithm
//!
//! 1. Compare nodes by StableId (not just position)
//! 2. Use LCS to detect moves, insertions, deletions
//! 3. Generate minimal patch operations
//!
//! # Key Features
//!
//! - **Move Detection**: Reordered nodes generate `Move` ops, not Delete+Insert
//! - **Stable Identity**: Same StableId = same node across edits
//! - **Incremental Updates**: Only changed subtrees are patched
//!
//! # Complexity
//!
//! - Time: O(n * d) where d is the edit distance
//! - Space: O(n + m) for patch list

use crate::attr::{AttrKey, AttrValue};
use crate::core::{HasStableId, PhaseExt};
use crate::id::StableId;
use crate::node::{Document, Element, Node};

use super::myers::{diff_sequences, Edit};

/// Default maximum depth for recursive diffing before fallback to full replace.
const DEFAULT_MAX_DIFF_DEPTH: usize = 500;

/// Default maximum number of operations before fallback to full reload.
const DEFAULT_MAX_OPS: usize = 2000;

// =============================================================================
// Public Types
// =============================================================================

/// Configuration for diff algorithm limits.
///
/// Use this to tune diff behavior for specific document types:
/// - Increase limits for large documents (generated docs, long tables)
/// - Decrease limits for faster fallback on complex changes
#[derive(Debug, Clone, Copy)]
pub struct DiffConfig {
    /// Maximum recursion depth before fallback to full replace.
    /// Default: 500
    pub max_depth: usize,
    /// Maximum number of patch operations before fallback to full reload.
    /// Default: 2000
    pub max_ops: usize,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DIFF_DEPTH,
            max_ops: DEFAULT_MAX_OPS,
        }
    }
}

impl DiffConfig {
    /// Create config with custom limits.
    pub fn new(max_depth: usize, max_ops: usize) -> Self {
        Self { max_depth, max_ops }
    }

    /// Create config for large documents (higher limits).
    pub fn large() -> Self {
        Self {
            max_depth: 1000,
            max_ops: 5000,
        }
    }

    /// Create config for small documents (lower limits, faster fallback).
    pub fn small() -> Self {
        Self {
            max_depth: 100,
            max_ops: 500,
        }
    }
}

/// Statistics from diff operation
#[derive(Debug, Default, Clone, Copy)]
#[must_use]
pub struct DiffStats {
    /// Number of elements compared
    pub elements_compared: usize,
    /// Number of text nodes compared
    pub text_nodes_compared: usize,
    /// Number of nodes kept unchanged
    pub nodes_kept: usize,
    /// Number of nodes moved
    pub nodes_moved: usize,
    /// Number of nodes replaced
    pub nodes_replaced: usize,
    /// Number of text updates
    pub text_updates: usize,
    /// Number of attribute updates
    pub attr_updates: usize,
}

/// Result of StableId-based diff operation
#[derive(Debug)]
#[must_use]
pub struct DiffResult<P: PhaseExt> {
    /// Generated patch operations (pure data, no HTML)
    pub ops: Vec<PatchOp<P>>,
    /// Whether diff exceeded limits and should fallback to reload
    pub should_reload: bool,
    /// Reason for reload (if should_reload is true)
    pub reload_reason: Option<String>,
    /// Statistics about the diff
    pub stats: DiffStats,
}

impl<P: PhaseExt> DiffResult<P> {
    /// Create a result that triggers reload
    pub fn reload(reason: impl Into<String>) -> Self {
        Self {
            ops: vec![],
            should_reload: true,
            reload_reason: Some(reason.into()),
            stats: DiffStats::default(),
        }
    }

    /// Check if any changes were detected
    pub fn has_changes(&self) -> bool {
        !self.ops.is_empty() || self.should_reload
    }
}

// =============================================================================
// Anchor System
// =============================================================================

/// Anchor for insert/move operations
///
/// Specifies WHERE to place an element relative to existing nodes.
/// All anchors reference elements by StableId, never by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// Insert/move after an element
    After(StableId),
    /// Insert/move before an element
    Before(StableId),
    /// Insert/move as first child
    FirstChildOf(StableId),
    /// Insert/move as last child
    LastChildOf(StableId),
}

impl Anchor {
    /// Get the StableId referenced by this anchor
    pub fn target_id(&self) -> StableId {
        match self {
            Self::After(id) | Self::Before(id) | Self::FirstChildOf(id) | Self::LastChildOf(id) => {
                *id
            }
        }
    }
}

// =============================================================================
// PatchOp: Pure diff output (no HTML)
// =============================================================================

/// Patch operation with node data (no HTML rendering).
///
/// This is the **pure diff output**. Use `render_patches()` to convert to HTML.
/// Separating diff from render enables:
/// - Testing diff logic without render dependencies
/// - Different render strategies (dev vs prod)
/// - Lazy/deferred rendering
#[derive(Debug, Clone)]
pub enum PatchOp<P: PhaseExt> {
    /// Replace entire element
    Replace {
        target: StableId,
        element: Box<Element<P>>,
    },

    /// Update text content (for single-text-child elements)
    UpdateText { target: StableId, text: String },

    /// Replace all children
    ReplaceChildren {
        target: StableId,
        children: Vec<Node<P>>,
        /// Whether parent is SVG (affects text escaping)
        is_svg: bool,
    },

    /// Remove element by ID
    Remove { target: StableId },

    /// Insert new node at anchor position
    Insert {
        anchor: Anchor,
        node: Node<P>,
    },

    /// Move existing element to new anchor position
    Move { target: StableId, to: Anchor },

    /// Update attributes
    UpdateAttrs {
        target: StableId,
        /// Attribute changes: (name, Some(value)) for set, (name, None) for remove
        changes: Vec<(AttrKey, Option<AttrValue>)>,
    },
}

impl<P: PhaseExt> PatchOp<P> {
    /// Get the primary target StableId of this patch
    pub fn target(&self) -> StableId {
        match self {
            Self::Replace { target, .. } => *target,
            Self::UpdateText { target, .. } => *target,
            Self::ReplaceChildren { target, .. } => *target,
            Self::Remove { target } => *target,
            Self::Insert { anchor, .. } => anchor.target_id(),
            Self::Move { target, .. } => *target,
            Self::UpdateAttrs { target, .. } => *target,
        }
    }
}

// =============================================================================
// Patch: Rendered output (with HTML)
// =============================================================================

/// Rendered patch operation ready for client execution.
///
/// This is the **output of `render_patches()`**. Contains pre-rendered HTML.
#[derive(Debug, Clone)]
pub enum Patch {
    /// Replace entire element's outerHTML
    Replace { target: StableId, html: String },

    /// Update text content (element.textContent = text)
    UpdateText { target: StableId, text: String },

    /// Replace inner HTML (element.innerHTML = html)
    /// `is_svg` indicates content should be parsed as SVG namespace
    ReplaceChildren { target: StableId, html: String, is_svg: bool },

    /// Remove element by ID
    Remove { target: StableId },

    /// Insert new content at anchor position
    Insert { anchor: Anchor, html: String },

    /// Move existing element to new anchor position
    Move { target: StableId, to: Anchor },

    /// Update attributes
    UpdateAttrs {
        target: StableId,
        attrs: Vec<(AttrKey, Option<AttrValue>)>,
    },
}

impl Patch {
    /// Get the primary target StableId of this patch
    pub fn target(&self) -> StableId {
        match self {
            Self::Replace { target, .. } => *target,
            Self::UpdateText { target, .. } => *target,
            Self::ReplaceChildren { target, .. } => *target,
            Self::Remove { target } => *target,
            Self::Insert { anchor, .. } => anchor.target_id(),
            Self::Move { target, .. } => *target,
            Self::UpdateAttrs { target, .. } => *target,
        }
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Diff two VDOM documents using StableId-based comparison
///
/// Returns `PatchOp` which contains node data but no rendered HTML.
/// Use `render_patches()` to convert to `Patch` with HTML.
///
/// # Example
///
/// ```ignore
/// let result = diff(&old_doc, &new_doc);
/// let patches = render_patches(&result.ops, true); // emit_ids = true for dev
/// ```
pub fn diff<P>(old: &Document<P>, new: &Document<P>) -> DiffResult<P>
where
    P: PhaseExt,
    P::Ext: HasStableId + Clone,
{
    diff_with_config(old, new, DiffConfig::default())
}

/// Diff two VDOM documents with custom configuration.
///
/// Use this when you need to tune diff limits for specific document types.
///
/// # Example
///
/// ```ignore
/// // For large generated documentation
/// let config = DiffConfig::large_document();
/// let result = diff_with_config(&old_doc, &new_doc, config);
/// ```
pub fn diff_with_config<P>(old: &Document<P>, new: &Document<P>, config: DiffConfig) -> DiffResult<P>
where
    P: PhaseExt,
    P::Ext: HasStableId + Clone,
{
    let mut ctx = DiffContext::<P>::new(config);
    ctx.diff_element(&old.root, &new.root);
    ctx.into_result()
}

// =============================================================================
// Internal Context
// =============================================================================

struct DiffContext<P: PhaseExt> {
    ops: Vec<PatchOp<P>>,
    depth: usize,
    should_reload: bool,
    reload_reason: Option<String>,
    stats: DiffStats,
    config: DiffConfig,
}

impl<P: PhaseExt> DiffContext<P>
where
    P::Ext: HasStableId + Clone,
{
    fn new(config: DiffConfig) -> Self {
        Self {
            ops: Vec::new(),
            depth: 0,
            should_reload: false,
            reload_reason: None,
            stats: DiffStats::default(),
            config,
        }
    }

    fn into_result(self) -> DiffResult<P> {
        DiffResult {
            ops: self.ops,
            should_reload: self.should_reload,
            reload_reason: self.reload_reason,
            stats: self.stats,
        }
    }

    fn should_abort(&self) -> bool {
        self.should_reload || self.ops.len() > self.config.max_ops || self.depth > self.config.max_depth
    }

    /// Diff two elements by StableId
    fn diff_element(&mut self, old: &Element<P>, new: &Element<P>) {
        if self.should_abort() {
            return;
        }

        self.stats.elements_compared += 1;
        let old_id = old.ext.stable_id();

        // If tags differ, must replace entirely
        if old.tag != new.tag {
            self.ops.push(PatchOp::Replace {
                target: old_id,
                element: Box::new(new.clone()),
            });
            self.stats.nodes_replaced += 1;
            return;
        }

        // Diff attributes
        self.diff_attrs(old, new);

        // Check if this is an SVG element - SVG text children contain raw markup,
        // must use innerHTML (ReplaceChildren) not textContent (UpdateText)
        let is_svg = old.tag == "svg";

        // Fast path: single text child optimization
        // SKIP for SVG: SVG text children contain raw markup (<path>, <g>, etc.)
        // that must be set via innerHTML, not textContent
        let old_single_text = get_single_text_child(&old.children);
        let new_single_text = get_single_text_child(&new.children);

        if !is_svg {
            match (old_single_text, new_single_text) {
                // Both have single text child
                (Some(old_text), Some(new_text)) => {
                    if old_text != new_text {
                        self.ops.push(PatchOp::UpdateText {
                            target: old_id,
                            text: new_text.to_string(),
                        });
                        self.stats.text_updates += 1;
                    }
                    self.stats.nodes_kept += 1;
                    return;
                }
                // Old has text, new is empty
                (Some(_), None) if new.children.is_empty() => {
                    self.ops.push(PatchOp::UpdateText {
                        target: old_id,
                        text: String::new(),
                    });
                    self.stats.text_updates += 1;
                    self.stats.nodes_kept += 1;
                    return;
                }
                // Old is empty, new has text
                (None, Some(new_text)) if old.children.is_empty() => {
                    self.ops.push(PatchOp::UpdateText {
                        target: old_id,
                        text: new_text.to_string(),
                    });
                    self.stats.text_updates += 1;
                    self.stats.nodes_kept += 1;
                    return;
                }
                _ => {}
            }
        }

        // Diff children
        self.depth += 1;
        self.diff_children(&old.children, &new.children, old_id, &old.tag);
        self.depth -= 1;

        self.stats.nodes_kept += 1;
    }

    /// Diff element attributes
    ///
    /// Special handling for resource URL changes:
    /// - `<link href>` change: Use `Replace` to trigger CSS reload
    /// - `<script src>` change: Trigger full reload (re-execution has side effects)
    fn diff_attrs(&mut self, old: &Element<P>, new: &Element<P>) {
        if self.should_abort() {
            return;
        }

        let mut changes: Vec<(AttrKey, Option<AttrValue>)> = Vec::new();

        // Check for changed/added attributes
        for (name, value) in &new.attrs {
            let old_value = old.get_attr(name);
            if old_value != Some(value.as_str()) {
                changes.push((name.clone(), Some(value.clone())));
            }
        }

        // Check for removed attributes
        for (name, _) in &old.attrs {
            if new.get_attr(name).is_none() {
                changes.push((name.clone(), None));
            }
        }

        if !changes.is_empty() {
            // Check for resource URL changes that need special handling
            let href_changed = old.tag == "link" && changes.iter().any(|(n, _)| n == "href");
            let src_changed = old.tag == "script" && changes.iter().any(|(n, _)| n == "src");

            if href_changed {
                // <link href> changed: use Replace to trigger CSS reload
                self.ops.push(PatchOp::Replace {
                    target: old.ext.stable_id(),
                    element: Box::new(new.clone()),
                });
                self.stats.nodes_replaced += 1;
            } else if src_changed {
                // <script src> changed: trigger reload (re-execution has side effects)
                self.should_reload = true;
                self.reload_reason = Some("script src changed".to_string());
            } else {
                self.ops.push(PatchOp::UpdateAttrs {
                    target: old.ext.stable_id(),
                    changes,
                });
                self.stats.attr_updates += 1;
            }
        }
    }

    /// Diff child nodes
    fn diff_children(
        &mut self,
        old_children: &[Node<P>],
        new_children: &[Node<P>],
        parent_id: StableId,
        parent_tag: &str,
    ) {
        if self.should_abort() {
            return;
        }

        // Quick path: both empty
        if old_children.is_empty() && new_children.is_empty() {
            return;
        }

        // SVG: must use innerHTML, deep compare entire subtree
        let is_svg = parent_tag == "svg";
        if is_svg {
            if !svg_subtrees_equal(old_children, new_children) {
                self.ops.push(PatchOp::ReplaceChildren {
                    target: parent_id,
                    children: new_children.to_vec(),
                    is_svg: true,
                });
                self.stats.nodes_replaced += 1;
            }
            return;
        }

        // Non-SVG: use optimized paths
        if old_children.is_empty() {
            self.insert_all_children(new_children, parent_id);
            return;
        }

        if new_children.is_empty() {
            self.remove_all_element_children(old_children);
            return;
        }

        // Check content types
        let old_has_text = old_children.iter().any(|n| matches!(n, Node::Text(_)));
        let new_has_text = new_children.iter().any(|n| matches!(n, Node::Text(_)));

        if !old_has_text && !new_has_text {
            self.diff_element_children(old_children, new_children, parent_id);
        } else {
            self.diff_mixed_children(old_children, new_children, parent_id, parent_tag);
        }
    }

    /// Insert all children
    fn insert_all_children(&mut self, children: &[Node<P>], parent_id: StableId) {
        let mut last_element_id: Option<StableId> = None;

        for child in children {
            if self.should_abort() {
                return;
            }

            let anchor = match last_element_id {
                Some(prev_id) => Anchor::After(prev_id),
                None => Anchor::FirstChildOf(parent_id),
            };

            self.ops.push(PatchOp::Insert {
                anchor,
                node: child.clone(),
            });

            if let Node::Element(elem) = child {
                last_element_id = Some(elem.ext.stable_id());
            }
        }
    }

    /// Remove all element children
    fn remove_all_element_children(&mut self, children: &[Node<P>]) {
        for child in children {
            if self.should_abort() {
                return;
            }

            if let Node::Element(elem) = child {
                self.ops.push(PatchOp::Remove {
                    target: elem.ext.stable_id(),
                });
            }
        }
    }

    /// Diff pure element children using LCS
    fn diff_element_children(
        &mut self,
        old_children: &[Node<P>],
        new_children: &[Node<P>],
        parent_id: StableId,
    ) {
        let old_ids: Vec<StableId> = old_children.iter().map(get_node_stable_id).collect();
        let new_ids: Vec<StableId> = new_children.iter().map(get_node_stable_id).collect();

        let lcs_result = diff_sequences(&old_ids, &new_ids);

        let mut keeps: Vec<(usize, usize)> = Vec::new();
        let mut moves: Vec<(usize, usize)> = Vec::new();
        let mut deletes: Vec<usize> = Vec::new();
        let mut inserts: Vec<usize> = Vec::new();

        for edit in &lcs_result.edits {
            match edit {
                Edit::Keep { old_idx, new_idx } => keeps.push((*old_idx, *new_idx)),
                Edit::Move { old_idx, new_idx } => moves.push((*old_idx, *new_idx)),
                Edit::Delete { old_idx } => deletes.push(*old_idx),
                Edit::Insert { new_idx } => inserts.push(*new_idx),
            }
        }

        // 1. Remove deleted elements
        for old_idx in &deletes {
            if self.should_abort() {
                return;
            }
            self.ops.push(PatchOp::Remove {
                target: old_ids[*old_idx],
            });
        }

        // 2. Apply moves
        moves.sort_unstable_by_key(|(_, new_idx)| *new_idx);
        for (old_idx, new_idx) in &moves {
            if self.should_abort() {
                return;
            }
            let anchor = self.compute_anchor(*new_idx, new_children, parent_id);
            self.ops.push(PatchOp::Move {
                target: old_ids[*old_idx],
                to: anchor,
            });
            self.stats.nodes_moved += 1;
        }

        // 3. Insert new elements
        inserts.sort_unstable();
        for new_idx in &inserts {
            if self.should_abort() {
                return;
            }
            let anchor = self.compute_anchor(*new_idx, new_children, parent_id);
            self.ops.push(PatchOp::Insert {
                anchor,
                node: new_children[*new_idx].clone(),
            });
        }

        // 4. Recursively diff kept and moved elements
        for (old_idx, new_idx) in keeps.iter().chain(moves.iter()) {
            self.diff_nodes(&old_children[*old_idx], &new_children[*new_idx]);
        }
    }

    /// Compute anchor for a position
    fn compute_anchor(
        &self,
        new_idx: usize,
        new_children: &[Node<P>],
        parent_id: StableId,
    ) -> Anchor {
        for i in (0..new_idx).rev() {
            if let Node::Element(elem) = &new_children[i] {
                return Anchor::After(elem.ext.stable_id());
            }
        }
        Anchor::FirstChildOf(parent_id)
    }

    /// Diff mixed children (contains text nodes)
    fn diff_mixed_children(
        &mut self,
        old_children: &[Node<P>],
        new_children: &[Node<P>],
        parent_id: StableId,
        parent_tag: &str,
    ) {
        // Check if parent is SVG - affects text escaping in render
        let is_svg = parent_tag == "svg";

        if self.children_structure_matches(old_children, new_children) {
            let text_changed = old_children.iter().zip(new_children.iter()).any(|(old, new)| {
                matches!(
                    (old, new),
                    (Node::Text(old_t), Node::Text(new_t)) if old_t.content != new_t.content
                )
            });

            if text_changed {
                self.ops.push(PatchOp::ReplaceChildren {
                    target: parent_id,
                    children: new_children.to_vec(),
                    is_svg,
                });
                self.stats.text_updates += 1;
            } else {
                for (old, new) in old_children.iter().zip(new_children.iter()) {
                    self.diff_nodes(old, new);
                }
            }
        } else {
            self.ops.push(PatchOp::ReplaceChildren {
                target: parent_id,
                children: new_children.to_vec(),
                is_svg,
            });
            self.stats.nodes_replaced += 1;
        }
    }

    /// Check if structure matches
    fn children_structure_matches(&self, old: &[Node<P>], new: &[Node<P>]) -> bool {
        if old.len() != new.len() {
            return false;
        }
        old.iter().zip(new.iter()).all(|(o, n)| {
            matches!(
                (o, n),
                (Node::Element(_), Node::Element(_)) | (Node::Text(_), Node::Text(_))
            )
        })
    }

    /// Diff two nodes
    fn diff_nodes(&mut self, old: &Node<P>, new: &Node<P>) {
        if self.should_abort() {
            return;
        }

        match (old, new) {
            (Node::Element(old_elem), Node::Element(new_elem)) => {
                self.diff_element(old_elem, new_elem);
            }
            (Node::Text(old_text), Node::Text(new_text)) => {
                self.stats.text_nodes_compared += 1;
                if old_text.content != new_text.content {
                    debug_assert!(false, "Text node diff should be handled by parent");
                    self.stats.text_updates += 1;
                }
            }
            _ => {
                debug_assert!(false, "diff_nodes called with mismatched types");
            }
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn get_single_text_child<P: PhaseExt>(children: &[Node<P>]) -> Option<&str> {
    if children.len() == 1
        && let Node::Text(text) = &children[0]
    {
        return Some(&text.content);
    }
    None
}

fn get_node_stable_id<P: PhaseExt>(node: &Node<P>) -> StableId
where
    P::Ext: HasStableId,
{
    match node {
        Node::Element(elem) => elem.ext.stable_id(),
        // Text nodes use content hash as fallback ID
        Node::Text(text) => {
            use crate::algo::StableHasher;
            let hash = StableHasher::new().update_str(&text.content).finish();
            StableId::from_raw(hash)
        }
    }
}

/// Deep compare SVG subtrees (SVG requires innerHTML, no fine-grained patch).
fn svg_subtrees_equal<P: PhaseExt>(old: &[Node<P>], new: &[Node<P>]) -> bool
where
    P::Ext: HasStableId,
{
    if old.len() != new.len() {
        return false;
    }

    old.iter().zip(new.iter()).all(|(o, n)| match (o, n) {
        (Node::Text(old_t), Node::Text(new_t)) => old_t.content == new_t.content,
        (Node::Element(old_e), Node::Element(new_e)) => {
            old_e.tag == new_e.tag
                && old_e.attrs == new_e.attrs
                && old_e.ext.stable_id() == new_e.ext.stable_id()
                && svg_subtrees_equal(&old_e.children, &new_e.children)
        }
        _ => false, // Different node types
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(all(test, feature = "macros"))]
mod tests {
    use super::*;
    use crate::families::{LinkFamily, HeadingFamily, SvgFamily, MediaFamily};
    use crate::vdom;

    #[vdom::families]
    pub struct DiffTestSite {
        link: LinkFamily,
        heading: HeadingFamily,
        svg: SvgFamily,
        media: MediaFamily,
    }

    fn indexed_elem(tag: &str, id: u64) -> Element<DiffTestSite::Indexed> {
        let ext = DiffTestSite::IndexedExt::None(crate::core::NoneIndexed {
            stable_id: StableId::from_raw(id),
        });
        Element::with_ext(tag, ext)
    }

    fn indexed_text(content: &str) -> crate::node::Text<DiffTestSite::Indexed> {
        crate::node::Text::new(content)
    }

    #[test]
    fn test_patch_op_target() {
        let patch: PatchOp<DiffTestSite::Indexed> = PatchOp::Replace {
            target: StableId::from_raw(42),
            element: Box::new(indexed_elem("div", 42)),
        };
        assert_eq!(patch.target().as_raw(), 42);
    }

    #[test]
    fn test_patch_target() {
        let patch = Patch::Replace {
            target: StableId::from_raw(42),
            html: "<div></div>".to_string(),
        };
        assert_eq!(patch.target().as_raw(), 42);
    }

    #[test]
    fn test_diff_result_reload() {
        let result: DiffResult<DiffTestSite::Indexed> = DiffResult::reload("test reason");
        assert!(result.should_reload);
        assert_eq!(result.reload_reason, Some("test reason".to_string()));
        assert!(result.has_changes());
    }

    #[test]
    fn test_diff_stats_default() {
        let stats = DiffStats::default();
        assert_eq!(stats.elements_compared, 0);
        assert_eq!(stats.nodes_kept, 0);
        assert_eq!(stats.nodes_moved, 0);
    }

    #[test]
    fn test_diff_detects_text_update_and_revert() {
        fn build_doc(texts: &[&str], base_id: u64) -> Document<DiffTestSite::Indexed> {
            let mut root = indexed_elem("body", base_id);
            for (i, t) in texts.iter().enumerate() {
                let mut p = indexed_elem("p", base_id + i as u64 + 1);
                let txt = indexed_text(t);
                p.children.push(Node::Text(txt));
                root.children.push(Node::Element(Box::new(p)));
            }
            Document::new(root)
        }

        let old = build_doc(&["cosplay堂吉珂德", "浊富 or 清贫?", "GALGAME!"], 1000);
        let new1 = build_doc(&["cosplay堂吉珂德", "浊富 or 清?", "GALGAME!"], 1000);
        let new2 = build_doc(&["cosplay堂吉珂德", "浊富 or 清贫?", "GALGAME!"], 1000);

        let r1 = diff(&old, &new1);
        assert!(!r1.should_reload);
        let has_update1 = r1.ops.iter().any(|op| match op {
            PatchOp::UpdateText { text, .. } => text.contains("浊富"),
            _ => false,
        });
        assert!(has_update1, "Expected update, got: {:?}", r1.ops);

        let r2 = diff(&new1, &new2);
        assert!(!r2.should_reload);
    }

    #[test]
    fn test_edit_ordering_prevents_duplicates() {
        let mut root_old = indexed_elem("div", 0);
        root_old
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 1))));
        root_old
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 2))));
        root_old
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 3))));
        let old = Document::new(root_old);

        let mut root_new = indexed_elem("div", 0);
        root_new
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 1))));
        root_new
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 3))));
        root_new
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 2))));
        root_new
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 4))));
        let new = Document::new(root_new);

        let r = diff(&old, &new);
        let mut first_insert = None;
        let mut first_remove = None;
        for (i, op) in r.ops.iter().enumerate() {
            match op {
                PatchOp::Insert { .. } => {
                    first_insert.get_or_insert(i);
                }
                PatchOp::Remove { .. } => {
                    first_remove.get_or_insert(i);
                }
                _ => {}
            }
        }
        assert!(
            first_remove.is_none()
                || first_insert.is_none()
                || first_remove.unwrap() < first_insert.unwrap(),
            "Removals should come before inserts: {:?}",
            r.ops
        );
    }

    #[test]
    fn test_single_text_child_empty_to_text() {
        let mut root_old = indexed_elem("body", 0);
        root_old
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 100))));
        let old = Document::new(root_old);

        let mut root_new = indexed_elem("body", 0);
        let mut p_new = indexed_elem("p", 100);
        p_new.children.push(Node::Text(indexed_text("Hello")));
        root_new.children.push(Node::Element(Box::new(p_new)));
        let new = Document::new(root_new);

        let result = diff(&old, &new);
        let has_update = result.ops.iter().any(|op| {
            matches!(op, PatchOp::UpdateText { target, text } if target.as_raw() == 100 && text == "Hello")
        });
        assert!(has_update, "Expected UpdateText, got: {:?}", result.ops);
    }

    #[test]
    fn test_single_text_child_text_to_empty() {
        let mut root_old = indexed_elem("body", 0);
        let mut p_old = indexed_elem("p", 100);
        p_old.children.push(Node::Text(indexed_text("Hello")));
        root_old.children.push(Node::Element(Box::new(p_old)));
        let old = Document::new(root_old);

        let mut root_new = indexed_elem("body", 0);
        root_new
            .children
            .push(Node::Element(Box::new(indexed_elem("p", 100))));
        let new = Document::new(root_new);

        let result = diff(&old, &new);
        let has_update = result.ops.iter().any(|op| {
            matches!(op, PatchOp::UpdateText { target, text } if target.as_raw() == 100 && text.is_empty())
        });
        assert!(
            has_update,
            "Expected UpdateText with empty, got: {:?}",
            result.ops
        );
    }

    #[test]
    fn test_svg_nested_child_attr_change() {
        // Test: SVG with nested children where a grandchild's attr changes
        // <svg id=1>
        //   <g id=2>
        //     <path id=3 d="old" />
        //   </g>
        // </svg>
        // ->
        // <svg id=1>
        //   <g id=2>
        //     <path id=3 d="new" />
        //   </g>
        // </svg>

        fn build_svg_doc(path_d: &str) -> Document<DiffTestSite::Indexed> {
            let mut root = indexed_elem("body", 0);

            let mut svg = indexed_elem("svg", 1);
            let mut g = indexed_elem("g", 2);
            let mut path = indexed_elem("path", 3);
            path.set_attr("d", path_d);

            g.children.push(Node::Element(Box::new(path)));
            svg.children.push(Node::Element(Box::new(g)));
            root.children.push(Node::Element(Box::new(svg)));

            Document::new(root)
        }

        let old = build_svg_doc("M0 0 L10 10");
        let new = build_svg_doc("M0 0 L20 20");

        let result = diff(&old, &new);

        // Should detect the change and generate ReplaceChildren for SVG
        assert!(
            result.has_changes(),
            "Expected changes for SVG nested attr change, got: {:?}",
            result.ops
        );
    }
}
