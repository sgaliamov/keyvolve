use crate::app::EMPTY_SLOT;
use crate::app::optimization::{OptimizationCache, OptimizationConfig, are_roll_neighbors};
use crate::models::slot_row;
use rand::seq::SliceRandom;
use rustc_hash::FxHashSet;

/// Slots and letters removed together during mutation.
pub struct Unplaced {
    pub free: Vec<u8>,
    pub letters: Vec<char>,
}

/// Re-place `letters` into `free` slots using the same layered flow as the generator:
/// 2. Rolls around frozen → 3. Allowed (with roll co-placement) → 4. Remaining rolls → 5. Free.
/// Step 1 (frozen) is the caller's responsibility.
pub fn place_letters(
    genome: &mut [char],
    free: &mut Vec<u8>,
    letters: &[char],
    opt: &OptimizationConfig,
    cache: &OptimizationCache,
) {
    let unplaced: FxHashSet<char> = letters.iter().copied().collect();
    let mut placed: FxHashSet<char> = FxHashSet::default();

    // ── 2. Rolls around frozen ───────────────────────────────────────────────
    for &[a, b] in &opt.rolls {
        let a_frozen = cache.frozen_chars.contains(&a);
        let b_frozen = cache.frozen_chars.contains(&b);
        match (a_frozen, b_frozen) {
            (true, false) if unplaced.contains(&b) && !placed.contains(&b) => {
                let anchor = opt.frozen[&a];
                if let Some(j) = find_roll_neighbor(genome, free, anchor, b, opt) {
                    genome[free[j] as usize] = b;
                    placed.insert(b);
                    free.swap_remove(j);
                }
            }
            (false, true) if unplaced.contains(&a) && !placed.contains(&a) => {
                let anchor = opt.frozen[&b];
                if let Some(i) = find_roll_neighbor(genome, free, anchor, a, opt) {
                    genome[free[i] as usize] = a;
                    placed.insert(a);
                    free.swap_remove(i);
                }
            }
            _ => {}
        }
    }

    // ── 3. Allowed (most-constrained first) ──────────────────────────────────
    // Sort by allowed-set size ascending: tight letters (e.g. a roll pair locked
    // to one column triplet) claim their few slots before wide letters can steal
    // them. Without this, a wide letter grabs a tight letter's only slot → the
    // tight letter is starved and spills onto a disallowed slot.
    let mut constrained: Vec<char> = letters
        .iter()
        .copied()
        .filter(|c| opt.allowed.contains_key(c))
        .collect();
    constrained.sort_by_key(|c| opt.allowed[c].len());

    for ch in constrained {
        if placed.contains(&ch) {
            continue;
        }
        let partner = cache.roll_partner.get(&ch).copied().filter(|p| {
            unplaced.contains(p) && !placed.contains(p) && !cache.frozen_chars.contains(p)
        });
        if let Some(partner) = partner
            && let Some((i, j)) = find_roll_slots(genome, free, ch, partner, opt)
        {
            place_pair(genome, free, &mut placed, i, j, ch, partner);
            continue;
        }
        place_constrained(genome, free, &mut placed, ch, opt, cache);
    }

    // ── 4. Remaining rolls ───────────────────────────────────────────────────
    for &[a, b] in &opt.rolls {
        let a_placed = placed.contains(&a);
        let b_placed = placed.contains(&b);
        let a_unplaced = unplaced.contains(&a) && !cache.frozen_chars.contains(&a);
        let b_unplaced = unplaced.contains(&b) && !cache.frozen_chars.contains(&b);

        match (a_placed, b_placed) {
            // Both free — place as roll pair.
            (false, false) if a_unplaced && b_unplaced => {
                if let Some((i, j)) = find_roll_slots(genome, free, a, b, opt) {
                    place_pair(genome, free, &mut placed, i, j, a, b);
                }
            }
            // `a` already placed (by step 3), `b` still free — anchor on `a`.
            (true, false) if b_unplaced => {
                let anchor = genome.iter().position(|&c| c == a).unwrap() as u8;
                if let Some(j) = find_roll_neighbor(genome, free, anchor, b, opt) {
                    genome[free[j] as usize] = b;
                    placed.insert(b);
                    free.swap_remove(j);
                }
            }
            // `b` already placed (by step 3), `a` still free — anchor on `b`.
            (false, true) if a_unplaced => {
                let anchor = genome.iter().position(|&c| c == b).unwrap() as u8;
                if let Some(i) = find_roll_neighbor(genome, free, anchor, a, opt) {
                    genome[free[i] as usize] = a;
                    placed.insert(a);
                    free.swap_remove(i);
                }
            }
            _ => {}
        }
    }

    // ── 5. Free ──────────────────────────────────────────────────────────────
    for &ch in letters {
        if placed.contains(&ch) {
            continue;
        }
        let idx = free
            .iter()
            .position(|&s| opt.is_slot_allowed(ch, s) && is_contiguous_slot(genome, s))
            .or_else(|| free.iter().position(|&s| opt.is_slot_allowed(ch, s)))
            .or((!free.is_empty()).then_some(0));
        if let Some(idx) = idx {
            genome[free[idx] as usize] = ch;
            free.swap_remove(idx);
        }
    }
}

