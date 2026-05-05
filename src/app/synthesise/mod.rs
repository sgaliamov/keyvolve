mod count;
pub mod config;

pub use config::SynthesiseConfig;
use count::count_digraphs;
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use std::fs;
use std::io::{BufReader, Write};
use std::path::Path;

/// Run the full synthesise pipeline.
pub fn run(input: &Path, cfg: SynthesiseConfig) -> Result<()> {
    let output = cfg
        .output
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.output` path")?;

    let counts = read_counts(input)?;
    let total_raw: u64 = counts.values().sum();

    write_csv(&counts, total_raw, &output.with_extension("csv"))?;

    let scaled = filter_and_scale(&counts, total_raw, cfg.min_freq, cfg.target);
    let words = build_corpus(&scaled);
    write_corpus(&words, &output.with_extension("txt"))?;

    tracing::info!(
        csv = %output.with_extension("csv").display(),
        corpus = %output.with_extension("txt").display(),
        words = words.len(),
        "Synthesise complete"
    );
    Ok(())
}

/// Open input file and count all `a-z` digraph pairs.
fn read_counts(input: &Path) -> Result<FxHashMap<[char; 2], u64>> {
    let file = fs::File::open(input)
        .into_diagnostic()
        .wrap_err("Failed to open input text file")?;
    Ok(count_digraphs(BufReader::new(file)))
}

/// Filter by min relative frequency, then scale counts to `target` total edges.
/// Rounding error is redistributed to the top pairs.
fn filter_and_scale(
    counts: &FxHashMap<[char; 2], u64>,
    total_raw: u64,
    min_freq: f64,
    target: usize,
) -> Vec<([char; 2], usize)> {
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

/// Write `pair,count,frequency` CSV sorted by frequency desc.
fn write_csv(counts: &FxHashMap<[char; 2], u64>, total: u64, path: &Path) -> Result<()> {
    let mut pairs: Vec<_> = counts.iter().collect();
    pairs.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create CSV file")?;
    writeln!(out, "pair,count,frequency").into_diagnostic()?;
    for ([a, b], c) in &pairs {
        writeln!(out, "{}{},{},{:.6}", a, b, c, **c as f64 / total as f64).into_diagnostic()?;
    }
    Ok(())
}

/// Write space-separated fake words to a text file.
fn write_corpus(words: &[String], path: &Path) -> Result<()> {
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create corpus output file")?;
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            out.write_all(b" ").into_diagnostic()?;
        }
        out.write_all(word.as_bytes()).into_diagnostic()?;
    }
    out.write_all(b"\n").into_diagnostic()?;
    Ok(())
}

/// Build fake words from scaled digraph edges via Eulerian path decomposition.
fn build_corpus(edges: &[([char; 2], usize)]) -> Vec<String> {
    let mut adj = build_adj(edges);
    balance_degrees(&mut adj, edges);
    extract_paths(&mut adj)
}

/// Build directed adjacency list from edge list.
fn build_adj(edges: &[([char; 2], usize)]) -> FxHashMap<char, Vec<char>> {
    let mut adj: FxHashMap<char, Vec<char>> = FxHashMap::default();
    for ([a, b], n) in edges {
        let entry = adj.entry(*a).or_default();
        for _ in 0..*n {
            entry.push(*b);
        }
    }
    adj
}

/// Add self-loop bridge edges to nodes where out-degree exceeds in-degree,
/// making each node Eulerian.
fn balance_degrees(adj: &mut FxHashMap<char, Vec<char>>, edges: &[([char; 2], usize)]) {
    let mut in_deg: FxHashMap<char, i64> = FxHashMap::default();
    let mut out_deg: FxHashMap<char, i64> = FxHashMap::default();
    for ([a, b], n) in edges {
        *out_deg.entry(*a).or_insert(0) += *n as i64;
        *in_deg.entry(*b).or_insert(0) += *n as i64;
    }
    let mut nodes: Vec<char> = adj.keys().cloned().collect();
    nodes.sort();
    for node in nodes {
        let deficit =
            out_deg.get(&node).copied().unwrap_or(0) - in_deg.get(&node).copied().unwrap_or(0);
        if deficit > 0 {
            let entry = adj.entry(node).or_default();
            for _ in 0..deficit {
                entry.push(node);
            }
        }
    }
}

/// Run Hierholzer per start node; collect non-trivial paths as fake words.
fn extract_paths(adj: &mut FxHashMap<char, Vec<char>>) -> Vec<String> {
    let starts: Vec<char> = {
        let mut s: Vec<char> = adj.keys().cloned().collect();
        s.sort();
        s
    };
    let mut result = Vec::new();
    for start in starts {
        if adj.get(&start).is_none_or(|v| v.is_empty()) {
            continue;
        }
        let path = hierholzer(adj, start);
        if path.len() > 1 {
            result.push(path.iter().collect());
        }
    }
    result
}

/// Hierholzer's algorithm — extracts an Eulerian path from `start`.
fn hierholzer(adj: &mut FxHashMap<char, Vec<char>>, start: char) -> Vec<char> {
    let mut stack = vec![start];
    let mut path = Vec::new();
    while let Some(&top) = stack.last() {
        if adj.get(&top).is_some_and(|v| !v.is_empty()) {
            let next = adj.get_mut(&top).unwrap().pop().unwrap();
            stack.push(next);
        } else {
            path.push(stack.pop().unwrap());
        }
    }
    path.reverse();
    path
}
