use crate::app::EMPTY_SLOT;
use crate::app::optimization::{OptimizationCache, OptimizationConfig, are_roll_neighbors};
use rand::seq::SliceRandom;
use rustc_hash::FxHashSet;

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
                if let Some(j) = find_roll_neighbor(free, anchor, b, opt) {
                    genome[free[j] as usize] = b;
                    placed.insert(b);
                    free.swap_remove(j);
                }
            }
            (false, true) if unplaced.contains(&a) && !placed.contains(&a) => {
                let anchor = opt.frozen[&b];
                if let Some(i) = find_roll_neighbor(free, anchor, a, opt) {
                    genome[free[i] as usize] = a;
                    placed.insert(a);
                    free.swap_remove(i);
                }
            }
            _ => {}
        }
    }

    // ── 3. Allowed ───────────────────────────────────────────────────────────
    for &ch in letters.iter().filter(|c| opt.allowed.contains_key(c)) {
        if placed.contains(&ch) {
            continue;
        }
        let partner = cache.roll_partner.get(&ch).copied().filter(|p| {
            unplaced.contains(p) && !placed.contains(p) && !cache.frozen_chars.contains(p)
        });
        if let Some(partner) = partner
            && let Some((i, j)) = find_roll_slots(free, ch, partner, opt)
        {
            place_pair(genome, free, &mut placed, i, j, ch, partner);
            continue;
        }
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
            || !unplaced.contains(&a)
            || !unplaced.contains(&b)
            || cache.frozen_chars.contains(&a)
            || cache.frozen_chars.contains(&b)
        {
            continue;
        }
        if let Some((i, j)) = find_roll_slots(free, a, b, opt) {
            place_pair(genome, free, &mut placed, i, j, a, b);
        }
    }

    // ── 5. Free ──────────────────────────────────────────────────────────────
    for &ch in letters {
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
}

/// Unplace `count` random movable units from `genome` back into a freed-slots vec.
/// Roll pairs currently at neighbor slots are unplaced together as one unit.
pub fn unplace_units(
    genome: &mut [char],
    opt: &OptimizationConfig,
    cache: &OptimizationCache,
    count: usize,
    rng: &mut impl rand::Rng,
) -> Vec<u8> {
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
    let mut freed = Vec::new();
    for unit in units.iter().take(count) {
        for &idx in unit {
            freed.push(idx as u8);
            genome[idx] = EMPTY_SLOT;
        }
    }
    freed
}

/// Find two indices into `free` whose slots are roll-neighbors and valid for `(anchor, other)`.
pub fn find_roll_slots(
    free: &[u8],
    anchor: char,
    other: char,
    opt: &OptimizationConfig,
) -> Option<(usize, usize)> {
    (0..free.len())
        .filter(|&i| opt.is_slot_allowed(anchor, free[i]))
        .find_map(|i| find_roll_neighbor(free, free[i], other, opt).map(|j| (i, j)))
}

/// Find index into `free` of a slot that is a roll-neighbor of `anchor` and valid for `ch`.
pub fn find_roll_neighbor(
    free: &[u8],
    anchor: u8,
    other: char,
    opt: &OptimizationConfig,
) -> Option<usize> {
    free.iter()
        .position(|&s| are_roll_neighbors(anchor, s) && opt.is_slot_allowed(other, s))
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
