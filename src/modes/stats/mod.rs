pub mod config;

use crate::modes::synthesise::{
    CachedSourceStats, CorpusStatsCounter, filter_stats_bigrams, write_stats_cache,
};
pub use config::*;
use miette::{Context, IntoDiagnostic, Result};
use std::{
    fs,
    io::{BufRead, BufReader},
};

/// Build cached corpus stats from a source text file and write them to JSON.
pub fn stats(cfg: BuildStatsConfig) -> Result<()> {
    let input = cfg
        .input
        .wrap_err("Stats mode requires `stats.input` path")?;
    let output = cfg
        .output
        .wrap_err("Stats mode requires `stats.output` path")?;

    let file = fs::File::open(&input)
        .into_diagnostic()
        .wrap_err("Failed to open source text for stats")?;
    let reader = BufReader::new(file);

    let mut counter = CorpusStatsCounter::default();
    let mut word_count = 0usize;
    for line in reader.lines() {
        let line = line.into_diagnostic()?;
        for word in line.split_ascii_whitespace() {
            if word.is_empty() {
                continue;
            }
            counter.add_word(word);
            word_count += 1;
        }
    }

    let mut stats = counter.finish();
    filter_stats_bigrams(&mut stats, cfg.min_frequency);
    let cached = CachedSourceStats { stats, word_count };
    write_stats_cache(&output, &cached)?;

    tracing::info!(
        input = %input.display(),
        output = %output.display(),
        words = word_count,
        "Stats written"
    );
    Ok(())
}
