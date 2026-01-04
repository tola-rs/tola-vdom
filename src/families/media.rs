//! Media family: `<img>`, `<video>`, `<audio>`, etc.

use crate::attr::Attrs;
use crate::core::{Family, HasStableId};
use crate::id::StableId;

// =============================================================================
// MediaFamily
// =============================================================================

/// Media family for image, video, and audio elements.
pub struct MediaFamily;

impl Family for MediaFamily {
    const NAME: &'static str = "media";

    type Raw = MediaRaw;
    type Indexed = MediaIndexed;
    type Processed = MediaProcessed;

    fn identify(tag: &str, _attrs: &Attrs) -> bool {
        matches!(
            tag,
            "img" | "video" | "audio" | "source" | "track" | "picture" | "canvas" | "embed" | "object"
        )
    }

    fn index(raw: Self::Raw, id: StableId) -> Self::Indexed {
        let is_svg_image = raw
            .src
            .as_ref()
            .map(|s| s.to_lowercase().ends_with(".svg"))
            .unwrap_or(false);

        MediaIndexed {
            stable_id: id,
            src: raw.src,
            is_svg_image,
        }
    }

    fn process(indexed: &Self::Indexed) -> Self::Processed {
        MediaProcessed {
            stable_id: indexed.stable_id,
            resolved_src: indexed.src.clone(),
            width: None,
            height: None,
            lazy_load: true,
        }
    }
}

// =============================================================================
// Data Types
// =============================================================================

/// Media data at Raw phase
#[derive(Debug, Clone, Default)]
pub struct MediaRaw {
    pub src: Option<String>,
}

impl MediaRaw {
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            src: Some(src.into()),
        }
    }
}

/// Media data at Indexed phase
#[derive(Debug, Clone, Default)]
pub struct MediaIndexed {
    pub stable_id: StableId,
    pub src: Option<String>,
    pub is_svg_image: bool,
}

impl HasStableId for MediaIndexed {
    fn stable_id(&self) -> StableId {
        self.stable_id
    }
}

impl MediaIndexed {
    /// Infer media type from file extension
    pub fn media_type(&self) -> MediaType {
        let src = match &self.src {
            Some(s) => s.to_lowercase(),
            None => return MediaType::Unknown,
        };

        if src.ends_with(".svg") {
            MediaType::Svg
        } else if src.ends_with(".png")
            || src.ends_with(".jpg")
            || src.ends_with(".jpeg")
            || src.ends_with(".gif")
            || src.ends_with(".webp")
            || src.ends_with(".avif")
        {
            MediaType::Image
        } else if src.ends_with(".mp4") || src.ends_with(".webm") || src.ends_with(".ogg") {
            MediaType::Video
        } else if src.ends_with(".mp3") || src.ends_with(".wav") || src.ends_with(".flac") {
            MediaType::Audio
        } else {
            MediaType::Unknown
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(self.media_type(), MediaType::Image | MediaType::Svg)
    }

    pub fn is_video(&self) -> bool {
        matches!(self.media_type(), MediaType::Video)
    }
}

/// Media type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Svg,
    Video,
    Audio,
    Unknown,
}

/// Media data at Processed phase
#[derive(Debug, Clone, Default)]
pub struct MediaProcessed {
    pub stable_id: StableId,
    pub resolved_src: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub lazy_load: bool,
}

impl HasStableId for MediaProcessed {
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
    pub struct FlatMedia {
        pub src: Option<String>,
        pub is_svg_image: bool,
    }

    impl SerializableFamily for MediaFamily {
        type Flat = FlatMedia;

        fn to_flat(indexed: &MediaIndexed) -> FlatMedia {
            FlatMedia {
                src: indexed.src.clone(),
                is_svg_image: indexed.is_svg_image,
            }
        }

        fn from_flat(flat: &FlatMedia, id: StableId) -> MediaIndexed {
            MediaIndexed {
                stable_id: id,
                src: flat.src.clone(),
                is_svg_image: flat.is_svg_image,
            }
        }
    }
}

#[cfg(feature = "cache")]
pub use serialization::FlatMedia;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_type_detection() {
        let indexed = MediaIndexed {
            src: Some("image.png".into()),
            ..Default::default()
        };
        assert_eq!(indexed.media_type(), MediaType::Image);

        let indexed = MediaIndexed {
            src: Some("video.mp4".into()),
            ..Default::default()
        };
        assert_eq!(indexed.media_type(), MediaType::Video);
    }

    #[test]
    fn test_media_family_lifecycle() {
        let raw = MediaRaw::new("photo.jpg");
        let indexed = MediaFamily::index(raw, StableId::from_raw(111));

        assert_eq!(indexed.stable_id().as_raw(), 111);
        assert!(!indexed.is_svg_image);
        assert!(indexed.is_image());
    }
}
