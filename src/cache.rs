//! VDOM Cache types for hot reload.
//!
//! Provides shared cache for indexed documents.

use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::core::PhaseExt;
use crate::node::Document;

// =============================================================================
// Cache Key
// =============================================================================

/// Cache key for URL-based lookup.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey(Arc<str>);

impl CacheKey {
    /// Create a new cache key from a URL path.
    pub fn new(url: &str) -> Self {
        Self(Arc::from(url))
    }

    /// Get the URL path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Cache Entry
// =============================================================================

/// A cached VDOM document with version tracking.
#[derive(Debug, Clone)]
pub struct CacheEntry<P: PhaseExt> {
    /// The cached document.
    pub doc: Document<P>,
    /// Version number for change detection.
    pub version: u64,
}

impl<P: PhaseExt> CacheEntry<P> {
    /// Create a new cache entry with version 0.
    pub fn new(doc: Document<P>) -> Self {
        Self { doc, version: 0 }
    }

    /// Create a new cache entry with default version.
    pub fn with_default_version(doc: Document<P>) -> Self {
        Self::new(doc)
    }

    /// Create a new cache entry with a specific version.
    pub fn with_version(doc: Document<P>, version: u64) -> Self {
        Self { doc, version }
    }

    /// Increment the version and update the document.
    pub fn update(&mut self, doc: Document<P>) {
        self.doc = doc;
        self.version += 1;
    }
}

// =============================================================================
// VDOM Cache
// =============================================================================

/// Non-thread-safe VDOM cache.
pub type VdomCache<P> = FxHashMap<CacheKey, CacheEntry<P>>;

/// Thread-safe shared VDOM cache.
///
/// Uses `parking_lot::RwLock` for better performance under contention.
#[derive(Debug)]
pub struct SharedVdomCache<P: PhaseExt> {
    inner: Arc<RwLock<VdomCache<P>>>,
}

impl<P: PhaseExt> Clone for SharedVdomCache<P> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<P: PhaseExt> Default for SharedVdomCache<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: PhaseExt> SharedVdomCache<P> {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(FxHashMap::default())),
        }
    }

    /// Execute a closure with read access to the cache.
    pub fn with_read<R>(&self, f: impl FnOnce(&VdomCache<P>) -> R) -> R {
        let guard = self.inner.read();
        f(&guard)
    }

    /// Execute a closure with write access to the cache.
    pub fn with_write<R>(&self, f: impl FnOnce(&mut VdomCache<P>) -> R) -> R {
        let mut guard = self.inner.write();
        f(&mut guard)
    }

    /// Get a clone of a cached entry.
    pub fn get(&self, key: &CacheKey) -> Option<CacheEntry<P>>
    where
        P::Ext: Clone,
        P::DocExt: Clone,
        P::TextExt: Clone,
    {
        self.with_read(|c| c.get(key).cloned())
    }

    /// Insert or update a cache entry.
    pub fn insert(&self, key: CacheKey, entry: CacheEntry<P>) {
        self.with_write(|c| {
            c.insert(key, entry);
        });
    }

    /// Remove an entry from the cache.
    pub fn remove(&self, key: &CacheKey) -> Option<CacheEntry<P>> {
        self.with_write(|c| c.remove(key))
    }

    /// Check if the cache contains a key.
    pub fn contains(&self, key: &CacheKey) -> bool {
        self.with_read(|c| c.contains_key(key))
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.with_read(|c| c.len())
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        self.with_write(|c| c.clear());
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(all(test, feature = "macros"))]
mod tests {
    use super::*;
    use crate::families::{HeadingFamily, LinkFamily, MediaFamily, SvgFamily};
    use crate::node::Element;
    use crate::vdom;

    #[vdom::families]
    pub struct CacheTestSite {
        link: LinkFamily,
        heading: HeadingFamily,
        svg: SvgFamily,
        media: MediaFamily,
    }

    #[test]
    fn test_cache_key() {
        let key = CacheKey::new("/blog/post");
        assert_eq!(key.as_str(), "/blog/post");
    }

    #[test]
    fn test_shared_cache() {
        let cache: SharedVdomCache<CacheTestSite::Indexed> = SharedVdomCache::new();
        let elem = Element::new("div");
        let doc = Document::new(elem);
        let entry = CacheEntry::new(doc);

        cache.insert(CacheKey::new("/test"), entry);
        assert!(cache.contains(&CacheKey::new("/test")));
        assert_eq!(cache.len(), 1);

        cache.remove(&CacheKey::new("/test"));
        assert!(!cache.contains(&CacheKey::new("/test")));
    }
}
