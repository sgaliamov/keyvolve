use crate::app::OptimizationConfig;
use crate::app::optimization::are_roll_neighbors;
use crate::app::{EMPTY_SLOT, GaContext, KeysGenome};
use rand::seq::SliceRandom;

/// Generate a genome for optimization, respecting frozen/blocked constraints.
pub fn generate(ctx: &GaContext) -> KeysGenome {
    let state = ctx.state.as_ref().expect("state must be set");
    let opt = &state.optimization;
    constrained_keys(opt)
}

/// Place frozen chars at fixed positions, shuffle the rest into remaining free slots.
/// Per-letter `allowed` constraints are respected; unconstrained letters fill the rest.
fn constrained_keys(opt: &OptimizationConfig) -> KeysGenome {
    let frozen = &opt.frozen;
    let blocked = &opt.blocked;
    let mut genome = vec![EMPTY_SLOT; 30];

    // Pin frozen keys.
    for (&ch, &idx) in frozen {
        genome[idx as usize] = ch;
    }

    let frozen_positions: rustc_hash::FxHashSet<u8> = frozen.values().copied().collect();
    let frozen_chars: rustc_hash::FxHashSet<char> = frozen.keys().copied().collect();

    // Remaining letters and positions.
    let mut letters: Vec<char> = ('a'..='z').filter(|c| !frozen_chars.contains(c)).collect();
    let mut free: Vec<u8> = (0u8..30)
        .filter(|i| !blocked.contains(i) && !frozen_positions.contains(i))
        .collect();

    letters.shuffle(&mut rand::rng());
    free.shuffle(&mut rand::rng());

    // Place roll pairs together in neighbor slots first.
    // Both chars must be free (not frozen). We try each combination of two free
    // slots and pick the first pair that satisfies are_roll_neighbors and each
    // char's allowed constraint.
    let mut placed_chars: rustc_hash::FxHashSet<char> = rustc_hash::FxHashSet::default();
    for [a, b] in &opt.rolls {
        if placed_chars.contains(a)
            || placed_chars.contains(b)
            || frozen_chars.contains(a)
            || frozen_chars.contains(b)
        {
            continue;
        }
        // Find a pair of free slots (sa, sb) that are roll-neighbors and valid for (a, b).
        'outer: for i in 0..free.len() {
            for j in 0..free.len() {
                if i == j {
                    continue;
                }
                let (sa, sb) = (free[i], free[j]);
                if are_roll_neighbors(sa, sb)
                    && opt.is_slot_valid(*a, sa)
                    && opt.is_slot_valid(*b, sb)
                {
                    genome[sa as usize] = *a;
                    genome[sb as usize] = *b;
                    placed_chars.insert(*a);
                    placed_chars.insert(*b);
                    // Remove sb first (larger index if i < j, order matters for swap_remove).
                    let (ri, rj) = if i > j { (i, j) } else { (j, i) };
                    free.swap_remove(ri);
                    free.swap_remove(rj);
                    break 'outer;
                }
            }
        }
    }

    // Constrained letters first (has `allowed` entry), then unconstrained.
    // Ensures constrained letters get priority picking their valid slots.
    letters.sort_by_key(|c| if opt.allowed.contains_key(c) { 0u8 } else { 1 });

    // Assign each letter to its first valid free slot; fall back to any free slot.
    for ch in letters {
        if placed_chars.contains(&ch) {
            continue;
        }
        let pos = free
            .iter()
            .position(|&s| opt.is_slot_valid(ch, s))
            .or(if free.is_empty() { None } else { Some(0) });

        if let Some(idx) = pos {
            genome[free[idx] as usize] = ch;
            free.swap_remove(idx);
        }
    }

    genome
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
