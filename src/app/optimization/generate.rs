use crate::app::optimization::are_roll_neighbors;
use crate::app::{EMPTY_SLOT, GaContext, KeysGenome, OptimizationConfig};
use rand::seq::SliceRandom;
use rustc_hash::FxHashSet;

/// Generate a genome for optimization, respecting frozen/blocked/roll constraints.
pub fn generate(ctx: &GaContext) -> KeysGenome {
    constrained_keys(&ctx.state.as_ref().expect("state must be set").optimization)
}

/// Build a genome placing chars into slots under three layers of constraints:
/// 1. **Frozen** — pinned chars stay at their fixed slot.
/// 2. **Rolls** — paired chars are co-placed into adjacent-column, ≤1-row-apart slots.
/// 3. **Allowed** — constrained letters pick valid slots first; unconstrained fill the rest.
fn constrained_keys(opt: &OptimizationConfig) -> KeysGenome {
    let mut genome = vec![EMPTY_SLOT; 30];
    let mut rng = rand::rng();

    // ── 1. Frozen ────────────────────────────────────────────────────────────
    for (&ch, &idx) in &opt.frozen {
        genome[idx as usize] = ch;
    }
    // todo: cache on start
    let frozen_slots: FxHashSet<u8> = opt.frozen.values().copied().collect();
    let frozen_chars: FxHashSet<char> = opt.frozen.keys().copied().collect();

    // Shuffled pools of unplaced letters and available slots.
    let mut letters: Vec<char> = ('a'..='z').filter(|c| !frozen_chars.contains(c)).collect();
    let mut free: Vec<u8> = (0u8..30)
        .filter(|s| !opt.blocked.contains(s) && !frozen_slots.contains(s))
        .collect();
    letters.shuffle(&mut rng);
    free.shuffle(&mut rng);

    // ── 2. Rolls ─────────────────────────────────────────────────────────────
    // For each pair find two free neighbor slots satisfying per-char allowed constraints.
    // If one char is frozen its fixed slot acts as the anchor; the other char is placed
    // into a free roll-neighbor slot next to it.
    let mut placed: FxHashSet<char> = FxHashSet::default();
    for &[a, b] in &opt.rolls {
        let a_done = placed.contains(&a) || frozen_chars.contains(&a);
        let b_done = placed.contains(&b) || frozen_chars.contains(&b);
        match (a_done, b_done) {
            (true, true) => continue,
            (true, false) => {
                // a is frozen — anchor on its slot, place b next to it.
                let anchor = opt.frozen[&a];
                if let Some(j) = find_neighbor_to_frozen(&free, anchor, b, opt) {
                    genome[free[j] as usize] = b;
                    placed.insert(b);
                    free.swap_remove(j);
                }
            }
            (false, true) => {
                // b is frozen — anchor on its slot, place a next to it.
                let anchor = opt.frozen[&b];
                if let Some(i) = find_neighbor_to_frozen(&free, anchor, a, opt) {
                    genome[free[i] as usize] = a;
                    placed.insert(a);
                    free.swap_remove(i);
                }
            }
            (false, false) => {
                if let Some((i, j)) = find_neighbor_slots(&free, a, b, opt) {
                    place_pair(&mut genome, &mut free, &mut placed, i, j, a, b);
                }
            }
        }
    }

    // ── 3. Remaining letters ─────────────────────────────────────────────────
    // Constrained letters first so they get priority over their allowed slots.
    letters.sort_by_key(|c| if opt.allowed.contains_key(c) { 0u8 } else { 1 });
    for ch in letters {
        if placed.contains(&ch) {
            continue;
        }
        let idx = free
            .iter()
            .position(|&s| opt.is_slot_valid(ch, s))
            .or((!free.is_empty()).then_some(0));
        if let Some(idx) = idx {
            genome[free[idx] as usize] = ch;
            free.swap_remove(idx);
        }
    }

    genome
}

/// Find indices into `free` of two slots that are roll-neighbors and valid for `(a, b)`.
fn find_neighbor_slots(
    free: &[u8],
    a: char,
    b: char,
    opt: &OptimizationConfig,
) -> Option<(usize, usize)> {
    for i in 0..free.len() {
        for j in 0..free.len() {
            if i != j
                && are_roll_neighbors(free[i], free[j])
                && opt.is_slot_valid(a, free[i])
                && opt.is_slot_valid(b, free[j])
            {
                return Some((i, j));
            }
        }
    }
    None
}

