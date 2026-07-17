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

    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];

    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;

        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }

        std::mem::swap(&mut previous, &mut current);
    }

    previous[b.len()]
}

/// Levenshtein distance, giving up once it exceeds `max`.
///
/// Returns `None` when the distance is above `max`. That is the common case
/// during a dictionary scan, and returning early is what makes the scan
/// affordable.
#[must_use]
pub fn bounded_levenshtein(a: &str, b: &str, max: usize) -> Option<usize> {
    if a == b {
        return Some(0);
    }

    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    // A length difference alone already exceeds the budget.
    if a.len().abs_diff(b.len()) > max {
        return None;
    }

    if a.is_empty() {
        return (b.len() <= max).then_some(b.len());
    }
    if b.is_empty() {
        return (a.len() <= max).then_some(a.len());
    }

    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];

    for (i, &ca) in a.iter().enumerate() {
        let row = i + 1;

        // Only cells within `max` of the diagonal can be reached inside the
        // budget. Everything outside the band is marked rather than computed.
        let lo = row.saturating_sub(max);
        let hi = (row + max).min(b.len());

        for cell in current.iter_mut().take(lo) {
            *cell = UNREACHABLE;
        }
        for cell in current.iter_mut().skip(hi + 1) {
            *cell = UNREACHABLE;
        }

        // Column zero is only reachable while the band still touches it;
        // past that, reaching it would need more deletions than the budget.
        if lo == 0 {
            current[0] = row;
        }

        // Writes every cell of the band, column `lo` included. Starting the
        // loop one column further left — writing `current[j + 1]` from `j` —
        // leaves `current[lo]` holding whatever the buffer had two rows ago,
        // and the stale value makes the distance come out too low.
        for j in lo.max(1)..=hi {
            let cost = usize::from(ca != b[j - 1]);
            let deletion = previous[j].saturating_add(1);
            let insertion = current[j - 1].saturating_add(1);
            let substitution = previous[j - 1].saturating_add(cost);

            current[j] = deletion.min(insertion).min(substitution);
        }

        // Nothing in the band is within budget, so no completion can be.
        let best = current[lo..=hi]
            .iter()
            .copied()
            .min()
            .unwrap_or(UNREACHABLE);
        if best > max {
            return None;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    let distance = previous[b.len()];
    (distance <= max).then_some(distance)
}

/// Damerau-Levenshtein: also counts a transposition as one edit.
///
/// `teh` -> `the` is one transposition, which plain Levenshtein charges two
/// substitutions for. Transposed letters are the single most common typing
/// error, so this ranks real typos better.
#[must_use]
pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    // The full table is needed here: a transposition looks two rows back.
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];

    for (i, row) in table.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in table[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);

            table[i][j] = (table[i - 1][j] + 1)
                .min(table[i][j - 1] + 1)
                .min(table[i - 1][j - 1] + cost);

            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                table[i][j] = table[i][j].min(table[i - 2][j - 2] + 1);
            }
        }
    }

    table[a.len()][b.len()]
}
