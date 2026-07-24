use super::fit::{expected_score, information_score};
use crate::app::rank::{RankConfig, RankState};
use rand::RngExt;
use rand::seq::SliceRandom;
use std::collections::{BTreeMap, VecDeque};

/// How the pair was chosen — affects contradiction handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    /// Uncertain pair — normal rating refinement.
    Explore,
    /// Settled pair — transitivity/consistency check.
    Audit,
}

/// Minimum rating gap for a meaningful audit question.
const AUDIT_GAP: f64 = 200.0;
/// Candidate pool size for random tie-breaking.
const POOL: usize = 10;
/// Confidence required before one answer can contradict the fitted order.
const CONTRADICTION_Z: f64 = 1.96;
const MIN_RESIDUAL_VARIANCE: f64 = 1e-6;

/// Pick the next question: `(a, b, kind)` — item indexes into `state.items`.
/// In verify mode (finished session) every question is an audit check.
pub fn pick(
    state: &RankState,
    cfg: &RankConfig,
    rng: &mut impl RngExt,
) -> Option<(usize, usize, PickKind)> {
    let audit = state.finished || rng.random_bool(cfg.audit_rate.clamp(0.0, 1.0));
    if audit && let Some(pair) = pick_audit(state, cfg, rng) {
        return Some((pair.0, pair.1, PickKind::Audit));
    }
    if let Some((a, b)) = pick_explore(state, cfg, rng) {
        return Some((a, b, PickKind::Explore));
    }
    pick_audit(state, cfg, rng).map(|(a, b)| (a, b, PickKind::Audit))
}

/// Audit: two settled same-start items, preferring residuals then clear gaps.
fn pick_audit(
    state: &RankState,
    cfg: &RankConfig,
    rng: &mut impl RngExt,
) -> Option<(usize, usize)> {
    let settled_flags = state.settled_flags(cfg);
    // Recheck direct comparisons least compatible with the global model.
    let mut residuals = BTreeMap::<(usize, usize), (f64, usize)>::new();
    for answer in &state.history {
        if !shares_key(state, answer.a, answer.b)
            || !settled_flags[answer.a]
            || !settled_flags[answer.b]
        {
            continue;
        }
        let predicted = expected_score(state.items[answer.a].rating, state.items[answer.b].rating);
        let key = (answer.a.min(answer.b), answer.a.max(answer.b));
        let entry = residuals.entry(key).or_default();
        entry.0 += pearson_residual(answer.score, predicted);
        entry.1 += 1;
    }
    let mut ranked = residuals
        .into_iter()
        .map(|(pair, (total, count))| (pair, total / count as f64))
        .collect::<Vec<_>>();
    ranked.shuffle(rng);
    ranked.sort_by(|(_, a), (_, b)| b.total_cmp(a));
    if !ranked.is_empty() {
        return Some(ranked[rng.random_range(0..POOL.min(ranked.len()))].0);
    }

    // Legacy/fresh fallback when no comparable residual exists.
    let settled: Vec<usize> = (0..state.items.len())
        .filter(|&i| settled_flags[i])
        .collect();
    let mut pairs = settled
        .iter()
        .enumerate()
        .flat_map(|(position, &a)| {
            settled[position + 1..]
                .iter()
                .copied()
                .filter(move |&b| shares_key(state, a, b))
                .map(move |b| {
                    let gap = (state.items[a].rating - state.items[b].rating).abs();
                    (a, b, gap)
                })
        })
        .collect::<Vec<_>>();
    pairs.shuffle(rng);
    pairs.sort_by(|(_, _, a), (_, _, b)| b.total_cmp(a));
    let far = pairs
        .iter()
        .take_while(|(_, _, gap)| *gap >= AUDIT_GAP)
        .count();
    let top = POOL.min(if far == 0 { pairs.len() } else { far });
    let &(a, b, _) = pairs.get(rng.random_range(0..top.max(1)))?;
    Some((a, b))
}

/// Explore: maximize expected Fisher information while both items need work.
fn pick_explore(
    state: &RankState,
    cfg: &RankConfig,
    rng: &mut impl RngExt,
) -> Option<(usize, usize)> {
    let settled = state.settled_flags(cfg);
    let unsettled = (0..state.items.len())
        .filter(|&i| !settled[i])
        .collect::<Vec<_>>();
    let n = state.items.len();
    let mut comparisons = vec![0usize; n * n];
    for answer in &state.history {
        comparisons[answer.a * n + answer.b] += 1;
        comparisons[answer.b * n + answer.a] += 1;
    }
    let mut pairs = informative_pairs(state, &unsettled, true, &comparisons);
    if pairs.is_empty() {
        pairs = informative_pairs(state, &unsettled, false, &comparisons);
    }
    pairs.shuffle(rng);
    let top = POOL.min(pairs.len());
    if top == 0 {
        return None;
    }
    pairs.select_nth_unstable_by(top - 1, |(_, _, a), (_, _, b)| b.total_cmp(a));
    let &(a, b, _) = &pairs[rng.random_range(0..top)];
    Some((a, b))
}

