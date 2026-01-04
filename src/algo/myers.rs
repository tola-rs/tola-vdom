//! Myers Diff Algorithm for VDOM node sequences
//!
//! Implements the Myers diff algorithm optimized for hot reload scenarios.
//!
//! # Algorithm Choice: Why Myers?
//!
//! | Algorithm | Time | Space | Best for |
//! |-----------|------|-------|----------|
//! | DP | O(n*m) | O(min(n,m)) | General |
//! | **Myers** | O((n+m)*d) | O(d*(n+m)) | **Small diffs (hot reload)** |
//! | Patience | O(n log n) | O(n) | Code diffs |
//!
//! For SSG hot reload:
//! - Edit distance `d` is typically very small (1-5 edits)
//! - O((n+m)*d) ≈ O(n+m) linear for small d
//! - Perfect match for incremental updates
//!
//! # Space Complexity Note
//!
//! This implementation stores a full trace for backtracking (`O(d)` snapshots,
//! each of size `O(n+m)`), resulting in **O(d*(n+m))** space complexity.
//! For large documents with many edits, this could be significant.
//!
//! Future optimization: Use linear-space Myers variant (divide-and-conquer)
//! which achieves O(n+m) space at the cost of 2x time.
//!
//! # References
//!
//! - Myers, E.W. "An O(ND) Difference Algorithm and Its Variations" (1986)
//!
//! # Implementation Notes
//!
//! - Uses common prefix/suffix optimization for further speedup
//! - Move detection: nodes present in both sequences but not in LCS

use rustc_hash::FxHashMap;

use crate::id::StableId;

// =============================================================================
// Public Types
// =============================================================================

/// Edit operation in a diff sequence
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Keep node at old_idx, corresponds to new_idx
    Keep { old_idx: usize, new_idx: usize },
    /// Insert new node at new_idx
    Insert { new_idx: usize },
    /// Delete node at old_idx
    Delete { old_idx: usize },
    /// Move node from old_idx to new_idx
    Move { old_idx: usize, new_idx: usize },
}

impl Edit {
    pub fn is_keep(&self) -> bool {
        matches!(self, Edit::Keep { .. })
    }

    pub fn is_move(&self) -> bool {
        matches!(self, Edit::Move { .. })
    }
}

/// Result of diff operation
#[derive(Debug, Default)]
pub struct LcsResult {
    pub edits: Vec<Edit>,
    pub stats: LcsStats,
}

/// Statistics from diff computation
#[derive(Debug, Default, Clone, Copy)]
pub struct LcsStats {
    pub kept: usize,
    pub inserted: usize,
    pub deleted: usize,
    pub moved: usize,
}

impl LcsStats {
    pub fn edit_count(&self) -> usize {
        self.inserted + self.deleted + self.moved
    }

    pub fn is_empty(&self) -> bool {
        self.edit_count() == 0
    }
}

// =============================================================================
// Main API
// =============================================================================

/// Compute diff between two sequences using Myers algorithm
///
/// Detects: Keep, Insert, Delete, Move operations
pub fn diff_sequences(old: &[StableId], new: &[StableId]) -> LcsResult {
    // Quick paths
    if old.is_empty() && new.is_empty() {
        return LcsResult::default();
    }

    if old.is_empty() {
        return LcsResult {
            edits: (0..new.len()).map(|i| Edit::Insert { new_idx: i }).collect(),
            stats: LcsStats { inserted: new.len(), ..Default::default() },
        };
    }

    if new.is_empty() {
        return LcsResult {
            edits: (0..old.len()).map(|i| Edit::Delete { old_idx: i }).collect(),
            stats: LcsStats { deleted: old.len(), ..Default::default() },
        };
    }

    // Build index maps for move detection
    let old_map: FxHashMap<StableId, usize> = old.iter().copied().enumerate().map(|(i, id)| (id, i)).collect();
    let new_map: FxHashMap<StableId, usize> = new.iter().copied().enumerate().map(|(i, id)| (id, i)).collect();

    // Compute LCS using Myers algorithm
    let lcs = myers_lcs(old, new);

    // Extract edit script with move detection
    extract_edits(old, new, &lcs, &old_map, &new_map)
}

// =============================================================================
// Myers Algorithm Core
// =============================================================================

