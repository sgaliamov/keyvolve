use miette::{Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
/// Deviation floor — confidence never becomes absolute.
pub const MIN_DEV: f64 = 40.0;
/// Per-match deviation decay factor.
const DEV_DECAY: f64 = 0.93;
/// Base K-factor at full deviation.
const K_BASE: f64 = 64.0;

/// One ordered left-hand bigram pair with its Glicko-lite rating.
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

    /// Settled = enough matches and low uncertainty.
    pub fn settled(&self, min_matches: u32, max_deviation: f64) -> bool {
        self.matches >= min_matches && self.deviation <= max_deviation
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
}

/// Full ranking session state, persisted after every answer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankState {
    pub items: Vec<Item>,
    pub history: Vec<Answer>,
}

impl RankState {
    /// Fresh state with all 210 ordered pairs (from ≠ to) at default rating.
    pub fn new() -> Self {
        let items = (0..HAND_SLOTS)
            .flat_map(|from| (0..HAND_SLOTS).map(move |to| (from, to)))
            .filter(|(from, to)| from != to)
            .map(|(from, to)| Item {
                from,
                to,
                rating: START_RATING,
                deviation: START_DEV,
                matches: 0,
            })
            .collect();
        Self {
            items,
            history: vec![],
        }
    }

    /// Load session from file, or start fresh when absent.
    pub fn load_or_new(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let json = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read session file: {}", path.display()))?;
        serde_json::from_str(&json)
            .into_diagnostic()
            .wrap_err("Failed to parse session file")
    }

    /// Persist session to file (parent dirs created).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).into_diagnostic()?;
        }
        let json = serde_json::to_string(self).into_diagnostic()?;
        std::fs::write(path, json)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write session file: {}", path.display()))
    }

    /// Apply an answer: Glicko-lite update of both items, record history.
    /// `score` is for `a`: 1.0 win, 0.0 loss, 0.5 tie.
    pub fn answer(&mut self, a: usize, b: usize, score: f64) {
        let snap = |i: &Item| (i.rating, i.deviation, i.matches);
        self.history.push(Answer {
            a,
            b,
            score,
            prev_a: snap(&self.items[a]),
            prev_b: snap(&self.items[b]),
        });

        let (ra, rb) = (self.items[a].rating, self.items[b].rating);
        let expected = 1.0 / (1.0 + 10f64.powf((rb - ra) / 400.0));
        let update = |item: &mut Item, delta: f64| {
            item.rating += K_BASE * (item.deviation / START_DEV) * delta;
            item.deviation = (item.deviation * DEV_DECAY).max(MIN_DEV);
            item.matches += 1;
        };
        update(&mut self.items[a], score - expected);
        update(&mut self.items[b], expected - score);
    }

    /// Revert the most recent answer. Returns false when history is empty.
    pub fn undo(&mut self) -> bool {
        let Some(ans) = self.history.pop() else {
            return false;
        };
        let restore = |item: &mut Item, (rating, deviation, matches): (f64, f64, u32)| {
            item.rating = rating;
            item.deviation = deviation;
            item.matches = matches;
        };
        restore(&mut self.items[ans.a], ans.prev_a);
        restore(&mut self.items[ans.b], ans.prev_b);
        true
    }

    /// Re-open a pair after a contradictory audit answer: bump uncertainty.
    pub fn reopen(&mut self, a: usize, b: usize) {
        for i in [a, b] {
            self.items[i].deviation = self.items[i].deviation.max(START_DEV * 0.6);
        }
    }

    /// Count of settled items.
    pub fn settled_count(&self, min_matches: u32, max_deviation: f64) -> usize {
        self.items
            .iter()
            .filter(|i| i.settled(min_matches, max_deviation))
            .count()
    }
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
        state.answer(0, 1, 1.0);
        assert!(state.items[0].rating > START_RATING);
        assert!(state.items[1].rating < START_RATING);
        assert!(state.items[0].deviation < START_DEV);
        assert_eq!(state.items[0].matches, 1);
    }

    #[test]
    fn tie_keeps_equal_ratings_equal() {
        let mut state = RankState::new();
        state.answer(0, 1, 0.5);
        assert_eq!(state.items[0].rating, state.items[1].rating);
    }

    #[test]
    fn undo_restores_previous_state() {
        let mut state = RankState::new();
        let before = state.clone();
        state.answer(3, 7, 1.0);
        assert!(state.undo());
        assert_eq!(state, before);
        assert!(!state.undo());
    }

    #[test]
    fn session_roundtrip() {
        let mut state = RankState::new();
        state.answer(0, 1, 1.0);
        let dir = std::env::temp_dir().join("keyvolve-rank-test");
        let path = dir.join("session.json");
        state.save(&path).unwrap();
        let loaded = RankState::load_or_new(&path).unwrap();
        assert_eq!(state, loaded);
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
    fn reopen_bumps_deviation() {
        let mut state = RankState::new();
        for _ in 0..50 {
            state.answer(0, 1, 1.0);
        }
        state.reopen(0, 1);
        assert!(state.items[0].deviation >= START_DEV * 0.6);
    }
}
