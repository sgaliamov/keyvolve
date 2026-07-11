/// Full breakdown of a scoring pass over a word or corpus.
#[derive(Debug, Clone, Default)]
pub struct ScoreResult {
    /// Total raw effort before corpus-level penalties.
    pub effort: f64,

    /// Total effort after corpus-level penalties.
    pub fitness: f64,

    /// Number of consecutive same-hand bigrams on the left.
    pub left_count: u64,

    /// Number of consecutive same-hand bigrams on the right.
    pub right_count: u64,

    /// Same-hand bigrams fully on the left (both keys left).
    pub left_rolls: u64,

    /// Same-hand bigrams fully on the right (both keys right).
    pub right_rolls: u64,

    /// Number of hand switches.
    pub bigram_switches: u64,

    /// Weighted row-switch cost: adjacent-row move = 1, jump-over-row = 2.
    pub row_switch_cost: u64,

    /// Effort accumulated on the left hand.
    pub left_effort: f64,

    /// Effort accumulated on the right hand.
    pub right_effort: f64,
}

impl ScoreResult {
    /// Left share of same-hand counts.
    pub fn left_count_ratio(&self) -> f64 {
        ratio(
            self.left_count as f64,
            (self.left_count + self.right_count) as f64,
        )
    }

    /// Right share of same-hand counts.
    pub fn right_count_ratio(&self) -> f64 {
        ratio(
            self.right_count as f64,
            (self.left_count + self.right_count) as f64,
        )
    }

    /// Hand imbalance as a percent: how far the left/right same-hand count ratio
    /// strays from parity. 0% = balanced. Asymmetric by direction.
    pub fn hands_imbalance(&self) -> f64 {
        if self.right_count == 0 {
            0.0
        } else {
            (self.left_count as f64 / self.right_count as f64 - 1.0).abs() * 100.0
        }
    }

    /// Same-hand bigram imbalance as a percent: how far the left/right roll count
    /// ratio strays from parity. 0% = balanced. Asymmetric by direction.
    pub fn roll_imbalance(&self) -> f64 {
        if self.right_rolls == 0 {
            0.0
        } else {
            (self.left_rolls as f64 / self.right_rolls as f64 - 1.0).abs() * 100.0
        }
    }

    /// Left share of same-hand effort.
    pub fn left_effort_ratio(&self) -> f64 {
        ratio(self.left_effort, self.left_effort + self.right_effort)
    }

    /// Right share of same-hand effort.
    pub fn right_effort_ratio(&self) -> f64 {
        ratio(self.right_effort, self.left_effort + self.right_effort)
    }

    /// Share of hand switches among all bigram transitions.
    pub fn bigram_switch_ratio(&self) -> f64 {
        ratio(
            self.bigram_switches as f64,
            self.left_count as f64 + self.right_count as f64,
        )
    }

    /// Share of same-hand transitions that switch rows, weighted by jump severity.
    pub fn row_switch_ratio(&self) -> f64 {
        ratio(
            self.row_switch_cost as f64,
            self.left_count.saturating_sub(1) as f64 + self.right_count.saturating_sub(1) as f64,
        )
    }

    /// Average left-hand streak: consecutive presses before leaving the hand.
    /// A run of length k yields k presses and k−1 rolls, so streak = presses / runs.
    pub fn left_streak(&self) -> f64 {
        streak(self.left_count, self.left_rolls)
    }

    /// Average right-hand streak: consecutive presses before leaving the hand.
    pub fn right_streak(&self) -> f64 {
        streak(self.right_count, self.right_rolls)
    }

    /// Serialize as a CSV row (no header).
    pub fn to_csv(&self) -> String {
        format!(
            "{:.4},{:.2}%,{:.2}%,{:.2}%,{:.2}%,{:.2}%,{:.2}%,{:.2}%,{:.2}%,{:.2},{:.2},{:.2},{},{},{},{},{},{},{:.2},{:.2}",
            self.fitness,
            self.roll_imbalance(),
            self.hands_imbalance(),
            self.row_switch_ratio() * 100.0,
            self.bigram_switch_ratio() * 100.0,
            self.left_effort_ratio() * 100.0,
            self.right_effort_ratio() * 100.0,
            self.left_count_ratio() * 100.0,
            self.right_count_ratio() * 100.0,
            self.effort,
            self.left_effort,
            self.right_effort,
            self.left_count,
            self.right_count,
            self.bigram_switches,
            self.row_switch_cost,
            self.left_rolls,
            self.right_rolls,
            self.left_streak(),
            self.right_streak(),
        )
    }

