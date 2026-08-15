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

/// Build final tiers with rating-anchored effort values. A configured
/// `tierCount` fixes the count; otherwise tiers adapt to confidence.
pub fn tiers(state: &RankState, cfg: &RankConfig) -> Tiers {
    let (groups, count) = tier_groups(state, cfg);
    if count == 0 {
        return Tiers {
            efforts: vec![],
            groups,
        };
    }
    let efforts = tier_efforts(state, &groups, count, cfg);
    Tiers { efforts, groups }
}

/// Tier assignment + tier count: fixed optimal partition when `tierCount`
/// is set, adaptive confidence tiers otherwise.
pub fn tier_groups(state: &RankState, cfg: &RankConfig) -> (Vec<usize>, usize) {
    let groups = match cfg.tier_count {
        Some(k) if !state.items.is_empty() => fixed_tiers(state, k),
        _ => state.confidence_tiers(cfg),
    };
    let count = groups.iter().max().map_or(0, |&tier| tier + 1);
    (groups, count)
}

/// Tier id per item for a fixed tier count: the optimal same-count partition
/// (exact 1D k-means) over fitted ratings; tier 0 = best.
fn fixed_tiers(state: &RankState, count: usize) -> Vec<usize> {
    let mut order = (0..state.items.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        state.items[a]
            .rating
            .total_cmp(&state.items[b].rating)
            .then_with(|| a.cmp(&b))
    });
    let sorted: Vec<f64> = order.iter().map(|&i| state.items[i].rating).collect();
    let ids = ckmeans_partition(&sorted, count);
    // Ids ascend with rating; flip so tier 0 = highest rating.
    let top = ids.last().copied().unwrap_or(0);
    let mut groups = vec![0usize; state.items.len()];
    for (&item, &id) in order.iter().zip(&ids) {
        groups[item] = top - id;
    }
    groups
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

    // Sum of efforts per target slot, merged across every `from` slot.
    let mut sums = vec![(0.0f64, 0usize); HAND_SLOTS as usize];
    for item in &state.items {
        let slot = item.to as usize;
        sums[slot].0 += tiers.efforts[grid[item.from as usize][slot]];
        sums[slot].1 += 1;
    }
    let merged_efforts: Vec<f64> = sums.into_iter().map(|(sum, _count)| sum).collect();
    // Double-press efforts + summed efforts: two 3×5 matrices on the same rows.
    let _ = writeln!(out, "doubles:,,,,,,summed-efforts:");
    for row in 0..3u8 {
        let _ = writeln!(
            out,
            "{},,{}",
            (0..5u8)
                .map(|col| {
                    format!(
                        "{:.2}",
                        tiers.efforts[grid[(row * 5 + col) as usize][(row * 5 + col) as usize]]
                    )
                })
                .collect::<Vec<_>>()
                .join(","),
            (0..5u8)
                .map(|col| format!("{:.2}", merged_efforts[(row * 5 + col) as usize]))
                .collect::<Vec<_>>()
                .join(",")
        );
    }

    write_text(path, out)
}

/// Write flat per-bigram CSV sorted by fitted rating, with majority summary.
pub fn write_bigrams_csv(path: &Path, state: &RankState, tiers: &Tiers) -> Result<()> {
    let majority = majority_stats(state);
    let majority_rank = majority_ranks(state, &majority);

    let mut rating_order = (0..state.items.len()).collect::<Vec<_>>();
    rating_order.sort_by(|&a, &b| {
        state.items[b]
            .rating
            .total_cmp(&state.items[a].rating)
            .then_with(|| a.cmp(&b))
    });

    let tier_count = tiers.efforts.len();

    // Trivial staggered-grid distance per pair.
    let distances: Vec<f64> = state
        .items
        .iter()
        .map(|item| slot_distance(item.from, item.to))
        .collect();

    let mut out = String::from(
        "rating_rank,bigram,mirror,tier,majority_rank,rating,deviation,effort,distance,matches,majority_score,majority_wins,majority_losses,majority_ties,majority_unseen\n",
    );
    for (rating_rank, &index) in rating_order.iter().enumerate() {
        let item = &state.items[index];
        let (wins, losses, ties, unseen) = majority[index];
        let score = wins as isize - losses as isize;
        let tier = tiers.groups[index];
        let effort = tiers.efforts[tier];
        let _ = writeln!(
            out,
            "{},{},{},{}/{},{},{:.6},{:.6},{:.6},{:.3},{},{},{},{},{},{}",
            rating_rank + 1,
            csv_text(&item.label()),
            csv_text(&item.label_right()),
            tier + 1,
            tier_count,
            majority_rank[index],
            item.rating,
            item.deviation,
            effort,
            distances[index],
            item.matches,
            score,
            wins,
            losses,
            ties,
            unseen,
        );
    }

    // Rank/distance alignment: positive = worse-ranked pairs sit farther
    // apart on the trivial staggered grid.
    let _ = writeln!(
        out,
        "spearman_rating_vs_distance,{:.4}",
        distance_correlation(state)
    );
    // Tier boundary quality: current vs optimal same-count partition.
    let (r2, optimal) = tier_quality(state, &tiers.groups, tier_count);
    let _ = writeln!(out, "tier_r2,{r2:.4},{optimal:.4}");
    write_text(path, out)
}

