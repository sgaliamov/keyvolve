use crate::app::GaContext;
use crate::models::Layout;

/// Progress callback for optimize mode. Returns `false` to stop early.
pub fn callback(ctx: &GaContext) -> bool {
    if ctx.state.as_ref().unwrap().app.should_finish() {
        return false;
    }

    let best = ctx
        .pools
        .iter()
        .flat_map(|p| p.individuals.iter())
        .filter(|ind| ind.fitness.is_finite())
        .max_by(|a, b| a.fitness.total_cmp(&b.fitness));

    let Some(best) = best else {
        return true;
    };

    let name = Layout::from_keys(&best.genome).to_string();

    let ratio_str = best.state.as_ref().map_or(String::new(), |s| {
        format!(
            " | Δ {:.2}% | ⟳Δ {:.2}% | ↕ {:.2}%",
            s.count_imbalance(),
            s.roll_imbalance(),
            s.row_switch_ratio() * 100.0
        )
    });

    let min_div = ctx
        .pools
        .iter()
        .min_by(|a, b| a.diversity().partial_cmp(&b.diversity()).unwrap());

    let div_str = match min_div {
        Some(p) => format!(" | δ: {:.4}", p.diversity()),
        None => String::new(),
    };

    println!(
        "{:>3}: {} | φ: {:.4}{}{}",
        ctx.generation, name, best.fitness, ratio_str, div_str
    );
    true
}