/// Unplace `count` random movable units from `genome` back into a freed-slots vec.
/// Roll pairs currently at neighbor slots are unplaced together as one unit.
/// Letters sitting on blocked or disallowed slots (stale seed/dump genomes) are
/// always unplaced first, so mutation self-heals constraint violations.
pub fn unplace_units(
    genome: &mut [char],
    opt: &OptimizationConfig,
    cache: &OptimizationCache,
    count: usize,
    rng: &mut impl rand::Rng,
) -> Unplaced {
    let mut freed = Vec::new();
    let mut letters = Vec::new();

    // ── Heal ── violators always unplace; blocked slots never rejoin the pool.
    (0..genome.len()).for_each(|i| {
        let (slot, ch) = (i as u8, genome[i]);
        if ch != EMPTY_SLOT
            && !cache.frozen_chars.contains(&ch)
            && (opt.blocked.contains(&slot) || !opt.is_slot_allowed(ch, slot))
        {
            letters.push(ch);
            genome[i] = EMPTY_SLOT;
            if !opt.blocked.contains(&slot) {
                freed.push(slot);
            }
        }
    });
    // Letters healed off blocked slots need landing room: open existing empties.
    if freed.len() < letters.len() {
        let healed: FxHashSet<u8> = freed.iter().copied().collect();
        (0..genome.len()).for_each(|i| {
            let slot = i as u8;
            if genome[i] == EMPTY_SLOT
                && !healed.contains(&slot)
                && !opt.blocked.contains(&slot)
                && !cache.frozen_slots.contains(&slot)
            {
                freed.push(slot);
            }
        });
    }

    let mut used: FxHashSet<usize> = FxHashSet::default();
    let mut units: Vec<Vec<usize>> = Vec::new();

    for &[a, b] in &opt.rolls {
        let Some(ia) = genome.iter().position(|&c| c == a) else {
            continue;
        };
        let Some(ib) = genome.iter().position(|&c| c == b) else {
            continue;
        };
        if !cache.frozen_chars.contains(&a)
            && !cache.frozen_chars.contains(&b)
            && !opt.blocked.contains(&(ia as u8))
            && !opt.blocked.contains(&(ib as u8))
            && !used.contains(&ia)
            && !used.contains(&ib)
            && are_roll_neighbors(ia as u8, ib as u8)
        {
            used.insert(ia);
            used.insert(ib);
            units.push(vec![ia, ib]);
        }
    }
    for (i, &ch) in genome.iter().enumerate() {
        if ch != EMPTY_SLOT
            && !cache.frozen_chars.contains(&ch)
            && !opt.blocked.contains(&(i as u8))
            && !used.contains(&i)
        {
            units.push(vec![i]);
        }
    }

    units.shuffle(rng);
    for unit in units.iter().take(count) {
        for &idx in unit {
            freed.push(idx as u8);
            letters.push(genome[idx]);
            genome[idx] = EMPTY_SLOT;
        }
    }
    Unplaced {
        free: freed,
        letters,
    }
}

