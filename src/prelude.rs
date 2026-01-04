//! Prelude module for common imports.
//!
//! ```ignore
//! use tola_vdom::prelude::*;
//! ```

// Core traits
pub use crate::core::{
    ElementExt, ExtractFamily, Family, FamilyData, FamilySet, HasStableId, IndexedExt,
    IndexedPhaseMarker, NoneFamily, NoneIndexed, Phase, PhaseExt, ProcessedPhaseMarker,
    RawPhaseMarker,
};

// Node types
pub use crate::node::{Children, Document, Element, Node, Text, TextKind};

// Transform
pub use crate::transform::{IdentityTransform, IndexStats, Indexer, Pipeline, Processor, Transform};

#[cfg(feature = "async")]
pub use crate::transform::{AsyncPipeline, NoopValidator, ValidateError, ValidateErrors, Validator};

// Attributes
pub use crate::attr::{AttrKey, AttrValue, Attrs, Tag, TextContent};

// Identity
pub use crate::id::{PageSeed, StableId};

// Algorithms
pub use crate::algo::{
    diff, diff_with_config, Anchor, DiffConfig, DiffResult, DiffStats, Patch, PatchOp,
    StableHasher,
};

// Span
pub use crate::span::SourceSpan;

// Error
pub use crate::error::{VdomError, VdomResult};

// Families (built-in)
pub use crate::families::{HeadingFamily, LinkFamily, MediaFamily, SvgFamily};

// Cache
pub use crate::cache::{CacheEntry, CacheKey, SharedVdomCache, VdomCache};

// Render
pub use crate::render::{
    render_document, render_document_bytes, render_patches, RenderConfig, DEFAULT_ID_ATTR,
};

// Serialization
#[cfg(feature = "cache")]
pub use crate::serialize::{from_bytes, to_bytes, SCHEMA_VERSION};