/// Tier boundary quality over fitted ratings: R² of the given tier assignment
/// and of the optimal same-count 1D k-means partition (Ckmeans DP). The gap
/// between the two isolates boundary placement quality from tier count.
/// See docs/rank-mode.md "Reading the tier quality line".
pub fn tier_quality(state: &RankState, groups: &[usize], count: usize) -> (f64, f64) {
    if count == 0 || state.items.is_empty() {
        return (1.0, 1.0);
    }
    let ratings: Vec<f64> = state.items.iter().map(|item| item.rating).collect();
    let current = variance_explained(&ratings, groups, count);
    let mut sorted = ratings;
    sorted.sort_by(f64::total_cmp);
    let optimal = variance_explained(&sorted, &ckmeans_partition(&sorted, count), count);
    (current, optimal)
}

/// Spearman rho of the fitted rating order against trivial slot distance,
/// oriented so positive = worse-rated pairs sit physically farther apart.
pub fn distance_correlation(state: &RankState) -> f64 {
    let distances: Vec<f64> = state
        .items
        .iter()
        .map(|item| slot_distance(item.from, item.to))
        .collect();
    // Negated rating: low rating (worse pair) → high value, matching rank order.
    let rating_key: Vec<f64> = state.items.iter().map(|item| -item.rating).collect();
    spearman(&rating_key, &distances)
}

/// 1-based majority rank per item derived from majority stats.
pub fn majority_ranks(state: &RankState, majority: &[(usize, usize, usize, usize)]) -> Vec<usize> {
    let mut order = (0..state.items.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        let score_a = majority[a].0 as isize - majority[a].1 as isize;
        let score_b = majority[b].0 as isize - majority[b].1 as isize;
        score_b
            .cmp(&score_a)
            .then_with(|| majority[b].0.cmp(&majority[a].0))
            .then_with(|| majority[a].1.cmp(&majority[b].1))
            .then_with(|| state.items[b].rating.total_cmp(&state.items[a].rating))
            .then_with(|| a.cmp(&b))
    });
    let mut ranks = vec![0usize; state.items.len()];
    for (rank, &index) in order.iter().enumerate() {
        ranks[index] = rank + 1;
    }
    ranks
}

/// Per-item majority stats (wins, losses, ties, unseen) over head-to-head answers.
pub fn majority_stats(state: &RankState) -> Vec<(usize, usize, usize, usize)> {
    let edges = majority_edges(state);
    let head_to_head = head_to_head(state);
    let mut losses = vec![0usize; state.items.len()];
    for losers in &edges {
        for &loser in losers {
            losses[loser] += 1;
        }
    }
    let mut compared = vec![0usize; state.items.len()];
    for &(a, b) in head_to_head.keys() {
        compared[a] += 1;
        compared[b] += 1;
    }
    (0..state.items.len())
        .map(|index| {
            let wins = edges[index].len();
            let ties = compared[index].saturating_sub(wins + losses[index]);
            let unseen = state.items.len().saturating_sub(1 + compared[index]);
            (wins, losses[index], ties, unseen)
        })
        .collect()
}

/// Euclidean distance between two slots on a staggered 3×5 grid
/// (standard row stagger: top 0.0, home 0.25, bottom 0.75; unit key pitch).
pub fn slot_distance(from: u8, to: u8) -> f64 {
    const STAGGER: [f64; 3] = [0.0, 0.25, 0.75];
    let pos = |s: u8| ((s % 5) as f64 + STAGGER[(s / 5) as usize], (s / 5) as f64);
    let ((xa, ya), (xb, yb)) = (pos(from), pos(to));
    (xa - xb).hypot(ya - yb)
}

