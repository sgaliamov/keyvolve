use miette::{Context, IntoDiagnostic, Result};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use rustc_hash::FxHashMap;

/// Open input file and count all `a-z` digraph pairs.
pub fn read_counts(input: &Path) -> Result<FxHashMap<[char; 2], u64>> {
    let file = fs::File::open(input)
        .into_diagnostic()
        .wrap_err("Failed to open input text file")?;
    Ok(count_digraphs(BufReader::new(file)))
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
    filtered.sort_by_key(|&(_, c)| std::cmp::Reverse(c));

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

/// Write scaled digraph pairs to CSV: `pair,count` (count = scaled edge frequency).
pub fn write_scaled_csv(scaled: &[([char; 2], usize)], path: &Path) -> Result<()> {
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create CSV file")?;
    writeln!(out, "pair,count").into_diagnostic()?;
    for ([a, b], count) in scaled {
        writeln!(out, "{}{},{}", a, b, count).into_diagnostic()?;
    }
    Ok(())
}

/// Read scaled digraph pairs from CSV back into memory.
pub fn read_scaled_csv(path: &Path) -> Result<Vec<([char; 2], usize)>> {
    let content = fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err("Failed to read CSV file")?;
    let mut result = Vec::new();

    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 2 {
            continue;
        }
        let pair_str = parts[0];
        let count: usize = parts[1]
            .parse()
            .into_diagnostic()
            .wrap_err(format!("Failed to parse count in line: {}", line))?;
        if pair_str.len() == 2 {
            let chars: Vec<char> = pair_str.chars().collect();
            result.push(([chars[0], chars[1]], count));
        }
    }
    Ok(result)
}

/// Count all `a-z` digraph pairs from a buffered reader, skipping cross-whitespace pairs.
fn count_digraphs(reader: impl BufRead) -> FxHashMap<[char; 2], u64> {
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