/// True when placing a letter at `slot` keeps letters in its row-hand segment contiguous.
/// Letters within the 5-slot row must form a single unbroken block; empties only at edges.
pub fn is_contiguous_slot(genome: &[char], slot: u8) -> bool {
    let hand = slot / 15;
    let row = slot_row(slot);
    let col = slot % 5;
    let row_start = hand * 15 + row * 5;
    let mut min_col = u8::MAX;
    let mut max_col = 0u8;
    let mut any = false;
    for c in 0..5u8 {
        let s = row_start + c;
        if s != slot && genome[s as usize] != EMPTY_SLOT {
            if !any || c < min_col {
                min_col = c;
            }
            if !any || c > max_col {
                max_col = c;
            }
            any = true;
        }
    }
    !any || (col >= min_col.saturating_sub(1) && col <= max_col + 1)
}

/// Find two indices into `free` whose slots are roll-neighbors and valid for `(anchor, other)`.
pub fn find_roll_slots(
    genome: &[char],
    free: &[u8],
    anchor: char,
    other: char,
    opt: &OptimizationConfig,
) -> Option<(usize, usize)> {
    (0..free.len())
        .filter(|&i| opt.is_slot_allowed(anchor, free[i]) && is_contiguous_slot(genome, free[i]))
        .find_map(|i| find_roll_neighbor(genome, free, free[i], other, opt).map(|j| (i, j)))
}

/// Find index into `free` of a slot that is a roll-neighbor of `anchor` and valid for `ch`.
pub fn find_roll_neighbor(
    genome: &[char],
    free: &[u8],
    anchor: u8,
    other: char,
    opt: &OptimizationConfig,
) -> Option<usize> {
    free.iter().position(|&s| {
        are_roll_neighbors(anchor, s)
            && opt.is_slot_allowed(other, s)
            && is_contiguous_slot(genome, s)
    })
}

