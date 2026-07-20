use crate::app::rank::{HAND_SLOTS, QWERTY, RankConfig, RankState};
use crate::models::Keyboard;
use miette::{IntoDiagnostic, Result};
use std::fmt::Write as _;
use std::path::Path;

/// Bucketed ranking result: per-item group plus the effort scale.
pub struct Buckets {
    /// Effort per group index, ascending (group 0 = most preferable).
    pub efforts: Vec<f64>,
    /// group[item_index] — parallel to `RankState::items`.
    pub groups: Vec<usize>,
}

/// Quantile-bucket items by rating (highest rating = lowest effort).
pub fn bucketize(state: &RankState, cfg: &RankConfig) -> Buckets {
    let n = state.items.len();
    let groups_n = cfg.groups.max(1);
    let span = cfg.effort_max - cfg.effort_min;
    let efforts = (0..groups_n)
        .map(|g| cfg.effort_min + span * g as f64 / (groups_n - 1).max(1) as f64)
        .collect();

    // Sort by rating descending: best first → bucket 0.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&x, &y| state.items[y].rating.total_cmp(&state.items[x].rating));

    let mut groups = vec![0usize; n];
    for (pos, &item) in order.iter().enumerate() {
        groups[item] = pos * groups_n / n;
    }
    Buckets { efforts, groups }
}

/// Write ranked keyboard JSON (left-hand pairs; diagonal copied from `keyboard`).
pub fn write_keyboard_json(
    path: &Path,
    state: &RankState,
    buckets: &Buckets,
    keyboard: &Keyboard,
) -> Result<()> {
    let grid = pair_groups(state, buckets, keyboard);

    let mut out = String::from("{\n");
    let efforts = buckets
        .efforts
        .iter()
        .map(|e| format!("{}", (e * 100.0).round() / 100.0))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "  \"efforts\": [{efforts}],");
    let _ = writeln!(out, "  \"pairs\": {{");
    for from in 0..HAND_SLOTS {
        let row = (0..HAND_SLOTS)
            .map(|to| format!("\"{to}\": {}", grid[from as usize][to as usize]))
            .collect::<Vec<_>>()
            .join(", ");
        let comma = if from + 1 < HAND_SLOTS { "," } else { "" };
        let _ = writeln!(out, "    \"{from}\": {{ {row} }}{comma}");
    }
    out.push_str("  }\n}\n");

    std::fs::write(path, out).into_diagnostic()
}

/// Write CSV visual report: 15 blocks (one per starting key), each a 3×5 grid
/// of efforts matching the physical layout, plus rating/matches grids and stats.
pub fn write_report_csv(
    path: &Path,
    state: &RankState,
    buckets: &Buckets,
    keyboard: &Keyboard,
) -> Result<()> {
    let grid = pair_groups(state, buckets, keyboard);
    // item lookup by (from, to)
    let item = |from: u8, to: u8| {
        state
            .items
            .iter()
            .position(|i| i.from == from && i.to == to)
    };

    let mut out = String::new();
    for from in 0..HAND_SLOTS {
        let _ = writeln!(
            out,
            "from: {} (slot {from})",
            QWERTY[from as usize].to_ascii_uppercase()
        );

        // Effort grid (3 rows × 5 cols).
        let effort_of = |to: u8| buckets.efforts[grid[from as usize][to as usize]];
        for row in 0..3u8 {
            let cells = (0..5u8)
                .map(|col| format!("{:.2}", effort_of(row * 5 + col)))
                .collect::<Vec<_>>()
                .join(",");
            let _ = writeln!(out, "{cells}");
        }

        // Analytical grids: raw rating and matches (diagonal blank).
        for (name, cell) in [
            (
                "rating",
                &(|i: usize| format!("{:.0}", state.items[i].rating)) as &dyn Fn(usize) -> String,
            ),
            ("matches", &(|i: usize| state.items[i].matches.to_string())),
        ] {
            let _ = writeln!(out, "{name}:");
            for row in 0..3u8 {
                let cells = (0..5u8)
                    .map(|col| item(from, row * 5 + col).map(cell).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(",");
                let _ = writeln!(out, "{cells}");
            }
        }

        // Block stats over the 14 ranked targets.
        let efforts: Vec<f64> = (0..HAND_SLOTS)
            .filter(|&to| to != from)
            .map(effort_of)
            .collect();
        let (min, max) = (
            efforts.iter().copied().fold(f64::INFINITY, f64::min),
            efforts.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        );
        let mean = efforts.iter().sum::<f64>() / efforts.len() as f64;
        let _ = writeln!(out, "min,{min:.2},max,{max:.2},mean,{mean:.2}");
        out.push('\n');
    }

    std::fs::write(path, out).into_diagnostic()
}

/// Full 15×15 group table: ranked pairs bucketed, diagonal mapped from the
/// existing keyboard's repeat efforts to the nearest new group.
fn pair_groups(state: &RankState, buckets: &Buckets, keyboard: &Keyboard) -> Vec<Vec<usize>> {
    let mut grid = vec![vec![0usize; HAND_SLOTS as usize]; HAND_SLOTS as usize];
    for (idx, item) in state.items.iter().enumerate() {
        grid[item.from as usize][item.to as usize] = buckets.groups[idx];
    }
    for from in 0..HAND_SLOTS {
        let old_effort = keyboard
            .pairs
            .get(&from)
            .and_then(|t| t.get(&from))
            .map(|&g| keyboard.efforts[g])
            .unwrap_or(buckets.efforts[buckets.efforts.len() - 1]);
        grid[from as usize][from as usize] = nearest_group(&buckets.efforts, old_effort);
    }
    grid
}

/// Index of the effort closest to `value`.
fn nearest_group(efforts: &[f64], value: f64) -> usize {
    efforts
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - value).abs().total_cmp(&(*b - value).abs()))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranked_state() -> RankState {
        let mut state = RankState::new();
        for (i, item) in state.items.iter_mut().enumerate() {
            item.rating = 2000.0 - i as f64; // item 0 best
        }
        state
    }

    #[test]
    fn bucketize_is_monotone_and_spans_groups() {
        let state = ranked_state();
        let cfg = RankConfig::default();
        let b = bucketize(&state, &cfg);
        assert_eq!(b.efforts.len(), cfg.groups);
        assert_eq!(b.groups[0], 0); // best rating → best bucket
        assert_eq!(b.groups[209], cfg.groups - 1); // worst rating → worst bucket
        assert!(b.efforts.windows(2).all(|w| w[0] < w[1]));
        // Higher rating never lands in a worse bucket.
        assert!(b.groups.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn written_json_parses_as_keyboard() {
        let state = ranked_state();
        let cfg = RankConfig::default();
        let buckets = bucketize(&state, &cfg);
        let keyboard = Keyboard::load("data/keyboard.json").unwrap();

        let dir = std::env::temp_dir().join("keyvolve-rank-out-test");
        std::fs::create_dir_all(&dir).unwrap();
        let json = dir.join("keyboard.json");
        write_keyboard_json(&json, &state, &buckets, &keyboard).unwrap();
        let loaded = Keyboard::load(&json).unwrap();
        assert_eq!(loaded.efforts.len(), cfg.groups);
        assert_eq!(loaded.pairs.len(), 30); // left + mirrored right
        assert!(loaded.pairs[&0].len() == 15);

        let csv = dir.join("keyboard.csv");
        write_report_csv(&csv, &state, &buckets, &keyboard).unwrap();
        let text = std::fs::read_to_string(&csv).unwrap();
        assert_eq!(text.matches("from: ").count(), 15);
        std::fs::remove_dir_all(&dir).ok();
    }
}
