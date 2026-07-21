use miette::Result;
use serde::Deserialize;
use std::path::PathBuf;

const RANKED_PAIR_COUNT: usize = 210;

fn default_audit_rate() -> f64 {
    0.0
}

fn default_min_matches() -> u32 {
    10
}

fn default_max_matches() -> u32 {
    30
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

fn default_bucket_tolerance() -> usize {
    1
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

    /// Probability of an audit question (consistency re-check of settled pairs).
    #[serde(default = "default_audit_rate")]
    pub audit_rate: f64,

    /// Matches per item required to consider it settled.
    #[serde(default = "default_min_matches")]
    pub min_matches: u32,

    /// Hard confirmation cap for items sitting exactly on a bucket boundary.
    #[serde(default = "default_max_matches")]
    pub max_matches: u32,

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

    /// Allowed neighboring-bucket movement when declaring rank confidence.
    #[serde(default = "default_bucket_tolerance")]
    pub bucket_tolerance: usize,

    /// Optional RNG seed for reproducible question order.
    pub seed: Option<u64>,
}

impl RankConfig {
    /// Reject settings that break ranking, confidence, or output semantics.
    pub fn validate(&self) -> Result<()> {
        if !self.audit_rate.is_finite() || !(0.0..=1.0).contains(&self.audit_rate) {
            return Err(miette::miette!("rank.auditRate must be between 0 and 1"));
        }
        if self.min_matches == 0 {
            return Err(miette::miette!("rank.minMatches must be greater than 0"));
        }
        if self.max_matches < self.min_matches {
            return Err(miette::miette!(
                "rank.maxMatches must be at least rank.minMatches"
            ));
        }
        if !self.max_deviation.is_finite() || self.max_deviation <= 0.0 {
            return Err(miette::miette!(
                "rank.maxDeviation must be finite and greater than 0"
            ));
        }
        if !self.effort_min.is_finite()
            || !self.effort_max.is_finite()
            || self.effort_min >= self.effort_max
        {
            return Err(miette::miette!(
                "rank effortMin and effortMax must be finite, with effortMin < effortMax"
            ));
        }
        if !(1..=RANKED_PAIR_COUNT).contains(&self.groups) {
            return Err(miette::miette!(
                "rank.groups must be between 1 and {RANKED_PAIR_COUNT}"
            ));
        }
        if self.bucket_tolerance >= self.groups {
            return Err(miette::miette!(
                "rank.bucketTolerance must be smaller than rank.groups"
            ));
        }
        Ok(())
    }

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
            max_matches: default_max_matches(),
            max_deviation: default_max_deviation(),
            effort_min: default_effort_min(),
            effort_max: default_effort_max(),
            groups: default_groups(),
            bucket_tolerance: default_bucket_tolerance(),
            seed: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        assert!(RankConfig::default().validate().is_ok());
    }

    #[test]
    fn rejects_invalid_match_bounds() {
        let cfg = RankConfig {
            min_matches: 10,
            max_matches: 9,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn rejects_invalid_bucket_settings() {
        let cfg = RankConfig {
            groups: 20,
            bucket_tolerance: 20,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }
}
