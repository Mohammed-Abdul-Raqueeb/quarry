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