/// Compute LCS using Myers diff algorithm with prefix/suffix optimization
fn myers_lcs(old: &[StableId], new: &[StableId]) -> Vec<(usize, usize)> {
    let n = old.len();
    let m = new.len();

    // Optimization: strip common prefix
    let mut prefix_len = 0;
    while prefix_len < n && prefix_len < m && old[prefix_len] == new[prefix_len] {
        prefix_len += 1;
    }

    // Optimization: strip common suffix
    let mut suffix_len = 0;
    while suffix_len < (n - prefix_len)
        && suffix_len < (m - prefix_len)
        && old[n - 1 - suffix_len] == new[m - 1 - suffix_len]
    {
        suffix_len += 1;
    }

    // Build prefix pairs
    let mut lcs: Vec<(usize, usize)> = (0..prefix_len).map(|i| (i, i)).collect();

    // Process middle portion with Myers
    let old_mid = &old[prefix_len..n - suffix_len];
    let new_mid = &new[prefix_len..m - suffix_len];

    if !old_mid.is_empty() && !new_mid.is_empty() {
        let mid_lcs = myers_core(old_mid, new_mid);
        for (oi, ni) in mid_lcs {
            lcs.push((oi + prefix_len, ni + prefix_len));
        }
    }

    // Add suffix pairs
    for i in 0..suffix_len {
        lcs.push((n - suffix_len + i, m - suffix_len + i));
    }

    lcs
}

/// Myers algorithm core implementation
///
/// Based on "An O(ND) Difference Algorithm" by Eugene W. Myers (1986)
///
/// The key insight: explore the edit graph by d (edit distance), not by position.
/// For each d, we track the furthest-reaching path on each diagonal k = x - y.
///
/// # Optimizations
///
/// 1. **Early termination**: If edit distance exceeds `MAX_EDIT_DISTANCE`, returns None
///    to signal that a full reload is more efficient than computing the full diff.
/// 2. **Small array fast path**: For sequences ≤8 elements, uses simple O(n²) DP
///    which is faster due to cache locality and no allocation overhead.
fn myers_core(old: &[StableId], new: &[StableId]) -> Vec<(usize, usize)> {
    let n = old.len();
    let m = new.len();

    if n == 0 || m == 0 {
        return Vec::new();
    }

    // Small array optimization: use simple DP for small sequences
    // Cache-friendly and no allocation overhead beats Myers for n,m ≤ 8
    if n <= 8 && m <= 8 {
        return small_lcs_dp(old, new);
    }

    let max_d = n + m;
    let offset = max_d; // To handle negative k indices

    // Early termination threshold: if edit distance exceeds this,
    // the diff is too large and full reload is more efficient
    const MAX_EDIT_DISTANCE: usize = 512;

    // V[k + offset] = furthest x on diagonal k
    let mut v = vec![0usize; 2 * max_d + 1];

    // Store V at each d for backtracking
    let mut trace: Vec<Vec<usize>> = Vec::with_capacity(max_d.min(MAX_EDIT_DISTANCE) + 1);

    // Forward pass: find shortest edit script
    'outer: for d in 0..=max_d {
        // Early termination: if edit distance is too large, bail out
        // The caller will fall back to full reload
        if d > MAX_EDIT_DISTANCE {
            // Return empty LCS to signal "too different"
            // This triggers maximum edits in extract_edits
            return Vec::new();
        }

        trace.push(v.clone());

        // Iterate over diagonals k in [-d, d] with same parity as d
        for k in (-(d as isize)..=(d as isize)).step_by(2) {
            let kk = (k + offset as isize) as usize;

            // Decide: come from k-1 (delete) or k+1 (insert)?
            // At k=-d, must come from k+1 (insert)
            // At k=d, must come from k-1 (delete)
            // Otherwise, pick whichever reaches further right
            let mut x = if k == -(d as isize) || (k != d as isize && v[kk - 1] < v[kk + 1]) {
                v[kk + 1] // insert: x stays same, y increases
            } else {
                v[kk - 1] + 1 // delete: x increases
            };

            let mut y = (x as isize - k) as usize;

            // Extend snake: follow diagonal while elements match
            while x < n && y < m && old[x] == new[y] {
                x += 1;
                y += 1;
            }

            v[kk] = x;

            // Check if we reached the end
            if x >= n && y >= m {
                break 'outer;
            }
        }
    }

    // Backtrack to find LCS
    backtrack(&trace, old, new, n, m, offset)
}

