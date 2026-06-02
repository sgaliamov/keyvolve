mod bigram_markov;
pub mod config;
mod corpus;
mod counter;
mod digraph;
mod digraph_method;
mod sample_method;
mod shared;

use bigram_markov::synthesise_bigram_markov;
pub use config::*;
pub use counter::CorpusStatsCounter;
use digraph_method::synthesise_digraph;
use miette::Result;
use sample_method::synthesise_sample_words;
pub use shared::{CachedSourceStats, filter_stats_bigrams, stats_cache_path, write_stats_cache};

/// Run the configured synthesise pipeline.
pub fn synthesise(cfg: SynthesiseConfig) -> Result<()> {
    match cfg.method {
        SynthesiseMethod::Digraph => synthesise_digraph(cfg),
        SynthesiseMethod::Sample => synthesise_sample_words(cfg),
        SynthesiseMethod::Markov => synthesise_bigram_markov(cfg),
    }
}
