use super::counter::{count_bigrams, count_letters};
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use std::fs;
use std::io::{BufReader, Write};
use std::path::Path;

/// Open input file and count all `a-z` digraph pairs.
pub fn read_counts(input: &Path) -> Result<FxHashMap<[char; 2], u64>> {
    let file = fs::File::open(input)
        .into_diagnostic()
        .wrap_err("Failed to open input text file")?;
    Ok(count_bigrams(BufReader::new(file)))
}

/// Load bigram raw counts from an existing stats CSV (`pair,count,%,raw,raw%`).
/// Uses `raw` column when present; falls back to `count`.
pub fn read_counts_csv(path: &Path) -> Result<FxHashMap<[char; 2], u64>> {
    let text = fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err("Failed to read bigram stats CSV")?;
    let mut counts: FxHashMap<[char; 2], u64> = FxHashMap::default();

    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let pair_text = parts[0].trim();
        let mut chars = pair_text.chars();
        let (Some(a), Some(b), None) = (chars.next(), chars.next(), chars.next()) else {
            continue;
        };

        let raw_idx = if parts.len() >= 4 { 3 } else { 1 };
        let raw = parts[raw_idx].trim().parse::<u64>().unwrap_or(0);
        counts.insert([a, b], raw);
    }

    Ok(counts)
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

/// Write scaled bigram pairs to CSV: `pair,count,%,raw,raw%` (count = scaled edge frequency, raw = original corpus count).
/// Percentage precision is derived from `min_freq` so the smallest value is always readable.
pub fn write_bigrams(
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

/// Count `a-z` letter frequencies from an input file.
pub fn read_letter_counts(input: &Path) -> Result<FxHashMap<char, u64>> {
    let file = fs::File::open(input)
        .into_diagnostic()
        .wrap_err("Failed to open file for letter frequency counting")?;
    Ok(count_letters(BufReader::new(file)))
}

/// Load original letter counts from an existing combined letter-frequency CSV
/// (`letter,orig_count,orig_%,synth_count,synth_%`).
pub fn read_letter_counts_csv(path: &Path) -> Result<FxHashMap<char, u64>> {
    let text = fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err("Failed to read letter stats CSV")?;
    let mut counts: FxHashMap<char, u64> = FxHashMap::default();

    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let mut chars = parts[0].trim().chars();
        let Some(ch) = chars.next() else {
            continue;
        };
        if chars.next().is_some() {
            continue;
        }
        let count = parts[1].trim().parse::<u64>().unwrap_or(0);
        counts.insert(ch, count);
    }

    Ok(counts)
}

/// Count `a-z` letter frequencies from corpus word list.
pub fn count_corpus_letters(words: &[String]) -> FxHashMap<char, u64> {
    let mut counts: FxHashMap<char, u64> = FxHashMap::default();
    for w in words {
        for ch in w.chars() {
            if ch.is_ascii_alphabetic() {
                *counts.entry(ch.to_ascii_lowercase()).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Write combined letter frequency CSV: `letter,orig_count,orig_%,synth_count,synth_%`.
/// Rows sorted by original frequency descending; all 26 letters always emitted.
pub fn write_letter_freq_combined(
    orig: &FxHashMap<char, u64>,
    synth: &FxHashMap<char, u64>,
    path: &Path,
) -> Result<()> {
    let orig_total: u64 = orig.values().sum();
    let synth_total: u64 = synth.values().sum();

    let mut letters: Vec<char> = (b'a'..=b'z').map(|b| b as char).collect();
    letters.sort_unstable_by(|a, b| {
        orig.get(b)
            .unwrap_or(&0)
            .cmp(orig.get(a).unwrap_or(&0))
            .then(a.cmp(b))
    });

    let pct = |n: u64, total: u64| -> f64 {
        if total > 0 {
            n as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    };

    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create letter frequency CSV")?;
    writeln!(out, "letter,orig_count,orig_%,synth_count,synth_%").into_diagnostic()?;
    for ch in &letters {
        let oc = orig.get(ch).copied().unwrap_or(0);
        let sc = synth.get(ch).copied().unwrap_or(0);
        writeln!(
            out,
            "{},{},{:.2},{},{:.2}",
            ch,
            oc,
            pct(oc, orig_total),
            sc,
            pct(sc, synth_total)
        )
        .into_diagnostic()?;
    }
    Ok(())
}

/// Aggregate symmetric bigram pairs (AB + BA → canonical min pair), write CSV: `pair,count,%,raw,raw%`.
/// Source is the already-scaled slice so counts stay consistent with the regular bigrams CSV.
pub fn write_bigrams_aggregated(
    scaled: &[([char; 2], usize)],
    counts: &FxHashMap<[char; 2], u64>,
    min_freq: f64,
    path: &Path,
) -> Result<()> {
    // Aggregate by canonical key = lexicographically smaller of (ab, ba).
    let mut agg: FxHashMap<[char; 2], (usize, u64)> = FxHashMap::default();
    for &([a, b], count) in scaled {
        let key = if [a, b] <= [b, a] { [a, b] } else { [b, a] };
        let raw = counts.get(&[a, b]).copied().unwrap_or(0);
        let entry = agg.entry(key).or_insert((0, 0));
        entry.0 += count;
        entry.1 += raw;
    }

    let total: usize = agg.values().map(|(c, _)| c).sum();
    let raw_total: u64 = agg.values().map(|(_, r)| r).sum();
    let precision = pct_precision(min_freq);

    let mut rows: Vec<([char; 2], usize, u64)> =
        agg.into_iter().map(|(k, (c, r))| (k, c, r)).collect();
    rows.sort_unstable_by(|&(a, ca, _), &(b, cb, _)| cb.cmp(&ca).then(a.cmp(&b)));

    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create aggregated CSV file")?;
    writeln!(out, "pair,count,%,raw,raw%").into_diagnostic()?;
    for ([a, b], count, raw) in rows {
        let pct = if total > 0 {
            count as f64 / total as f64 * 100.0
        } else {
            0.0
        };
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

#[cfg(test)]
mod tests {
    use super::*;

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
