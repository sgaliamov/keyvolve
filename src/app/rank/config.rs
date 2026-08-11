use miette::Result;
use serde::Deserialize;
use std::path::PathBuf;

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

fn default_uphill_gap() -> f64 {
    100.0
}

fn default_thin_margin() -> f64 {
    1.0
}

fn default_forced_answer_weight() -> u32 {
    3
}

fn is_pair_label(value: &str) -> bool {
    value.len() == 2 && value.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn is_forced_check_pair_format(value: &str) -> bool {
    let Some((left, right)) = value.split_once('-') else {
        return false;
    };
    !left.is_empty() && !right.is_empty() && is_pair_label(left) && is_pair_label(right)
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

    /// Hard confirmation cap for items that remain uncertain.
    #[serde(default = "default_max_matches")]
    pub max_matches: u32,

    /// Rating deviation below which an item counts as settled.
    #[serde(default = "default_max_deviation")]
    pub max_deviation: f64,

    /// Lower bound for adaptive tier efforts.
    #[serde(default = "default_effort_min")]
    pub effort_min: f64,

    /// Upper bound for adaptive tier efforts.
    #[serde(default = "default_effort_max")]
    pub effort_max: f64,

    /// Minimum fitted rating gap for an edge to qualify as uphill (cycle-prone).
    #[serde(default = "default_uphill_gap")]
    pub uphill_gap: f64,

    /// Maximum head-to-head margin for an edge to stay thin (fragile, re-askable).
    #[serde(default = "default_thin_margin")]
    pub thin_margin: f64,

    /// Number of times to record a forced answer (with `!` suffix).
    #[serde(default = "default_forced_answer_weight")]
    pub forced_answer_weight: u32,

    /// Optional first question forced once per run, formatted as `AF-VE`.
    #[serde(default)]
    pub force_check_pair: Option<String>,

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
        if !self.uphill_gap.is_finite() || self.uphill_gap <= 0.0 {
            return Err(miette::miette!(
                "rank.uphillGap must be finite and greater than 0"
            ));
        }
        if !self.thin_margin.is_finite() || self.thin_margin < 0.0 {
            return Err(miette::miette!(
                "rank.thinMargin must be finite and non-negative"
            ));
        }
        if self.forced_answer_weight == 0 {
            return Err(miette::miette!(
                "rank.forcedAnswerWeight must be greater than 0"
            ));
        }
        if let Some(pair) = &self.force_check_pair
            && !is_forced_check_pair_format(pair)
        {
            return Err(miette::miette!(
                "rank.forceCheckPair must be in XX-YY format (letters only), e.g. AF-VE"
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

    /// Resolved flat per-bigram CSV path, derived from the report path.
    pub fn bigrams_path(&self) -> PathBuf {
        let report = self.report_path();
        let stem = report
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("report");
        let extension = report.extension().and_then(|s| s.to_str());
        let file_name = match extension {
            Some(ext) if !ext.is_empty() => format!("{stem}.bigrams.{ext}"),
            _ => format!("{stem}.bigrams"),
        };
        report.with_file_name(file_name)
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
            uphill_gap: default_uphill_gap(),
            thin_margin: default_thin_margin(),
            forced_answer_weight: default_forced_answer_weight(),
            force_check_pair: None,
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
    fn accepts_forced_check_pair_format() {
        let cfg = RankConfig {
            force_check_pair: Some("AF-VE".to_owned()),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_forced_check_pair_format() {
        let cfg = RankConfig {
            force_check_pair: Some("AF/VE".to_owned()),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn derives_bigrams_path_from_report_path() {
        let cfg = RankConfig {
            report: Some(PathBuf::from("data/rank/report.csv")),
            ..Default::default()
        };
        assert_eq!(
            cfg.bigrams_path(),
            PathBuf::from("data/rank/report.bigrams.csv")
        );
    }
}
