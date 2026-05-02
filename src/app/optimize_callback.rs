use crate::models::{GaContext, Layout};

/// Progress callback for optimize mode.
pub fn optimize_callback(ctx: &GaContext) {
    let Some((genome, fitness)) = ctx.pools.best() else {
        return;
    };
    let name = Layout::from_keys(genome).name();
    println!("gen {:>6}  fit {:.4}  {}", ctx.generation, fitness, name);
}
