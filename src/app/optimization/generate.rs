use crate::app::optimization::{OptimizationCache, are_roll_neighbors};
use crate::app::{EMPTY_SLOT, GaContext, KeysGenome, OptimizationConfig};
use rand::seq::SliceRandom;
use rustc_hash::FxHashSet;

/// Generate a genome for optimization, respecting frozen/blocked/roll constraints.
pub fn generate(ctx: &GaContext) -> KeysGenome {
    let state = ctx.state.as_ref().expect("state must be set");
    constrained_keys(&state.optimization, &state.cache)
}

/// Build a genome placing chars into slots under four layers of constraints:
/// 1. **Frozen** — pinned chars stay at their fixed slot.
/// 2. **Rolls around frozen** — free partner of a frozen char placed in a roll-neighbor slot.
/// 3. **Allowed** — constrained letters placed first; if in a roll, partner co-placed as neighbor.
/// 4. **Remaining rolls** — unconstrained pairs placed as neighbors.
/// 5. **Free** — unconstrained letters fill remaining slots.
fn constrained_keys(opt: &OptimizationConfig, cache: &OptimizationCache) -> KeysGenome {
    let mut genome = vec![EMPTY_SLOT; 30];
    let mut rng = rand::rng();

    // ── 1. Frozen ────────────────────────────────────────────────────────────
    for (&ch, &idx) in &opt.frozen {
        genome[idx as usize] = ch;
    }

    // Shuffled pools of unplaced letters and available slots.
    let mut letters: Vec<char> = ('a'..='z')
        .filter(|c| !cache.frozen_chars.contains(c))
        .collect();
    let mut free: Vec<u8> = (0u8..30)
        .filter(|s| !opt.blocked.contains(s) && !cache.frozen_slots.contains(s))
        .collect();
    letters.shuffle(&mut rng);
    free.shuffle(&mut rng);

    let mut placed: FxHashSet<char> = FxHashSet::default();

    // ── 2. Rolls around frozen ───────────────────────────────────────────────
    // For each roll pair where exactly one char is frozen, place the free partner
    // into a roll-neighbor slot relative to the frozen anchor.
    for &[a, b] in &opt.rolls {
        let a_frozen = cache.frozen_chars.contains(&a);
        let b_frozen = cache.frozen_chars.contains(&b);
        match (a_frozen, b_frozen) {
            (true, false) => {
                let anchor = opt.frozen[&a];
                if let Some(j) = find_neighbor_to_frozen(&free, anchor, b, opt) {
                    genome[free[j] as usize] = b;
                    placed.insert(b);
                    free.swap_remove(j);
                }
            }
            (false, true) => {
                let anchor = opt.frozen[&b];
                if let Some(i) = find_neighbor_to_frozen(&free, anchor, a, opt) {
                    genome[free[i] as usize] = a;
                    placed.insert(a);
                    free.swap_remove(i);
                }
            }
            _ => continue,
        }
    }

    // ── 3. Allowed ───────────────────────────────────────────────────────────
    // Place allowed-constrained chars into their valid slots first.
    // If the char is in a roll and its partner is free, place partner as roll neighbor.
    for &ch in letters.iter().filter(|c| opt.allowed.contains_key(c)) {
        if placed.contains(&ch) {
            continue;
        }
        let partner = cache
            .roll_partner
            .get(&ch)
            .copied()
            .filter(|p| !placed.contains(p) && !cache.frozen_chars.contains(p));
        if let Some(partner) = partner
            && let Some((i, j)) = find_neighbor_slots_anchored(&free, ch, partner, opt)
        {
            place_pair(&mut genome, &mut free, &mut placed, i, j, ch, partner);
            continue;
        }
        // No roll partner to co-place — place ch alone.
        let idx = free
            .iter()
            .position(|&s| opt.is_slot_allowed(ch, s))
            .or((!free.is_empty()).then_some(0));
        if let Some(idx) = idx {
            genome[free[idx] as usize] = ch;
            placed.insert(ch);
            free.swap_remove(idx);
        }
    }

    // ── 4. Remaining rolls ───────────────────────────────────────────────────
    for &[a, b] in &opt.rolls {
        if placed.contains(&a)
            || placed.contains(&b)
            || cache.frozen_chars.contains(&a)
            || cache.frozen_chars.contains(&b)
        {
            continue;
        }
        if let Some((i, j)) = find_neighbor_slots_anchored(&free, a, b, opt) {
            place_pair(&mut genome, &mut free, &mut placed, i, j, a, b);
        }
    }

    // ── 5. Remaining letters ─────────────────────────────────────────────────
    for ch in letters {
        if placed.contains(&ch) {
            continue;
        }
        let idx = free
            .iter()
            .position(|&s| opt.is_slot_allowed(ch, s))
            .or((!free.is_empty()).then_some(0));
        if let Some(idx) = idx {
            genome[free[idx] as usize] = ch;
            free.swap_remove(idx);
        }
    }

    genome
}

