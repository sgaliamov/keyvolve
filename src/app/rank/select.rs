use super::fit::{expected_score, information_score};
use crate::app::rank::{RankConfig, RankState};
use rand::RngExt;
use rand::seq::SliceRandom;
use std::collections::BTreeMap;

/// How the pair was chosen — affects contradiction handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    /// Uncertain pair — normal rating refinement.
    Explore,
    /// Settled, far-apart pair — transitivity/consistency check.
    Audit,
}

/// Minimum rating gap for a meaningful audit question.
const AUDIT_GAP: f64 = 200.0;
/// Candidate pool size for random tie-breaking.
const POOL: usize = 10;

/// Pick the next question: `(a, b, kind)` — item indexes into `state.items`.
/// In verify mode (finished session) every question is an audit check.
pub fn pick(
    state: &RankState,
    cfg: &RankConfig,
    rng: &mut impl RngExt,
) -> (usize, usize, PickKind) {
    let audit = state.finished || rng.random_bool(cfg.audit_rate.clamp(0.0, 1.0));
    if audit && let Some(pair) = pick_audit(state, cfg, rng) {
        return (pair.0, pair.1, PickKind::Audit);
    }
    let (a, b) = pick_explore(state, cfg, rng);
    (a, b, PickKind::Explore)
}

/// Audit: two settled items with a clear rating gap. None when unavailable.
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
        entry.0 += (answer.score - predicted).abs();
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
    if settled.len() < 2 {
        return None;
    }
    let a = settled[rng.random_range(0..settled.len())];
    let far: Vec<usize> = settled
        .iter()
        .copied()
        .filter(|&i| i != a && (state.items[i].rating - state.items[a].rating).abs() >= AUDIT_GAP)
        .collect();
    // Prefer an opponent sharing a key with the candidate; fall back to any far one.
    let shared: Vec<usize> = far
        .iter()
        .copied()
        .filter(|&i| shares_key(state, a, i))
        .collect();
    let pool = if shared.is_empty() { &far } else { &shared };
    let b = *pool.get(rng.random_range(0..pool.len().max(1)))?;
    Some((a, b))
}

/// Explore: maximize expected Fisher information while both items need work.
fn pick_explore(state: &RankState, cfg: &RankConfig, rng: &mut impl RngExt) -> (usize, usize) {
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
    pairs.select_nth_unstable_by(top - 1, |(_, _, a), (_, _, b)| b.total_cmp(a));
    let &(a, b, _) = &pairs[rng.random_range(0..top)];
    (a, b)
}

/// Candidate shared-key comparisons, preferring two unfinished items.
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

/// True when the two items share a physical key (from or to slot).
fn shares_key(state: &RankState, a: usize, b: usize) -> bool {
    let (x, y) = (&state.items[a], &state.items[b]);
    x.from == y.from || x.from == y.to || x.to == y.from || x.to == y.to
}

/// True when a settled-pair answer contradicts current ratings.
pub fn contradicts(state: &RankState, a: usize, b: usize, score: f64) -> bool {
    let gap = state.items[a].rating - state.items[b].rating;
    (score > 0.5 && gap < 0.0) || (score < 0.5 && gap > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::rank::{Answer, bucketize};
    use rand::{RngExt, SeedableRng, rngs::StdRng};

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
        let picked: Vec<_> = (0..20).map(|_| pick(&state, &cfg, &mut rng)).collect();
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
        let (a, b, kind) = pick(&state, &cfg, &mut rng);
        assert_eq!(kind, PickKind::Audit);
        assert!((state.items[a].rating - state.items[b].rating).abs() >= 200.0);
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
            let (a, b) = pick_explore(&state, &cfg, &mut rng);
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
        for _ in 0..20 {
            let (a, b) = pick_explore(&state, &cfg, &mut rng);
            // Both sides of the question still need answers.
            assert!(!state.item_settled(a, &cfg));
            assert!(!state.item_settled(b, &cfg));
        }
    }

    #[test]
    fn contradiction_detected() {
        let mut state = RankState::new();
        state.items[0].rating = 2000.0;
        state.items[1].rating = 1000.0;
        assert!(!contradicts(&state, 0, 1, 1.0)); // higher wins — consistent
        assert!(contradicts(&state, 0, 1, 0.0)); // higher loses — contradiction
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
            let (_, _, kind) = pick(&state, &cfg, &mut rng);
            assert_eq!(kind, PickKind::Audit);
        }
    }

    #[test]
    #[ignore = "statistical simulation benchmark"]
    fn simulation_benchmark() {
        // Accuracy grows with the confirmation cap; thresholds per cap.
        for (max_matches, min_accuracy) in [(15, 0.5), (30, 0.65)] {
            for seed in [17, 29, 43] {
                let (answers, bucket_accuracy, rho) = simulate(seed, max_matches);
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

    fn simulate(seed: u64, max_matches: u32) -> (usize, f64, f64) {
        let mut rng = StdRng::seed_from_u64(seed);
        let hidden = (0..210)
            .map(|_| rng.random_range(1_000.0..2_000.0))
            .collect::<Vec<_>>();
        let cfg = RankConfig {
            max_matches,
            ..Default::default()
        };
        let mut state = RankState::new();

        while state.settled_count(&cfg) < state.items.len() && state.history.len() < 3_200 {
            let (a, b, kind) = pick(&state, &cfg, &mut rng);
            assert_eq!(kind, PickKind::Explore);
            let score = f64::from(rng.random_bool(expected_score(hidden[a], hidden[b])));
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
            // Batch for benchmark speed; production refits after every answer.
            if state.history.len().is_multiple_of(50) {
                state.refit();
            }
        }
        state.refit();

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
