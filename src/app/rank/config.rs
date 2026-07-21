use serde::Deserialize;
use std::path::PathBuf;

fn default_audit_rate() -> f64 {
    0.0
}

fn default_min_matches() -> u32 {
    10
}

fn default_max_deviation() -> f64 {
    170.0
}

fn default_effort_min() -> f64 {
    1.0
}

fn default_effort_max() -> f64 {
    10.0
}

fn default_groups() -> usize {
    20
}

/// Settings for the interactive pair-ranking mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RankConfig {
    /// Output keyboard JSON path (ranked efforts + pairs).
    pub output: Option<PathBuf>,

    /// CSV visual report path; defaults to `output` with `.csv` extension.
    pub report: Option<PathBuf>,

    /// Session state file for pause/resume; defaults to `data/rank-session.json`.
    pub session: Option<PathBuf>,

    /// Probability of an audit question (re-check of a settled, far-apart pair).
    #[serde(default = "default_audit_rate")]
    pub audit_rate: f64,

    /// Matches per item required to consider it settled.
    #[serde(default = "default_min_matches")]
    pub min_matches: u32,

    /// Rating deviation below which an item counts as settled.
    #[serde(default = "default_max_deviation")]
    pub max_deviation: f64,

    /// Effort assigned to the most preferable bucket.
    #[serde(default = "default_effort_min")]
    pub effort_min: f64,

    /// Effort assigned to the least preferable bucket.
    #[serde(default = "default_effort_max")]
    pub effort_max: f64,

    /// Number of effort buckets in the output.
    #[serde(default = "default_groups")]
    pub groups: usize,

    /// Optional RNG seed for reproducible question order.
    pub seed: Option<u64>,
}

impl RankConfig {
    /// Resolved session path.
    pub fn session_path(&self) -> PathBuf {
        self.session
            .clone()
            .unwrap_or_else(|| PathBuf::from("data/rank-session.json"))
    }

    /// Resolved output JSON path.
    pub fn output_path(&self) -> PathBuf {
        self.output
            .clone()
            .unwrap_or_else(|| PathBuf::from("data/keyboard.ranked.json"))
    }

    /// Resolved CSV report path: explicit `report`, or output with `.csv`.
    pub fn report_path(&self) -> PathBuf {
        self.report
            .clone()
            .unwrap_or_else(|| self.output_path().with_extension("csv"))
    }
}

impl Default for RankConfig {
    fn default() -> Self {
        Self {
            output: None,
            report: None,
            session: None,
            audit_rate: default_audit_rate(),
            min_matches: default_min_matches(),
            max_deviation: default_max_deviation(),
            effort_min: default_effort_min(),
            effort_max: default_effort_max(),
            groups: default_groups(),
            seed: None,
        }
    }
}
