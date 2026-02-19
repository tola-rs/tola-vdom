//! Serialization support for VDOM documents.
//!
//! This module provides serialization/deserialization for `Document<Indexed>`
//! using a flat serialization format to avoid recursive type issues with rkyv.

use crate::core::{HasStableId, PhaseExt};
use crate::node::Document;

/// Current schema version for cache validation.
/// Increment this when making breaking changes to SerDocument structure.
pub const SCHEMA_VERSION: u32 = 1;

/// Magic bytes for tola-vdom cache files.
const MAGIC: [u8; 4] = *b"TOLA";

#[cfg(feature = "cache")]
mod concrete {
    //! Concrete serialization types using a flat structure.
    //!
    //! Instead of recursive Element/Node types, we use indices to reference
    //! children, which allows rkyv to serialize without recursion issues.

    use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

    /// Serializable document format with flat node storage.
    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
    pub struct SerDocument {
        /// Magic bytes for validation
        pub magic: [u8; 4],
        /// Schema version for compatibility checking
        pub schema_version: u32,
        /// All elements in the document (flattened)
        pub elements: Vec<SerElement>,
        /// All text nodes in the document (flattened)
        pub texts: Vec<SerText>,
        /// Root element index (always 0)
        pub root_idx: u32,
        /// Document metadata
        pub meta: SerDocMeta,
    }

    /// Serializable document metadata.
    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Default)]
    pub struct SerDocMeta {
        pub source_path: Option<String>,
        pub node_count: u32,
    }

    /// Serializable element using indices instead of nested children.
    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
    pub struct SerElement {
        pub tag: String,
        pub attrs: Vec<(String, String)>,
        /// Children as (is_element, index) pairs
        /// is_element=true means index into elements[], false means index into texts[]
        pub children: Vec<(bool, u32)>,
        pub ext: SerExt,
    }

    /// Serializable text node.
    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
    pub struct SerText {
        pub content: String,
        pub is_raw: bool,
        pub stable_id: u64,
    }

    /// Serializable element extension.
    #[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
    pub struct SerExt {
        pub stable_id: u64,
        pub family_name: String,
    }
}

#[cfg(feature = "cache")]
pub use concrete::{ArchivedSerDocMeta, ArchivedSerExt, SerDocMeta, SerExt};

#[cfg(feature = "cache")]
use concrete::*;

#[cfg(feature = "cache")]
use rkyv::rancor::Error as RkyvError;

/// Serialize a document to bytes.
#[cfg(feature = "cache")]
pub fn to_bytes<P>(doc: &Document<P>) -> Result<Vec<u8>, String>
where
    P: PhaseExt,
    P::Ext: HasStableId + SerializableExt,
    P::TextExt: SerializableTextExt,
    P::DocExt: SerializableDocExt,
{
    let ser_doc = to_serializable(doc);
    rkyv::to_bytes::<RkyvError>(&ser_doc)
        .map(|bytes| bytes.to_vec())
        .map_err(|e| format!("Serialization failed: {}", e))
}

/// Deserialize bytes to a document.
///
/// # Errors
///
/// Returns an error if:
/// - Magic bytes don't match (not a valid tola-vdom cache)
/// - Schema version is incompatible
/// - Archive data is corrupted
#[cfg(feature = "cache")]
pub fn from_bytes<P>(bytes: &[u8]) -> Result<Document<P>, String>
where
    P: PhaseExt,
    P::Ext: HasStableId + DeserializableExt + Default,
    P::TextExt: DeserializableTextExt + Default,
    P::DocExt: DeserializableDocExt + Default,
{
    let archived = rkyv::access::<ArchivedSerDocument, RkyvError>(bytes)
        .map_err(|e| format!("Failed to access archived data: {}", e))?;

    // Validate magic bytes
    let magic: [u8; 4] = archived.magic;
    if magic != MAGIC {
        return Err(format!(
            "Invalid cache file: expected magic {:?}, got {:?}",
            MAGIC, magic
        ));
    }

    // Validate schema version
    let version: u32 = archived.schema_version.into();
    if version != SCHEMA_VERSION {
        return Err(format!(
            "Incompatible cache version: expected {}, got {} (cache file may be from a different tola-vdom version)",
            SCHEMA_VERSION, version
        ));
    }

    from_serializable::<P>(archived)
}

