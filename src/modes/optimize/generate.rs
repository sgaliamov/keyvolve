use crate::modes::optimize::{GaContext, KeysGenome};

/// Generate a complete layout satisfying the compiled placement constraints.
pub fn generate(ctx: &GaContext) -> KeysGenome {
    let state = ctx.state.as_ref().expect("state must be set");
    state.constraints.generate(&mut rand::rng()).to_vec()
}
