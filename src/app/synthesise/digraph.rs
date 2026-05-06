use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Open input file and count all `a-z` digraph pairs.
pub fn read_counts(input: &Path) -> Result<FxHashMap<[char; 2], u64>> {
    let file = fs::File::open(input)
        .into_diagnostic()
        .wrap_err("Failed to open input text file")?;
    Ok(count(BufReader::new(file)))
}

/// Filter by min relative frequency, then scale counts to `target` total edges.
/// Rounding error is redistributed to the top pairs.
pub fn filter_and_scale(
    counts: &FxHashMap<[char; 2], u64>,
    min_freq: f64,
    target: usize,
) -> Vec<([char; 2], usize)> {
    let total_raw: u64 = counts.values().sum();
    let threshold = total_raw as f64 * min_freq;
    let mut filtered: Vec<([char; 2], u64)> = counts
        .iter()
        .filter(|(_, c)| **c as f64 >= threshold)
        .map(|(&pair, &c)| (pair, c))
        .collect();
    filtered.sort_unstable_by(|&(a, ca), &(b, cb)| cb.cmp(&ca).then(a.cmp(&b)));

    let filtered_total: u64 = filtered.iter().map(|(_, c)| c).sum();
    let mut scaled: Vec<([char; 2], usize)> = filtered
        .iter()
        .map(|&(pair, c)| {
            (
                pair,
                (c as f64 / filtered_total as f64 * target as f64) as usize,
            )
        })
        .collect();

    // Redistribute rounding remainder to highest-frequency pairs.
    let assigned: usize = scaled.iter().map(|(_, n)| n).sum();
    let mut remainder = target.saturating_sub(assigned);
    for (_, n) in scaled.iter_mut() {
        if remainder == 0 {
            break;
        }
        *n += 1;
        remainder -= 1;
    }
    scaled
}

