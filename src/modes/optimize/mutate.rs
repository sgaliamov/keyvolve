use crate::modes::optimize::{GaContext, KeysGenome, KeysIndividual};

/// Produce independent valid mutations; repair stale parents before local moves.
pub fn mutate(ind: &KeysIndividual, ctx: &GaContext) -> Vec<KeysGenome> {
    let state = ctx.state.as_ref().expect("state must be set");
    let mut rng = rand::rng();
    let parent = state.constraints.repair(&ind.genome, &mut rng);
    if parent.as_slice() != ind.genome {
        tracing::warn!("Repaired invalid optimizer parent before mutation");
    }
    (0..state.mutation_count)
        .map(|_| state.constraints.mutate(&parent, &mut rng).to_vec())
        .collect()
}
