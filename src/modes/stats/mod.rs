pub mod config;

use crate::modes::synthesise::{
    CachedSourceStats, CorpusStatsCounter, filter_stats_frequencies, write_stats_cache,
};
use cliffa::cli::AppHandle;
pub use config::*;
use miette::{Context, IntoDiagnostic, Result};
use std::{
    fs,
    io::{BufRead, BufReader},
};

/// Build cached corpus stats from a source text file and write them to JSON.
pub fn stats(cfg: BuildStatsConfig, app: AppHandle) -> Result<()> {
    let input = cfg
        .input
        .wrap_err("Stats mode requires `stats.input` path")?;
    let output = cfg
        .output
        .wrap_err("Stats mode requires `stats.output` path")?;

    if app.should_finish() {
        tracing::info!(input = %input.display(), "Stats interrupted before reading input");
        return Ok(());
    }

    let file = fs::File::open(&input)
        .into_diagnostic()
        .wrap_err("Failed to open source text for stats")?;
    let reader = BufReader::new(file);

    let mut counter = CorpusStatsCounter::default();
    let mut word_count = 0usize;
    for line in reader.lines() {
        if app.should_finish() {
            tracing::info!(input = %input.display(), "Stats interrupted while reading input");
            return Ok(());
        }

        let line = line.into_diagnostic()?;
        for word in line.split_ascii_whitespace() {
            if word.is_empty() {
                continue;
            }
            counter.add_word(word);
            word_count += 1;
        }
    }

    if app.should_finish() {
        tracing::info!(output = %output.display(), "Stats interrupted before writing output");
        return Ok(());
    }

    let mut stats = counter.finish();
    filter_stats_frequencies(&mut stats, 0.0, cfg.min_trigram_frequency);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn interrupt_before_write_keeps_existing_output() -> Result<()> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let input = std::env::temp_dir().join(format!("keyvolve-stats-input-{unique}.txt"));
        let output = std::env::temp_dir().join(format!("keyvolve-stats-output-{unique}.json"));

        fs::write(&input, "hello world\nalpha beta\n").unwrap();
        fs::write(&output, "{\"keep\":\"old\"}").unwrap();

        let app = AppHandle::new();
        app.finish();

        let cfg = BuildStatsConfig {
            input: Some(input.clone()),
            output: Some(output.clone()),
            min_trigram_frequency: 0.0,
        };

        stats(cfg, app)?;

        assert_eq!(fs::read_to_string(&output).unwrap(), "{\"keep\":\"old\"}");

        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
        Ok(())
    }
}
