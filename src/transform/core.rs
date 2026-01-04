//! Core transform trait.

use crate::core::PhaseExt;
use crate::node::Document;

/// Transform a document from one phase to another.
pub trait Transform<From: PhaseExt>: Sized {
    /// Target phase.
    type To: PhaseExt;

    /// Transform the document.
    fn transform(self, doc: Document<From>) -> Document<Self::To>;
}

// =============================================================================
// IdentityTransform
// =============================================================================

/// Identity transform that returns the document unchanged.
///
/// Useful for conditional transforms where one branch doesn't need to modify
/// the document.
///
/// # Example
///
/// ```ignore
/// Pipeline::new(doc)
///     .pipe(if condition { some_transform } else { IdentityTransform })
///     .into_inner()
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityTransform;

impl<P: PhaseExt> Transform<P> for IdentityTransform {
    type To = P;

    #[inline]
    fn transform(self, doc: Document<P>) -> Document<P> {
        doc
    }
}