/// Candidate same-start comparisons, preferring two unfinished items.
fn informative_pairs(
    state: &RankState,
    unsettled: &[usize],
    both_unsettled: bool,
    comparisons: &[usize],
) -> Vec<(usize, usize, f64)> {
    let n = state.items.len();
    let opponent = |i: usize| !both_unsettled || unsettled.contains(&i);
    unsettled
        .iter()
        .flat_map(|&a| {
            (0..state.items.len())
                .filter(move |&b| {
                    a != b && (!both_unsettled || a < b) && opponent(b) && shares_key(state, a, b)
                })
                .map(move |b| {
                    let (x, y) = (&state.items[a], &state.items[b]);
                    let repeats = comparisons[a * n + b] as f64;
                    let information =
                        information_score(x.rating, y.rating, state.difference_deviation(a, b))
                            / (1.0 + repeats * 0.25);
                    (a, b, information)
                })
        })
        .collect()
}

/// True when the two items start from the same physical key.
fn shares_key(state: &RankState, a: usize, b: usize) -> bool {
    state.items[a].from == state.items[b].from
}

/// Squared Pearson residual; expected value is one for a calibrated binary fit.
fn pearson_residual(score: f64, predicted: f64) -> f64 {
    let variance = (predicted * (1.0 - predicted)).max(MIN_RESIDUAL_VARIANCE);
    (score - predicted).powi(2) / variance
}

/// True when an answer contradicts a confidently ordered pair.
pub fn contradicts(state: &RankState, a: usize, b: usize, score: f64) -> bool {
    let gap = state.items[a].rating - state.items[b].rating;
    if gap.abs() <= CONTRADICTION_Z * state.difference_deviation(a, b) {
        return false;
    }
    score == 0.5 || (score > 0.5 && gap < 0.0) || (score < 0.5 && gap > 0.0)
}

/// Shortest majority-preference cycle through `winner → loser`, if any.
/// Nodes are item indexes; first and last entry are both `winner`.
pub fn find_cycle(state: &RankState, winner: usize, loser: usize) -> Option<Vec<usize>> {
    let edges = majority_edges(state);
    // The fresh answer may not have flipped the head-to-head majority.
    if !edges[winner].contains(&loser) {
        return None;
    }
    // BFS from loser back to winner finds the shortest return path.
    let mut previous = vec![usize::MAX; edges.len()];
    previous[loser] = loser;
    let mut queue = VecDeque::from([loser]);
    while let Some(node) = queue.pop_front() {
        for &next in &edges[node] {
            if previous[next] != usize::MAX {
                continue;
            }
            previous[next] = node;
            if next == winner {
                let mut path = vec![winner];
                let mut node = winner;
                while node != loser {
                    node = previous[node];
                    path.push(node);
                }
                path.push(winner);
                path.reverse();
                return Some(path);
            }
            queue.push_back(next);
        }
    }
    None
}

