//! Link family: `<a>`, elements with `href`/`src` attributes.

use crate::attr::Attrs;
use crate::core::{Family, HasStableId};
use crate::id::StableId;

// =============================================================================
// LinkFamily
// =============================================================================

/// Link family for anchor and href-bearing elements.
pub struct LinkFamily;

impl Family for LinkFamily {
    const NAME: &'static str = "link";

    type Raw = LinkRaw;
    type Indexed = LinkIndexed;
    type Processed = LinkProcessed;

    fn identify(tag: &str, attrs: &Attrs) -> bool {
        tag == "a" || tag == "area" || attrs.get("href").is_some() || attrs.get("src").is_some()
    }

    fn index(raw: Self::Raw, id: StableId) -> Self::Indexed {
        let link_type = raw
            .href
            .as_deref()
            .map(LinkType::from_href)
            .unwrap_or_default();

        LinkIndexed {
            stable_id: id,
            href: raw.href,
            link_type,
        }
    }

    fn process(indexed: &Self::Indexed) -> Self::Processed {
        LinkProcessed {
            stable_id: indexed.stable_id,
            resolved_url: indexed.href.clone(),
            is_external: indexed.link_type == LinkType::External,
            is_broken: false,
        }
    }
}

// =============================================================================
// Data Types
// =============================================================================

/// Link data at Raw phase
#[derive(Debug, Clone, Default)]
pub struct LinkRaw {
    pub href: Option<String>,
}

impl LinkRaw {
    pub fn new(href: impl Into<String>) -> Self {
        Self {
            href: Some(href.into()),
        }
    }
}

/// Link data at Indexed phase
#[derive(Debug, Clone, Default)]
pub struct LinkIndexed {
    pub stable_id: StableId,
    pub href: Option<String>,
    pub link_type: LinkType,
}

impl HasStableId for LinkIndexed {
    fn stable_id(&self) -> StableId {
        self.stable_id
    }
}

/// Link data at Processed phase
#[derive(Debug, Clone, Default)]
pub struct LinkProcessed {
    pub stable_id: StableId,
    pub resolved_url: Option<String>,
    pub is_external: bool,
    pub is_broken: bool,
}

impl HasStableId for LinkProcessed {
    fn stable_id(&self) -> StableId {
        self.stable_id
    }
}

// =============================================================================
// LinkType
// =============================================================================

/// Link type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkType {
    #[default]
    None,
    Absolute,  // /path
    Relative,  // ./file
    Fragment,  // #anchor
    External,  // https://...
    Email,     // mailto:...
}

impl LinkType {
    /// Infer link type from href string
    pub fn from_href(href: &str) -> Self {
        let href = href.trim();
        if href.is_empty() {
            return Self::None;
        }
        if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("//") {
            Self::External
        } else if href.starts_with("mailto:") {
            Self::Email
        } else if href.starts_with('/') {
            Self::Absolute
        } else if href.starts_with('#') {
            Self::Fragment
        } else {
            Self::Relative
        }
    }

    pub fn is_external(&self) -> bool {
        matches!(self, Self::External)
    }

    pub fn is_internal(&self) -> bool {
        !matches!(self, Self::External | Self::None | Self::Email)
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
    pub struct FlatLink {
        pub link_type: u8,
        pub href: Option<String>,
    }

    impl SerializableFamily for LinkFamily {
        type Flat = FlatLink;

        fn to_flat(indexed: &LinkIndexed) -> FlatLink {
            FlatLink {
                link_type: match indexed.link_type {
                    LinkType::None => 0,
                    LinkType::Absolute => 1,
                    LinkType::Relative => 2,
                    LinkType::Fragment => 3,
                    LinkType::External => 4,
                    LinkType::Email => 5,
                },
                href: indexed.href.clone(),
            }
        }

        fn from_flat(flat: &FlatLink, id: StableId) -> LinkIndexed {
            LinkIndexed {
                stable_id: id,
                href: flat.href.clone(),
                link_type: match flat.link_type {
                    0 => LinkType::None,
                    1 => LinkType::Absolute,
                    2 => LinkType::Relative,
                    3 => LinkType::Fragment,
                    4 => LinkType::External,
                    _ => LinkType::Email,
                },
            }
        }
    }
}

#[cfg(feature = "cache")]
pub use serialization::FlatLink;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_type() {
        assert_eq!(LinkType::from_href("https://example.com"), LinkType::External);
        assert_eq!(LinkType::from_href("/about"), LinkType::Absolute);
        assert_eq!(LinkType::from_href("#section"), LinkType::Fragment);
        assert_eq!(LinkType::from_href("./file"), LinkType::Relative);
        assert_eq!(LinkType::from_href("mailto:a@b.com"), LinkType::Email);
    }

    #[test]
    fn test_link_family_lifecycle() {
        let raw = LinkRaw::new("https://example.com");
        let indexed = LinkFamily::index(raw, StableId::from_raw(123));

        assert_eq!(indexed.stable_id().as_raw(), 123);
        assert_eq!(indexed.link_type, LinkType::External);

        let processed = LinkFamily::process(&indexed);
        assert!(processed.is_external);
        assert_eq!(processed.stable_id().as_raw(), 123);
    }
}
