use crate::app::synthesise::{
    CachedSourceStats, SynthesiseConfig,
    counter::{CorpusStats, CorpusStatsCounter, score_stats},
    shared::{read_stats_cache, report_path, stats_cache_path, write_corpus, write_report, write_stats_cache},
};
use miette::{Context, IntoDiagnostic, Result};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{
    fs,
    io::{BufRead, BufReader},
};

/// Run the sample-word synthesise pipeline.
///
/// Streams the source file in one pass: accumulates full-corpus stats and
/// builds a reservoir sample of N words simultaneously, so the file never
/// needs to fit in memory.
pub(super) fn synthesise_sample_words(cfg: SynthesiseConfig) -> Result<()> {
    let input = cfg
        .text
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.text` path")?;
    let output = cfg
        .output
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.output` path")?;

    let n = cfg.sample.word_count;
    let mut rng = StdRng::seed_from_u64(cfg.sample.seed.unwrap_or(0xcafe_babe_dead_beef));

    let cache_path = stats_cache_path(input);
    let cached_stats: Option<(CorpusStats, usize)> = if cache_path.exists() {
        tracing::info!(cache = %cache_path.display(), "Using saved source stats");
        let c = read_stats_cache(&cache_path)?;
        Some((c.stats, c.word_count))
    } else {
        None
    };

    let mut reservoir: Vec<String> = Vec::new();
    let mut total_words: usize = 0;
    let mut source_counter = CorpusStatsCounter::default();
    let track_stats = cached_stats.is_none();

    {
        let file = fs::File::open(input)
            .into_diagnostic()
            .wrap_err("Failed to open synth source text")?;
        let reader = BufReader::new(file);

        // Algorithm R reservoir sampling; optionally accumulate stats in same pass.
        for line in reader.lines() {
            let line = line
                .into_diagnostic()
                .wrap_err("Failed to read synth source text")?;
            for word in line.split_ascii_whitespace() {
                if word.is_empty() {
                    continue;
                }
                if track_stats {
                    source_counter.add_word(word);
                }
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
    let source_stats = if let Some((stats, wc)) = cached_stats {
        total_words = wc;
        stats
    } else {
        let stats = source_counter.finish();
        let cached = CachedSourceStats { stats: stats.clone(), word_count: total_words };
        write_stats_cache(&cache_path, &cached)?;
        tracing::info!(cache = %cache_path.display(), "Source stats saved");
        stats
    };

    let mut sample_counter = CorpusStatsCounter::default();
    for word in &reservoir {
        sample_counter.add_word(word);
    }
    let sample_stats = sample_counter.finish();

    let score = score_stats(&source_stats, &sample_stats);

    write_corpus(&reservoir, output)?;
    let report = report_path(output, "sample");
    write_report(
        &report,
        &score,
        total_words,
        sampled_n,
        cfg.sample.tolerance,
    )?;

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
            report_path(path, "sample"),
            PathBuf::from("data").join("synthesised.sample.txt")
        );
    }

    #[test]
    fn samples_requested_count() {
        let path = fixture_path("aa bb cc dd ee");
        let cfg = SynthesiseConfig {
            text: Some(path.clone()),
            output: Some(path.clone()),
            sample: crate::app::synthesise::SampleSynthesiseConfig {
                word_count: 3,
                seed: Some(42),
                ..crate::app::synthesise::SampleSynthesiseConfig::default()
            },
            ..SynthesiseConfig::default()
        };
        synthesise_sample_words(cfg).unwrap();
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
