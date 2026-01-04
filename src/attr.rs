//! Attribute and string types for VDOM elements
//!
//! All string types use `CompactString` for efficient storage:
//! - Short strings (≤24 bytes on 64-bit) are stored inline without heap allocation
//! - Longer strings use standard heap allocation

use compact_str::CompactString;
use smallvec::SmallVec;
use std::ops::{Deref, DerefMut};

// =============================================================================
// Type aliases for string types
// =============================================================================

/// Element tag name (e.g., "div", "span", "custom-element")
/// Uses CompactString for efficient storage of typical short tag names.
pub type Tag = CompactString;

/// Text node content
/// Uses CompactString for small text nodes (inline allocation ≤24 bytes).
pub type TextContent = CompactString;

/// Attribute key
pub type AttrKey = CompactString;

/// Attribute value
pub type AttrValue = CompactString;

/// Element attributes as key-value pairs.
/// SmallVec avoids heap allocation for elements with ≤8 attributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attrs(SmallVec<[(AttrKey, AttrValue); 8]>);

impl Attrs {
    /// Create empty attrs
    #[inline]
    pub fn new() -> Self {
        Self(SmallVec::new())
    }

    /// Create from iterator of key-value pairs
    #[inline]
    pub fn from_pairs<K, V, I>(iter: I) -> Self
    where
        K: Into<AttrKey>,
        V: Into<AttrValue>,
        I: IntoIterator<Item = (K, V)>,
    {
        Self(iter.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
    }

    /// Check if spilled to heap
    #[inline]
    pub fn spilled(&self) -> bool {
        self.0.spilled()
    }

    /// Into inner SmallVec
    #[inline]
    pub fn into_inner(self) -> SmallVec<[(AttrKey, AttrValue); 8]> {
        self.0
    }
}

impl Deref for Attrs {
    type Target = [(AttrKey, AttrValue)];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Attrs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for Attrs
where
    K: Into<AttrKey>,
    V: Into<AttrValue>,
{
    fn from(arr: [(K, V); N]) -> Self {
        Self::from_pairs(arr)
    }
}

impl FromIterator<(AttrKey, AttrValue)> for Attrs {
    fn from_iter<I: IntoIterator<Item = (AttrKey, AttrValue)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<'a> IntoIterator for &'a Attrs {
    type Item = &'a (AttrKey, AttrValue);
    type IntoIter = std::slice::Iter<'a, (AttrKey, AttrValue)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut Attrs {
    type Item = &'a mut (AttrKey, AttrValue);
    type IntoIter = std::slice::IterMut<'a, (AttrKey, AttrValue)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for Attrs {
    type Item = (AttrKey, AttrValue);
    type IntoIter = smallvec::IntoIter<[(AttrKey, AttrValue); 8]>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Extend<(AttrKey, AttrValue)> for Attrs {
    fn extend<I: IntoIterator<Item = (AttrKey, AttrValue)>>(&mut self, iter: I) {
        self.0.extend(iter)
    }
}

// =============================================================================
// Attrs methods
// =============================================================================

impl Attrs {
    /// Get attribute value by name
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Check if attribute exists
    pub fn has(&self, name: &str) -> bool {
        self.0.iter().any(|(k, _)| k == name)
    }

    /// Set attribute (insert or update)
    pub fn set(&mut self, name: impl Into<AttrKey>, value: impl Into<AttrValue>) {
        let name = name.into();
        let value = value.into();
        if let Some(attr) = self.0.iter_mut().find(|(k, _)| k == name.as_str()) {
            attr.1 = value;
        } else {
            self.0.push((name, value));
        }
    }

    /// Remove attribute, returning old value
    pub fn remove(&mut self, name: &str) -> Option<AttrValue> {
        self.0.iter().position(|(k, _)| k == name).map(|pos| self.0.remove(pos).1)
    }

    /// Push a key-value pair (always appends, allows duplicates)
    pub fn push(&mut self, kv: (AttrKey, AttrValue)) {
        self.0.push(kv);
    }

    /// Push a key-value pair only if key doesn't already exist
    ///
    /// Returns true if inserted, false if key already exists.
    ///
    /// # Example
    /// ```ignore
    /// attrs.push_uniq("id", "main");  // inserted
    /// attrs.push_uniq("id", "other"); // ignored, returns false
    /// ```
    pub fn push_uniq(&mut self, name: impl Into<AttrKey>, value: impl Into<AttrValue>) -> bool {
        let name = name.into();
        if self.has(name.as_str()) {
            false
        } else {
            self.0.push((name, value.into()));
            true
        }
    }

    /// Set multiple attributes at once
    ///
    /// # Example
    /// ```ignore
    /// attrs.set_many([
    ///     ("data-id", "123"),
    ///     ("data-type", "user"),
    ///     ("aria-label", "User profile"),
    /// ]);
    /// ```
    pub fn set_many<K, V, I>(&mut self, iter: I)
    where
        K: Into<AttrKey>,
        V: Into<AttrValue>,
        I: IntoIterator<Item = (K, V)>,
    {
        for (k, v) in iter {
            self.set(k, v);
        }
    }

    /// Push multiple attributes without checking for duplicates
    ///
    /// More efficient than `set_many` when you know keys are unique.
    pub fn push_many<K, V, I>(&mut self, iter: I)
    where
        K: Into<AttrKey>,
        V: Into<AttrValue>,
        I: IntoIterator<Item = (K, V)>,
    {
        for (k, v) in iter {
            self.0.push((k.into(), v.into()));
        }
    }

    // ============ Data attribute helpers ============

    /// Set a `data-*` attribute
    ///
    /// # Example
    /// ```ignore
    /// attrs.set_data("id", "123");      // data-id="123"
    /// attrs.set_data("user-name", "x"); // data-user-name="x"
    /// ```
    #[inline]
    pub fn set_data(&mut self, suffix: &str, value: impl Into<AttrValue>) {
        self.set(format!("data-{}", suffix), value);
    }

    /// Get a `data-*` attribute value
    #[inline]
    pub fn get_data(&self, suffix: &str) -> Option<&str> {
        self.get(&format!("data-{}", suffix))
    }

    /// Check if a `data-*` attribute exists
    #[inline]
    pub fn has_data(&self, suffix: &str) -> bool {
        self.has(&format!("data-{}", suffix))
    }

    /// Set an `aria-*` attribute
    #[inline]
    pub fn set_aria(&mut self, suffix: &str, value: impl Into<AttrValue>) {
        self.set(format!("aria-{}", suffix), value);
    }

    /// Get all attributes matching a prefix
    ///
    /// # Example
    /// ```ignore
    /// // attrs: data-id="1", data-type="user", class="x"
    /// let data_attrs: Vec<_> = attrs.with_prefix("data-").collect();
    /// // [("data-id", "1"), ("data-type", "user")]
    /// ```
    pub fn with_prefix(&self, prefix: &str) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter()
            .filter(move |(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attrs_operations() {
        let mut attrs = Attrs::new();

        // Set
        attrs.set("id", "main");
        attrs.set("class", "container");
        assert_eq!(attrs.len(), 2);

        // Get
        assert_eq!(attrs.get("id"), Some("main"));
        assert_eq!(attrs.get("class"), Some("container"));
        assert_eq!(attrs.get("href"), None);

        // Has
        assert!(attrs.has("id"));
        assert!(!attrs.has("href"));

        // Update existing
        attrs.set("class", "wrapper");
        assert_eq!(attrs.get("class"), Some("wrapper"));
        assert_eq!(attrs.len(), 2);

        // Remove
        let removed = attrs.remove("id");
        assert_eq!(removed.as_deref(), Some("main"));
        assert!(!attrs.has("id"));
        assert_eq!(attrs.len(), 1);
    }

    #[test]
    fn test_smallvec_inline() {
        let mut attrs = Attrs::new();
        for i in 0..8 {
            attrs.set(format!("attr{}", i), format!("value{}", i));
        }
        assert!(!attrs.spilled()); // Still inline

        attrs.set("attr8", "value8");
        assert!(attrs.spilled()); // Now on heap
    }

    #[test]
    fn test_from_array() {
        let attrs = Attrs::from([("id", "main"), ("class", "box")]);
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs.get("id"), Some("main"));
    }
}
