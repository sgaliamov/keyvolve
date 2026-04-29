use super::{score_calculator::calculate_score, Behavior, FrozenKeys, Keyboard, Position};
use crate::keyboard::Keys;
use ed_balance::get_version;
use itertools::Itertools;
use rand::prelude::SliceRandom;
use std::collections::HashSet;

/// Creates a completely random keyboard to seed the initial population.
/// Scoring happens here so every individual enters the pool with a valid
/// fitness value and no extra pass is required after generation.
pub fn generate(this: &Behavior) -> Box<Keyboard> {
    let version = get_version();
    let keys = generate_keys(&this.frozen_keys, &this.blocked_keys);

    debug_assert_eq!(keys.len(), 26);
    debug_assert_eq!(keys.values().max().unwrap(), &29_u8);

    Keyboard::new(
        version.clone(),
        keys.clone(),
        calculate_score(this, &keys),
        Vec::new(),
        // parent_version == version for a freshly generated individual so it
        // can be crossed with its own children without a version mismatch.
        version,
        keys,
    )
}

/// Assigns all non-frozen letters to random non-blocked positions.
/// Both lists are shuffled independently so the mapping is truly random
/// and frozen keys keep their designated slots untouched.
fn generate_keys(frozen_keys: &FrozenKeys, blocked_keys: &HashSet<Position>) -> Keys {
    let rnd = &mut rand::thread_rng();
    // Only letters not already pinned can be freely placed.
    let mut letters = ('a'..='z')
        .filter(|x| !frozen_keys.contains_key(x))
        .collect_vec();
    letters.shuffle(rnd);

    // Exclude both physically blocked slots and slots already taken by
    // frozen keys, so we never place two characters at the same position.
    let frozen_values: HashSet<_> = frozen_keys.values().cloned().collect();
    let mut positions = (0..=29 as Position)
        .filter(|x| !blocked_keys.contains(x))
        .filter(|x| !frozen_values.contains(x))
        .collect_vec();
    positions.shuffle(rnd);

    // Zip the two shuffled lists to form random char→position pairs, then
    // merge frozen keys back so the final map is always complete.
    letters
        .into_iter()
        .zip(positions.into_iter())
        .merge(frozen_keys.clone())
        .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_generate_with_no_frozen() {
        let frozen_keys: FrozenKeys = [].iter().cloned().collect();
        let frozen_values: HashSet<_> = frozen_keys.values().cloned().collect();
        let blocked_keys: HashSet<Position> = [9, 14, 19, 24].iter().cloned().collect();

        let keys = generate_keys(&frozen_keys, &blocked_keys);

        let expected_keys = ('a'..='z').collect_vec();
        let actual_keys = keys
            .keys()
            .sorted_by(|a, b| a.cmp(b))
            .cloned()
            .collect_vec();

        let expected_values = (0..=29)
            .filter(|x| !blocked_keys.contains(x))
            .filter(|x| !frozen_values.contains(x))
            .merge(frozen_keys.values().cloned())
            .sorted_by(|a, b| a.cmp(b))
            .collect_vec();

        let actual_values = keys
            .values()
            .sorted_by(|a, b| a.cmp(b))
            .cloned()
            .collect_vec();

        assert_eq!(keys.len(), 26);
        assert_eq!(actual_keys, expected_keys);
        assert_eq!(actual_values, expected_values);
    }

    #[test]
    fn test_generate() {
        let frozen_keys: FrozenKeys = [('a', 1_u8), ('b', 2_u8), ('c', 29_u8)]
            .iter()
            .cloned()
            .collect();
        let frozen_values: HashSet<_> = frozen_keys.values().cloned().collect();
        let blocked_keys: HashSet<Position> = [0, 2, 15, 16, 17].iter().cloned().collect();

        let keys = generate_keys(&frozen_keys, &blocked_keys);

        let expected_keys = ('a'..='z').collect_vec();
        let actual_keys = keys
            .keys()
            .sorted_by(|a, b| a.cmp(b))
            .cloned()
            .collect_vec();

        let expected_values = (1..=29)
            .filter(|x| !blocked_keys.contains(x))
            .filter(|x| !frozen_values.contains(x))
            .merge(frozen_keys.values().cloned())
            .sorted_by(|a, b| a.cmp(b))
            .collect_vec();

        let actual_values = keys
            .values()
            .sorted_by(|a, b| a.cmp(b))
            .cloned()
            .collect_vec();

        assert_eq!(keys.len(), 26);
        assert_eq!(keys[&'a'], 1);
        assert_eq!(keys[&'b'], 2);
        assert_eq!(keys[&'c'], 29);
        assert_eq!(actual_keys, expected_keys);
        assert_eq!(actual_values, expected_values);
    }
}
