mod bigram_markov;
pub mod config;
mod corpus;
mod counter;
mod digraph;
mod digraph_method;
mod sample_words;
mod shared;

use bigram_markov::synthesise_bigram_markov;
pub use config::*;
use digraph_method::synthesise_digraph;
use miette::Result;
use sample_words::synthesise_sample_words;

/// Run the configured synthesise pipeline.
pub fn synthesise(cfg: SynthesiseConfig) -> Result<()> {
    match cfg.method {
        SynthesiseMethod::Digraph => synthesise_digraph(cfg),
        SynthesiseMethod::Sample => synthesise_sample_words(cfg),
        SynthesiseMethod::Markov => synthesise_bigram_markov(cfg),
    }
}
