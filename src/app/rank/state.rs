use super::fit::fit_bradley_terry;
use crate::app::rank::RankConfig;
use miette::{Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Slots per hand; ranking covers the left hand only (right inferred by symmetry).
pub const HAND_SLOTS: u8 = 15;

/// QWERTY reference chars for left-hand slots 0–14 (rows top→bottom).
pub const QWERTY: [char; 15] = [
    'q', 'w', 'e', 'r', 't', 'a', 's', 'd', 'f', 'g', 'z', 'x', 'c', 'v', 'b',
];

/// QWERTY reference chars for right-hand slots 15–29 (rows top→bottom).
pub const QWERTY_RIGHT: [char; 15] = [
    'y', 'u', 'i', 'o', 'p', 'h', 'j', 'k', 'l', ';', 'n', 'm', ',', '.', '/',
];

/// Initial rating for every pair.
pub const START_RATING: f64 = 1500.0;
/// Initial rating deviation (uncertainty).
pub const START_DEV: f64 = 350.0;
/// Current on-disk session schema.
const SESSION_VERSION: u32 = 2;
/// Normal 95% confidence interval multiplier.
const CONFIDENCE_Z: f64 = 1.96;

/// One ordered left-hand bigram pair with its Bradley–Terry rating.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub from: u8,
    pub to: u8,
    pub rating: f64,
    pub deviation: f64,
    pub matches: u32,
}

impl Item {
    /// QWERTY reference label, e.g. slots (8, 3) → "FR".
    pub fn label(&self) -> String {
        let ch = |s: u8| QWERTY[s as usize].to_ascii_uppercase();
        format!("{}{}", ch(self.from), ch(self.to))
    }

    /// Right-hand mirrored label (column symmetry), e.g. slots (8, 3) → "JU".
    pub fn label_right(&self) -> String {
        let ch = |s: u8| QWERTY_RIGHT[((s / 5) * 5 + (4 - s % 5)) as usize].to_ascii_uppercase();
        format!("{}{}", ch(self.from), ch(self.to))
    }
}

/// One recorded answer with pre-update snapshots for undo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Answer {
    /// Item indexes into `RankState::items`.
    pub a: usize,
    pub b: usize,
    /// Score for `a`: 1.0 win, 0.0 loss, 0.5 tie.
    pub score: f64,
    /// (rating, deviation, matches) of `a`/`b` before the update.
    pub prev_a: (f64, f64, u32),
    pub prev_b: (f64, f64, u32),
    /// Pending verification confirmations before this answer (v2+).
    #[serde(default)]
    pub prev_pending_a: u8,
    #[serde(default)]
    pub prev_pending_b: u8,
}

/// Full ranking session state, persisted after every answer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankState {
    /// On-disk schema version; missing means legacy v1.
    #[serde(default = "legacy_version")]
    pub version: u32,
    pub items: Vec<Item>,
    pub history: Vec<Answer>,
    /// Set when a run ends with every pair settled; next run verifies the ranking.
    #[serde(default)]
    pub finished: bool,
    /// Extra confirmations requested after contradictory verification answers.
    #[serde(default)]
    pending: Vec<u8>,
    /// Derived Bradley–Terry posterior covariance (squared Elo points).
    #[serde(skip)]
    covariance: Vec<f64>,
}

impl RankState {
    /// Fresh state with all 210 ordered pairs (from ≠ to) at default rating.
    pub fn new() -> Self {
        Self {
            version: SESSION_VERSION,
            items: fresh_items(),
            history: vec![],
            finished: false,
            pending: vec![0; pair_count()],
            covariance: prior_covariance(pair_count()),
        }
    }

    /// Load and losslessly migrate a session, falling back to its rolling backup.
    pub fn load_or_new(path: &Path) -> Result<Self> {
        let mut failures = vec![];
        if path.exists() {
            match Self::load(path) {
                Ok(state) => return Ok(state),
                Err(error) => failures.push(format!("{}: {error:?}", path.display())),
            }
        }

        for recovery in [appended_path(path, ".tmp"), backup_path(path)] {
            if !recovery.exists() {
                continue;
            }
            match Self::load(&recovery) {
                Ok(state) => {
                    eprintln!(
                        "Warning: recovered session {} from {}.",
                        path.display(),
                        recovery.display()
                    );
                    if path.exists() {
                        std::fs::remove_file(path).into_diagnostic()?;
                    }
                    state.save(path)?;
                    return Ok(state);
                }
                Err(error) => failures.push(format!("{}: {error:?}", recovery.display())),
            }
        }

        if failures.is_empty() {
            Ok(Self::new())
        } else {
            Err(miette::miette!(
                "No valid rank session found; refusing to start over:\n{}",
                failures.join("\n")
            ))
        }
    }