/// Find two indices into `free` whose slots are roll-neighbors and valid for `(anchor, other)`.
/// Iterates `anchor`'s valid slots in the outer loop — when `anchor` is unconstrained this is
/// equivalent to an unordered search.
fn find_neighbor_slots_anchored(
    free: &[u8],
    anchor: char,
    other: char,
    opt: &OptimizationConfig,
) -> Option<(usize, usize)> {
    for i in 0..free.len() {
        if !opt.is_slot_allowed(anchor, free[i]) {
            continue;
        }
        for j in 0..free.len() {
            if i != j && are_roll_neighbors(free[i], free[j]) && opt.is_slot_allowed(other, free[j])
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
    other: char,
    opt: &OptimizationConfig,
) -> Option<usize> {
    free.iter()
        .position(|&s| are_roll_neighbors(anchor, s) && opt.is_slot_allowed(other, s))
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

    fn run(opt: &OptimizationConfig) -> KeysGenome {
        constrained_keys(opt, &opt.cache())
    }

    fn make_frozen(pairs: &[(char, u8)]) -> OptimizationConfig {
        OptimizationConfig {
            frozen: pairs.iter().copied().collect(),
            ..Default::default()
        }
    }

    #[test]
    fn unconstrained_has_all_26_chars() {
        let g = run(&OptimizationConfig::default());
        assert_eq!(g.len(), 30);
        let mut chars: Vec<char> = g.iter().copied().filter(|&c| c != EMPTY_SLOT).collect();
        chars.sort_unstable();
        assert_eq!(chars, ('a'..='z').collect::<Vec<_>>());
    }

    #[test]
    fn frozen_keys_stay_in_place() {
        let opt = make_frozen(&[('a', 0), ('z', 29)]);
        let g = run(&opt);
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
        let g = run(&opt);
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
            let g = run(&opt);
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
            let g = run(&opt);
            assert_eq!(g[2], 't', "frozen 't' must stay at slot 2");
            let sh = g.iter().position(|&c| c == 'h').unwrap() as u8;
            assert!(
                are_roll_neighbors(2, sh),
                "h at {sh} — not roll neighbor of frozen t at 2"
            );
        }
    }

    #[test]
    fn roll_pair_allowed_anchor_respected() {
        // 't' allowed only at slots 0/19; 'h' unconstrained — roll must honour that.
        let mut opt = OptimizationConfig {
            rolls: vec![['t', 'h']],
            ..Default::default()
        };
        opt.allowed.insert('t', [0u8, 19].into_iter().collect());
        for _ in 0..20 {
            let g = run(&opt);
            let st = g.iter().position(|&c| c == 't').unwrap() as u8;
            let sh = g.iter().position(|&c| c == 'h').unwrap() as u8;
            assert!(st == 0 || st == 19, "t landed at {st}, expected 0 or 19");
            assert!(
                are_roll_neighbors(st, sh),
                "t at {st}, h at {sh} — not roll neighbors"
            );
        }
    }

    #[test]
    fn allowed_constraint_respected() {
        let mut opt = OptimizationConfig::default();
        opt.allowed.insert('a', [0u8, 19].into_iter().collect()); // 'a' only allowed at 0 or 19
        for _ in 0..20 {
            let g = run(&opt);
            let pos = g.iter().position(|&c| c == 'a').unwrap();
            assert!(pos == 0 || pos == 19, "a landed at {pos}");
        }
    }
}
