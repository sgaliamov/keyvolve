use crate::output::Side;
use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the evaluation mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateConfig {
    /// Keyboard JSON used for layout scoring.
    pub keyboard: PathBuf,

    /// Cached corpus stats JSON used for scoring.
    pub corpus_stats: PathBuf,

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

    /// Remove layouts violating current optimization placement constraints before scoring.
    #[serde(default)]
    pub filter_invalid: bool,
}

fn default_print() -> usize {
    10
}

impl Default for EvaluateConfig {
    fn default() -> Self {
        Self {
            keyboard: PathBuf::default(),
            corpus_stats: PathBuf::default(),
            input: Vec::new(),
            output: None,
            print: default_print(),
            e_side: Side::default(),
            filter_invalid: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn placement_filter_is_opt_in_and_requires_a_boolean() {
        let base = json!({
            "keyboard": "keyboard.json", "corpusStats": "stats.json", "input": []
        });
        let cfg: EvaluateConfig = serde_json::from_value(base.clone()).unwrap();
        assert!(!cfg.filter_invalid);
        assert!(!EvaluateConfig::default().filter_invalid);

        for enabled in [false, true] {
            let mut value = base.clone();
            value["filterInvalid"] = json!(enabled);
            let cfg: EvaluateConfig = serde_json::from_value(value).unwrap();
            assert_eq!(cfg.filter_invalid, enabled);
        }
        let mut value = base;
        value["filterInvalid"] = json!("true");
        assert!(serde_json::from_value::<EvaluateConfig>(value).is_err());
    }
}