    /// Persist session using a synced temporary file and rolling `.bak` copy.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).into_diagnostic()?;
        }
        let json = serde_json::to_vec_pretty(self).into_diagnostic()?;
        serde_json::from_slice::<serde_json::Value>(&json)
            .into_diagnostic()
            .wrap_err("Refusing to persist invalid session JSON")?;

        let temporary = appended_path(path, ".tmp");
        let backup = backup_path(path);
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)
            .into_diagnostic()?;
        file.write_all(&json).into_diagnostic()?;
        file.sync_all().into_diagnostic()?;
        drop(file);

        if path.exists() {
            if backup.exists() {
                std::fs::remove_file(&backup).into_diagnostic()?;
            }
            std::fs::rename(path, &backup).into_diagnostic()?;
        }
        if let Err(error) = std::fs::rename(&temporary, path) {
            if backup.exists() {
                let _ = std::fs::copy(&backup, path);
            }
            return Err(error)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to replace session file: {}", path.display()));
        }
        Ok(())
    }

    /// Record an answer, then deterministically refit all derived model state.
    /// `score` is for `a`: 1.0 win, 0.0 loss, 0.5 tie.
    pub fn answer(&mut self, a: usize, b: usize, score: f64) -> Result<()> {
        if a >= self.items.len() || b >= self.items.len() || a == b {
            return Err(miette::miette!("Rank answer contains invalid item indexes"));
        }
        if !matches!(score, 0.0 | 0.5 | 1.0) {
            return Err(miette::miette!("Rank answer contains an invalid score"));
        }
        let snap = |i: &Item| (i.rating, i.deviation, i.matches);
        self.history.push(Answer {
            a,
            b,
            score,
            prev_a: snap(&self.items[a]),
            prev_b: snap(&self.items[b]),
            prev_pending_a: self.pending[a],
            prev_pending_b: self.pending[b],
        });
        self.pending[a] = self.pending[a].saturating_sub(1);
        self.pending[b] = self.pending[b].saturating_sub(1);
        self.refit();
        Ok(())
    }

    /// Remove the most recent raw answer and rebuild derived state.
    pub fn undo(&mut self) -> Option<Answer> {
        let ans = self.history.pop()?;
        self.pending[ans.a] = ans.prev_pending_a;
        self.pending[ans.b] = ans.prev_pending_b;
        self.refit();
        Some(ans)
    }

    /// Require two more confirmations for items in a contradictory audit.
    pub fn reopen(&mut self, a: usize, b: usize) {
        for i in [a, b] {
            self.pending[i] = self.pending[i].max(2);
        }
    }

    /// Confidence-settled flags for every item.
    pub fn settled_flags(&self, cfg: &RankConfig) -> Vec<bool> {
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                self.pending[index] == 0
                    && (item.matches >= cfg.max_matches
                        || (item.matches >= cfg.min_matches && item.deviation <= cfg.max_deviation))
            })
            .collect()
    }

    /// Count of confidence-settled items.
    pub fn settled_count(&self, cfg: &RankConfig) -> usize {
        self.settled_flags(cfg).into_iter().filter(|&x| x).count()
    }

    /// Answer estimate: items already past `minMatches` that still are not
    /// settled almost always grind to the `maxMatches` cap, so count the full
    /// distance to the cap for them; fresh items get their `minMatches`
    /// shortfall. Each answer advances two items.
    pub fn steps_left(&self, cfg: &RankConfig) -> u64 {
        let settled = self.settled_flags(cfg);
        let needed: u64 = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                if settled[index] {
                    return 0;
                }
                let target = if item.matches >= cfg.min_matches {
                    cfg.max_matches
                } else {
                    cfg.min_matches
                };
                u64::from(
                    target
                        .saturating_sub(item.matches)
                        .max(u32::from(self.pending[index]))
                        .max(1),
                )
            })
            .sum();
        needed.div_ceil(2)
    }

    /// Posterior standard deviation of the rating difference `a - b`.
    pub fn difference_deviation(&self, a: usize, b: usize) -> f64 {
        let n = self.items.len();
        if self.covariance.len() != n * n {
            return (self.items[a].deviation.powi(2) + self.items[b].deviation.powi(2)).sqrt();
        }
        (self.covariance[a * n + a] + self.covariance[b * n + b] - 2.0 * self.covariance[a * n + b])
            .max(0.0)
            .sqrt()
    }

    /// Assign fitted items to confidence-aware tiers, best tier first.
    pub fn confidence_tiers(&self) -> Vec<usize> {
        let mut order = (0..self.items.len()).collect::<Vec<_>>();
        order.sort_by(|&a, &b| {
            self.items[b]
                .rating
                .total_cmp(&self.items[a].rating)
                .then_with(|| a.cmp(&b))
        });
        let mut groups = vec![0; self.items.len()];
        let Some(&first) = order.first() else {
            return groups;
        };
        let (mut tier, mut anchor) = (0, first);
        for &candidate in order.iter().skip(1) {
            let gap = self.items[anchor].rating - self.items[candidate].rating;
            if gap > CONFIDENCE_Z * self.difference_deviation(anchor, candidate) {
                tier += 1;
                anchor = candidate;
            }
            groups[candidate] = tier;
        }
        groups
    }

    /// Number of confidence-aware tiers in the current fit.
    pub fn confidence_tier_count(&self) -> usize {
        self.confidence_tiers()
            .into_iter()
            .max()
            .map_or(0, |tier| tier + 1)
    }

    /// Refit Bradley–Terry ratings, marginal uncertainty, and match counts.
    pub fn refit(&mut self) {
        if self.history.is_empty() {
            self.items = fresh_items();
            self.covariance = prior_covariance(self.items.len());
            self.version = SESSION_VERSION;
            return;
        }
        let initial = self
            .items
            .iter()
            .map(|item| item.rating)
            .collect::<Vec<_>>();
        let fit = fit_bradley_terry(&self.history, self.items.len(), &initial);
        let mut matches = vec![0u32; self.items.len()];
        for answer in &self.history {
            matches[answer.a] += 1;
            matches[answer.b] += 1;
        }
        for (index, item) in self.items.iter_mut().enumerate() {
            item.rating = fit.ratings[index];
            item.deviation = fit.deviations[index];
            item.matches = matches[index];
        }
        self.covariance = fit.covariance;
        self.version = SESSION_VERSION;
    }

    fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read session file: {}", path.display()))?;
        let mut state: Self = serde_json::from_str(&json)
            .into_diagnostic()
            .wrap_err("Failed to parse session file")?;
        if state.pending.is_empty() {
            if state.version >= SESSION_VERSION {
                return Err(miette::miette!(
                    "Rank session is missing v2 verification state"
                ));
            }
            state.pending = vec![0; state.items.len()];
        }
        state.validate()?;
        state.refit();
        Ok(state)
    }

    fn validate(&self) -> Result<()> {
        if self.version > SESSION_VERSION {
            return Err(miette::miette!(
                "Session version {} is newer than supported version {SESSION_VERSION}",
                self.version
            ));
        }
        let expected = fresh_items();
        if self.items.len() != expected.len() || self.pending.len() != expected.len() {
            return Err(miette::miette!("Rank session has invalid item count"));
        }
        if self.items.iter().zip(&expected).any(|(item, expected)| {
            item.from != expected.from
                || item.to != expected.to
                || !item.rating.is_finite()
                || !item.deviation.is_finite()
                || item.deviation <= 0.0
        }) {
            return Err(miette::miette!("Rank session item order is incompatible"));
        }
        if self.history.iter().any(|answer| {
            answer.a >= self.items.len()
                || answer.b >= self.items.len()
                || answer.a == answer.b
                || !matches!(answer.score, 0.0 | 0.5 | 1.0)
        }) {
            return Err(miette::miette!("Rank session contains an invalid answer"));
        }
        Ok(())
    }
}

