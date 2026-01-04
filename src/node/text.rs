//! Text node type for the new PhaseExt-based system.

use crate::attr::TextContent;
use crate::core::PhaseExt;

// =============================================================================
// TextKind
// =============================================================================

/// Controls how text content is rendered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TextKind {
    /// Standard text: HTML special characters are escaped.
    /// Use for normal text content.
    #[default]
    Escaped,
    /// Raw content: output as-is without escaping.
    /// Use for trusted HTML/SVG/XML content.
    Raw,
}

// =============================================================================
// Text<P>
// =============================================================================

/// Text content node.
#[derive(Debug, Clone)]
pub struct Text<P: PhaseExt> {
    /// Text content (CompactString for efficient small text storage)
    pub content: TextContent,
    /// How this text should be rendered (escaped or raw)
    pub kind: TextKind,
    /// Phase-specific extension
    pub ext: P::TextExt,
}

impl<P: PhaseExt> Text<P> {
    /// Create a new escaped text node.
    pub fn new(content: impl Into<TextContent>) -> Self {
        Self {
            content: content.into(),
            kind: TextKind::Escaped,
            ext: P::TextExt::default(),
        }
    }

    /// Create a raw (unescaped) text node.
    ///
    /// Use for trusted HTML/SVG content that should not be escaped.
    pub fn raw(content: impl Into<TextContent>) -> Self {
        Self {
            content: content.into(),
            kind: TextKind::Raw,
            ext: P::TextExt::default(),
        }
    }

    /// Create text node with explicit extension.
    pub fn with_ext(content: impl Into<TextContent>, ext: P::TextExt) -> Self {
        Self {
            content: content.into(),
            kind: TextKind::Escaped,
            ext,
        }
    }

    /// Create raw text node with explicit extension.
    pub fn raw_with_ext(content: impl Into<TextContent>, ext: P::TextExt) -> Self {
        Self {
            content: content.into(),
            kind: TextKind::Raw,
            ext,
        }
    }

    /// Create text node from another text node, preserving content and kind.
    ///
    /// Use when transforming text nodes between phases.
    pub fn from_other<Q: PhaseExt>(other: Text<Q>, ext: P::TextExt) -> Self {
        Self {
            content: other.content,
            kind: other.kind,
            ext,
        }
    }

    /// Create text node from another text node with default ext.
    ///
    /// Use when transforming to phases with `TextExt = ()`.
    pub fn from_other_default<Q: PhaseExt>(other: Text<Q>) -> Self {
        Self {
            content: other.content,
            kind: other.kind,
            ext: P::TextExt::default(),
        }
    }

    /// Check if this is a raw (unescaped) text node.
    pub fn is_raw(&self) -> bool {
        self.kind == TextKind::Raw
    }

    /// Check if text is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Get byte length.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if text is whitespace only.
    pub fn is_whitespace(&self) -> bool {
        self.content.trim().is_empty()
    }

    /// Get trimmed content.
    pub fn trimmed(&self) -> &str {
        self.content.trim()
    }
}
