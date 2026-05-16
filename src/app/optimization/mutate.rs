use crate::app::optimization::{place_letters, unplace_units};
use crate::app::{GaContext, KeysGenome, KeysIndividual};
use rand::seq::SliceRandom;

/// Mutate by unplacing N random units and re-placing under the same constraint flow as the generator.
/// Returns 10 independent mutants, each derived from a fresh clone of the parent genome.
pub fn mutate(ind: &KeysIndividual, ctx: &GaContext) -> Vec<KeysGenome> {
    let state = ctx.state.as_ref().expect("state must be set");
    let opt = &state.optimization;
    let cache = &state.cache;

    (0..10)
        .map(|_| {
            let mut genome = ind.genome.clone();
            let mut rng = rand::rng();
            let count = rand::random_range(1usize..=5);

            let mut free = unplace_units(&mut genome, opt, cache, count, &mut rng);
            if free.is_empty() {
                return genome;
            }

            // Collect chars that were just unplaced, in shuffled order.
            let mut letters: Vec<char> = genome
                .iter()
                .zip(ind.genome.iter())
                .filter(|(new, old)| *new != *old)
                .map(|(_, old)| *old)
                .collect();
            letters.shuffle(&mut rng);
            free.shuffle(&mut rng);

            place_letters(&mut genome, &mut free, &letters, opt, cache);
            genome
        })
        .collect()
}
