use crate::models::{Layout, ScoreResult};
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashSet;
use std::{fs, io::Write, path::Path};
use tracing::info;

/// Print the top N layouts and persist them, canonicalized so `a` always sits on
/// the left hand. Hand-swapped mirror twins (identical fitness) collapse to a
/// single row. In append mode the rows already on disk are folded in and the file
/// is rewritten deduped; in overwrite mode only the current batch is written.
pub fn write_layouts(
    layouts: &[(Layout, ScoreResult, usize)],
    to_print: usize,
    output_path: Option<&Path>,
    overwrite: bool,
) -> Result<()> {
    // Console: canonical batch, mirror twins deduped, best first.
    let mut seen = FxHashSet::default();
    layouts
        .iter()
        .filter_map(|(layout, score, pool)| {
            let (layout, score) = to_a_left(layout, score);
            seen.insert(layout.to_string())
                .then_some((layout, score, *pool))
        })
        .take(to_print)
        .for_each(|(layout, score, pool)| println!("[pool {pool:>2}] {layout} | {score}"));

    let Some(path) = output_path else {
        return Ok(());
    };

    // Fold disk rows (append mode) + current batch, canonicalize, dedup, rewrite.
    let existing = if overwrite {
        Vec::new()
    } else {
        read_rows(path)
    };
    let batch = layouts.iter().map(|(l, s, _)| to_a_left(l, s));
    let rows = dedup(existing.into_iter().chain(batch));

    write_csv(path, &rows)
}

/// Canonical orientation with `a` on the left hand. Mirrors both layout and score
/// when `a` is on the right; returns them unchanged otherwise.
fn to_a_left(layout: &Layout, score: &ScoreResult) -> (Layout, ScoreResult) {
    if layout.a_is_left() {
        (layout.clone(), score.clone())
    } else {
        (layout.mirrored(), score.mirror())
    }
}

/// Serialize the first row per distinct layout (mirror twins already collapsed by
/// canonicalization upstream).
fn dedup(rows: impl Iterator<Item = (Layout, ScoreResult)>) -> Vec<String> {
    let mut seen = FxHashSet::default();
    rows.filter(|(layout, _)| seen.insert(layout.to_string()))
        .map(|(layout, score)| format!("{layout}, {}", score.to_csv()))
        .collect()
}

/// Read persisted rows, canonicalized to `a`-left; skips header and blanks.
fn read_rows(path: &Path) -> Vec<(Layout, ScoreResult)> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("keys_1,"))
        .filter_map(|line| {
            let score = ScoreResult::from_csv(line)?;
            Some(to_a_left(&Layout::new(line), &score))
        })
        .collect()
}

/// Write the header plus all rows, truncating any existing file.
fn write_csv(path: &Path, rows: &[String]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .into_diagnostic()
        .wrap_err("Failed to open layouts file")?;

    writeln!(
        file,
        "keys_1, keys_2, keys_3, keys_4, keys_5, keys_6, {}",
        ScoreResult::csv_header()
    )
    .into_diagnostic()
    .wrap_err("Failed to write header")?;

    for line in rows {
        writeln!(file, "{line}")
            .into_diagnostic()
            .wrap_err("Failed to write layout row")?;
    }

    info!("Results written to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // a-right layout (`a` in keys_5 → slot 23) plus a score row.
    fn a_right() -> (Layout, ScoreResult) {
        let line = "_mub_, lreop, wfydx, _htc_, kinas, qgvzj, 5378.69, 0.96, 15.0, 7.0, 47%, 100, 49%, 8.0, 52%, 200, 50%, 17, 34%, 12, 24%";
        (Layout::new(line), ScoreResult::from_csv(line).unwrap())
    }

    #[test]
    fn to_a_left_mirrors_a_right_layout() {
        let (layout, score) = a_right();
        assert!(!layout.a_is_left());

        let (layout, score) = to_a_left(&layout, &score);

        assert!(layout.a_is_left());
        // L/R counts trade places under the mirror.
        assert_eq!(score.left_count, 200);
        assert_eq!(score.right_count, 100);
    }

    #[test]
    fn dedup_collapses_canonicalized_mirror_twins() {
        let (layout, score) = a_right();
        let already_left = (layout.mirrored(), score.mirror());
        let canonicalized = to_a_left(&layout, &score);

        let rows = dedup([already_left, canonicalized].into_iter());

        assert_eq!(rows.len(), 1);
    }
}