/// Spearman rank correlation with average ranks for ties
/// (Pearson correlation on tie-averaged ranks).
pub fn spearman(a: &[f64], b: &[f64]) -> f64 {
    let (ra, rb) = (tie_ranks(a), tie_ranks(b));
    let n = ra.len() as f64;
    let (ma, mb) = (ra.iter().sum::<f64>() / n, rb.iter().sum::<f64>() / n);
    let (mut cov, mut va, mut vb) = (0.0, 0.0, 0.0);
    for (x, y) in ra.iter().zip(&rb) {
        let (dx, dy) = (x - ma, y - mb);
        cov += dx * dy;
        va += dx * dx;
        vb += dy * dy;
    }
    cov / (va * vb).sqrt().max(f64::EPSILON)
}

/// 1-based ranks with ties averaged (e.g. two equal smallest values → 1.5, 1.5).
fn tie_ranks(values: &[f64]) -> Vec<f64> {
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start;
        while end + 1 < order.len() && values[order[end + 1]] == values[order[start]] {
            end += 1;
        }
        let rank = (start + end) as f64 / 2.0 + 1.0;
        for &index in &order[start..=end] {
            ranks[index] = rank;
        }
        start = end + 1;
    }
    ranks
}

/// Share of value variance explained by group means: 1 − SSE_within / SST.
fn variance_explained(values: &[f64], groups: &[usize], count: usize) -> f64 {
    let mut sums = vec![(0.0f64, 0usize); count];
    for (&value, &group) in values.iter().zip(groups) {
        sums[group].0 += value;
        sums[group].1 += 1;
    }
    let means: Vec<f64> = sums
        .iter()
        .map(|(sum, n)| sum / (*n).max(1) as f64)
        .collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let (sse, sst) = values
        .iter()
        .zip(groups)
        .fold((0.0, 0.0), |(sse, sst), (&value, &group)| {
            (
                sse + (value - means[group]).powi(2),
                sst + (value - mean).powi(2),
            )
        });
    1.0 - sse / sst.max(f64::EPSILON)
}