/// Alias for backward compatibility
#[cfg(feature = "cache")]
pub fn from_bytes_to_indexed<P>(bytes: &[u8]) -> Result<Document<P>, String>
where
    P: PhaseExt,
    P::Ext: HasStableId + DeserializableExt + Default,
    P::TextExt: DeserializableTextExt + Default,
    P::DocExt: DeserializableDocExt + Default,
{
    from_bytes(bytes)
}

// =============================================================================
// Conversion traits
// =============================================================================

/// Trait for serializing element extensions.
#[cfg(feature = "cache")]
pub trait SerializableExt {
    fn to_ser_ext(&self) -> SerExt;
}

/// Trait for deserializing element extensions.
#[cfg(feature = "cache")]
pub trait DeserializableExt: Sized {
    fn from_ser_ext(ext: &ArchivedSerExt) -> Result<Self, String>;
}

/// Trait for serializing text extensions.
#[cfg(feature = "cache")]
pub trait SerializableTextExt {
    fn stable_id(&self) -> u64;
}

/// Trait for deserializing text extensions.
#[cfg(feature = "cache")]
pub trait DeserializableTextExt: Sized {
    fn from_stable_id(id: u64) -> Self;
}

/// Trait for serializing document metadata.
#[cfg(feature = "cache")]
pub trait SerializableDocExt {
    fn to_ser_doc_meta(&self) -> SerDocMeta;
}

/// Trait for deserializing document metadata.
#[cfg(feature = "cache")]
pub trait DeserializableDocExt: Sized {
    fn from_ser_doc_meta(meta: &ArchivedSerDocMeta) -> Result<Self, String>;
}

// Stub traits for non-cache builds
#[cfg(not(feature = "cache"))]
pub trait SerializableExt {}
#[cfg(not(feature = "cache"))]
pub trait DeserializableExt: Sized {}
#[cfg(not(feature = "cache"))]
pub trait SerializableTextExt {}
#[cfg(not(feature = "cache"))]
pub trait DeserializableTextExt: Sized {}
#[cfg(not(feature = "cache"))]
pub trait SerializableDocExt {}
#[cfg(not(feature = "cache"))]
pub trait DeserializableDocExt: Sized {}

// =============================================================================
// Conversion functions
// =============================================================================

#[cfg(feature = "cache")]
fn to_serializable<P>(doc: &Document<P>) -> SerDocument
where
    P: PhaseExt,
    P::Ext: SerializableExt,
    P::TextExt: SerializableTextExt,
    P::DocExt: SerializableDocExt,
{
    let mut elements = Vec::new();
    let mut texts = Vec::new();

    // Recursively flatten the tree
    flatten_element(&doc.root, &mut elements, &mut texts);

    SerDocument {
        magic: MAGIC,
        schema_version: SCHEMA_VERSION,
        elements,
        texts,
        root_idx: 0,
        meta: doc.meta.to_ser_doc_meta(),
    }
}

