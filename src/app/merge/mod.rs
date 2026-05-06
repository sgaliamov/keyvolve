pub mod config;

pub use config::*;

use miette::{Context, IntoDiagnostic, Result};
use rayon::prelude::*;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

/// Merge all `.txt` files in a folder into one file.
/// Non-`a-z` chars (after lowercasing) become spaces; consecutive spaces on a line collapse to one.
/// Files are processed in parallel; results are written in sorted filename order.
pub fn merge(cfg: MergeConfig) -> Result<()> {
    let input = cfg
        .input
        .wrap_err("Merge mode requires `merge.input` path")?;
    let output = cfg
        .output
        .wrap_err("Merge mode requires `merge.output` path")?;

    // Collect sorted .txt paths.
    let mut paths: Vec<PathBuf> = fs::read_dir(&input)
        .into_diagnostic()
        .wrap_err("Failed to read input folder")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("txt"))
        .collect();
    paths.sort();

    tracing::info!(folder = %input.display(), count = paths.len(), "Merging files");

    // Process files in parallel → collect cleaned lines per file in order.
    let results: Vec<(PathBuf, Vec<String>)> = paths
        .par_iter()
        .map(|path| {
            let lines = process_file(path)?;
            Ok((path.clone(), lines))
        })
        .collect::<Result<Vec<_>>>()?;

    // Write sequentially to preserve order and avoid holding all data in memory.
    let out_file = File::create(&output)
        .into_diagnostic()
        .wrap_err("Failed to create output file")?;
    let mut writer = BufWriter::new(out_file);

    for (path, lines) in &results {
        tracing::debug!(file = %path.display(), lines = lines.len(), "Writing");
        for line in lines {
            writer.write_all(line.as_bytes()).into_diagnostic()?;
            writer.write_all(b"\n").into_diagnostic()?;
        }
    }

    tracing::info!(output = %output.display(), "Merge complete");
    Ok(())
}

/// Read a file line by line, clean each line: lowercase a-z only, collapse spaces.
fn process_file(path: &PathBuf) -> Result<Vec<String>> {
    let file = File::open(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open {}", path.display()))?;
    let reader = BufReader::new(file);

    let lines = reader
        .lines()
        .map(|l| {
            let raw = l.into_diagnostic()?;
            Ok(clean_line(&raw))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(lines)
}

/// Lowercase a-z only; everything else → space; collapse consecutive spaces.
fn clean_line(line: &str) -> String {
    let cleaned: String = line
        .chars()
        .map(|c| {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_alphabetic() {
                lower
            } else {
                ' '
            }
        })
        .collect();
    // collapse consecutive spaces
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}
