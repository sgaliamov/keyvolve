pub mod config;
mod counter;
mod sample_method;
mod shared;

pub use config::*;
pub use counter::CorpusStatsCounter;
use miette::Result;
use sample_method::synthesise_sample_words;
pub use shared::{
    CachedSourceStats, filter_stats_bigrams, read_stats_cache, stats_cache_path, write_stats_cache,
};

/// Run the sample-word synthesise pipeline.
pub fn synthesise(cfg: SynthesiseConfig) -> Result<()> {
    synthesise_sample_words(cfg)
}
