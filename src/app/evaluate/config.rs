use crate::app::Side;
use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the evaluation mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateConfig {
    /// input corpus text file used for scoring
    pub text: PathBuf,

    /// input layouts csv file
    pub input: PathBuf,

    /// output file path; overwrites input when omitted
    pub output: Option<PathBuf>,

    /// number of layouts to print to stdout
    #[serde(default = "default_print")]
    pub print: usize,

    /// Hand the letter `a` is pinned to when saving (layouts mirrored to that
    /// orientation, hand-swapped twins deduped). `any` keeps layouts verbatim.
    /// Default: `left`.
    #[serde(default)]
    pub a_side: Side,
}

fn default_print() -> usize {
    10
}

impl Default for EvaluateConfig {
    fn default() -> Self {
        Self {
            text: PathBuf::default(),
            input: PathBuf::default(),
            output: None,
            print: default_print(),
            a_side: Side::default(),
        }
    }
}