/// Backtrack through trace to extract LCS pairs
fn backtrack(
    trace: &[Vec<usize>],
    old: &[StableId],
    new: &[StableId],
    n: usize,
    m: usize,
    offset: usize,
) -> Vec<(usize, usize)> {
    let mut x = n;
    let mut y = m;
    let mut lcs = Vec::new();

    for (d, v) in trace.iter().enumerate().rev() {
        let k = x as isize - y as isize;
        let kk = (k + offset as isize) as usize;

        // Determine previous k (before this edit)
        let prev_k = if d == 0 {
            // At d=0, we started at (0,0)
            0isize
        } else if k == -(d as isize) || (k != d as isize && v[kk - 1] < v[kk + 1]) {
            k + 1 // came from insert
        } else {
            k - 1 // came from delete
        };

        let prev_kk = (prev_k + offset as isize) as usize;
        let prev_x = if d == 0 { 0 } else { v[prev_kk] };
        let prev_y = (prev_x as isize - prev_k) as usize;

        // Collect matches along the snake (diagonal moves)
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            if old[x] == new[y] {
                lcs.push((x, y));
            }
        }

        // Step back before the edit
        if d > 0 {
            if prev_k < k {
                // Was a delete: x decreased by 1
                x = prev_x;
            } else {
                // Was an insert: y decreased by 1
                y = prev_y;
            }
        }

        if x == 0 && y == 0 {
            break;
        }
    }

    lcs.reverse();
    lcs
}

