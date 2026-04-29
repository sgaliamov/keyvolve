use super::{score_calculator::calculate_score, Behavior};
use crate::keyboard::{Keyboard, Keys};
use ed_balance::get_version;
use itertools::Itertools;
use rand::prelude::SliceRandom;
use std::collections::HashMap;

/// Creates a child from two parents by replaying a random subset of their
/// combined mutation history on top of the *shared* parent layout.
/// Starting from the common parent ensures the child is always a legal
/// keyboard (no duplicate positions, no missing letters).
pub fn cross(this: &Behavior, individual: &Keyboard, partner: &Keyboard) -> Box<Keyboard> {
    // Invert the parent layout to a position→char map so individual swaps
    // can be applied in O(1) without scanning the whole keyboard each time.
    let mut keys: HashMap<_, _> = individual
        .parent
        .iter()
        .map(|(key, value)| (value, key))
        .collect();

    // Combine both parents' mutation lists; deduplication prevents the same
    // swap from being applied twice, which would cancel itself out.
    let mut mutations: Vec<_> = individual
        .mutations
        .iter()
        .chain(partner.mutations.iter())
        .unique()
        .map(|&x| x)
        .collect();

    // Shuffle before taking a prefix so we sample the union uniformly rather
    // than always preferring the first parent's mutations.
    mutations.shuffle(&mut rand::thread_rng());

    // Apply only `mutations_count` swaps, keeping the child's edit distance
    // from the parent bounded and comparable to a plain mutation.
    for mutation in mutations.iter().take(this.context.mutations_count) {
        let first_char = keys[&mutation.first];
        let second_char = keys[&mutation.second];
        *keys.entry(&mutation.first).or_insert(second_char) = second_char;
        *keys.entry(&mutation.second).or_insert(first_char) = first_char;
    }

    // Convert back from position→char to char→position for the Keyboard type.
    let keys: Keys = keys
        .into_iter()
        .map(|(&key, &value)| (value, key))
        .collect();

    let score = calculate_score(this, &keys);

    debug_assert_eq!(keys.len(), 26);
    debug_assert_eq!(keys.values().max().unwrap(), &29_u8);

    Keyboard::new(
        get_version(),
        keys,
        score,
        mutations,
        // Preserve the lineage so future crossovers can still trace back to
        // the common ancestor.
        individual.parent_version.clone(),
        individual.parent.clone(),
    )
}
