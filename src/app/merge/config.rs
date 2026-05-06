use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the merge mode.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MergeConfig {
    /// folder containing `.txt` files to merge
    pub input: Option<PathBuf>,

    /// output file path
    pub output: Option<PathBuf>,
}
