//! Heading family: `<h1>` through `<h6>`.

use crate::attr::Attrs;
use crate::core::{Family, HasStableId};
use crate::id::StableId;

// =============================================================================
// HeadingFamily
// =============================================================================

/// Heading family for `<h1>` through `<h6>` elements.
pub struct HeadingFamily;

impl Family for HeadingFamily {
    const NAME: &'static str = "heading";

    type Raw = HeadingRaw;
    type Indexed = HeadingIndexed;
    type Processed = HeadingProcessed;

    fn identify(tag: &str, _attrs: &Attrs) -> bool {
        matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
    }

    fn index(raw: Self::Raw, id: StableId) -> Self::Indexed {
        HeadingIndexed {
            stable_id: id,
            level: raw.level,
            original_id: raw.original_id,
        }
    }

    fn process(indexed: &Self::Indexed) -> Self::Processed {
        HeadingProcessed {
            stable_id: indexed.stable_id,
            anchor_id: indexed.original_id.clone().unwrap_or_default(),
            toc_text: String::new(),
            in_toc: true,
        }
    }
}

// =============================================================================
// Data Types
// =============================================================================

/// Heading data at Raw phase
#[derive(Debug, Clone, Default)]
pub struct HeadingRaw {
    pub level: u8,
    pub original_id: Option<String>,
}

impl HeadingRaw {
    pub fn new(level: u8) -> Self {
        Self {
            level,
            original_id: None,
        }
    }

    pub fn with_id(level: u8, id: impl Into<String>) -> Self {
        Self {
            level,
            original_id: Some(id.into()),
        }
    }

    /// Parse level from tag name: "h1" → 1
    pub fn level_from_tag(tag: &str) -> u8 {
        tag.chars()
            .last()
            .and_then(|c| c.to_digit(10))
            .unwrap_or(1) as u8
    }
}

/// Heading data at Indexed phase
#[derive(Debug, Clone, Default)]
pub struct HeadingIndexed {
    pub stable_id: StableId,
    pub level: u8,
    pub original_id: Option<String>,
}

impl HasStableId for HeadingIndexed {
    fn stable_id(&self) -> StableId {
        self.stable_id
    }
}

impl HeadingIndexed {
    pub fn is_h1(&self) -> bool {
        self.level == 1
    }

    pub fn is_h2(&self) -> bool {
        self.level == 2
    }

    pub fn is_top_level(&self) -> bool {
        self.level <= 2
    }
}

/// Heading data at Processed phase
#[derive(Debug, Clone, Default)]
pub struct HeadingProcessed {
    pub stable_id: StableId,
    pub anchor_id: String,
    pub toc_text: String,
    pub in_toc: bool,
}

impl HasStableId for HeadingProcessed {
    fn stable_id(&self) -> StableId {
        self.stable_id
    }
}

// =============================================================================
// Serialization
// =============================================================================

#[cfg(feature = "cache")]
mod serialization {
    use super::*;
    use crate::core::SerializableFamily;
    use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

    #[derive(Archive, RkyvDeserialize, RkyvSerialize, Debug, Clone, Default)]
    #[rkyv(crate = rkyv, derive(Debug))]
    pub struct FlatHeading {
        pub level: u8,
        pub original_id: Option<String>,
    }

    impl SerializableFamily for HeadingFamily {
        type Flat = FlatHeading;

        fn to_flat(indexed: &HeadingIndexed) -> FlatHeading {
            FlatHeading {
                level: indexed.level,
                original_id: indexed.original_id.clone(),
            }
        }

        fn from_flat(flat: &FlatHeading, id: StableId) -> HeadingIndexed {
            HeadingIndexed {
                stable_id: id,
                level: flat.level,
                original_id: flat.original_id.clone(),
            }
        }
    }
}

#[cfg(feature = "cache")]
pub use serialization::FlatHeading;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_level_from_tag() {
        assert_eq!(HeadingRaw::level_from_tag("h1"), 1);
        assert_eq!(HeadingRaw::level_from_tag("h6"), 6);
    }

    #[test]
    fn test_heading_family_lifecycle() {
        let raw = HeadingRaw::with_id(2, "my-heading");
        let indexed = HeadingFamily::index(raw, StableId::from_raw(456));

        assert_eq!(indexed.stable_id().as_raw(), 456);
        assert_eq!(indexed.level, 2);
        assert!(indexed.is_h2());

        let processed = HeadingFamily::process(&indexed);
        assert_eq!(processed.anchor_id, "my-heading");
    }
}
