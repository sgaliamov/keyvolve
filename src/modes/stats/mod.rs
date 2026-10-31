pub mod config;

use crate::modes::synthesise::{
    CachedSourceStats, CorpusStatsCounter, filter_stats_frequencies, write_stats_cache,
    write_stats_csv_reports,
};
use cliffa::cli::AppHandle;
pub use config::*;
use miette::{Context, IntoDiagnostic, Result, miette};
use std::{
    fs,
    io::{BufRead, BufReader},
};

/// Stats JSON file name inside the `stats.output` folder.
pub const STATS_FILE: &str = "stats.json";

/// Build corpus stats from a source text file and write JSON + CSV reports into the output folder.
pub fn stats(cfg: BuildStatsConfig, app: AppHandle) -> Result<()> {
    let input = cfg
        .input
        .wrap_err("Stats mode requires `stats.input` path")?;
    let output = cfg
        .output
        .wrap_err("Stats mode requires `stats.output` path")?;
    if output.is_file() {
        return Err(miette!(
            "`stats.output` must be a folder, found file: {}",
            output.display()
        ));
    }

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
    write_stats_cache(&output.join(STATS_FILE), &cached)?;
    write_stats_csv_reports(&output, &cached)?;

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

    fn unique() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    #[test]
    fn interrupt_before_write_keeps_existing_output() -> Result<()> {
        let unique = unique();
        let input = std::env::temp_dir().join(format!("ikeya-stats-input-{unique}.txt"));
        let output = std::env::temp_dir().join(format!("ikeya-stats-output-{unique}"));
        let json = output.join(STATS_FILE);

        fs::write(&input, "hello world\nalpha beta\n").unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(&json, "{\"keep\":\"old\"}").unwrap();

        let app = AppHandle::new();
        app.finish();

        let cfg = BuildStatsConfig {
            input: Some(input.clone()),
            output: Some(output.clone()),
            min_trigram_frequency: 0.0,
        };

        stats(cfg, app)?;

        assert_eq!(fs::read_to_string(&json).unwrap(), "{\"keep\":\"old\"}");

        let _ = fs::remove_file(input);
        let _ = fs::remove_dir_all(output);
        Ok(())
    }

    #[test]
    fn writes_all_files_into_output_folder() -> Result<()> {
        let unique = unique();
        let input = std::env::temp_dir().join(format!("ikeya-stats-input-{unique}.txt"));
        let output = std::env::temp_dir().join(format!("ikeya-stats-folder-{unique}"));
        fs::write(&input, "the tea a\n").unwrap();

        let cfg = BuildStatsConfig {
            input: Some(input.clone()),
            output: Some(output.clone()),
            min_trigram_frequency: 0.0,
        };
        stats(cfg, AppHandle::new())?;

        for name in [
            STATS_FILE,
            "letters.csv",
            "bigrams.csv",
            "bigrams.aggregated.csv",
            "first-letters.csv",
        ] {
            assert!(output.join(name).is_file(), "missing {name}");
        }

        let _ = fs::remove_file(input);
        let _ = fs::remove_dir_all(output);
        Ok(())
    }

    #[test]
    fn output_file_path_is_rejected() {
        let unique = unique();
        let input = std::env::temp_dir().join(format!("ikeya-stats-input-{unique}.txt"));
        let output = std::env::temp_dir().join(format!("ikeya-stats-file-{unique}.json"));
        fs::write(&input, "the\n").unwrap();
        fs::write(&output, "{}").unwrap();

        let cfg = BuildStatsConfig {
            input: Some(input.clone()),
            output: Some(output.clone()),
            min_trigram_frequency: 0.0,
        };
        let err = stats(cfg, AppHandle::new()).unwrap_err();
        assert!(err.to_string().contains("must be a folder"));
        assert_eq!(fs::read_to_string(&output).unwrap(), "{}");

        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }
}