#[cfg(feature = "cache")]
fn flatten_element<P>(
    elem: &crate::node::Element<P>,
    elements: &mut Vec<SerElement>,
    texts: &mut Vec<SerText>,
) -> u32
where
    P: PhaseExt,
    P::Ext: SerializableExt,
    P::TextExt: SerializableTextExt,
{
    // Reserve our index
    let elem_idx = elements.len() as u32;

    // Create placeholder element (children will be filled in)
    elements.push(SerElement {
        tag: elem.tag.to_string(),
        attrs: elem.attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        children: Vec::new(),
        ext: elem.ext.to_ser_ext(),
    });

    // Process children
    let mut children = Vec::new();
    for child in &elem.children {
        match child {
            crate::node::Node::Element(child_elem) => {
                let child_idx = flatten_element(child_elem, elements, texts);
                children.push((true, child_idx));
            }
            crate::node::Node::Text(text) => {
                let text_idx = texts.len() as u32;
                texts.push(SerText {
                    content: text.content.to_string(),
                    is_raw: text.is_raw(),
                    stable_id: text.ext.stable_id(),
                });
                children.push((false, text_idx));
            }
        }
    }

    // Update children
    elements[elem_idx as usize].children = children;

    elem_idx
}

#[cfg(feature = "cache")]
fn from_serializable<P>(archived: &ArchivedSerDocument) -> Result<Document<P>, String>
where
    P: PhaseExt,
    P::Ext: DeserializableExt + Default,
    P::TextExt: DeserializableTextExt + Default,
    P::DocExt: DeserializableDocExt + Default,
{
    let root_idx: u32 = archived.root_idx.into();
    let root = unflatten_element::<P>(root_idx as usize, &archived.elements, &archived.texts)?;
    let meta = P::DocExt::from_ser_doc_meta(&archived.meta)?;
    Ok(Document::with_meta(root, meta))
}

#[cfg(feature = "cache")]
fn unflatten_element<P>(
    idx: usize,
    elements: &rkyv::vec::ArchivedVec<ArchivedSerElement>,
    texts: &rkyv::vec::ArchivedVec<ArchivedSerText>,
) -> Result<crate::node::Element<P>, String>
where
    P: PhaseExt,
    P::Ext: DeserializableExt + Default,
    P::TextExt: DeserializableTextExt + Default,
{
    use crate::attr::{AttrKey, AttrValue, Attrs};
    use crate::node::{Element, Node, Text, TextKind};

    let archived = &elements[idx];

    let tag = archived.tag.as_str();
    let attrs = Attrs::from_pairs(archived.attrs.iter().map(|pair| {
        (AttrKey::from(pair.0.as_str()), AttrValue::from(pair.1.as_str()))
    }));
    let ext = P::Ext::from_ser_ext(&archived.ext)?;

    let mut elem = Element::with_ext(tag, ext);
    elem.attrs = attrs;

    for pair in archived.children.iter() {
        let is_element: bool = pair.0;
        let child_idx: u32 = pair.1.into();
        let child_idx = child_idx as usize;

        if is_element {
            let child_elem = unflatten_element::<P>(child_idx, elements, texts)?;
            elem.children.push(Node::Element(Box::new(child_elem)));
        } else {
            let text = &texts[child_idx];
            let ext = P::TextExt::from_stable_id(text.stable_id.into());
            let mut t = Text::with_ext(text.content.as_str(), ext);
            if text.is_raw {
                t.kind = TextKind::Raw;
            }
            elem.children.push(Node::Text(t));
        }
    }

    Ok(elem)
}

// =============================================================================
// Non-cache stubs
// =============================================================================

#[cfg(not(feature = "cache"))]
pub fn to_bytes<P>(_doc: &Document<P>) -> Result<Vec<u8>, String>
where
    P: PhaseExt,
    P::Ext: HasStableId,
{
    Err("Serialization requires 'cache' feature".to_string())
}

#[cfg(not(feature = "cache"))]
pub fn from_bytes<P>(_bytes: &[u8]) -> Result<Document<P>, String>
where
    P: PhaseExt,
    P::Ext: HasStableId + Default,
{
    Err("Deserialization requires 'cache' feature".to_string())
}

#[cfg(not(feature = "cache"))]
pub fn from_bytes_to_indexed<P>(_bytes: &[u8]) -> Result<Document<P>, String>
where
    P: PhaseExt,
    P::Ext: HasStableId + Default,
{
    Err("Deserialization requires 'cache' feature".to_string())
}