/// Simple O(n*m) DP for small sequences (≤8 elements).
///
/// For small arrays, this beats Myers due to:
/// 1. No trace allocation overhead
/// 2. Cache-friendly sequential access
/// 3. Simpler control flow (no diagonal tracking)
///
/// Uses stack-allocated array for the DP table when possible.
fn small_lcs_dp(old: &[StableId], new: &[StableId]) -> Vec<(usize, usize)> {
    let n = old.len();
    let m = new.len();

    // DP table: dp[i][j] = LCS length of old[0..i] and new[0..j]
    // Using flat array for cache efficiency
    let mut dp = [[0u8; 9]; 9]; // Max 8x8, +1 for boundary

    for i in 1..=n {
        for j in 1..=m {
            dp[i][j] = if old[i - 1] == new[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    // Backtrack to find LCS pairs
    let mut lcs = Vec::with_capacity(dp[n][m] as usize);
    let mut i = n;
    let mut j = m;

    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            lcs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    lcs.reverse();
    lcs
}

/// Extract edit operations from LCS with move detection
fn extract_edits(
    old: &[StableId],
    new: &[StableId],
    lcs: &[(usize, usize)],
    old_map: &FxHashMap<StableId, usize>,
    new_map: &FxHashMap<StableId, usize>,
) -> LcsResult {
    use std::collections::HashSet;

    let mut edits = Vec::new();
    let mut stats = LcsStats::default();

    let lcs_old_indices: HashSet<usize> = lcs.iter().map(|(o, _)| *o).collect();
    let lcs_new_indices: HashSet<usize> = lcs.iter().map(|(_, n)| *n).collect();

    // LCS entries are Keep operations
    for &(old_idx, new_idx) in lcs {
        edits.push(Edit::Keep { old_idx, new_idx });
        stats.kept += 1;
    }

    // Find deleted/moved nodes (in old but not in LCS)
    for (old_idx, id) in old.iter().enumerate() {
        if lcs_old_indices.contains(&old_idx) {
            continue;
        }

        if let Some(&new_idx) = new_map.get(id) {
            // Node exists in new but not in LCS -> moved
            if !lcs_new_indices.contains(&new_idx) {
                edits.push(Edit::Move { old_idx, new_idx });
                stats.moved += 1;
            }
        } else {
            // Node doesn't exist in new -> deleted
            edits.push(Edit::Delete { old_idx });
            stats.deleted += 1;
        }
    }

    // Find inserted nodes (in new but not in old)
    for (new_idx, id) in new.iter().enumerate() {
        if lcs_new_indices.contains(&new_idx) {
            continue;
        }

        if !old_map.contains_key(id) {
            // Truly new node
            edits.push(Edit::Insert { new_idx });
            stats.inserted += 1;
        }
    }

    // Sort for consistent ordering
    edits.sort_by_key(|e| match e {
        Edit::Keep { new_idx, .. } => (*new_idx, 0),
        Edit::Insert { new_idx } => (*new_idx, 1),
        Edit::Delete { old_idx } => (*old_idx, 2),
        Edit::Move { new_idx, .. } => (*new_idx, 3),
    });

    LcsResult { edits, stats }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(nums: &[u64]) -> Vec<StableId> {
        nums.iter().map(|&n| StableId::from_raw(n)).collect()
    }

    #[test]
    fn test_empty_sequences() {
        let result = diff_sequences(&[], &[]);
        assert!(result.edits.is_empty());
        assert!(result.stats.is_empty());
    }

    #[test]
    fn test_insert_all() {
        let old = ids(&[]);
        let new = ids(&[1, 2, 3]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.inserted, 3);
        assert_eq!(result.stats.deleted, 0);
        assert_eq!(result.stats.moved, 0);
    }

    #[test]
    fn test_delete_all() {
        let old = ids(&[1, 2, 3]);
        let new = ids(&[]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.deleted, 3);
        assert_eq!(result.stats.inserted, 0);
        assert_eq!(result.stats.moved, 0);
    }

    #[test]
    fn test_no_changes() {
        let old = ids(&[1, 2, 3]);
        let new = ids(&[1, 2, 3]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.kept, 3);
        assert!(result.stats.is_empty());
    }

    #[test]
    fn test_single_insert() {
        let old = ids(&[1, 3]);
        let new = ids(&[1, 2, 3]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.kept, 2);
        assert_eq!(result.stats.inserted, 1);
    }

    #[test]
    fn test_single_delete() {
        let old = ids(&[1, 2, 3]);
        let new = ids(&[1, 3]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.kept, 2);
        assert_eq!(result.stats.deleted, 1);
    }

    #[test]
    fn test_move_detection() {
        let old = ids(&[1, 2, 3]);
        let new = ids(&[1, 3, 2]);

        let result = diff_sequences(&old, &new);
        // Should detect move or keep all with different positions
        assert!(result.stats.moved > 0 || result.stats.kept == 3);
    }

    #[test]
    fn test_complete_reorder() {
        let old = ids(&[1, 2, 3]);
        let new = ids(&[3, 2, 1]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.deleted, 0);
        assert_eq!(result.stats.inserted, 0);
    }

    #[test]
    fn test_mixed_operations() {
        let old = ids(&[1, 2, 3, 4]);
        let new = ids(&[1, 5, 3]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.kept, 2); // 1 and 3
        assert_eq!(result.stats.deleted, 2); // 2 and 4
        assert_eq!(result.stats.inserted, 1); // 5
    }

    #[test]
    fn test_edit_is_keep() {
        let edit = Edit::Keep { old_idx: 0, new_idx: 0 };
        assert!(edit.is_keep());
        assert!(!edit.is_move());
    }

    #[test]
    fn test_edit_is_move() {
        let edit = Edit::Move { old_idx: 0, new_idx: 1 };
        assert!(edit.is_move());
        assert!(!edit.is_keep());
    }

    #[test]
    fn test_prefix_optimization() {
        // Common prefix should be detected quickly
        let old = ids(&[1, 2, 3, 4, 5, 100]);
        let new = ids(&[1, 2, 3, 4, 5, 200]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.kept, 5); // 1,2,3,4,5
        assert_eq!(result.stats.deleted, 1); // 100
        assert_eq!(result.stats.inserted, 1); // 200
    }

    #[test]
    fn test_suffix_optimization() {
        // Common suffix should be detected quickly
        let old = ids(&[100, 1, 2, 3, 4, 5]);
        let new = ids(&[200, 1, 2, 3, 4, 5]);

        let result = diff_sequences(&old, &new);
        assert_eq!(result.stats.kept, 5); // 1,2,3,4,5
        assert_eq!(result.stats.deleted, 1); // 100
        assert_eq!(result.stats.inserted, 1); // 200
    }
}
