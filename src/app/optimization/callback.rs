use crate::app::GaContext;
use crate::models::Layout;

/// Progress callback for optimize mode. Returns `false` to stop early.
pub fn callback(ctx: &GaContext) -> bool {
    if ctx.state.as_ref().unwrap().app.should_finish() {
        return false;
    }

    let Some((genome, fitness)) = ctx.pools.best() else {
        return true;
    };

    let name = Layout::from_keys(genome).to_string();

    let min_div = ctx
        .pools
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.diversity().partial_cmp(&b.diversity()).unwrap());

    let div_str = match min_div {
        Some((i, p)) => format!(" | div pool {} {:.4}", i, p.diversity()),
        None => String::new(),
    };

    println!(
        "{:>6}: {} | fit {:.4}{}",
        ctx.generation, name, fitness, div_str
    );
    true
}