    /// CSV header matching [`to_csv`] column order.
    pub fn csv_header() -> &'static str {
        "fitness,roll_imbalance,hands_imbalance,row_switch_ratio,switch_ratio,left_effort_ratio,right_effort_ratio,left_count_ratio,right_count_ratio,effort,left_effort,right_effort,left_count,right_count,bigram_switches,row_switch_cost,left_rolls,right_rolls,left_streak,right_streak"
    }

    /// Hand-swapped score: left/right counts and efforts trade places. Symmetric
    /// fields (fitness, effort, switches) stay — a layout and its mirror score
    /// identically apart from which hand owns each share.
    pub fn mirror(&self) -> Self {
        ScoreResult {
            left_count: self.right_count,
            right_count: self.left_count,
            left_rolls: self.right_rolls,
            right_rolls: self.left_rolls,
            left_effort: self.right_effort,
            right_effort: self.left_effort,
            ..self.clone()
        }
    }

    /// Parse the raw (non-derived) fields from a persisted CSV row, skipping the
    /// six key columns plus the optional `name` column. Derived ratios are
    /// recomputed by [`to_csv`], so they are ignored here. Returns `None` on a
    /// malformed row.
    pub fn from_csv(line: &str) -> Option<Self> {
        let skip = if super::name_field(line).is_some() {
            7
        } else {
            6
        };
        let c: Vec<&str> = line.split(',').skip(skip).map(str::trim).collect();
        Some(ScoreResult {
            fitness: c.first()?.parse().ok()?,
            effort: c.get(9)?.parse().ok()?,
            left_effort: c.get(10)?.parse().ok()?,
            right_effort: c.get(11)?.parse().ok()?,
            left_count: c.get(12)?.parse().ok()?,
            right_count: c.get(13)?.parse().ok()?,
            bigram_switches: c.get(14)?.parse().ok()?,
            row_switch_cost: c.get(15)?.parse().ok()?,
            left_rolls: c.get(16)?.parse().ok()?,
            right_rolls: c.get(17)?.parse().ok()?,
        })
    }
}

impl std::fmt::Display for ScoreResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "φ {:.4} | ⟳Δ {:.2}% | Δ {:.2}% | ↕ {:.2}% | ⇄ {:.2}% | Lε {:.1}% | Rε {:.1}% | L# {:.1}% | R# {:.1}% | ε {:.2} | Lε {:.2} | Rε {:.2} | L# {} | R# {} | ⇄ {} | ↕ {} | L⟳ {} | R⟳ {} | L→ {:.2} | R→ {:.2}",
            self.fitness,
            self.roll_imbalance(),
            self.hands_imbalance(),
            self.row_switch_ratio() * 100.0,
            self.bigram_switch_ratio() * 100.0,
            self.left_effort_ratio() * 100.0,
            self.right_effort_ratio() * 100.0,
            self.left_count_ratio() * 100.0,
            self.right_count_ratio() * 100.0,
            self.effort,
            self.left_effort,
            self.right_effort,
            self.left_count,
            self.right_count,
            self.bigram_switches,
            self.row_switch_cost,
            self.left_rolls,
            self.right_rolls,
            self.left_streak(),
            self.right_streak(),
        )
    }
}

/// Safe ratio helper.
fn ratio(value: f64, total: f64) -> f64 {
    if total == 0.0 { 0.0 } else { value / total }
}

/// Average run length from presses and same-hand transitions. Every press starts
/// a run or continues one; continuations are exactly the rolls, so
/// runs = count − rolls and streak = count / runs. `0.0` for an unused hand.
fn streak(count: u64, rolls: u64) -> f64 {
    match count.saturating_sub(rolls) {
        0 => 0.0,
        runs => count as f64 / runs as f64,
    }
}

impl std::ops::Add for ScoreResult {
    type Output = Self;

