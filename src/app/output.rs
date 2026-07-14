use crate::models::{Layout, ScoreResult};
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashSet;
use serde::Deserialize;
use std::{fs, io::Write, path::Path};
use tracing::info;

/// Hand that the letter `e` is pinned to when persisting layouts.
/// `Left`/`Right` mirror every layout to that orientation and collapse hand-swapped
/// twins (identical fitness) to one row. `Any` disables canonicalization: layouts
/// are written verbatim and mirror twins are kept.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// `e` on the left hand (slots 0–14).
    #[default]
    Left,
    /// `e` on the right hand (slots 15–29).
    Right,
    /// No canonicalization; keep layouts as produced.
    Any,
}

/// Print the top N layouts and persist them.
///
/// `side` picks the hand `e` is canonicalized to: `Left`/`Right` mirror every
/// layout to that orientation and collapse hand-swapped twins (identical fitness)
/// to one row — in append mode the rows already on disk are folded in and the file
/// rewritten deduped. `Any` writes layouts verbatim (plain append/overwrite),
/// leaving mirror twins in place.
pub fn write_layouts(
    layouts: &[(Layout, ScoreResult, usize)],
    to_print: usize,
    output_path: Option<&Path>,
    overwrite: bool,
    side: Side,
) -> Result<()> {
    match side {
        Side::Any => write_plain(layouts, to_print, output_path, overwrite),
        _ => write_canonical(layouts, to_print, output_path, overwrite, side),
    }
}

/// Canonicalize every layout so `e` sits on `side`, dedup mirror twins, and rewrite
/// the whole file (folding in any rows already on disk when appending).
fn write_canonical(
    layouts: &[(Layout, ScoreResult, usize)],
    to_print: usize,
    output_path: Option<&Path>,
    overwrite: bool,
    side: Side,
) -> Result<()> {
    // Console: canonical batch, mirror twins deduped, best first.
    let mut seen = FxHashSet::default();
    layouts
        .iter()
        .filter_map(|(layout, score, pool)| {
            let (layout, score) = to_side(layout, score, side);
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
        read_rows(path, side)
    };
    let batch = layouts.iter().map(|(l, s, _)| to_side(l, s, side));
    let rows = dedup(existing.into_iter().chain(batch));

    write_csv(path, &rows)
}

/// Write layouts verbatim: append (header when new) or overwrite, no dedup.
fn write_plain(
    layouts: &[(Layout, ScoreResult, usize)],
    to_print: usize,
    output_path: Option<&Path>,
    overwrite: bool,
) -> Result<()> {
    for (layout, score, pool) in layouts.iter().take(to_print) {
        println!("[pool {pool:>2}] {layout} | {score}");
    }

    let Some(path) = output_path else {
        return Ok(());
    };

    let is_new =
        overwrite || !path.exists() || path.metadata().map(|m| m.len() == 0).unwrap_or(true);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(overwrite)
        .truncate(overwrite)
        .append(!overwrite)
        .open(path)
        .into_diagnostic()
        .wrap_err("Failed to open layouts file")?;

    if is_new {
        writeln!(
            file,
            "keys_1,keys_2,keys_3,keys_4,keys_5,keys_6,name,{}",
            ScoreResult::csv_header()
        )
        .into_diagnostic()
        .wrap_err("Failed to write header")?;
    }

    for (layout, score, _) in layouts {
        writeln!(file, "{layout},{},{}", layout.name, score.to_csv())
            .into_diagnostic()
            .wrap_err("Failed to write layout row")?;
    }

    info!("Results written to {}", path.display());
    Ok(())
}

/// Orient `layout`/`score` so `e` sits on `side`. Mirrors both when `e` is on the
/// wrong hand; returns them unchanged otherwise (and for `Side::Any`).
fn to_side(layout: &Layout, score: &ScoreResult, side: Side) -> (Layout, ScoreResult) {
    let aligned = match side {
        Side::Left => layout.e_is_left(),
        Side::Right => !layout.e_is_left(),
        Side::Any => true,
    };
    if aligned {
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
        .map(|(layout, score)| format!("{layout},{},{}", layout.name, score.to_csv()))
        .collect()
}

/// Read persisted rows, canonicalized so `e` sits on `side`; skips header and blanks.
fn read_rows(path: &Path, side: Side) -> Vec<(Layout, ScoreResult)> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("keys_1,"))
        .filter_map(|line| {
            let score = ScoreResult::from_csv(line)?;
            Some(to_side(&Layout::new(line), &score, side))
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
        "keys_1,keys_2,keys_3,keys_4,keys_5,keys_6,name,{}",
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

    // e-left layout (`e` in keys_2 → slot 7) plus a score row.
    fn e_left() -> (Layout, ScoreResult) {
        let line = "_mub_,lreop,wfydx,_htc_,kinas,qgvzj,5378.69,0.00%,0.96,40%,24%,34%,1.10,1.85,47%,52%,49%,50%,15.0,7.0,8.0,100,200,17,7,5,30,40";
        (Layout::new(line), ScoreResult::from_csv(line).unwrap())
    }

    #[test]
    fn to_side_right_mirrors_e_left_layout() {
        let (layout, score) = e_left();
        assert!(layout.e_is_left());

        let (layout, score) = to_side(&layout, &score, Side::Right);

        assert!(!layout.e_is_left());
        // L/R counts trade places under the mirror.
        assert_eq!(score.left_count, 200);
        assert_eq!(score.right_count, 100);
        assert_eq!(score.left_rolls, 40);
        assert_eq!(score.right_rolls, 30);
    }

    #[test]
    fn to_side_left_keeps_e_left_layout() {
        let (layout, score) = e_left();

        let (out, _) = to_side(&layout, &score, Side::Left);

        assert!(out.e_is_left());
        assert_eq!(out.to_string(), layout.to_string());
    }

    #[test]
    fn dedup_collapses_canonicalized_mirror_twins() {
        let (layout, score) = e_left();
        let already_right = (layout.mirrored(), score.mirror());
        let canonicalized = to_side(&layout, &score, Side::Right);

        let rows = dedup([already_right, canonicalized].into_iter());

        assert_eq!(rows.len(), 1);
    }
}
