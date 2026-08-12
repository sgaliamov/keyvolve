use crate::app::rank::{HAND_SLOTS, QWERTY, RankConfig, RankState, head_to_head, majority_edges};
use miette::{IntoDiagnostic, Result};
use std::fmt::Write as _;
use std::path::Path;

/// Adaptive confidence tiers and their final effort values.
pub struct Tiers {
    /// Effort per tier index, ascending (tier 0 = most preferable).
    pub efforts: Vec<f64>,
    /// tier[item_index] — parallel to `RankState::items`.
    pub groups: Vec<usize>,
}

/// Build final adaptive tiers with rating-anchored effort values.
pub fn tiers(state: &RankState, cfg: &RankConfig) -> Tiers {
    let groups = state.confidence_tiers(cfg);
    let count = state.confidence_tier_count(cfg);
    if count == 0 {
        return Tiers {
            efforts: vec![],
            groups,
        };
    }
    let efforts = tier_efforts(state, &groups, count, cfg);
    Tiers { efforts, groups }
}

/// Rating-anchored tier efforts: mean fitted rating per tier mapped onto
/// `[effort_min, effort_max]` (best tier pinned to min, worst to max), with
/// `effort_gamma` bending the curve between the endpoints.
fn tier_efforts(state: &RankState, groups: &[usize], count: usize, cfg: &RankConfig) -> Vec<f64> {
    let mut sums = vec![(0.0f64, 0usize); count];
    for (item, &tier) in state.items.iter().zip(groups) {
        sums[tier].0 += item.rating;
        sums[tier].1 += 1;
    }
    let means: Vec<f64> = sums
        .into_iter()
        .map(|(sum, n)| sum / n.max(1) as f64)
        .collect();
    let (best, worst) = (means[0], means[count - 1]);
    let span = (best - worst).max(f64::EPSILON);
    means
        .into_iter()
        .map(|mean| {
            let t = ((best - mean) / span).clamp(0.0, 1.0);
            cfg.effort_min + (cfg.effort_max - cfg.effort_min) * t.powf(cfg.effort_gamma)
        })
        .collect()
}

/// Write ranked keyboard JSON (left-hand pairs; a repeated key gets its row's
/// best group).
pub fn write_keyboard_json(path: &Path, state: &RankState, tiers: &Tiers) -> Result<()> {
    let grid = pair_groups(state, tiers);

    let mut out = String::from("{\n");
    let efforts = tiers
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

    write_text(path, out)
}

