use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the build-stats mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct BuildStatsConfig {
    /// source text file to scan
    pub input: Option<PathBuf>,

    /// cached corpus stats JSON output path
    pub output: Option<PathBuf>,
}
