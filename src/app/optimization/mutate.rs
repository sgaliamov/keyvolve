use crate::app::optimization::{place_letters, unplace_units};
use crate::app::{GaContext, KeysGenome, KeysIndividual};
use rand::seq::SliceRandom;

const MUTATION_ATTEMPTS: usize = 3;

/// Mutate by unplacing N random units and re-placing under the same constraint flow as the generator.
/// Returns `opt.mutation_count` independent mutants, each derived from a fresh clone of the parent genome.
pub fn mutate(ind: &KeysIndividual, ctx: &GaContext) -> Vec<KeysGenome> {
    let state = ctx.state.as_ref().expect("state must be set");
    let opt = &state.optimization;
    let cache = &state.cache;

    (0..opt.mutation_count)
        .map(|_| {
            let mut rng = rand::rng();
            for _ in 0..MUTATION_ATTEMPTS {
                let mut genome = ind.genome.clone();
                let count = rand::random_range(2usize..=8);

                let unplaced = unplace_units(&mut genome, opt, cache, count, &mut rng);
                let mut free = unplaced.free;
                if free.is_empty() {
                    return genome;
                }

                // Collect chars that were just unplaced, in shuffled order.
                let mut letters = unplaced.letters;
                letters.shuffle(&mut rng);
                free.shuffle(&mut rng);

                place_letters(&mut genome, &mut free, &letters, opt, cache);
                if genome != ind.genome {
                    return genome;
                }
            }

            ind.genome.clone()
        })
        .collect()
}