/// Write `(a, b)` into `genome` at `free[i]`/`free[j]`, remove both from `free`.
pub fn place_pair(
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

/// Place a constrained `ch` onto one of its allowed slots, preferring a contiguous free one.
/// If no allowed slot is free, evict a movable occupant of an allowed slot to a free slot it
/// accepts — so `ch` never lands on a disallowed slot. Pairs with most-constrained-first
/// ordering, which keeps the swap path effectively unreachable for sane configs.
pub fn place_constrained(
    genome: &mut [char],
    free: &mut Vec<u8>,
    placed: &mut FxHashSet<char>,
    ch: char,
    opt: &OptimizationConfig,
    cache: &OptimizationCache,
) {
    // Direct: a free allowed slot (contiguous preferred).
    if let Some(idx) = free
        .iter()
        .position(|&s| opt.is_slot_allowed(ch, s) && is_contiguous_slot(genome, s))
        .or_else(|| free.iter().position(|&s| opt.is_slot_allowed(ch, s)))
    {
        genome[free[idx] as usize] = ch;
        placed.insert(ch);
        free.swap_remove(idx);
        return;
    }

    // Swap: relocate a movable occupant of an allowed slot to a free slot it accepts.
    for s in 0..genome.len() as u8 {
        let occ = genome[s as usize];
        if occ == EMPTY_SLOT || !opt.is_slot_allowed(ch, s) || cache.frozen_chars.contains(&occ) {
            continue;
        }
        if let Some(idx) = free.iter().position(|&f| opt.is_slot_allowed(occ, f)) {
            genome[free[idx] as usize] = occ;
            genome[s as usize] = ch;
            placed.insert(ch);
            free.swap_remove(idx);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::EMPTY_SLOT;
    use crate::app::optimization::config::{OptimizationCache, OptimizationConfig};
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rustc_hash::{FxHashMap, FxHashSet};

    /// Build a 30-char genome from a 30-char string; '_' → EMPTY_SLOT.
    fn genome(s: &str) -> Vec<char> {
        assert_eq!(s.len(), 30);
        s.chars()
            .map(|c| if c == '_' { EMPTY_SLOT } else { c })
            .collect()
    }

    fn test_opt(rolls: &[&str], blocked: &[u8]) -> OptimizationConfig {
        OptimizationConfig {
            text: std::path::PathBuf::default(),
            frozen: FxHashMap::default(),
            blocked: blocked.iter().copied().collect(),
            rolls: rolls
                .iter()
                .map(|pair| {
                    let mut chars = pair.chars();
                    [chars.next().unwrap(), chars.next().unwrap()]
                })
                .collect(),
            allowed: FxHashMap::default(),
            mutation_count: 10,
            input: None,
            output: None,
            canonicalize: true,
        }
    }

    fn test_cache(frozen_chars: &[char]) -> OptimizationCache {
        OptimizationCache {
            frozen_slots: FxHashSet::default(),
            frozen_chars: frozen_chars.iter().copied().collect(),
            roll_partner: FxHashMap::default(),
        }
    }

    // Row 0 of left hand = slots 0..5.
    // Placing at slot `s` in that row: genome has all others in the row at their positions.

    #[test]
    fn empty_row_allows_any_slot() {
        let g = genome("_____xxxxxxxxxxxxxxxxxxxxxxxxx");
        for s in 0u8..5 {
            assert!(is_contiguous_slot(&g, s), "empty row should allow slot {s}");
        }
    }

    #[test]
    fn single_letter_allows_neighbors_only() {
        // 'a' at col 2 (slot 2); slot 1 and 3 are the only valid neighbors.
        let g = genome("__a__xxxxxxxxxxxxxxxxxxxxxxxxx");
        assert!(!is_contiguous_slot(&g, 0), "col 0 is not adjacent to col 2");
        assert!(is_contiguous_slot(&g, 1));
        assert!(is_contiguous_slot(&g, 3));
        assert!(!is_contiguous_slot(&g, 4), "col 4 is not adjacent to col 2");
    }

    #[test]
    fn block_of_three_extends_at_edges_only() {
        // 'a','b','c' at cols 1,2,3 (slots 1,2,3); valid new placements: col 0 or col 4.
        let g = genome("_abc_xxxxxxxxxxxxxxxxxxxxxxxxx");
        assert!(is_contiguous_slot(&g, 0));
        assert!(is_contiguous_slot(&g, 4));
    }

    #[test]
    fn full_row_no_empty_slots_trivially_true() {
        // No free slots in the row, but is_contiguous_slot returns true regardless
        // (the slot itself is either occupied or out of scope — caller picks free slots).
        let g = genome("abcdexxxxxxxxxxxxxxxxxxxxxxxxx");
        for s in 0u8..5 {
            // col range is [0,4]; col is within [min-1, max+1] = [-1,5] → always true
            assert!(is_contiguous_slot(&g, s));
        }
    }

    #[test]
    fn gap_would_be_created_is_rejected() {
        // 'a' at col 0, 'b' at col 2; col 4 would leave gap (col 3 empty between 2 and 4).
        let g = genome("a_b__xxxxxxxxxxxxxxxxxxxxxxxxx");
        assert!(!is_contiguous_slot(&g, 4));
        assert!(is_contiguous_slot(&g, 1), "filling the gap is allowed");
        assert!(is_contiguous_slot(&g, 3), "extending right edge is allowed");
    }

    #[test]
    fn right_hand_row_independent() {
        // Right hand row 0 = slots 15..20. Place 'z' at slot 17 (col 2).
        let mut g = genome("______________________________");
        g[17] = 'z';
        // Left hand row 0 is all empty → all left slots allowed.
        assert!(is_contiguous_slot(&g, 0));
        assert!(is_contiguous_slot(&g, 4));
        // Right hand: col 1 and 3 adjacent to col 2 → allowed; col 0 and 4 → not.
        assert!(is_contiguous_slot(&g, 16));
        assert!(is_contiguous_slot(&g, 18));
        assert!(!is_contiguous_slot(&g, 15));
        assert!(!is_contiguous_slot(&g, 19));
    }

    #[test]
    fn unplace_units_returns_letters_for_freed_slots() {
        let mut g = genome("ab___cdefghijklmnopqrstuvwxyzz");
        let original = g.clone();
        let opt = test_opt(&["ab"], &[]);
        let cache = test_cache(&[]);
        let mut rng = StdRng::seed_from_u64(7);

        let unplaced = unplace_units(&mut g, &opt, &cache, 1, &mut rng);

        assert_eq!(unplaced.free.len(), unplaced.letters.len());
        for (&slot, &ch) in unplaced.free.iter().zip(&unplaced.letters) {
            assert_eq!(original[slot as usize], ch);
            assert_eq!(g[slot as usize], EMPTY_SLOT);
        }
    }

    #[test]
    fn unplace_units_keeps_roll_pair_together() {
        let mut g = genome("ab___xxxxxxxxxxxxxxxxxxxxxxxxx");
        let opt = test_opt(&["ab"], &[]);
        let cache = test_cache(&[]);
        let mut rng = StdRng::seed_from_u64(1);

        let unplaced = unplace_units(&mut g, &opt, &cache, 1, &mut rng);

        assert_eq!(unplaced.free.len(), 2);
        assert_eq!(unplaced.letters.len(), 2);
        assert!(unplaced.letters.contains(&'a'));
        assert!(unplaced.letters.contains(&'b'));
    }

    #[test]
    fn unplace_units_skips_blocked_and_frozen() {
        let mut g = genome("abcdexxxxxxxxxxxxxxxxxxxxxxxxx");
        let opt = test_opt(&["ab"], &[2]);
        let cache = test_cache(&['d']);
        let mut rng = StdRng::seed_from_u64(3);

        let unplaced = unplace_units(&mut g, &opt, &cache, 4, &mut rng);

        // 'c' is healed off the blocked slot, but the slot itself stays withheld.
        assert!(!unplaced.free.contains(&2));
        assert!(unplaced.letters.contains(&'c'));
        assert!(!unplaced.letters.contains(&'d'));
    }

    #[test]
    fn unplace_units_heals_disallowed_letter() {
        // 'a' allowed only at slot 0, but sits at slot 4 — must be unplaced even with count 0.
        let mut g = genome("bcd_axxxxxxxxxxxxxxxxxxxxxxxxx");
        let mut opt = test_opt(&[], &[]);
        opt.allowed.insert('a', [0u8].into_iter().collect());
        let cache = test_cache(&[]);
        let mut rng = StdRng::seed_from_u64(5);

        let unplaced = unplace_units(&mut g, &opt, &cache, 0, &mut rng);

        assert_eq!(unplaced.letters, vec!['a']);
        assert_eq!(unplaced.free, vec![4]);
        assert_eq!(g[4], EMPTY_SLOT);
    }

    #[test]
    fn unplace_units_heals_blocked_occupant_without_freeing_blocked_slot() {
        // 'c' sits on blocked slot 2 — unplaced, slot 2 withheld, empties opened as landing room.
        let mut g = genome("abcdefghijklmnopqrstuvwxyz____");
        let opt = test_opt(&[], &[2]);
        let cache = test_cache(&[]);
        let mut rng = StdRng::seed_from_u64(9);

        let unplaced = unplace_units(&mut g, &opt, &cache, 0, &mut rng);

        assert_eq!(unplaced.letters, vec!['c']);
        assert_eq!(g[2], EMPTY_SLOT);
        assert!(
            !unplaced.free.contains(&2),
            "blocked slot must not be freed"
        );
        assert!(
            unplaced.free.len() >= unplaced.letters.len(),
            "healed letters need landing room"
        );
    }
}
