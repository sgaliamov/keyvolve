mod count;

use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use std::fs;
use std::io::{BufReader, Write};
use std::path::Path;

pub use count::count_digraphs;

/// Run the synthesise pipeline: count digraphs → write CSV → write fake-word corpus.
pub fn run(input: &Path, output: &Path, target: usize) -> Result<()> {
    let file = fs::File::open(input)
        .into_diagnostic()
        .wrap_err("Failed to open input text file")?;
    let reader = BufReader::new(file);

    let counts = count_digraphs(reader);
    let total_raw: u64 = counts.values().sum();

    // Write frequency CSV.
    let csv_path = output.with_extension("csv");
    write_csv(&counts, total_raw, &csv_path)?;

    // Filter: drop pairs below 0.1% relative frequency.
    let min_freq = total_raw as f64 * 0.001;
    let mut filtered: Vec<([char; 2], u64)> = counts
        .into_iter()
        .filter(|(_, c)| *c as f64 >= min_freq)
        .collect();
    filtered.sort_by_key(|&(_, c)| std::cmp::Reverse(c));

    let filtered_total: u64 = filtered.iter().map(|(_, c)| c).sum();

    // Scale to target edge count; redistribute rounding error top-down.
    let mut scaled: Vec<([char; 2], usize)> = filtered
        .iter()
        .map(|(pair, c)| (*pair, ((*c as f64 / filtered_total as f64) * target as f64) as usize))
        .collect();
    let assigned: usize = scaled.iter().map(|(_, n)| n).sum();
    let mut remainder = target.saturating_sub(assigned);
    for (_, n) in scaled.iter_mut() {
        if remainder == 0 {
            break;
        }
        *n += 1;
        remainder -= 1;
    }

    // Build directed multigraph and extract Eulerian paths → fake words.
    let words = build_corpus(&scaled);

    // Write corpus.
    let corpus_path = output.with_extension("txt");
    let mut out = fs::File::create(&corpus_path)
        .into_diagnostic()
        .wrap_err("Failed to create corpus output file")?;
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            out.write_all(b" ").into_diagnostic()?;
        }
        out.write_all(word.as_bytes()).into_diagnostic()?;
    }
    out.write_all(b"\n").into_diagnostic()?;

    tracing::info!(
        csv = %csv_path.display(),
        corpus = %corpus_path.display(),
        words = words.len(),
        "Synthesise complete"
    );
    Ok(())
}

/// Write `pair,count,frequency` CSV sorted by frequency desc.
fn write_csv(counts: &FxHashMap<[char; 2], u64>, total: u64, path: &Path) -> Result<()> {
    let mut pairs: Vec<_> = counts.iter().collect();
    pairs.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create CSV file")?;
    writeln!(out, "pair,count,frequency").into_diagnostic()?;
    for ([a, b], c) in &pairs {
        writeln!(out, "{}{},{},{:.6}", a, b, c, **c as f64 / total as f64).into_diagnostic()?;
    }
    Ok(())
}

/// Build fake words from scaled digraph edges via Hierholzer's algorithm.
fn build_corpus(edges: &[([char; 2], usize)]) -> Vec<String> {
    let mut adj: FxHashMap<char, Vec<char>> = FxHashMap::default();
    let mut in_deg: FxHashMap<char, i64> = FxHashMap::default();
    let mut out_deg: FxHashMap<char, i64> = FxHashMap::default();

    for ([a, b], n) in edges {
        for _ in 0..*n {
            adj.entry(*a).or_default().push(*b);
        }
        *out_deg.entry(*a).or_insert(0) += *n as i64;
        *in_deg.entry(*b).or_insert(0) += *n as i64;
    }

    // Balance in/out degrees by adding bridge edges (minimal corrections).
    let mut nodes: Vec<char> = adj.keys().cloned().collect();
    nodes.sort();
    for &node in &nodes {
        let out = *out_deg.get(&node).unwrap_or(&0);
        let r#in = *in_deg.get(&node).unwrap_or(&0);
        if out > r#in {
            let diff = (out - r#in) as usize;
            for _ in 0..diff {
                adj.entry(node).or_default().push(node);
            }
        }
    }

    let start_nodes: Vec<char> = {
        let mut s: Vec<char> = adj.keys().cloned().collect();
        s.sort();
        s
    };

    let mut result: Vec<String> = Vec::new();
    for start in start_nodes {
        if adj.get(&start).is_none_or(|v| v.is_empty()) {
            continue;
        }
        let path = hierholzer(&mut adj, start);
        if path.len() > 1 {
            result.push(path.iter().collect());
        }
    }

    result
}

/// Hierholzer's algorithm on the adjacency list; returns char path.
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
