//! Document type for the new PhaseExt-based system.

use crate::core::PhaseExt;

use super::{Element, Node};

/// Root document container.
#[derive(Debug, Clone)]
pub struct Document<P: PhaseExt> {
    /// Root element
    pub root: Element<P>,
    /// Phase-specific metadata
    pub meta: P::DocExt,
}

impl<P: PhaseExt> Document<P> {
    /// Create document with root element.
    pub fn new(root: Element<P>) -> Self {
        Self {
            root,
            meta: P::DocExt::default(),
        }
    }

    /// Create document with explicit metadata.
    pub fn with_meta(root: Element<P>, meta: P::DocExt) -> Self {
        Self { root, meta }
    }

    /// Get phase name.
    pub fn phase_name(&self) -> &'static str {
        P::NAME
    }

    // -------------------------------------------------------------------------
    // Query API
    // -------------------------------------------------------------------------

    /// Find first element matching predicate (DFS).
    pub fn find<F>(&self, pred: F) -> Option<&Element<P>>
    where
        F: Fn(&Element<P>) -> bool,
    {
        Self::find_in(&self.root, &pred)
    }

    fn find_in<'a, F>(elem: &'a Element<P>, pred: &F) -> Option<&'a Element<P>>
    where
        F: Fn(&Element<P>) -> bool,
    {
        if pred(elem) {
            return Some(elem);
        }
        for child in &elem.children {
            if let Some(e) = child.as_element() {
                if let Some(found) = Self::find_in(e, pred) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Find first element matching predicate (mutable).
    pub fn find_mut<F>(&mut self, pred: F) -> Option<&mut Element<P>>
    where
        F: Fn(&Element<P>) -> bool + Copy,
    {
        Self::find_in_mut(&mut self.root, pred)
    }

    fn find_in_mut<F>(elem: &mut Element<P>, pred: F) -> Option<&mut Element<P>>
    where
        F: Fn(&Element<P>) -> bool + Copy,
    {
        if pred(elem) {
            return Some(elem);
        }
        for child in &mut elem.children {
            if let Some(e) = child.as_element_mut() {
                if let Some(found) = Self::find_in_mut(e, pred) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Find all elements matching predicate.
    pub fn find_all<F>(&self, pred: F) -> Vec<&Element<P>>
    where
        F: Fn(&Element<P>) -> bool,
    {
        let mut results = Vec::new();
        Self::collect(&self.root, &pred, &mut results);
        results
    }

    fn collect<'a, F>(elem: &'a Element<P>, pred: &F, out: &mut Vec<&'a Element<P>>)
    where
        F: Fn(&Element<P>) -> bool,
    {
        if pred(elem) {
            out.push(elem);
        }
        for child in &elem.children {
            if let Some(e) = child.as_element() {
                Self::collect(e, pred, out);
            }
        }
    }

    /// Check if any element matches.
    pub fn any<F>(&self, pred: F) -> bool
    where
        F: Fn(&Element<P>) -> bool,
    {
        self.find(pred).is_some()
    }

    /// Count total elements.
    pub fn element_count(&self) -> usize {
        Self::count(&self.root)
    }

    fn count(elem: &Element<P>) -> usize {
        let mut n = 1;
        for child in &elem.children {
            if let Some(e) = child.as_element() {
                n += Self::count(e);
            }
        }
        n
    }

    /// Iterate all elements (DFS).
    pub fn elements(&self) -> ElementIter<'_, P> {
        ElementIter::new(&self.root)
    }

    // -------------------------------------------------------------------------
    // Traversal
    // -------------------------------------------------------------------------

    /// Visit each element with a closure.
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&Element<P>),
    {
        Self::visit(&self.root, &mut f);
    }

    fn visit<F>(elem: &Element<P>, f: &mut F)
    where
        F: FnMut(&Element<P>),
    {
        f(elem);
        for child in &elem.children {
            if let Some(e) = child.as_element() {
                Self::visit(e, f);
            }
        }
    }

    /// Visit each element mutably.
    pub fn for_each_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Element<P>),
    {
        Self::visit_mut(&mut self.root, &mut f);
    }

    fn visit_mut<F>(elem: &mut Element<P>, f: &mut F)
    where
        F: FnMut(&mut Element<P>),
    {
        f(elem);
        for child in &mut elem.children {
            if let Some(e) = child.as_element_mut() {
                Self::visit_mut(e, f);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Family-based traversal
    // -------------------------------------------------------------------------

    /// Find all elements of a specific family (type-safe).
    ///
    /// Uses the `ExtractFamily` trait for compile-time type checking.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let links = doc.find_by::<TolaSite::FamilyKind::Link>();
    /// ```
    pub fn find_by<F: crate::core::Family>(&self) -> Vec<&Element<P>>
    where
        P::Ext: crate::core::ExtractFamily<F>,
    {
        self.find_all(|elem| crate::core::ExtractFamily::<F>::get(&elem.ext).is_some())
    }

    /// Modify elements of a specific family (type-safe).
    ///
    /// Uses the `ExtractFamily` trait for compile-time type checking.
    ///
    /// # Example
    ///
    /// ```ignore
    /// doc.modify_by::<TolaSite::FamilyKind::Link, _>(|elem| {
    ///     // modify link elements
    /// });
    /// ```
    pub fn modify_by<F: crate::core::Family, Func>(&mut self, mut f: Func)
    where
        P::Ext: crate::core::ExtractFamily<F>,
        Func: FnMut(&mut Element<P>),
    {
        self.for_each_mut(|elem| {
            if crate::core::ExtractFamily::<F>::get(&elem.ext).is_some() {
                f(elem);
            }
        });
    }
}

// =============================================================================
// ElementIter
// =============================================================================

/// DFS iterator over elements.
pub struct ElementIter<'a, P: PhaseExt> {
    stack: Vec<&'a Element<P>>,
}

impl<'a, P: PhaseExt> ElementIter<'a, P> {
    fn new(root: &'a Element<P>) -> Self {
        Self { stack: vec![root] }
    }
}

impl<'a, P: PhaseExt> Iterator for ElementIter<'a, P> {
    type Item = &'a Element<P>;

    fn next(&mut self) -> Option<Self::Item> {
        let elem = self.stack.pop()?;
        // Push children in reverse for correct DFS order
        for child in elem.children.iter().rev() {
            if let Some(e) = child.as_element() {
                self.stack.push(e);
            }
        }
        Some(elem)
    }
}

// =============================================================================
// Stats
// =============================================================================

/// Document statistics.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub elements: usize,
    pub text_nodes: usize,
    pub max_depth: usize,
}

impl<P: PhaseExt> Document<P> {
    /// Calculate document statistics.
    pub fn stats(&self) -> Stats {
        let mut stats = Stats::default();
        Self::calc_stats(&self.root, 1, &mut stats);
        stats
    }

    fn calc_stats(elem: &Element<P>, depth: usize, stats: &mut Stats) {
        stats.elements += 1;
        stats.max_depth = stats.max_depth.max(depth);
        for child in &elem.children {
            match child {
                Node::Element(e) => Self::calc_stats(e, depth + 1, stats),
                Node::Text(_) => stats.text_nodes += 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests will use macro-generated phases
}