/// Write CSV visual report: 15 blocks (one per starting key), each a 3×5 grid
/// of efforts matching the physical layout, plus rating/matches grids and stats.
pub fn write_report_csv(path: &Path, state: &RankState, tiers: &Tiers) -> Result<()> {
    let grid = pair_groups(state, tiers);
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
            "from: {} (slot {from}),,,,,,rating:,,,,,,deviation:,,,,,,matches:",
            QWERTY[from as usize].to_ascii_uppercase()
        );

        // Effort grid (3 rows × 5 cols) with the analytical grids as extra
        // column blocks on the same rows: effort | rating | deviation | matches.
        let effort_of = |to: u8| tiers.efforts[grid[from as usize][to as usize]];
        let analytic: [&dyn Fn(usize) -> String; 3] = [
            &|i| format!("{:.0}", state.items[i].rating),
            &|i| format!("{:.0}", state.items[i].deviation),
            &|i| state.items[i].matches.to_string(),
        ];
        for row in 0..3u8 {
            let mut blocks = vec![
                (0..5u8)
                    .map(|col| format!("{:.2}", effort_of(row * 5 + col)))
                    .collect::<Vec<_>>()
                    .join(","),
            ];
            for cell in analytic {
                blocks.push(
                    (0..5u8)
                        .map(|col| item(from, row * 5 + col).map(cell).unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            let _ = writeln!(out, "{}", blocks.join(",,"));
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

    // Double-press efforts only: 3×5 matrix, same physical layout as above.
    let _ = writeln!(out, "doubles:",);
    for row in 0..3u8 {
        let _ = writeln!(
            out,
            "{}",
            (0..5u8)
                .map(|col| {
                    format!(
                        "{:.2}",
                        tiers.efforts[grid[(row * 5 + col) as usize][(row * 5 + col) as usize]]
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        );
    }

    write_text(path, out)
}

/// Write flat per-bigram CSV sorted by fitted rating, with majority summary.
pub fn write_bigrams_csv(path: &Path, state: &RankState, tiers: &Tiers) -> Result<()> {
    let edges = majority_edges(state);
    let head_to_head = head_to_head(state);
    let mut majority_losses = vec![0usize; state.items.len()];
    for losers in &edges {
        for &loser in losers {
            majority_losses[loser] += 1;
        }
    }
    let mut compared = vec![0usize; state.items.len()];
    for &(a, b) in head_to_head.keys() {
        compared[a] += 1;
        compared[b] += 1;
    }
    let majority = state
        .items
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let wins = edges[index].len();
            let losses = majority_losses[index];
            let ties = compared[index].saturating_sub(wins + losses);
            let unseen = state.items.len().saturating_sub(1 + compared[index]);
            (wins, losses, ties, unseen)
        })
        .collect::<Vec<_>>();
    let mut majority_order = (0..state.items.len()).collect::<Vec<_>>();
    majority_order.sort_by(|&a, &b| {
        let score_a = majority[a].0 as isize - majority[a].1 as isize;
        let score_b = majority[b].0 as isize - majority[b].1 as isize;
        score_b
            .cmp(&score_a)
            .then_with(|| majority[b].0.cmp(&majority[a].0))
            .then_with(|| majority[a].1.cmp(&majority[b].1))
            .then_with(|| state.items[b].rating.total_cmp(&state.items[a].rating))
            .then_with(|| a.cmp(&b))
    });
    let mut majority_rank = vec![0usize; state.items.len()];
    for (rank, &index) in majority_order.iter().enumerate() {
        majority_rank[index] = rank + 1;
    }

    let mut rating_order = (0..state.items.len()).collect::<Vec<_>>();
    rating_order.sort_by(|&a, &b| {
        state.items[b]
            .rating
            .total_cmp(&state.items[a].rating)
            .then_with(|| a.cmp(&b))
    });

    let tier_count = tiers.efforts.len();

    let mut out = String::from(
        "rating_rank,bigram,mirror,tier,majority_rank,rating,deviation,effort,matches,majority_score,majority_wins,majority_losses,majority_ties,majority_unseen\n",
    );
    for (rating_rank, &index) in rating_order.iter().enumerate() {
        let item = &state.items[index];
        let (wins, losses, ties, unseen) = majority[index];
        let score = wins as isize - losses as isize;
        let tier = tiers.groups[index];
        let effort = tiers.efforts[tier];
        let _ = writeln!(
            out,
            "{},{},{},{}/{},{},{:.6},{:.6},{:.6},{},{},{},{},{},{}",
            rating_rank + 1,
            csv_text(&item.label()),
            csv_text(&item.label_right()),
            tier + 1,
            tier_count,
            majority_rank[index],
            item.rating,
            item.deviation,
            effort,
            item.matches,
            score,
            wins,
            losses,
            ties,
            unseen,
        );
    }
    write_text(path, out)
}

/// Quote one CSV text field and escape inner quotes.
fn csv_text(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Create the destination directory and write one generated text file.
fn write_text(path: &Path, text: String) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }
    std::fs::write(path, text).into_diagnostic()
}

/// Full 15×15 tier table: ranked pairs tiered; a double press (same key
/// twice, e.g. QQ) is estimated from key strength — the mean rating of all
/// ranked pairs touching that key — mapped onto the easy third of the group
/// scale: strongest key → group 0, weakest → `groups / 3`.
fn pair_groups(state: &RankState, tiers: &Tiers) -> Vec<Vec<usize>> {
    let slots = HAND_SLOTS as usize;
    let mut grid = vec![vec![0usize; slots]; slots];

    for (idx, item) in state.items.iter().enumerate() {
        grid[item.from as usize][item.to as usize] = tiers.groups[idx];
    }

    // Key strength: mean rating over every pair the key participates in.
    let mut sums = vec![(0.0f64, 0usize); slots];

    for item in &state.items {
        for slot in [item.from as usize, item.to as usize] {
            sums[slot].0 += item.rating;
            sums[slot].1 += 1;
        }
    }

    let strength: Vec<f64> = sums
        .iter()
        .map(|(sum, n)| sum / (*n).max(1) as f64)
        .collect();

    let (min, max) = strength
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &s| {
            (lo.min(s), hi.max(s))
        });
    let span = (max - min).max(f64::EPSILON);
    let cap = (tiers.efforts.len() / 3) as f64;

    for (slot, row) in grid.iter_mut().enumerate() {
        // Strong (high mean rating) → cheap double.
        row[slot] = ((max - strength[slot]) / span * cap).round() as usize;
    }
    grid
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
    fn tier_efforts_anchor_endpoints_and_track_rating_gaps() {
        let cfg = RankConfig::default();
        let mut state = RankState::new();
        for (item, rating) in state
            .items
            .iter_mut()
            .zip([2000.0, 2000.0, 1500.0, 1000.0, 1000.0, 1000.0])
        {
            item.rating = rating;
        }
        // Tier means: 2000, 1500, 1000 → t = 0, 0.5, 1.
        let efforts = tier_efforts(&state, &[0, 0, 1, 2, 2, 2], 3, &cfg);
        assert_eq!(efforts, vec![1.0, 5.5, 10.0]);
    }

    #[test]
    fn tier_efforts_gamma_bends_curve_between_pinned_endpoints() {
        let cfg = RankConfig {
            effort_gamma: 2.0,
            ..Default::default()
        };
        let mut state = RankState::new();
        for (item, rating) in state.items.iter_mut().zip([2000.0, 1500.0, 1000.0]) {
            item.rating = rating;
        }
        // Middle tier: t = 0.5 → 0.25 after gamma → 1 + 9 · 0.25.
        let efforts = tier_efforts(&state, &[0, 1, 2], 3, &cfg);
        assert_eq!(efforts, vec![1.0, 3.25, 10.0]);
    }

    #[test]
    fn tier_efforts_single_tier_uses_effort_min() {
        let cfg = RankConfig::default();
        let state = ranked_state();
        let efforts = tier_efforts(&state, &[0, 0], 1, &cfg);
        assert_eq!(efforts, vec![cfg.effort_min]);
    }

    #[test]
    fn double_press_tracks_key_strength() {
        let state = ranked_state();
        let tier_count = 6;
        // Create tiers directly with a simple assignment: each item to tier = (index * tier_count) / items_len
        let groups: Vec<usize> = (0..state.items.len())
            .map(|i| (i * tier_count) / state.items.len())
            .collect();
        let tiers = Tiers {
            efforts: (0..tier_count).map(|effort| effort as f64).collect(),
            groups,
        };
        let grid = pair_groups(&state, &tiers);
        let cap = tiers.efforts.len() / 3;
        // Items are ordered by (from, to); earlier slots hold higher ratings,
        // so key strength decreases with the slot index.
        let doubles: Vec<usize> = (0..HAND_SLOTS as usize).map(|s| grid[s][s]).collect();
        assert!(doubles.iter().all(|&g| g <= cap));
        assert!(doubles.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(doubles[0], 0);
        assert_eq!(*doubles.last().unwrap(), cap);
    }

    #[test]
    fn written_json_parses_as_keyboard() {
        let state = ranked_state();
        let cfg = RankConfig::default();
        let tiers = tiers(&state, &cfg);

        let dir = std::env::temp_dir().join("keyvolve-rank-out-test");
        std::fs::create_dir_all(&dir).unwrap();
        let json = dir.join("generated/json/keyboard.json");
        write_keyboard_json(&json, &state, &tiers).unwrap();
        let loaded = crate::models::Keyboard::load(&json).unwrap();
        assert_eq!(loaded.efforts.len(), state.confidence_tier_count(&cfg));
        assert_eq!(loaded.pairs.len(), 30); // left + mirrored right
        assert!(loaded.pairs[&0].len() == 15);

        let csv = dir.join("generated/reports/keyboard.csv");
        write_report_csv(&csv, &state, &tiers).unwrap();
        let text = std::fs::read_to_string(&csv).unwrap();
        assert_eq!(text.matches("from: ").count(), 15);

        let bigrams = dir.join("generated/reports/keyboard.bigrams.csv");
        write_bigrams_csv(&bigrams, &state, &tiers).unwrap();
        let text = std::fs::read_to_string(&bigrams).unwrap();
        assert_eq!(text.lines().count(), state.items.len() + 1);
        assert!(text.starts_with("rating_rank,"));
        assert!(text.contains(",\"QW\",\"PO\","));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn flat_bigrams_csv_keeps_rating_and_majority_columns_distinct() {
        let mut state = RankState::new();
        state.items[0].rating = 2_000.0;
        state.items[1].rating = 1_500.0;
        state.items[2].rating = 1_000.0;
        for (index, item) in state.items.iter_mut().enumerate().skip(3) {
            item.rating = 900.0 - index as f64;
        }
        state.history.push(crate::app::rank::Answer {
            a: 1,
            b: 0,
            score: 1.0,
        });
        state.history.push(crate::app::rank::Answer {
            a: 0,
            b: 2,
            score: 1.0,
        });
        state.history.push(crate::app::rank::Answer {
            a: 1,
            b: 2,
            score: 1.0,
        });
        let cfg = RankConfig::default();
        let tiers = tiers(&state, &cfg);

        let dir = std::env::temp_dir().join("keyvolve-rank-bigram-export-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keyboard.bigrams.csv");
        write_bigrams_csv(&path, &state, &tiers).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let row0 = text
            .lines()
            .find(|line| line.contains(",\"QW\","))
            .expect("QW row exists");
        let row1 = text
            .lines()
            .find(|line| line.contains(",\"QE\","))
            .expect("QE row exists");
        let cols0 = row0.split(',').collect::<Vec<_>>();
        let cols1 = row1.split(',').collect::<Vec<_>>();
        // Column order: rating_rank,bigram,mirror,tier,majority_rank,...
        // Item 0 (QW): rating_rank=1, majority_rank=2 (2nd in majority: 1>0>2)
        // Item 1 (QE): rating_rank=2, majority_rank=1 (1st in majority: 1>0>2)
        assert_eq!(cols0[0], "1"); // QW rating_rank
        assert_eq!(
            cols0[3],
            format!("{}/{}", tiers.groups[0] + 1, tiers.efforts.len())
        );
        assert_eq!(cols0[4], "2"); // QW majority_rank
        assert_eq!(cols1[0], "2"); // QE rating_rank
        assert_eq!(cols1[4], "1"); // QE majority_rank
        std::fs::remove_dir_all(&dir).ok();
    }
}
