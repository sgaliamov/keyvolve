use crate::models::Layout;
use crate::modes::evaluate::optimization::GaContext;

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

    let indent = "     ".to_string();
    let mut out = String::new();

    out.push_str(&prefix);
    out.push('\n');

    let mut lines = score.lines();
    if let Some(first) = lines.next() {
        out.push_str(&indent);
        out.push_str(first);
        out.push_str(&suffix);
    }

    for line in lines {
        out.push('\n');
        out.push_str(&indent);
        out.push_str(line);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::format_progress_line;

    #[test]
    fn formats_multiline_progress_with_alignment() {
        let out = format_progress_line(
            "  7: alpha | ".to_string(),
            "first\nsecond\nthird".to_string(),
            " | δ: 1.2345".to_string(),
        );

        let lines: Vec<_> = out.lines().collect();
        assert_eq!(lines[0], "  7: alpha | ");
        assert_eq!(lines[1], "     first | δ: 1.2345");
        assert_eq!(lines[2], "     second");
        assert_eq!(lines[3], "     third");
    }
}