/// Exact 1D k-means (Ckmeans DP) on sorted values → cluster id per index,
/// ascending. O(k·n²) — fine for 210 items.
fn ckmeans_partition(sorted: &[f64], k: usize) -> Vec<usize> {
    let n = sorted.len();
    let k = k.min(n);
    let (mut sum, mut sq) = (vec![0.0; n + 1], vec![0.0; n + 1]);
    for (i, &value) in sorted.iter().enumerate() {
        sum[i + 1] = sum[i] + value;
        sq[i + 1] = sq[i] + value * value;
    }
    // Within-cluster sum of squared deviations for the half-open range [lo, hi).
    let sse = |lo: usize, hi: usize| {
        let (s, len) = (sum[hi] - sum[lo], (hi - lo) as f64);
        sq[hi] - sq[lo] - s * s / len
    };
    let mut dp = vec![vec![f64::INFINITY; n + 1]; k + 1];
    let mut cut = vec![vec![0usize; n + 1]; k + 1];
    dp[0][0] = 0.0;
    for j in 1..=k {
        for i in j..=n {
            for m in (j - 1)..i {
                let cost = dp[j - 1][m] + sse(m, i);
                if cost < dp[j][i] {
                    dp[j][i] = cost;
                    cut[j][i] = m;
                }
            }
        }
    }
    let mut ids = vec![0usize; n];
    let (mut i, mut j) = (n, k);
    while j > 0 {
        let m = cut[j][i];
        ids[m..i].iter_mut().for_each(|id| *id = j - 1);
        i = m;
        j -= 1;
    }
    ids
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
        assert!(text.contains("doubles:,,,,,,summed-efforts:"));

        let bigrams = dir.join("generated/reports/keyboard.bigrams.csv");
        write_bigrams_csv(&bigrams, &state, &tiers).unwrap();
        let text = std::fs::read_to_string(&bigrams).unwrap();
        // items + header + spearman row + tier_r2 row
        assert_eq!(text.lines().count(), state.items.len() + 3);
        assert!(text.starts_with("rating_rank,"));
        assert!(text.contains(",\"QW\",\"PO\","));
        assert!(text.contains("spearman_rating_vs_distance,"));
        assert!(!text.contains("spearman_majority_vs_distance"));
        assert!(text.contains("tier_r2,"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn slot_distance_follows_staggered_grid() {
        // Same-row neighbors: Q(0) → W(1).
        assert_eq!(slot_distance(0, 1), 1.0);
        // Symmetric.
        assert_eq!(slot_distance(3, 12), slot_distance(12, 3));
        // Stagger shifts cross-row distance: Q(0) → A(5) = hypot(0.25, 1).
        assert!((slot_distance(0, 5) - 0.25f64.hypot(1.0)).abs() < 1e-12);
        // Q(0) → Z(10) = hypot(0.75, 2).
        assert!((slot_distance(0, 10) - 0.75f64.hypot(2.0)).abs() < 1e-12);
    }

    #[test]
    fn spearman_detects_agreement_reversal_and_ties() {
        let a = [1.0, 2.0, 3.0, 4.0];
        assert!((spearman(&a, &[10.0, 20.0, 30.0, 40.0]) - 1.0).abs() < 1e-12);
        assert!((spearman(&a, &[40.0, 30.0, 20.0, 10.0]) + 1.0).abs() < 1e-12);
        // Ties get averaged ranks; still monotone overall → positive rho.
        assert!(spearman(&a, &[1.0, 1.0, 2.0, 3.0]) > 0.9);
    }

    #[test]
    fn tie_ranks_average_equal_values() {
        assert_eq!(tie_ranks(&[3.0, 1.0, 1.0, 2.0]), vec![4.0, 1.5, 1.5, 3.0]);
    }

    #[test]
    fn distance_correlation_tracks_rating_alignment() {
        let mut state = RankState::new();
        // Rating = −distance → perfect alignment (worse pair = farther).
        for item in state.items.iter_mut() {
            item.rating = 2000.0 - 100.0 * slot_distance(item.from, item.to);
        }
        assert!((distance_correlation(&state) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ckmeans_splits_at_natural_gap() {
        let sorted = [1.0, 1.1, 1.2, 9.0, 9.1, 9.2];
        assert_eq!(ckmeans_partition(&sorted, 2), vec![0, 0, 0, 1, 1, 1]);
        // k capped at n; ids ascending.
        let ids = ckmeans_partition(&sorted, 10);
        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn variance_explained_ranks_partitions() {
        let values = [1.0, 1.1, 9.0, 9.1];
        let good = variance_explained(&values, &[0, 0, 1, 1], 2);
        let bad = variance_explained(&values, &[0, 1, 0, 1], 2);
        assert!(good > 0.99);
        assert!(bad < good);
        // Single group explains nothing.
        assert!(variance_explained(&values, &[0, 0, 0, 0], 1).abs() < 1e-12);
    }

    #[test]
    fn tier_quality_optimal_bounds_current() {
        let cfg = RankConfig::default();
        let mut state = RankState::new();
        for (i, item) in state.items.iter_mut().enumerate() {
            // Two lumps with noise → non-trivial boundaries.
            item.rating = if i % 3 == 0 { 1800.0 } else { 1200.0 } + (i % 7) as f64 * 10.0;
        }
        let groups = state.confidence_tiers(&cfg);
        let count = state.confidence_tier_count(&cfg);
        let (current, optimal) = tier_quality(&state, &groups, count);
        assert!(optimal >= current - 1e-12);
        assert!(optimal <= 1.0 + 1e-12);
    }

    #[test]
    fn fixed_tier_count_overrides_adaptive_splitting() {
        let cfg = RankConfig {
            tier_count: Some(5),
            ..Default::default()
        };
        let state = ranked_state();
        let tiers = tiers(&state, &cfg);
        assert_eq!(tiers.efforts.len(), 5);
        // Tier 0 = best rating; last tier = worst.
        let best = state
            .items
            .iter()
            .enumerate()
            .max_by(|(_, x), (_, y)| x.rating.total_cmp(&y.rating))
            .unwrap()
            .0;
        assert_eq!(tiers.groups[best], 0);
        let (groups, count) = tier_groups(&state, &cfg);
        assert_eq!(count, 5);
        assert_eq!(groups, tiers.groups);
    }

    #[test]
    fn fixed_tiers_split_at_natural_gap() {
        let mut state = RankState::new();
        for (i, item) in state.items.iter_mut().enumerate() {
            // Two lumps: first 100 items strong, rest weak.
            item.rating = if i < 100 { 2000.0 } else { 1000.0 } + (i % 5) as f64;
        }
        let groups = fixed_tiers(&state, 2);
        assert!((0..100).all(|i| groups[i] == 0));
        assert!((100..state.items.len()).all(|i| groups[i] == 1));
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
