//! SVG family: `<svg>`, `<path>`, `<circle>`, etc.

use crate::attr::Attrs;
use crate::core::{Family, HasStableId};
use crate::id::StableId;

// =============================================================================
// SvgFamily
// =============================================================================

/// SVG family for SVG container and shape elements.
pub struct SvgFamily;

impl Family for SvgFamily {
    const NAME: &'static str = "svg";

    type Raw = SvgRaw;
    type Indexed = SvgIndexed;
    type Processed = SvgProcessed;

    fn identify(tag: &str, _attrs: &Attrs) -> bool {
        is_svg_tag(tag)
    }

    fn index(raw: Self::Raw, id: StableId) -> Self::Indexed {
        SvgIndexed {
            stable_id: id,
            is_root: raw.is_root,
            viewbox: raw.viewbox,
            dimensions: raw.dimensions,
        }
    }

    fn process(indexed: &Self::Indexed) -> Self::Processed {
        SvgProcessed {
            stable_id: indexed.stable_id,
            optimized: false,
            original_bytes: 0,
            optimized_bytes: 0,
        }
    }
}

/// Check if a tag is an SVG element
pub fn is_svg_tag(tag: &str) -> bool {
    matches!(
        tag,
        // Container elements
        "svg" | "g" | "defs" | "symbol" | "use" | "switch"
        // Shape elements
        | "path" | "circle" | "rect" | "line" | "polyline" | "polygon" | "ellipse"
        // Text elements
        | "text" | "tspan" | "textPath"
        // Gradient elements
        | "linearGradient" | "radialGradient" | "stop"
        // Clipping and masking
        | "clipPath" | "mask" | "pattern"
        // Filter elements
        | "filter" | "feBlend" | "feColorMatrix" | "feComponentTransfer"
        | "feComposite" | "feConvolveMatrix" | "feDiffuseLighting"
        | "feDisplacementMap" | "feDistantLight" | "feDropShadow"
        | "feFlood" | "feFuncR" | "feFuncG" | "feFuncB" | "feFuncA"
        | "feGaussianBlur" | "feImage" | "feMerge" | "feMergeNode"
        | "feMorphology" | "feOffset" | "fePointLight" | "feSpecularLighting"
        | "feSpotLight" | "feTile" | "feTurbulence"
        // Animation elements
        | "animate" | "animateMotion" | "animateTransform" | "set" | "mpath"
        // Other SVG elements
        | "image" | "foreignObject" | "marker" | "metadata"
        | "view" | "cursor" | "font" | "glyph" | "hkern" | "vkern"
        | "font-face" | "font-face-src" | "font-face-uri" | "font-face-format"
        | "font-face-name" | "missing-glyph"
    )
}

// =============================================================================
// Data Types
// =============================================================================

/// SVG data at Raw phase
#[derive(Debug, Clone, Default)]
pub struct SvgRaw {
    pub is_root: bool,
    pub viewbox: Option<String>,
    pub dimensions: Option<(f32, f32)>,
}

impl SvgRaw {
    pub fn root() -> Self {
        Self {
            is_root: true,
            ..Default::default()
        }
    }

    pub fn with_viewbox(viewbox: impl Into<String>) -> Self {
        Self {
            viewbox: Some(viewbox.into()),
            ..Default::default()
        }
    }
}

/// SVG data at Indexed phase
#[derive(Debug, Clone, Default)]
pub struct SvgIndexed {
    pub stable_id: StableId,
    pub is_root: bool,
    pub viewbox: Option<String>,
    pub dimensions: Option<(f32, f32)>,
}

impl HasStableId for SvgIndexed {
    fn stable_id(&self) -> StableId {
        self.stable_id
    }
}

impl SvgIndexed {
    /// Parse viewBox string into (min_x, min_y, width, height)
    pub fn parse_viewbox(&self) -> Option<(f32, f32, f32, f32)> {
        let vb = self.viewbox.as_ref()?;
        let parts: Vec<f32> = vb.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        if parts.len() == 4 {
            Some((parts[0], parts[1], parts[2], parts[3]))
        } else {
            None
        }
    }

    /// Get effective dimensions
    pub fn effective_dimensions(&self) -> Option<(f32, f32)> {
        self.dimensions.or_else(|| {
            self.parse_viewbox().map(|(_, _, w, h)| (w, h))
        })
    }
}

/// SVG data at Processed phase
#[derive(Debug, Clone, Default)]
pub struct SvgProcessed {
    pub stable_id: StableId,
    pub optimized: bool,
    pub original_bytes: usize,
    pub optimized_bytes: usize,
}

impl HasStableId for SvgProcessed {
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
    pub struct FlatSvg {
        pub is_root: bool,
        pub viewbox: Option<String>,
        pub dimensions: Option<(f32, f32)>,
    }

    impl SerializableFamily for SvgFamily {
        type Flat = FlatSvg;

        fn to_flat(indexed: &SvgIndexed) -> FlatSvg {
            FlatSvg {
                is_root: indexed.is_root,
                viewbox: indexed.viewbox.clone(),
                dimensions: indexed.dimensions,
            }
        }

        fn from_flat(flat: &FlatSvg, id: StableId) -> SvgIndexed {
            SvgIndexed {
                stable_id: id,
                is_root: flat.is_root,
                viewbox: flat.viewbox.clone(),
                dimensions: flat.dimensions,
            }
        }
    }
}

#[cfg(feature = "cache")]
pub use serialization::FlatSvg;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_svg_tag() {
        assert!(is_svg_tag("svg"));
        assert!(is_svg_tag("path"));
        assert!(is_svg_tag("circle"));
        assert!(!is_svg_tag("div"));
        assert!(!is_svg_tag("a"));
    }

    #[test]
    fn test_svg_family_lifecycle() {
        let raw = SvgRaw::root();
        let indexed = SvgFamily::index(raw, StableId::from_raw(789));

        assert_eq!(indexed.stable_id().as_raw(), 789);
        assert!(indexed.is_root);
    }
}
