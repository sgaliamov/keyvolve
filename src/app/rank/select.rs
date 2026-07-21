use crate::app::rank::{RankConfig, RankState};
use rand::RngExt;
use rand::seq::SliceRandom;

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
    let settled: Vec<usize> = (0..state.items.len())
        .filter(|&i| state.items[i].settled(cfg.min_matches, cfg.max_deviation))
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

/// Explore: item with most remaining work vs a close-rated opponent that
/// still needs matches too — every answer then refines two unsettled items.
fn pick_explore(state: &RankState, cfg: &RankConfig, rng: &mut impl RngExt) -> (usize, usize) {
    let steps = |i: usize| state.items[i].steps_needed(cfg.min_matches, cfg.max_deviation);
    // Candidate: random among the POOL items with most answers still needed.
    // Pre-shuffle so stable sort breaks ties randomly, not by enumeration order.
    let mut order: Vec<usize> = (0..state.items.len()).collect();
    order.shuffle(rng);
    let mut others = order.clone();
    order.sort_by_key(|&i| std::cmp::Reverse(steps(i)));
    // Pool never spills into already-settled items while unsettled ones exist.
    let pool = |sorted: &[usize]| {
        let unsettled = sorted.iter().take_while(|&&i| steps(i) > 0).count();
        POOL.min(sorted.len()).min(unsettled.max(1))
    };
    let a = order[rng.random_range(0..pool(&order))];

    // Opponent: shares a key with the candidate (easier to compare); unsettled
    // first so the answer advances both items, then closest by rating.
    others.retain(|&i| i != a && shares_key(state, a, i));
    let ra = state.items[a].rating;
    others.sort_by(|&x, &y| {
        let unsettled = |i: usize| steps(i) > 0;
        let gap = |i: usize| (state.items[i].rating - ra).abs();
        unsettled(y)
            .cmp(&unsettled(x))
            .then(gap(x).total_cmp(&gap(y)))
    });
    let b = others[rng.random_range(0..pool(&others))];
    (a, b)
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
    use rand::{SeedableRng, rngs::StdRng};

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
            assert!(state.items[a].steps_needed(cfg.min_matches, cfg.max_deviation) > 0);
            assert!(state.items[b].steps_needed(cfg.min_matches, cfg.max_deviation) > 0);
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
}
