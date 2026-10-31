use crate::models::{Layout, ScoreResult};
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashSet;
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
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

/// Stage all evaluation files, then replace them after the final cancellation check.
/// Once commit starts, finish the batch even if cancellation arrives between replacements.
pub fn write_evaluation_files(
    files: &[(PathBuf, Vec<crate::modes::evaluate::ScoredLayout>)],
    side: Side,
    cancelled: impl Fn() -> bool,
) -> Result<bool> {
    let mut staged = Vec::with_capacity(files.len());
    for (path, layouts) in files {
        if cancelled() {
            return Ok(false);
        }

        // Preserve a symlink itself by replacing its target, not the link.
        let destination = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                fs::canonicalize(path).into_diagnostic().wrap_err_with(|| {
                    format!("Failed to resolve output symlink: {}", path.display())
                })?
            }
            Ok(_) => path.clone(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.clone(),
            Err(error) => {
                return Err(error)
                    .into_diagnostic()
                    .wrap_err("Failed to inspect output");
            }
        };
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::Builder::new()
            .prefix(".ikeya-")
            .tempfile_in(parent)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to stage output: {}", path.display()))?;

        let rows: Box<dyn Iterator<Item = String> + '_> =
            if side == Side::Any {
                Box::new(layouts.iter().map(|(layout, score, _)| {
                    format!("{layout},{},{}", layout.name, score.to_csv())
                }))
            } else {
                Box::new(
                    dedup(
                        layouts
                            .iter()
                            .map(|(layout, score, _)| to_side(layout, score, side)),
                    )
                    .into_iter(),
                )
            };
        if !write_csv_content(temp.as_file_mut(), rows, &cancelled)? {
            return Ok(false);
        }
        let permissions = match fs::metadata(&destination) {
            Ok(metadata) => Some(metadata.permissions()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error)
                    .into_diagnostic()
                    .wrap_err("Failed to inspect output permissions");
            }
        };
        staged.push((path, destination, temp, permissions));
        if cancelled() {
            return Ok(false);
        }
    }

    // This check is the commit boundary; a multi-file replacement is not a transaction.
    if cancelled() {
        return Ok(false);
    }
    for (path, destination, temp, permissions) in staged {
        if let Some(permissions) = permissions {
            temp.as_file()
                .set_permissions(permissions)
                .into_diagnostic()
                .wrap_err("Failed to preserve output permissions")?;
        }
        temp.persist(&destination)
            .map_err(|error| error.error)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to replace output: {}", path.display()))?;
        info!("Results written to {}", path.display());
    }
    Ok(true)
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
        .filter_map(|(layout, score, _)| {
            let (layout, score) = to_side(layout, score, side);
            seen.insert(layout.to_string()).then_some((layout, score))
        })
        .take(to_print)
        .for_each(|(layout, score)| println!("{}", console_row(&layout, &score)));

    let Some(path) = output_path else {
        return Ok(());
    };

    // Fold disk rows (append mode) + current batch, canonicalize, dedup, rewrite.
    let existing = if overwrite {
        Vec::new()
    } else {
        read_rows(path, side)?
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
    for (layout, score, _) in layouts.iter().take(to_print) {
        println!("{}", console_row(layout, score));
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

/// Layout and essential scores, shared by plain and canonical console output.
fn console_row(layout: &Layout, score: &ScoreResult) -> String {
    format!("{layout} | {score}")
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
fn read_rows(path: &Path, side: Side) -> Result<Vec<(Layout, ScoreResult)>> {
    let content = fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err("Failed to read layouts file")?;
    let mut rows = Vec::new();
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("keys_1,") {
            continue;
        }
        let score = ScoreResult::from_csv(line)
            .ok_or_else(|| miette::miette!("Invalid score row: {line}"))?;
        rows.push(to_side(&Layout::try_new(line)?, &score, side));
    }
    Ok(rows)
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

    write_csv_content(&mut file, rows.iter(), &|| false)?;

    info!("Results written to {}", path.display());
    Ok(())
}

/// Write the standard CSV header and serialized rows.
fn write_csv_content(
    file: &mut impl Write,
    rows: impl IntoIterator<Item = impl AsRef<str>>,
    cancelled: &impl Fn() -> bool,
) -> Result<bool> {
    writeln!(
        file,
        "keys_1,keys_2,keys_3,keys_4,keys_5,keys_6,name,{}",
        ScoreResult::csv_header()
    )
    .into_diagnostic()
    .wrap_err("Failed to write header")?;

    for line in rows {
        if cancelled() {
            return Ok(false);
        }
        writeln!(file, "{}", line.as_ref())
            .into_diagnostic()
            .wrap_err("Failed to write layout row")?;
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // e-left layout (`e` in keys_2 → slot 7) plus a score row.
    fn e_left() -> (Layout, ScoreResult) {
        use crate::models::ScoreResult;

        let score = ScoreResult {
            fitness: 5378.69,
            effort: 10000.0,
            left_count: 100,
            right_count: 200,
            left_rolls: 30,
            right_rolls: 40,
            inward_count: 12,
            outward_count: 9,
            hand_switches: 5,
            left_row_switch_cost: 30,
            right_row_switch_cost: 40,
            left_effort: 5000.0,
            right_effort: 5000.0,
            left_column_effort: [1500.0, 1050.0, 1300.0, 1050.0, 100.0],
            right_column_effort: [1500.0, 1050.0, 1300.0, 1050.0, 100.0],
            left_row_effort: [2500.0, 6000.0, 1500.0],
            right_row_effort: [2500.0, 6000.0, 1500.0],
            left_column_count: [20, 14, 26, 21, 19],
            right_column_count: [30, 42, 52, 63, 13],
            left_finger_row_switch_cost: [8, 7, 6, 9],
            right_finger_row_switch_cost: [4, 5, 6, 15],
            sfs_count: 0,
        };

        let csv_line = format!("_mub_,lreop,wfydx,_htc_,kinas,qgvzj,, {}", score.to_csv());
        (Layout::new(&csv_line), score)
    }

    #[test]
    fn console_row_omits_name_and_keeps_core_scores() {
        let (mut layout, score) = e_left();
        layout.name = "example".to_owned();
        assert_eq!(
            console_row(&layout, &score),
            "_mub_,lreop,wfydx,_htc_,kinas,qgvzj | fitness: 5378.6900 | effort: 10000.00"
        );
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

    #[test]
    fn staged_cancellation_preserves_existing_and_absent_outputs() {
        for existing in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("output.csv");
            if existing {
                fs::write(&path, "original").unwrap();
            }
            let (layout, score) = e_left();
            let files = [(path.clone(), vec![(layout, score, 0)])];
            let app = cliffa::cli::AppHandle::new();
            let checks = Cell::new(0);
            let committed = write_evaluation_files(&files, Side::Any, || {
                checks.set(checks.get() + 1);
                if checks.get() == 2 {
                    // The header is staged; cancellation arrives before the first row.
                    assert_eq!(
                        fs::read_dir(dir.path()).unwrap().count(),
                        1 + usize::from(existing)
                    );
                    app.finish();
                }
                app.should_finish()
            })
            .unwrap();

            assert!(!committed);
            if existing {
                assert_eq!(fs::read_to_string(&path).unwrap(), "original");
            } else {
                assert!(!path.exists());
            }
            assert_eq!(
                fs::read_dir(dir.path()).unwrap().count(),
                usize::from(existing)
            );
        }
    }

    #[test]
    fn staged_output_matches_existing_writer_for_all_sides() {
        for side in [Side::Any, Side::Left, Side::Right] {
            for existing in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let output = dir.path().join("output.csv");
                let expected = dir.path().join("expected.csv");
                if existing {
                    fs::write(&output, "old contents").unwrap();
                }
                let (layout, score) = e_left();
                let rows = vec![
                    (layout.clone(), score.clone(), 0),
                    (layout.mirrored(), score.mirror(), 0),
                ];
                write_layouts(&rows, 0, Some(&expected), true, side).unwrap();
                assert!(write_evaluation_files(&[(output.clone(), rows)], side, || false).unwrap());
                assert_eq!(fs::read(&output).unwrap(), fs::read(&expected).unwrap());
            }
        }
    }

    #[test]
    fn in_place_cancellation_after_staging_all_files_preserves_every_input() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.csv");
        let second = dir.path().join("second.csv");
        fs::write(&first, "first original").unwrap();
        fs::write(&second, "second original").unwrap();
        let (layout, score) = e_left();
        let rows = vec![(layout, score, 0)];
        let files = [(first.clone(), rows.clone()), (second.clone(), rows)];
        let checks = Cell::new(0);

        let committed = write_evaluation_files(&files, Side::Left, || {
            checks.set(checks.get() + 1);
            checks.get() == 7 // Final check, after both files are staged.
        })
        .unwrap();

        assert!(!committed);
        assert_eq!(fs::read_to_string(first).unwrap(), "first original");
        assert_eq!(fs::read_to_string(second).unwrap(), "second original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn staging_failure_does_not_replace_earlier_files() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.csv");
        fs::write(&first, "original").unwrap();
        let (layout, score) = e_left();
        let rows = vec![(layout, score, 0)];
        let files = [
            (first.clone(), rows.clone()),
            (dir.path().join("missing/second.csv"), rows),
        ];

        assert!(write_evaluation_files(&files, Side::Any, || false).is_err());
        assert_eq!(fs::read_to_string(first).unwrap(), "original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn cancellation_after_commit_boundary_finishes_the_batch() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.csv");
        let second = dir.path().join("second.csv");
        let (layout, score) = e_left();
        let rows = vec![(layout, score, 0)];
        let files = [(first.clone(), rows.clone()), (second.clone(), rows)];
        let checks = Cell::new(0);
        let app = cliffa::cli::AppHandle::new();

        let committed = write_evaluation_files(&files, Side::Any, || {
            checks.set(checks.get() + 1);
            let cancelled = app.should_finish();
            if checks.get() == 7 {
                // Signal arrives immediately after the final check's snapshot.
                app.finish();
            }
            cancelled
        })
        .unwrap();

        assert!(committed);
        assert!(app.should_finish());
        assert_eq!(checks.get(), 7);
        assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_symlink_and_target_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.csv");
        let link = dir.path().join("link.csv");
        fs::write(&target, "original").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&target, &link).unwrap();
        let (layout, score) = e_left();
        assert!(
            write_evaluation_files(
                &[(link.clone(), vec![(layout, score, 0)])],
                Side::Any,
                || false
            )
            .unwrap()
        );
        assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert!(fs::read_to_string(target).unwrap().starts_with("keys_1,"));
    }
}
