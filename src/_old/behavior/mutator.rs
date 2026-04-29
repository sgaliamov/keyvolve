use super::{score_calculator::calculate_score, Behavior};
use crate::keyboard::{Keyboard, Keys, Mutation};
use ed_balance::get_version;
use itertools::Itertools;
use rand::{prelude::SliceRandom, thread_rng, RngCore};

/// Produces one mutated offspring by swapping a random number of key pairs.
/// Mutations are recorded so the crossover operator can replay them when
/// combining two parents.
pub fn mutate(this: &Behavior, individual: &Keyboard) -> Box<Keyboard> {
    let mut rng = thread_rng();
    let mut mutations: Vec<Mutation> = Vec::with_capacity(this.context.mutations_count);
    // Frozen keys must never move, so we exclude them before shuffling.
    let mut keys = individual
        .keys
        .iter()
        .filter(|(c, _)| !this.frozen_keys.contains_key(c))
        .map(|(&key, &value)| (key, value))
        .collect_vec();

    // Shuffling lets us pick swap pairs uniformly without bias toward any
    // particular region of the keyboard.
    keys.shuffle(&mut rng);
    // A variable number of swaps (at least 1) adds diversity; using the
    // configured max as the modulus keeps it within user-controlled bounds.
    let mutations_count = 1 + (rng.next_u32() as usize % this.context.mutations_count);

    for index in 0..mutations_count {
        // Pair the front of the shuffled list with the back so that each
        // iteration swaps two distinct keys without re-visiting earlier pairs.
        let second_index = keys.len() - index - 1;
        let (first_char, first) = keys[index];
        let (second_char, second) = keys[second_index];

        // Record the position pair (not the characters) so the crossover
        // operator can replay the swap regardless of what characters end up
        // at those positions in the partner parent.
        mutations.push(Mutation { first, second });
        keys[index] = (first_char, second);
        keys[second_index] = (second_char, first);
    }

    let version = get_version();
    // Merge frozen keys back so the full 26-key map is always intact.
    let keys: Keys = keys.into_iter().merge(this.frozen_keys.clone()).collect();
    debug_assert_eq!(keys.len(), individual.keys.len());
    debug_assert_eq!(keys.values().max().unwrap(), &29_u8);

    let score = calculate_score(this, &keys);

    Keyboard::new(
        version,
        keys.clone(),
        score,
        mutations,
        // Store the parent's current version and key map so the offspring can
        // be crossed with other children of the same parent later.
        individual.version.clone(),
        individual.keys.clone(),
    )
}
