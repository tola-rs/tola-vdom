//! tola-vdom - Type-safe Virtual DOM with Phase-based Transformations
//!
//! ## Core Concepts
//!
//! **GAT-based Family System**: Elements are classified into families (Svg, Link, Heading, etc.)
//! with compile-time type safety using Generic Associated Types.
//!
//! ## Modules
//! - `core`: Core traits (`Family`, `Phase`, `PhaseExt`, `ElementExt`)
//! - `families`: Built-in family definitions (Link, Heading, Svg, Media)
//! - `node`: Node/Element/Text/Document types
//! - `transform`: Phase transformations (Indexer, Processor)
//! - `attr`: Attribute system
//! - `algo`: Diff algorithms
//!
//! ## Usage
//!
//! ```ignore
//! use tola_vdom::vdom;
//! use tola_vdom::node::{Document, Element};
//! use tola_vdom::transform::{Transform, Indexer, Processor};
//! use tola_vdom::families::{LinkFamily, HeadingFamily};
//!
//! #[vdom::families]
//! pub struct MySite {
//!     link: LinkFamily,
//!     heading: HeadingFamily,
//! }
//!
//! // Create a raw document
//! let doc: Document<MySite::Raw> = Document::new(Element::new("html"));
//!
//! // Transform through phases using pipe()
//! let processed = doc
//!     .pipe(make_indexer())
//!     .pipe(make_processor());
//! ```

extern crate self as tola_vdom;

// =============================================================================
// Core modules
// =============================================================================

/// Core traits: Family, Phase, PhaseExt, ElementExt, HasStableId
pub mod core;

/// Built-in family definitions
pub mod families;

/// Node types: Document, Element, Node, Text
pub mod node;

/// Phase transformations: Indexer, Processor, Transform
pub mod transform;

/// Attribute types
pub mod attr;

/// Stable identity for diffing
pub mod id;

/// Source span information
pub mod span;

/// Algorithms: diff, myers
pub mod algo;

/// Error types
pub mod error;

/// Prelude for common imports
pub mod prelude;

/// HTML rendering
pub mod render;

/// Serialization support
pub mod serialize;

/// Cache types for hot reload
pub mod cache;

// =============================================================================
// Re-exports
// =============================================================================

// Core traits
pub use crate::core::{
    Family, Phase, PhaseExt, ElementExt, HasStableId, ExtractFamily,
    FamilyData, FamilySet, NoneFamily, NoneIndexed,
};

// Node types
pub use node::{Document, Element, Node, Text, TextKind, Children};

// Transform
pub use transform::{IndexStats, Indexer, Pipeline, Processor, Transform};

#[cfg(feature = "async")]
pub use transform::{AsyncPipeline, ValidateError, ValidateErrors, Validator};

// Attribute types
pub use attr::{Attrs, AttrKey, AttrValue, Tag, TextContent};

// Identity
pub use id::{PageSeed, StableId};

// Algorithms
pub use algo::StableHasher;

// Span
pub use span::SourceSpan;

// Error types
pub use error::{VdomError, VdomResult};

// Cache types
pub use cache::{CacheEntry, CacheKey, SharedVdomCache, VdomCache};

// Re-export rkyv for proc macros (only available with cache feature)
#[cfg(feature = "cache")]
pub use rkyv;

// Proc macros for custom families
#[cfg(feature = "macros")]
pub use tola_vdom_macros as vdom;

// =============================================================================
// Tests
// =============================================================================

#[cfg(all(test, feature = "macros"))]
mod tests {
    use super::*;
    use crate::core::ExtractFamily;
    use crate::families::{LinkFamily, HeadingFamily, SvgFamily, MediaFamily};
    use crate::families::link::LinkRaw;
    use crate::families::heading::HeadingRaw;

    #[vdom::families]
    pub struct TestSite {
        link: LinkFamily,
        heading: HeadingFamily,
        svg: SvgFamily,
        media: MediaFamily,
    }

