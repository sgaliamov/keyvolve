use crate::models::Layout;
use crate::modes::optimize::GaContext;

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

    let score_str = best.state.as_ref().map_or(String::new(), |s| s.to_string());

    let min_div = ctx
        .pools
        .iter()
        .min_by(|a, b| a.diversity().partial_cmp(&b.diversity()).unwrap());

    let div_str = match min_div {
        Some(p) => format!(" | δ: {:.4}", p.diversity()),
        None => String::new(),
    };

    println!(
        "{}",
        format_progress_line(
            format!("{:>3}: {}", ctx.generation, name),
            score_str,
            div_str,
        )
    );
    true
}

fn format_progress_line(prefix: String, score: String, suffix: String) -> String {
    if score.is_empty() {
        return format!("{prefix}{suffix}");
    }

    format!(
        "{prefix} | {}{suffix}",
        score.lines().collect::<Vec<_>>().join(" | ")
    )
}

#[cfg(test)]
mod tests {
    use super::format_progress_line;

    #[test]
    fn formats_multiline_score_on_one_line() {
        let out = format_progress_line(
            "  7: alpha".to_string(),
            "first\nsecond\nthird".to_string(),
            " | δ: 1.2345".to_string(),
        );

        assert_eq!(out, "  7: alpha | first | second | third | δ: 1.2345");
    }

    #[test]
    fn formats_single_line_score() {
        let out = format_progress_line(
            "  7: alpha".to_string(),
            "fitness: 1.0000 | effort: 2.00".to_string(),
            " | δ: 1.2345".to_string(),
        );

        assert_eq!(
            out,
            "  7: alpha | fitness: 1.0000 | effort: 2.00 | δ: 1.2345"
        );
    }

    #[test]
    fn formats_missing_score() {
        let out = format_progress_line(
            "  7: alpha".to_string(),
            String::new(),
            " | δ: 1.2345".to_string(),
        );

        assert_eq!(out, "  7: alpha | δ: 1.2345");
    }
}
