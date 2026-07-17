//! Edit distance, for typo-tolerant matching.
//!
//! The naive Levenshtein algorithm fills an `n × m` table, which is fine for
//! comparing two words but not for comparing a query term against every term in
//! a dictionary. Two things fix that:
//!
//! - **Banding.** If only `k` edits are allowed, any cell more than `k` off the
//!   diagonal is unreachable, so the table is a `k`-wide band rather than a
//!   rectangle. Cost drops from `O(nm)` to `O(n·k)`.
//! - **Early exit.** If the whole band exceeds `k`, no completion can come back
//!   under it, so the comparison stops there.
//!
//! Together those make a dictionary scan practical without building an
//! automaton.

/// Stands in for "unreachable" inside the banded table. Half of `usize::MAX`
/// so that adding one during a comparison cannot overflow.
const UNREACHABLE: usize = usize::MAX / 2;

/// Largest edit distance supported.
///
/// Beyond two, a fuzzy match stops being a typo correction and starts matching
/// unrelated words — `cat` within three edits reaches `dogs`.
pub const MAX_DISTANCE: u8 = 2;

/// Levenshtein distance: insertions, deletions and substitutions.
///
/// Uses two rows rather than the full table, since each row only depends on the
/// one before it.
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }

    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
