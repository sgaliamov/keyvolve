use crate::models::{GaContext, Layout};

/// Progress callback for optimize mode. Returns `false` to stop early.
pub fn optimize_callback(ctx: &GaContext) -> bool {
    let Some((genome, fitness)) = ctx.pools.best() else {
        return true;
    };
    let name = Layout::from_keys(genome).name();
    println!("gen {:>6}  fit {:.4}  {}", ctx.generation, fitness, name);
    true
}