/// Find index into `free` of a slot that is a roll-neighbor of `anchor` and valid for `ch`.
fn find_neighbor_to_frozen(
    free: &[u8],
    anchor: u8,
    ch: char,
    opt: &OptimizationConfig,
) -> Option<usize> {
    free.iter()
        .position(|&s| are_roll_neighbors(anchor, s) && opt.is_slot_valid(ch, s))
}

/// Write `(a, b)` into `genome` at `free[i]`/`free[j]`, then remove both from `free`.
fn place_pair(
    genome: &mut [char],
    free: &mut Vec<u8>,
    placed: &mut FxHashSet<char>,
    i: usize,
    j: usize,
    a: char,
    b: char,
) {
    genome[free[i] as usize] = a;
    genome[free[j] as usize] = b;
    placed.insert(a);
    placed.insert(b);
    // Remove higher index first to keep the lower index valid.
    let (hi, lo) = if i > j { (i, j) } else { (j, i) };
    free.swap_remove(hi);
    free.swap_remove(lo);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::OptimizationConfig;
    use rustc_hash::FxHashSet;

    fn make_frozen(pairs: &[(char, u8)]) -> OptimizationConfig {
        OptimizationConfig {
            frozen: pairs.iter().copied().collect(),
            ..Default::default()
        }
    }

    #[test]
    fn unconstrained_has_all_26_chars() {
        let g = constrained_keys(&OptimizationConfig::default());
        assert_eq!(g.len(), 30);
        let mut chars: Vec<char> = g.iter().copied().filter(|&c| c != EMPTY_SLOT).collect();
        chars.sort_unstable();
        assert_eq!(chars, ('a'..='z').collect::<Vec<_>>());
    }

    #[test]
    fn frozen_keys_stay_in_place() {
        let opt = make_frozen(&[('a', 0), ('z', 29)]);
        let g = constrained_keys(&opt);
        assert_eq!(g[0], 'a');
        assert_eq!(g[29], 'z');
    }

    #[test]
    fn blocked_slots_are_empty() {
        let blocked: FxHashSet<u8> = [5, 6, 7, 8].iter().copied().collect();
        let opt = OptimizationConfig {
            blocked,
            ..Default::default()
        };
        let g = constrained_keys(&opt);
        for &b in &[5usize, 6, 7, 8] {
            assert_eq!(g[b], EMPTY_SLOT, "slot {b} should be empty");
        }
    }

    #[test]
    fn roll_pair_placed_as_neighbors() {
        let opt = OptimizationConfig {
            rolls: vec![['t', 'h']],
            ..Default::default()
        };
        for _ in 0..20 {
            let g = constrained_keys(&opt);
            let st = g.iter().position(|&c| c == 't').unwrap() as u8;
            let sh = g.iter().position(|&c| c == 'h').unwrap() as u8;
            assert!(
                are_roll_neighbors(st, sh),
                "t at {st}, h at {sh} — not roll neighbors"
            );
        }
    }

    #[test]
    fn roll_pair_frozen_anchor_respected() {
        // 't' is frozen at slot 2; 'h' must land in a roll-neighbor slot.
        let mut opt = OptimizationConfig {
            rolls: vec![['t', 'h']],
            ..Default::default()
        };
        opt.frozen.insert('t', 2);
        for _ in 0..20 {
            let g = constrained_keys(&opt);
            assert_eq!(g[2], 't', "frozen 't' must stay at slot 2");
            let sh = g.iter().position(|&c| c == 'h').unwrap() as u8;
            assert!(
                are_roll_neighbors(2, sh),
                "h at {sh} — not roll neighbor of frozen t at 2"
            );
        }
    }

    #[test]
    fn allowed_constraint_respected() {
        use crate::app::optimization::expand_half;
        let mut opt = OptimizationConfig::default();
        opt.allowed.insert('a', expand_half(&[0])); // 'a' only allowed at 0 or 19
        for _ in 0..20 {
            let g = constrained_keys(&opt);
            let pos = g.iter().position(|&c| c == 'a').unwrap();
            assert!(pos == 0 || pos == 19, "a landed at {pos}");
        }
    }
}
