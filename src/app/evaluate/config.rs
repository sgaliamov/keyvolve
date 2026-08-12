use crate::app::Side;
use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the evaluation mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateConfig {
    /// input layouts csv files
    pub input: Vec<PathBuf>,

    /// output file path; overwrites input when omitted
    pub output: Option<PathBuf>,

    /// number of layouts to print to stdout
    #[serde(default = "default_print")]
    pub print: usize,

    /// Hand the letter `e` is pinned to when saving (layouts mirrored to that
    /// orientation, hand-swapped twins deduped). `any` keeps layouts verbatim.
    /// Default: `left`.
    #[serde(default)]
    pub e_side: Side,
}

fn default_print() -> usize {
    10
}

impl Default for EvaluateConfig {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            output: None,
            print: default_print(),
            e_side: Side::default(),
        }
    }
}