    fn add(self, other: ScoreResult) -> Self {
        ScoreResult {
            effort: self.effort + other.effort,
            fitness: self.fitness + other.fitness,
            left_count: self.left_count + other.left_count,
            right_count: self.right_count + other.right_count,
            left_rolls: self.left_rolls + other.left_rolls,
            right_rolls: self.right_rolls + other.right_rolls,
            bigram_switches: self.bigram_switches + other.bigram_switches,
            row_switch_cost: self.row_switch_cost + other.row_switch_cost,
            left_effort: self.left_effort + other.left_effort,
            right_effort: self.right_effort + other.right_effort,
        }
    }
}

/// Scale every field by a corpus frequency; lets one unit score stand in for `n` occurrences.
impl std::ops::Mul<u64> for ScoreResult {
    type Output = Self;

    fn mul(self, n: u64) -> Self {
        let f = n as f64;
        ScoreResult {
            effort: self.effort * f,
            fitness: self.fitness * f,
            left_count: self.left_count * n,
            right_count: self.right_count * n,
            left_rolls: self.left_rolls * n,
            right_rolls: self.right_rolls * n,
            bigram_switches: self.bigram_switches * n,
            row_switch_cost: self.row_switch_cost * n,
            left_effort: self.left_effort * f,
            right_effort: self.right_effort * f,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_swaps_hands() {
        let s = ScoreResult {
            left_count: 3,
            right_count: 5,
            left_rolls: 4,
            right_rolls: 7,
            left_effort: 1.0,
            right_effort: 2.0,
            ..Default::default()
        };
        let m = s.mirror();

        assert_eq!(m.left_count, 5);
        assert_eq!(m.right_count, 3);
        assert_eq!(m.left_rolls, 7);
        assert_eq!(m.right_rolls, 4);
        assert_eq!(m.left_effort, 2.0);
        assert_eq!(m.right_effort, 1.0);
    }

    #[test]
    fn streak_averages_run_lengths() {
        // Corpus "flask" + "jaded": left runs "flas" (4) and "aded" (4) → avg 4.0;
        // right runs "k" and "j" singles → avg 1.0.
        let s = ScoreResult {
            left_count: 8,
            left_rolls: 6,
            right_count: 2,
            right_rolls: 0,
            ..Default::default()
        };
        assert_eq!(s.left_streak(), 4.0);
        assert_eq!(s.right_streak(), 1.0);

        // Unused hand → 0.0, no division blowup.
        assert_eq!(ScoreResult::default().left_streak(), 0.0);
    }

    #[test]
    fn roll_imbalance_measures_left_right_roll_skew() {
        let balanced = ScoreResult {
            left_rolls: 5,
            right_rolls: 5,
            ..Default::default()
        };
        assert_eq!(balanced.roll_imbalance(), 0.0);

        // 6/3 - 1 = 1 → 100%.
        let skewed = ScoreResult {
            left_rolls: 6,
            right_rolls: 3,
            ..Default::default()
        };
        assert!((skewed.roll_imbalance() - 100.0).abs() < 1e-9);

        // Asymmetric guard: no right rolls → 0%.
        let zero_right = ScoreResult {
            left_rolls: 4,
            right_rolls: 0,
            ..Default::default()
        };
        assert_eq!(zero_right.roll_imbalance(), 0.0);
    }

    #[test]
    fn from_csv_roundtrips_raw_fields() {
        let s = ScoreResult {
            effort: 10.0,
            fitness: 5.0,
            left_count: 3,
            right_count: 5,
            left_rolls: 7,
            right_rolls: 9,
            bigram_switches: 2,
            row_switch_cost: 1,
            left_effort: 4.0,
            right_effort: 6.0,
        };
        let check = |line: &str| {
            let parsed = ScoreResult::from_csv(line).unwrap();
            assert_eq!(parsed.fitness, 5.0);
            assert_eq!(parsed.effort, 10.0);
            assert_eq!(parsed.left_count, 3);
            assert_eq!(parsed.right_count, 5);
            assert_eq!(parsed.left_rolls, 7);
            assert_eq!(parsed.right_rolls, 9);
            assert_eq!(parsed.left_effort, 4.0);
            assert_eq!(parsed.right_effort, 6.0);
            assert_eq!(parsed.bigram_switches, 2);
            assert_eq!(parsed.row_switch_cost, 1);
        };

        // Old headerless rows (fitness right after keys) and new rows (name column).
        check(&format!("k1, k2, k3, k4, k5, k6, {}", s.to_csv()));
        check(&format!("k1, k2, k3, k4, k5, k6, homerow, {}", s.to_csv()));
    }
}