fn legacy_version() -> u32 {
    1
}

fn pair_count() -> usize {
    (HAND_SLOTS as usize) * (HAND_SLOTS as usize - 1)
}

fn fresh_items() -> Vec<Item> {
    (0..HAND_SLOTS)
        .flat_map(|from| (0..HAND_SLOTS).map(move |to| (from, to)))
        .filter(|(from, to)| from != to)
        .map(|(from, to)| Item {
            from,
            to,
            rating: START_RATING,
            deviation: START_DEV,
            matches: 0,
        })
        .collect()
}

fn prior_covariance(n: usize) -> Vec<f64> {
    let mut covariance = vec![0.0; n * n];
    for i in 0..n {
        covariance[i * n + i] = START_DEV * START_DEV;
    }
    covariance
}

fn backup_path(path: &Path) -> PathBuf {
    appended_path(path, ".bak")
}

fn appended_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    value.into()
}

impl Default for RankState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_210_ordered_pairs() {
        let state = RankState::new();
        assert_eq!(state.items.len(), 210);
        assert!(state.items.iter().all(|i| i.from != i.to));
    }

    #[test]
    fn winner_gains_loser_drops_and_deviation_shrinks() {
        let mut state = RankState::new();
        state.answer(0, 1, 1.0).unwrap();
        assert!(state.items[0].rating > START_RATING);
        assert!(state.items[1].rating < START_RATING);
        assert!(state.items[0].deviation < START_DEV);
        assert_eq!(state.items[0].matches, 1);
    }

    #[test]
    fn tie_keeps_equal_ratings_equal() {
        let mut state = RankState::new();
        state.answer(0, 1, 0.5).unwrap();
        assert_eq!(state.items[0].rating, state.items[1].rating);
    }

    #[test]
    fn invalid_answer_is_rejected_without_mutation() {
        let mut state = RankState::new();
        let before = state.clone();
        assert!(state.answer(0, 0, 1.0).is_err());
        assert!(state.answer(0, 1, f64::NAN).is_err());
        assert_eq!(state, before);
    }

    #[test]
    fn undo_restores_previous_state() {
        let mut state = RankState::new();
        let before = state.clone();
        state.answer(3, 7, 1.0).unwrap();
        let undone = state.undo().unwrap();
        assert_eq!((undone.a, undone.b, undone.score), (3, 7, 1.0));
        assert_eq!(state, before);
        assert!(state.undo().is_none());
    }

    #[test]
    fn session_roundtrip() {
        let mut state = RankState::new();
        state.answer(0, 1, 1.0).unwrap();
        let dir = std::env::temp_dir().join("keyvolve-rank-roundtrip-test");
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("session.json");
        state.save(&path).unwrap();
        let loaded = RankState::load_or_new(&path).unwrap();
        assert_eq!(state.history, loaded.history);
        assert_eq!(state.pending, loaded.pending);
        for (a, b) in state.items.iter().zip(loaded.items) {
            assert!((a.rating - b.rating).abs() < 1e-8);
            assert!((a.deviation - b.deviation).abs() < 1e-8);
            assert_eq!(a.matches, b.matches);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_session_migrates_without_losing_answers() {
        let mut state = RankState::new();
        state.answer(0, 1, 1.0).unwrap();
        let mut json = serde_json::to_value(&state).unwrap();
        let object = json.as_object_mut().unwrap();
        object.remove("version");
        object.remove("pending");
        for answer in object["history"].as_array_mut().unwrap() {
            let answer = answer.as_object_mut().unwrap();
            answer.remove("prev_pending_a");
            answer.remove("prev_pending_b");
        }
        let dir = std::env::temp_dir().join("keyvolve-rank-migration-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();

        let loaded = RankState::load_or_new(&path).unwrap();
        assert_eq!(loaded.version, SESSION_VERSION);
        assert_eq!(loaded.history.len(), 1);
        assert!(loaded.items[0].rating > loaded.items[1].rating);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn v2_session_missing_pending_state_is_rejected() {
        let state = RankState::new();
        let mut json = serde_json::to_value(&state).unwrap();
        json.as_object_mut().unwrap().remove("pending");
        let dir = std::env::temp_dir().join("keyvolve-rank-invalid-v2-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();

        assert!(RankState::load_or_new(&path).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_primary_recovers_rolling_backup() {
        let dir = std::env::temp_dir().join("keyvolve-rank-recovery-test");
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("session.json");
        let mut state = RankState::new();
        state.save(&path).unwrap();
        state.answer(0, 1, 1.0).unwrap();
        state.save(&path).unwrap();
        std::fs::write(&path, "not json").unwrap();

        let recovered = RankState::load_or_new(&path).unwrap();
        assert!(recovered.history.is_empty());
        assert!(RankState::load(&path).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_primary_recovers_synced_temporary_file() {
        let dir = std::env::temp_dir().join("keyvolve-rank-temp-recovery-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        let temporary = appended_path(&path, ".tmp");
        let mut state = RankState::new();
        state.answer(0, 1, 1.0).unwrap();
        std::fs::write(&temporary, serde_json::to_vec(&state).unwrap()).unwrap();

        let recovered = RankState::load_or_new(&path).unwrap();
        assert_eq!(recovered.history.len(), 1);
        assert!(path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_primary_with_invalid_recovery_refuses_to_start_over() {
        let dir = std::env::temp_dir().join("keyvolve-rank-invalid-recovery-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        std::fs::write(appended_path(&path, ".tmp"), "partial json").unwrap();

        assert!(RankState::load_or_new(&path).is_err());
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn label_uses_qwerty_reference() {
        let item = Item {
            from: 8,
            to: 3,
            rating: 0.0,
            deviation: 0.0,
            matches: 0,
        };
        assert_eq!(item.label(), "FR");
        assert_eq!(item.label_right(), "JU");
    }

    #[test]
    fn reopen_requires_more_confirmations() {
        let mut state = RankState::new();
        state.items[0].matches = 100;
        state.items[1].matches = 100;
        state.reopen(0, 1);
        let cfg = RankConfig::default();
        let settled = state.settled_flags(&cfg);
        assert!(!settled[0]);
        assert!(!settled[1]);
    }

    #[test]
    fn confidence_tiers_merge_ties_and_split_clear_items() {
        let cfg = RankConfig::default();
        let mut state = RankState::new();
        for (index, item) in state.items.iter_mut().enumerate() {
            item.rating = 30_000.0 - index as f64 * 100.0;
            item.deviation = 1.0;
            item.matches = cfg.min_matches;
        }
        state.covariance = prior_covariance(state.items.len());
        for i in 0..state.items.len() {
            state.covariance[i * state.items.len() + i] = 1.0;
        }
        assert_eq!(state.confidence_tier_count(), state.items.len());
        assert_eq!(state.settled_count(&cfg), state.items.len());

        for item in &mut state.items {
            item.rating = START_RATING;
        }
        assert_eq!(state.confidence_tier_count(), 1);
        assert_eq!(state.settled_count(&cfg), state.items.len());
    }

    #[test]
    fn confidence_tiers_allow_uneven_populations() {
        let mut state = RankState::new();
        for item in &mut state.items {
            item.rating = 0.0;
            item.deviation = 1.0;
        }
        state.items[0].rating = 100.0;
        state.items[1].rating = 99.0;
        state.items[2].rating = 97.0;
        state.items[3].rating = 96.0;
        state.covariance = prior_covariance(state.items.len());
        for i in 0..state.items.len() {
            state.covariance[i * state.items.len() + i] = 1.0;
        }

        let tiers = state.confidence_tiers();
        assert_eq!(&tiers[..4], &[0, 0, 1, 1]);
        assert!(tiers[4..].iter().all(|&tier| tier == 2));
    }

    #[test]
    fn steps_left_shrinks_and_reaches_zero() {
        let mut state = RankState::new();
        let cfg = RankConfig {
            min_matches: 6,
            max_deviation: 120.0,
            ..Default::default()
        };
        let before = state.steps_left(&cfg);
        assert!(before >= 210 * 6 / 2); // fresh: at least min_matches bound
        state.answer(0, 1, 1.0).unwrap();
        assert!(state.steps_left(&cfg) < before);
        for item in &mut state.items {
            item.matches = 100;
            item.deviation = 50.0;
        }
        assert_eq!(state.steps_left(&cfg), 0);
    }

    /// Read-only diagnostic over the live session: why is each item unsettled?
    /// Run with: cargo test -q scan_live_session_settled -- --ignored --nocapture
    #[test]
    #[ignore]
    fn scan_live_session_settled_breakdown() {
        let path = Path::new("data/rank-session.json");
        if !path.exists() {
            return;
        }
        let state = RankState::load(path).unwrap();
        // Mirror keyvolve.yaml rank settings (no yaml parser in this crate's deps).
        let cfg = RankConfig {
            min_matches: 10,
            max_matches: 20,
            max_deviation: 130.0,
            ..Default::default()
        };
        let settled = state.settled_flags(&cfg);
        let (mut capped, mut ok, mut pending_n, mut low_matches, mut high_dev) = (0, 0, 0, 0, 0);
        for (i, item) in state.items.iter().enumerate() {
            if settled[i] {
                if item.matches >= cfg.max_matches {
                    capped += 1;
                } else {
                    ok += 1;
                }
                continue;
            }
            if state.pending[i] > 0 {
                pending_n += 1;
            } else if item.matches < cfg.min_matches {
                low_matches += 1;
            } else if item.deviation > cfg.max_deviation {
                high_dev += 1;
            }
        }
        println!(
            "settled: {capped} capped + {ok} confident; unsettled: {pending_n} pending, \
             {low_matches} low matches, {high_dev} high deviation"
        );
    }
}