/// Write scaled digraph pairs to CSV: `pair,count,%,raw,raw%` (count = scaled edge frequency, raw = original corpus count).
/// Percentage precision is derived from `min_freq` so the smallest value is always readable.
pub fn write_digraphs(
    scaled: &[([char; 2], usize)],
    counts: &FxHashMap<[char; 2], u64>,
    min_freq: f64,
    path: &Path,
) -> Result<()> {
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create CSV file")?;

    writeln!(out, "pair,count,%,raw,raw%").into_diagnostic()?;

    let total: usize = scaled.iter().map(|(_, n)| n).sum();
    let raw_total: u64 = counts.values().sum();
    let precision = pct_precision(min_freq);

    for &([a, b], count) in scaled {
        let pct = if total > 0 {
            count as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        let raw = counts.get(&[a, b]).copied().unwrap_or(0);
        let raw_pct = if raw_total > 0 {
            raw as f64 / raw_total as f64 * 100.0
        } else {
            0.0
        };
        writeln!(
            out,
            "{}{},{},{:.prec$},{},{:.prec$}",
            a,
            b,
            count,
            pct,
            raw,
            raw_pct,
            prec = precision
        )
        .into_diagnostic()?;
    }

    Ok(())
}

/// Decimal places needed to display a percentage whose smallest meaningful value is `min_freq * 100`.
/// E.g. min_freq=0.001 → smallest pct=0.1% → 1 significant decimal → 1 decimal place.
fn pct_precision(min_freq: f64) -> usize {
    if min_freq <= 0.0 || min_freq >= 1.0 || !min_freq.is_finite() {
        return 2;
    }
    // min_freq * 100 is the smallest percentage value that will appear.
    // We want enough decimals to show one significant digit of that value.
    let smallest_pct = min_freq * 100.0;
    ((-smallest_pct.log10()).ceil() + 1.0).max(2.0) as usize
}

/// Count all `a-z` digraph pairs from a buffered reader, skipping cross-whitespace pairs.
pub fn count(reader: impl BufRead) -> FxHashMap<[char; 2], u64> {
    let mut counts: FxHashMap<[char; 2], u64> = FxHashMap::default();
    let mut prev: Option<char> = None;

    for line in reader.lines().map_while(Result::ok) {
        for ch in line.chars() {
            if ch.is_ascii_alphabetic() {
                let lc = ch.to_ascii_lowercase();
                if let Some(p) = prev {
                    *counts.entry([p, lc]).or_insert(0) += 1;
                }
                prev = Some(lc);
            } else {
                // Whitespace or punctuation — break digraph chain.
                prev = None;
            }
        }
        // Line boundary = word boundary.
        prev = None;
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // --- count_digraphs ---

    #[test]
    fn digraphs_counts_pairs_and_breaks_on_whitespace() {
        // basic pairs, case folding, space as separator, ab ≠ ba
        let counts = count(Cursor::new("ab BC ba ba aa"));
        assert_eq!(counts[&['a', 'b']], 1);
        assert_eq!(counts[&['b', 'c']], 1);
        assert_eq!(counts[&['b', 'a']], 2);
        assert_eq!(counts[&['a', 'a']], 1);
        // space breaks chain → no repeated cross-space pair
        assert!(!counts.contains_key(&['b', 'b']));
    }

    #[test]
    fn digraphs_boundary_and_punctuation_break_chain() {
        // line boundary
        let counts = count(Cursor::new("ab\nbc"));
        assert_eq!(counts[&['a', 'b']], 1);
        assert_eq!(counts[&['b', 'c']], 1);
        assert!(!counts.contains_key(&['b', 'b']));

        // punctuation
        let counts = count(Cursor::new("a.b"));
        assert!(counts.is_empty());

        // empty
        assert!(count(Cursor::new("")).is_empty());
    }

    // --- filter_and_scale ---

    #[test]
    fn filter_and_scale_target_sum_and_order() {
        let counts = make_counts(&[(['a', 'b'], 10), (['b', 'c'], 90)]);
        let scaled = filter_and_scale(&counts, 0.0, 100);

        // sums to target
        assert_eq!(scaled.iter().map(|(_, n)| n).sum::<usize>(), 100);
        // sorted descending by frequency
        assert_eq!(scaled[0].0, ['b', 'c']);
        assert_eq!(scaled[1].0, ['a', 'b']);
    }

    #[test]
    fn filter_and_scale_drops_rare_pairs_and_empty() {
        // total = 100; min_freq = 0.25 → threshold = 25 → ['c','d'] (20) dropped
        let counts = make_counts(&[(['a', 'b'], 50), (['b', 'c'], 30), (['c', 'd'], 20)]);
        let scaled = filter_and_scale(&counts, 0.25, 100);
        let pairs: Vec<_> = scaled.iter().map(|(p, _)| *p).collect();
        assert!(pairs.contains(&['a', 'b']));
        assert!(pairs.contains(&['b', 'c']));
        assert!(!pairs.contains(&['c', 'd']));

        // empty input → empty output
        assert!(filter_and_scale(&FxHashMap::default(), 0.0, 100).is_empty());
    }

    // --- pct_precision ---

    #[test]
    fn pct_precision_common_values() {
        // min_freq=0.1  → smallest pct=10%   → 2 decimals (10.00)
        assert_eq!(pct_precision(0.1), 2);
        // min_freq=0.05 → smallest pct=5%    → 2 decimals (5.00)
        assert_eq!(pct_precision(0.05), 2);
        // min_freq=0.01 → smallest pct=1%    → 2 decimals (1.00)
        assert_eq!(pct_precision(0.01), 2);
        // min_freq=0.001 → smallest pct=0.1% → 2 decimals (0.10)
        assert_eq!(pct_precision(0.001), 2);
        // min_freq=0.0001 → smallest pct=0.01% → 3 decimals (0.010)
        assert_eq!(pct_precision(0.0001), 3);
        // min_freq=0.00001 → smallest pct=0.001% → 4 decimals (0.0010)
        assert_eq!(pct_precision(0.00001), 4);
    }

    #[test]
    fn pct_precision_edge_cases() {
        assert_eq!(pct_precision(0.0), 2); // zero → default
        assert_eq!(pct_precision(1.0), 2); // 100% → default
        assert_eq!(pct_precision(2.0), 2); // >1 → default
        assert_eq!(pct_precision(f64::NAN), 2);
        assert_eq!(pct_precision(f64::INFINITY), 2);
        assert_eq!(pct_precision(-0.01), 2); // negative → default
    }

    // --- Helpers ---

    fn make_counts(pairs: &[([char; 2], u64)]) -> FxHashMap<[char; 2], u64> {
        pairs.iter().cloned().collect()
    }
}
