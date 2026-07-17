use crate::app::rank::{RankConfig, RankState};
use rand::RngExt;

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
pub fn pick(
    state: &RankState,
    cfg: &RankConfig,
    rng: &mut impl RngExt,
) -> (usize, usize, PickKind) {
    if rng.random_bool(cfg.audit_rate.clamp(0.0, 1.0))
        && let Some(pair) = pick_audit(state, cfg, rng)
    {
        return (pair.0, pair.1, PickKind::Audit);
    }
    let (a, b) = pick_explore(state, rng);
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
    let b = *far.get(rng.random_range(0..far.len().max(1)))?;
    Some((a, b))
}

/// Explore: most uncertain item vs a close-rated opponent (informative match).
fn pick_explore(state: &RankState, rng: &mut impl RngExt) -> (usize, usize) {
    // Candidate: random among the POOL least-played / most-uncertain items.
    let mut order: Vec<usize> = (0..state.items.len()).collect();
    order.sort_by(|&x, &y| {
        let (ix, iy) = (&state.items[x], &state.items[y]);
        ix.matches
            .cmp(&iy.matches)
            .then(iy.deviation.total_cmp(&ix.deviation))
    });
    let a = order[rng.random_range(0..POOL.min(order.len()))];

    // Opponent: random among the POOL closest by rating.
    let mut others: Vec<usize> = (0..state.items.len()).filter(|&i| i != a).collect();
    let ra = state.items[a].rating;
    others.sort_by(|&x, &y| {
        (state.items[x].rating - ra)
            .abs()
            .total_cmp(&(state.items[y].rating - ra).abs())
    });
    let b = others[rng.random_range(0..POOL.min(others.len()))];
    (a, b)
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
    fn contradiction_detected() {
        let mut state = RankState::new();
        state.items[0].rating = 2000.0;
        state.items[1].rating = 1000.0;
        assert!(!contradicts(&state, 0, 1, 1.0)); // higher wins — consistent
        assert!(contradicts(&state, 0, 1, 0.0)); // higher loses — contradiction
    }
}