/// Directed majority edges (winner → loser) from head-to-head history.
fn majority_edges(state: &RankState) -> Vec<Vec<usize>> {
    let mut totals = BTreeMap::<(usize, usize), (f64, usize)>::new();
    for answer in &state.history {
        let (lo, hi) = (answer.a.min(answer.b), answer.a.max(answer.b));
        let score = if answer.a == lo {
            answer.score
        } else {
            1.0 - answer.score
        };
        let entry = totals.entry((lo, hi)).or_default();
        entry.0 += score;
        entry.1 += 1;
    }
    let mut edges = vec![Vec::new(); state.items.len()];
    for ((lo, hi), (wins, count)) in totals {
        let half = count as f64 / 2.0;
        if wins > half {
            edges[lo].push(hi);
        } else if wins < half {
            edges[hi].push(lo);
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::rank::{Answer, bucketize};
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    #[test]
    fn cycle_detected_and_absent() {
        let mut state = RankState::new();
        state.answer(0, 1, 1.0).unwrap(); // 0 beats 1
        state.answer(1, 2, 1.0).unwrap(); // 1 beats 2
        assert_eq!(find_cycle(&state, 1, 2), None);
        state.answer(2, 0, 1.0).unwrap(); // 2 beats 0 → cycle
        assert_eq!(find_cycle(&state, 2, 0), Some(vec![2, 0, 1, 2]));
        // Majority flips back: 0 beats 2 twice more → no edge, no cycle.
        state.answer(0, 2, 1.0).unwrap();
        state.answer(0, 2, 1.0).unwrap();
        assert_eq!(find_cycle(&state, 2, 0), None);
    }

    #[test]
    fn explore_prefers_unplayed_items() {
        let mut state = RankState::new();
        // Play everything except item 0 heavily.
        for i in 1..state.items.len() {
            state.items[i].matches = 100;
            state.items[i].deviation = 50.0;
        }
        let mut rng = StdRng::seed_from_u64(1);
        let cfg = RankConfig {
            audit_rate: 0.0,
            ..Default::default()
        };
        let picked: Vec<_> = (0..20)
            .map(|_| pick(&state, &cfg, &mut rng).unwrap())
            .collect();
        assert!(picked.iter().any(|&(a, b, _)| a == 0 || b == 0));
        assert!(picked.iter().all(|&(_, _, k)| k == PickKind::Explore));
    }

    #[test]
    fn audit_picks_settled_far_apart_pairs() {
        let mut state = RankState::new();
        for (i, item) in state.items.iter_mut().enumerate() {
            item.matches = 100;
            item.deviation = 50.0;
            item.rating = 1000.0 + i as f64 * 10.0;
        }
        let mut rng = StdRng::seed_from_u64(2);
        let cfg = RankConfig {
            audit_rate: 1.0,
            ..Default::default()
        };
        let (a, b, kind) = pick(&state, &cfg, &mut rng).unwrap();
        assert_eq!(kind, PickKind::Audit);
        // Same-start groups span 130 rating points here; expect a wide gap pick.
        assert!((state.items[a].rating - state.items[b].rating).abs() >= 100.0);
        assert!(shares_key(&state, a, b));
    }

    #[test]
    fn audit_targets_contradictory_history() {
        let mut state = RankState::new();
        for item in &mut state.items {
            item.matches = 100;
            item.deviation = 50.0;
        }
        state.items[0].rating = 2_000.0;
        state.items[1].rating = 1_000.0;
        state.history.push(Answer {
            a: 0,
            b: 1,
            score: 0.0,
            prev_a: (1_500.0, 350.0, 0),
            prev_b: (1_500.0, 350.0, 0),
            prev_pending_a: 0,
            prev_pending_b: 0,
        });
        let cfg = RankConfig::default();
        let mut rng = StdRng::seed_from_u64(5);
        assert_eq!(pick_audit(&state, &cfg, &mut rng), Some((0, 1)));
    }

    #[test]
    fn explore_pairs_share_a_key() {
        let state = RankState::new();
        let cfg = RankConfig::default();
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..50 {
            let (a, b) = pick_explore(&state, &cfg, &mut rng).unwrap();
            assert!(shares_key(&state, a, b));
        }
    }

    #[test]
    fn explore_prefers_unsettled_opponents() {
        let mut state = RankState::new();
        // Everything settled except items 0 and 1 (which share key 0).
        for item in state.items.iter_mut().skip(2) {
            item.matches = 100;
            item.deviation = 50.0;
        }
        let cfg = RankConfig::default();
        let mut rng = StdRng::seed_from_u64(11);
        let settled = state.settled_flags(&cfg);
        for _ in 0..20 {
            let (a, b) = pick_explore(&state, &cfg, &mut rng).unwrap();
            // Both sides of the question still need answers.
            assert!(!settled[a]);
            assert!(!settled[b]);
        }
    }

    #[test]
    fn contradiction_detected() {
        let mut state = RankState::new();
        state.items[0].rating = 2000.0;
        state.items[1].rating = 1000.0;
        assert!(!contradicts(&state, 0, 1, 1.0)); // higher wins — consistent
        assert!(contradicts(&state, 0, 1, 0.0)); // higher loses — contradiction
        assert!(contradicts(&state, 0, 1, 0.5)); // confident gap ties — contradiction

        state.items[0].rating = 1501.0;
        state.items[1].rating = 1500.0;
        assert!(!contradicts(&state, 0, 1, 0.0)); // uncertain order — ordinary noise
    }

    #[test]
    fn pearson_residual_prioritizes_surprising_upsets() {
        assert!(pearson_residual(0.0, 0.9) > pearson_residual(0.0, 0.5));
    }

    #[test]
    fn finished_state_forces_audit_picks() {
        let mut state = RankState::new();
        for (i, item) in state.items.iter_mut().enumerate() {
            item.matches = 100;
            item.deviation = 50.0;
            item.rating = 1000.0 + i as f64 * 10.0;
        }
        state.finished = true;
        let mut rng = StdRng::seed_from_u64(3);
        let cfg = RankConfig {
            audit_rate: 0.0,
            ..Default::default()
        };
        for _ in 0..10 {
            let (_, _, kind) = pick(&state, &cfg, &mut rng).unwrap();
            assert_eq!(kind, PickKind::Audit);
        }
    }

    #[test]
    fn finished_equal_state_has_shared_audit_fallback() {
        let mut state = RankState::new();
        let cfg = RankConfig::default();
        for item in &mut state.items {
            item.matches = cfg.max_matches;
        }
        state.finished = true;
        let mut rng = StdRng::seed_from_u64(13);

        let (a, b, kind) = pick(&state, &cfg, &mut rng).unwrap();
        assert_eq!(kind, PickKind::Audit);
        assert!(shares_key(&state, a, b));
    }

    #[test]
    #[ignore = "statistical simulation benchmark"]
    fn simulation_benchmark() {
        // Batched refits keep the broad statistical benchmark fast.
        // Same-start-only pairs disconnect from-groups; cross-group order rests
        // on the prior, so accuracy sits below the old shared-key thresholds.
        for (max_matches, min_accuracy) in [(15, 0.4), (30, 0.5)] {
            for seed in [17, 29, 43] {
                let (answers, bucket_accuracy, rho) = simulate(seed, max_matches, 50);
                println!(
                    "cap={max_matches}: answers={answers}, adjacent-bucket={bucket_accuracy:.1}%, rho={rho:.3}",
                    bucket_accuracy = bucket_accuracy * 100.0,
                );
                assert!(answers <= 3_200);
                assert!(bucket_accuracy > min_accuracy);
                assert!(rho > 0.75);
            }
        }
    }

    #[test]
    #[ignore = "production-path simulation benchmark"]
    fn production_path_simulation_smoke() {
        let (answers, bucket_accuracy, rho) = simulate(59, 1, 1);
        println!(
            "exact: answers={answers}, adjacent-bucket={bucket_accuracy:.1}%, rho={rho:.3}",
            bucket_accuracy = bucket_accuracy * 100.0,
        );
        assert!(answers <= 210);
        assert!(bucket_accuracy.is_finite());
        assert!(rho.is_finite());
    }

    fn simulate(seed: u64, max_matches: u32, refit_every: usize) -> (usize, f64, f64) {
        assert!(refit_every > 0);
        let mut rng = StdRng::seed_from_u64(seed);
        let hidden = (0..210)
            .map(|_| rng.random_range(1_000.0..2_000.0))
            .collect::<Vec<_>>();
        let cfg = RankConfig {
            min_matches: max_matches.min(10),
            max_matches,
            ..Default::default()
        };
        cfg.validate().unwrap();
        let mut state = RankState::new();

        while state.settled_count(&cfg) < state.items.len() && state.history.len() < 3_200 {
            let (a, b, kind) = pick(&state, &cfg, &mut rng).unwrap();
            assert_eq!(kind, PickKind::Explore);
            let score = f64::from(rng.random_bool(expected_score(hidden[a], hidden[b])));
            if refit_every == 1 {
                state.answer(a, b, score).unwrap();
            } else {
                let snap = |i: usize| {
                    let item = &state.items[i];
                    (item.rating, item.deviation, item.matches)
                };
                state.history.push(Answer {
                    a,
                    b,
                    score,
                    prev_a: snap(a),
                    prev_b: snap(b),
                    prev_pending_a: 0,
                    prev_pending_b: 0,
                });
                state.items[a].matches += 1;
                state.items[b].matches += 1;
                if state.history.len().is_multiple_of(refit_every) {
                    state.refit();
                }
            }
        }
        state.refit();
        assert_eq!(state.settled_count(&cfg), state.items.len());

        let estimated = bucketize(&state, &cfg).groups;
        let mut truth = state.clone();
        for (item, &rating) in truth.items.iter_mut().zip(&hidden) {
            item.rating = rating;
        }
        let expected = bucketize(&truth, &cfg).groups;
        let bucket_accuracy = estimated
            .iter()
            .zip(expected)
            .filter(|(a, b)| a.abs_diff(*b) <= 1)
            .count() as f64
            / state.items.len() as f64;
        (
            state.history.len(),
            bucket_accuracy,
            spearman(&hidden, &state),
        )
    }

    fn spearman(hidden: &[f64], state: &RankState) -> f64 {
        let ranks = |values: &[f64]| {
            let mut order = (0..values.len()).collect::<Vec<_>>();
            order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
            let mut ranks = vec![0usize; values.len()];
            for (rank, index) in order.into_iter().enumerate() {
                ranks[index] = rank;
            }
            ranks
        };
        let actual = ranks(hidden);
        let fitted = ranks(
            &state
                .items
                .iter()
                .map(|item| item.rating)
                .collect::<Vec<_>>(),
        );
        let n = hidden.len() as f64;
        let squared = actual
            .iter()
            .zip(fitted)
            .map(|(&a, b)| (a as f64 - b as f64).powi(2))
            .sum::<f64>();
        1.0 - 6.0 * squared / (n * (n * n - 1.0))
    }
}
