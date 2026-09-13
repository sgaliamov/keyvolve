use crate::evaluator::EMPTY_SLOT;
use crate::modes::optimize::{
    GaContext, KeysGenome, OptimizationCache, OptimizationConfig, place_letters,
};
use rand::seq::SliceRandom;

/// Generate a genome for optimization, respecting frozen/blocked/pair constraints.
pub fn generate(ctx: &GaContext) -> KeysGenome {
    let state = ctx.state.as_ref().expect("state must be set");
    constrained_keys(&state.optimization, &state.cache)
}

/// Build a genome placing chars into slots under four layers of constraints:
/// 1. **Frozen** — pinned chars stay at their fixed slot.
/// 2. **Same-side pairs around frozen** — free partner of a frozen char placed on the same hand.
/// 3. **Allowed** — constrained letters placed first; if in a pair, partner co-placed on same hand.
/// 4. **Remaining pairs** — unconstrained pairs placed on one hand.
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

    place_letters(&mut genome, &mut free, &letters, opt, cache);
    genome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::optimize::OptimizationConfig;
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
    fn same_side_pair_placed_on_same_hand() {
        let opt = OptimizationConfig {
            same_side: vec![['t', 'h']],
            ..Default::default()
        };
        for _ in 0..20 {
            let g = run(&opt);
            let st = g.iter().position(|&c| c == 't').unwrap() as u8;
            let sh = g.iter().position(|&c| c == 'h').unwrap() as u8;
            assert_eq!(st / 15, sh / 15, "t at {st}, h at {sh} — must share hand");
        }
    }

    #[test]
    fn same_side_pair_frozen_anchor_respected() {
        // 't' is frozen at slot 2 (left hand); 'h' must land on left hand.
        let mut opt = OptimizationConfig {
            same_side: vec![['t', 'h']],
            ..Default::default()
        };
        opt.frozen.insert('t', 2);
        for _ in 0..20 {
            let g = run(&opt);
            assert_eq!(g[2], 't', "frozen 't' must stay at slot 2");
            let sh = g.iter().position(|&c| c == 'h').unwrap() as u8;
            assert!(sh < 15, "h at {sh} — must share hand with frozen t at 2");
        }
    }

    #[test]
    fn same_side_pair_allowed_anchor_respected() {
        // 't' allowed only at slots 0/19; 'h' unconstrained — pair must share hand.
        let mut opt = OptimizationConfig {
            same_side: vec![['t', 'h']],
            ..Default::default()
        };
        opt.allowed.insert('t', [0u8, 19].into_iter().collect());
        for _ in 0..20 {
            let g = run(&opt);
            let st = g.iter().position(|&c| c == 't').unwrap() as u8;
            let sh = g.iter().position(|&c| c == 'h').unwrap() as u8;
            assert!(st == 0 || st == 19, "t landed at {st}, expected 0 or 19");
            assert_eq!(st / 15, sh / 15, "t at {st}, h at {sh} — must share hand");
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

    /// Stress test against the real `keyvolve.yaml` constraints: every placed
    /// char must satisfy `is_slot_allowed` after generation AND after mutation.
    #[test]
    fn real_config_respects_allowed() {
        use crate::modes::optimize::{place_letters, unplace_units};
        use rand::seq::SliceRandom;

        let json = r#"{
            "blocked": [19],
            "allowed": {
                "t":[1,2,3],"h":[1,2,3],
                "c":[1,2,3,6,7,8,11,12,13],
                "e":[6,7,8],"a":[1,2,3,6,7,8],
                "o":[1,2,3,6,7,8],"r":[1,2,6,7,8],
                "s":[1,2,3,6,7,8,11,12,13]
            },
            "sameSide":["ht","er"]
        }"#;
        let opt: OptimizationConfig = serde_json::from_str(json).unwrap();
        let cache = opt.cache();

        let check = |g: &[char], when: &str| {
            for (slot, &ch) in g.iter().enumerate() {
                if ch != EMPTY_SLOT {
                    assert!(
                        opt.is_slot_allowed(ch, slot as u8),
                        "{when}: '{ch}' at slot {slot} violates allowed"
                    );
                }
            }
            let mut present: Vec<char> = g.iter().copied().filter(|&c| c != EMPTY_SLOT).collect();
            present.sort_unstable();
            assert_eq!(
                present,
                ('a'..='z').collect::<Vec<_>>(),
                "{when}: genome must contain every letter exactly once"
            );
        };

        let mut rng = rand::rng();
        for _ in 0..5_000 {
            let mut g = constrained_keys(&opt, &cache);
            check(&g, "generate");

            // Exercise the mutate core: unplace then re-place.
            let unplaced = unplace_units(&mut g, &opt, &cache, 6, &mut rng);
            let mut free = unplaced.free;
            let mut letters = unplaced.letters;
            letters.shuffle(&mut rng);
            free.shuffle(&mut rng);
            place_letters(&mut g, &mut free, &letters, &opt, &cache);
            check(&g, "mutate");
        }
    }
}
