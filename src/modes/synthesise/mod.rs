pub mod config;
mod counter;
mod shared;

pub use config::*;
pub use counter::CorpusStatsCounter;
use miette::{Context, IntoDiagnostic, Result};
use rand::{RngExt, SeedableRng, rngs::StdRng};
pub use shared::{
    CachedSourceStats, filter_stats_frequencies, read_stats_cache, score_with_filter,
    write_stats_cache,
};
use shared::{report_path, write_corpus, write_report};
use std::{
    fs,
    io::{BufRead, BufReader},
};

/// Run the sample-word synthesise pipeline.
///
/// Streams the source file in one pass: accumulates full-corpus stats and
/// builds a reservoir sample of N words simultaneously, so the file never
/// needs to fit in memory.
pub fn synthesise(cfg: SynthesiseConfig) -> Result<()> {
    let input = cfg
        .text
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.text` path")?;
    let output = cfg
        .output
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.output` path")?;

    let n = cfg.sample.target;
    let mut rng = StdRng::seed_from_u64(cfg.seed.unwrap_or(0xcafe_babe_dead_beef));

    let mut reservoir: Vec<String> = Vec::new();
    let mut total_words: usize = 0;
    let mut source_counter = CorpusStatsCounter::default();

    {
        let file = fs::File::open(input)
            .into_diagnostic()
            .wrap_err("Failed to open synth source text")?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line
                .into_diagnostic()
                .wrap_err("Failed to read synth source text")?;
            for word in line.split_ascii_whitespace() {
                if word.is_empty() {
                    continue;
                }
                source_counter.add_word(word);
                total_words += 1;

                if reservoir.len() < n {
                    reservoir.push(word.to_owned());
                } else {
                    let j = rng.random_range(0..total_words);
                    if j < n {
                        reservoir[j] = word.to_owned();
                    }
                }
            }
        }
    }

    let sampled_n = reservoir.len();
    let source_stats = source_counter.finish();

    let mut sample_counter = CorpusStatsCounter::default();
    for word in &reservoir {
        sample_counter.add_word(word);
    }
    let sample_stats = sample_counter.finish();

    let score = score_with_filter(&source_stats, &sample_stats, cfg.min_frequency);

    write_corpus(&reservoir, output)?;
    let report = report_path(output);
    write_report(&report, &score, total_words, sampled_n)?;

    tracing::info!(
        input = %input.display(),
        output = %output.display(),
        report = %report.display(),
        source_words = total_words,
        sampled_words = sampled_n,
        max_error = score.max_error,
        "Synthesise complete"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn report_path_uses_output_stem() {
        let path = Path::new("data/synthesised.txt");
        assert_eq!(
            report_path(path),
            PathBuf::from("data").join("synthesised.rpt")
        );
    }

    #[test]
    fn samples_requested_count() {
        let path = fixture_path("aa bb cc dd ee");
        let cfg = SynthesiseConfig {
            text: Some(path.clone()),
            output: Some(path.clone()),
            seed: Some(42),
            sample: SampleSynthesiseConfig { target: 3 },
            ..SynthesiseConfig::default()
        };
        synthesise(cfg).unwrap();
    }

    fn fixture_path(contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("keyvolve-sample-words-{stamp}.txt"));
        fs::write(&path, contents).unwrap();
        path
    }
}
