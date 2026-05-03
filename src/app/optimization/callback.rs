use crate::app::GaContext;
use crate::models::Layout;

/// Progress callback for optimize mode. Returns `false` to stop early.
pub fn callback(ctx: &GaContext) -> bool {
    if ctx.state.as_ref().unwrap().1.should_finish() {
        return false;
    }

    let Some((genome, fitness)) = ctx.pools.best() else {
        return true;
    };

    let name = Layout::from_keys(genome).to_string();
    println!("gen {:>6}  fit {:.4}  {}", ctx.generation, fitness, name);
    true
}