    #[test]
    fn test_macro_generates_phases() {
        use crate::core::Phase;
        assert!(TestSite::Raw::NAME.contains("TestSite"));
        assert!(TestSite::Indexed::NAME.contains("TestSite"));
        assert!(TestSite::Processed::NAME.contains("TestSite"));
    }

    #[test]
    fn test_macro_generates_ext_enums() {
        let raw_ext = TestSite::RawExt::Link(Default::default());
        assert_eq!(raw_ext.family_name(), "link");

        let raw_ext = TestSite::RawExt::Heading(Default::default());
        assert_eq!(raw_ext.family_name(), "heading");

        let raw_ext = TestSite::RawExt::None;
        assert_eq!(raw_ext.family_name(), "none");
    }

    #[test]
    fn test_index_ext_preserves_family_data() {
        let raw_ext = TestSite::RawExt::Link(LinkRaw::new("https://example.com"));
        let indexed_ext = TestSite::index_ext(raw_ext, StableId::from_raw(123));

        match &indexed_ext {
            TestSite::IndexedExt::Link(data) => {
                assert_eq!(data.stable_id().as_raw(), 123);
                assert_eq!(data.href.as_deref(), Some("https://example.com"));
            }
            _ => panic!("Expected Link variant"),
        }
    }

    #[test]
    fn test_process_ext_transforms_data() {
        let raw_ext = TestSite::RawExt::Link(LinkRaw::new("https://example.com"));
        let indexed_ext = TestSite::index_ext(raw_ext, StableId::from_raw(456));
        let processed_ext = TestSite::process_ext(&indexed_ext);

        match &processed_ext {
            TestSite::ProcessedExt::Link(data) => {
                assert_eq!(data.stable_id().as_raw(), 456);
                assert!(data.is_external);
            }
            _ => panic!("Expected Link variant"),
        }
    }

    #[test]
    fn test_family_identification() {
        assert_eq!(TestSite::identify("a", &Attrs::new()), "link");
        assert_eq!(TestSite::identify("h1", &Attrs::new()), "heading");
        assert_eq!(TestSite::identify("svg", &Attrs::new()), "svg");
        assert_eq!(TestSite::identify("img", &Attrs::new()), "media");
        assert_eq!(TestSite::identify("div", &Attrs::new()), "none");
    }

    #[test]
    fn test_extract_family_type_safe_access() {
        let raw_ext = TestSite::RawExt::Link(LinkRaw::new("https://example.com"));
        let indexed_ext = TestSite::index_ext(raw_ext, StableId::from_raw(100));

        if let Some(link_data) = ExtractFamily::<LinkFamily>::get(&indexed_ext) {
            assert_eq!(link_data.href.as_deref(), Some("https://example.com"));
            assert_eq!(link_data.stable_id().as_raw(), 100);
        } else {
            panic!("Expected Link family");
        }

        let heading_data = ExtractFamily::<HeadingFamily>::get(&indexed_ext);
        assert!(heading_data.is_none());
    }

    #[test]
    fn test_extract_family_all_phases() {
        let raw_ext = TestSite::RawExt::Heading(HeadingRaw::with_id(2, "my-heading"));
        let heading_raw = ExtractFamily::<HeadingFamily>::get(&raw_ext).unwrap();
        assert_eq!(heading_raw.level, 2);

        let indexed_ext = TestSite::index_ext(raw_ext, StableId::from_raw(300));
        let heading_indexed = ExtractFamily::<HeadingFamily>::get(&indexed_ext).unwrap();
        assert_eq!(heading_indexed.level, 2);
        assert_eq!(heading_indexed.original_id.as_deref(), Some("my-heading"));

        let processed_ext = TestSite::process_ext(&indexed_ext);
        let heading_processed = ExtractFamily::<HeadingFamily>::get(&processed_ext).unwrap();
        assert_eq!(heading_processed.anchor_id, "my-heading");
    }
}
