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

    /// Hand switches between consecutive presses.
    pub hand_switches: u64,

    /// Weighted row-switch cost on the left hand: adjacent-row move = 1, jump-over-row = 2.
    pub left_row_switch_cost: u64,

    /// Weighted row-switch cost on the right hand.
    pub right_row_switch_cost: u64,

    /// Effort accumulated on the left hand.
    pub left_effort: f64,

    /// Effort accumulated on the right hand.
    pub right_effort: f64,
}

impl ScoreResult {
    // Ratios: normalized per-hand shares (0-1 range).

    /// Left share of same-hand counts. Range: [0.0, 1.0], where 0 = unused left hand,
    /// 1 = all bigrams on left.
    pub fn left_count_ratio(&self) -> f64 {
        crate::math::ratio(
            self.left_count as f64,
            (self.left_count + self.right_count) as f64,
        )
    }

    /// Right share of same-hand counts. Range: [0.0, 1.0], where 0 = unused right hand,
    /// 1 = all bigrams on right.
    pub fn right_count_ratio(&self) -> f64 {
        crate::math::ratio(
            self.right_count as f64,
            (self.left_count + self.right_count) as f64,
        )
    }

    /// Left share of same-hand effort. Range: [0.0, 1.0], proportional to key travel distance.
    pub fn left_effort_ratio(&self) -> f64 {
        crate::math::ratio(self.left_effort, self.left_effort + self.right_effort)
    }

    /// Right share of same-hand effort. Range: [0.0, 1.0], proportional to key travel distance.
    pub fn right_effort_ratio(&self) -> f64 {
        crate::math::ratio(self.right_effort, self.left_effort + self.right_effort)
    }

    /// Share of hand switches among all bigram transitions. Range: [0.0, 1.0],
    /// where 0 = no alternation, 1 = every transition switches hands.
    pub fn hand_switch_ratio(&self) -> f64 {
        crate::math::ratio(
            self.hand_switches as f64,
            (self.left_count + self.right_count) as f64,
        )
    }

    // Imbalances: percent deviation from parity (0% = balanced).

    /// Hand imbalance as a percent: how far the left/right same-hand count ratio
    /// strays from parity. Range: [0.0, ∞). 0% = balanced, >0% = left-skewed,
    /// <0% = right-skewed.
    pub fn hands_imbalance(&self) -> f64 {
        crate::math::signed_imbalance_percent(self.left_count as f64, self.right_count as f64)
    }

    /// Row-switch cost imbalance as a percent: how far the left/right row-switch
    /// cost ratio strays from parity. Range: [0.0, ∞). 0% = balanced, asymmetric by sign.
    pub fn row_switch_imbalance(&self) -> f64 {
        crate::math::signed_imbalance_percent(
            self.left_row_switch_cost as f64,
            self.right_row_switch_cost as f64,
        )
    }

    /// Same-hand bigram imbalance as a percent: how far the left/right roll count
    /// ratio strays from parity. Range: [0.0, ∞). 0% = balanced, asymmetric by sign.
    pub fn roll_imbalance(&self) -> f64 {
        crate::math::signed_imbalance_percent(self.left_rolls as f64, self.right_rolls as f64)
    }

    /// Effort imbalance as a percent: how far the left/right effort ratio
    /// strays from parity. Range: [0.0, ∞). 0% = balanced, asymmetric by sign.
    pub fn efforts_imbalance(&self) -> f64 {
        crate::math::signed_imbalance_percent(self.left_effort, self.right_effort)
    }

    // Streaks: average consecutive-press run length per hand.

    /// Average left-hand streak: consecutive presses before leaving the hand.
    /// A run of length k yields k presses and k−1 rolls, so streak = presses / runs.
    /// Range: [0.0, ∞). 0 = unused, 1.0 = constant alternation, >1 = sustained runs.
    pub fn left_streak(&self) -> f64 {
        crate::math::streak(self.left_count, self.left_rolls)
    }

    /// Average right-hand streak: consecutive presses before leaving the hand.
    /// Range: [0.0, ∞). 0 = unused, 1.0 = constant alternation, >1 = sustained runs.
    pub fn right_streak(&self) -> f64 {
        crate::math::streak(self.right_count, self.right_rolls)
    }

    /// Streak imbalance as a percent: how far the left/right streak length ratio
    /// strays from parity. Range: (-∞, ∞). 0% = balanced, >0% = left-lean (longer left runs),
    /// <0% = right-lean (longer right runs).
    pub fn streak_imbalance(&self) -> f64 {
        crate::math::signed_imbalance_percent(self.left_streak(), self.right_streak())
    }

    /// Overall average streak: all presses over all runs, both hands.
    /// Range: [0.0, ∞). 1.0 = every press switches hands, >1 = sustained multi-press runs.
    pub fn mean_streak(&self) -> f64 {
        crate::math::streak(
            self.left_count + self.right_count,
            self.left_rolls + self.right_rolls,
        )
    }

    // Aggregates: totals combining both hands.

    /// Total vertical row-switch distance, both hands (adjacent row = 1, jump = 2).
    pub fn row_switch_distance(&self) -> u64 {
        self.left_row_switch_cost + self.right_row_switch_cost
    }

    /// Share of same-hand moves that cross rows, weighted by jump severity.
    /// Average vertical distance per same-hand press. Numerator: total row-switch distance
    /// (adjacent row = 1, jump = 2). Denominator: all same-hand presses. Range: [0.0, ∞).
    /// 0 = every same-hand move stays in its row. Example: 8 distance / 16 presses = 0.5 avg distance per press.
    pub fn row_switch_ratio(&self) -> f64 {
        crate::math::ratio(
            self.row_switch_distance() as f64,
            (self.left_count + self.right_count) as f64,
        )
    }

    // Transformations: generate derived scores.

    /// Hand-swapped score: left/right counts and efforts trade places. Symmetric
    /// fields (fitness, effort, switches) stay — a layout and its mirror score
    /// identically apart from which hand owns each share.
    pub fn mirror(&self) -> Self {
        ScoreResult {
            left_count: self.right_count,
            right_count: self.left_count,
            left_rolls: self.right_rolls,
            right_rolls: self.left_rolls,
            left_row_switch_cost: self.right_row_switch_cost,
            right_row_switch_cost: self.left_row_switch_cost,
            left_effort: self.right_effort,
            right_effort: self.left_effort,
            ..self.clone()
        }
    }

    // CSV serialization: (de)serialize rows.

    /// Format signed imbalance with directional symbol: negative = left (←), positive = right (→).
    fn format_imbalance(value: f64) -> String {
        let symbol = if value < 0.0 {
            "→"
        } else if value > 0.0 {
            "←"
        } else {
            "·"
        };
        format!("{}{:.2}%", symbol, value.abs())
    }

    /// Serialize as a CSV row (no header).
    pub fn to_csv(&self) -> String {
        format!(
            "{:.6},{:.2}%,{},{:.2}%,{},{},{},{:.2},{},{:.2}%,{:.2}%,{:.2}%,{:.2}%,{:.2},{:.2},{:.2},{},{},{},{:.2},{:.2},{},{},{:.2},{:.2}",
            self.fitness,
            self.row_switch_ratio() * 100.0,
            Self::format_imbalance(self.row_switch_imbalance()),
            self.hand_switch_ratio() * 100.0,
            Self::format_imbalance(self.hands_imbalance()),
            Self::format_imbalance(self.efforts_imbalance()),
            Self::format_imbalance(self.roll_imbalance()),
            self.mean_streak(),
            Self::format_imbalance(self.streak_imbalance()),
            self.left_effort_ratio() * 100.0,
            self.right_effort_ratio() * 100.0,
            self.left_count_ratio() * 100.0,
            self.right_count_ratio() * 100.0,
            self.effort,
            self.left_effort,
            self.right_effort,
            self.left_count,
            self.right_count,
            self.hand_switches,
            self.left_row_switch_cost,
            self.right_row_switch_cost,
            self.left_rolls,
            self.right_rolls,
            self.left_streak(),
            self.right_streak(),
        )
    }

    /// CSV header matching [`to_csv`] column order.
    /// Columns: fitness (normalized quality), row_switch_ratio (row jumps %), row_switch_imbalance (hand row asymmetry),
    /// hand_switch_ratio (hand alternation %), hands_imbalance (left/right count %), efforts_imbalance (left/right effort %),
    /// roll_imbalance (left/right roll %), mean_streak (avg consecutive presses per hand), streak_imbalance (left/right streak %),
    /// left_effort_ratio (left % of total), right_effort_ratio (right % of total),
    /// left_count_ratio (left % of bigrams), right_count_ratio (right % of bigrams),
    /// effort (raw total), left_effort/right_effort (per-hand), left_count/right_count (bigrams per hand),
    /// hand_switches (transitions), left_row_switch_cost/right_row_switch_cost (weighted jumps),
    /// left_rolls/right_rolls (same-hand bigrams), left_streak/right_streak (avg run length).
    pub fn csv_header() -> &'static str {
        "fitness,row_switch_ratio,row_switch_imbalance,hand_switch_ratio,hands_imbalance,efforts_imbalance,roll_imbalance,mean_streak,streak_ratio,left_effort_ratio,right_effort_ratio,left_count_ratio,right_count_ratio,effort,left_effort,right_effort,left_count,right_count,hand_switches,left_row_switch_cost,right_row_switch_cost,left_rolls,right_rolls,left_streak,right_streak"
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
            effort: c.get(13)?.parse().ok()?,
            left_effort: c.get(14)?.parse().ok()?,
            right_effort: c.get(15)?.parse().ok()?,
            left_count: c.get(16)?.parse().ok()?,
            right_count: c.get(17)?.parse().ok()?,
            hand_switches: c.get(18)?.parse().ok()?,
            left_row_switch_cost: c.get(19)?.parse().ok()?,
            right_row_switch_cost: c.get(20)?.parse().ok()?,
            left_rolls: c.get(21)?.parse().ok()?,
            right_rolls: c.get(22)?.parse().ok()?,
        })
    }
}

impl std::fmt::Display for ScoreResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "φ {:.4} | ↕ {:+06.2}% | ⇄ {:+06.2}% | ⟳Δ {:+06.2}% | Δ {:+06.2}% | εΔ {:+06.2}% | ↕↔ {:+06.2}% | →Δ {:+06.2}% | → {:.2} | ε {:.2}",
            self.fitness,
            self.row_switch_ratio() * 100.0,
            self.hand_switch_ratio() * 100.0,
            self.roll_imbalance(),
            self.hands_imbalance(),
            self.efforts_imbalance(),
            self.row_switch_imbalance(),
            self.streak_imbalance(),
            self.mean_streak(),
            self.effort,
        )
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
            hand_switches: self.hand_switches + other.hand_switches,
            left_row_switch_cost: self.left_row_switch_cost + other.left_row_switch_cost,
            right_row_switch_cost: self.right_row_switch_cost + other.right_row_switch_cost,
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
            hand_switches: self.hand_switches * n,
            left_row_switch_cost: self.left_row_switch_cost * n,
            right_row_switch_cost: self.right_row_switch_cost * n,
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
        assert_eq!(ScoreResult::default().right_streak(), 0.0);
    }

    #[test]
    fn roll_imbalance_measures_left_right_roll_skew() {
        let balanced = ScoreResult {
            left_rolls: 5,
            right_rolls: 5,
            ..Default::default()
        };
        assert_eq!(balanced.roll_imbalance(), 0.0);

        // 6/3 - 1 = 1 → 100% (left-heavy, positive).
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
    fn directional_symbols_show_which_hand_is_heavier() {
        // Left-heavy: left_rolls > right_rolls → negative value → ← symbol.
        let left_heavy = ScoreResult {
            left_rolls: 9,
            right_rolls: 3,
            ..Default::default()
        };
        // 9/3 - 1 = 2 → 200% (left-heavy, positive because left > right).
        assert_eq!(left_heavy.roll_imbalance(), 200.0);

        // Right-heavy: left_rolls < right_rolls → negative value → → symbol.
        let right_heavy = ScoreResult {
            left_rolls: 2,
            right_rolls: 8,
            ..Default::default()
        };
        // 2/8 - 1 = -0.75 → -75% (right-heavy, negative because left < right).
        assert!((right_heavy.roll_imbalance() - (-75.0)).abs() < 1e-9);

        // Balanced: left_rolls == right_rolls → 0.0 → · symbol.
        let balanced = ScoreResult {
            left_rolls: 5,
            right_rolls: 5,
            ..Default::default()
        };
        assert_eq!(balanced.roll_imbalance(), 0.0);
    }

    #[test]
    fn row_switch_ratio_is_cost_per_same_hand_move() {
        // Row cost only accrues on same-hand bigrams, denominator is all same-hand counts.
        let sample = |left_rolls, right_rolls, left_cost, right_cost| ScoreResult {
            left_count: 8,
            right_count: 8,
            left_rolls,
            right_rolls,
            left_row_switch_cost: left_cost,
            right_row_switch_cost: right_cost,
            ..Default::default()
        };

        // Every same-hand move stays in its row: 0 cost / 16 counts = 0.0.
        assert_eq!(sample(3, 3, 0, 0).row_switch_ratio(), 0.0);
        // Every same-hand move steps one row: 6 cost / 16 counts = 0.375.
        assert_eq!(sample(3, 3, 3, 3).row_switch_ratio(), 0.375);
        // Upper bound: every same-hand move jumps over a row, worth 2 each: 12 cost / 16 counts = 0.75.
        assert_eq!(sample(3, 3, 6, 6).row_switch_ratio(), 0.75);
        // Fully alternating layout has no same-hand moves to charge — 0.0, not NaN.
        assert_eq!(sample(0, 0, 0, 0).row_switch_ratio(), 0.0);
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
            hand_switches: 2,
            left_row_switch_cost: 1,
            right_row_switch_cost: 3,
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
            assert_eq!(parsed.hand_switches, 2);
            assert_eq!(parsed.left_row_switch_cost, 1);
            assert_eq!(parsed.right_row_switch_cost, 3);
        };

        // Old headerless rows (fitness right after keys) and new rows (name column).
        check(&format!("k1, k2, k3, k4, k5, k6, {}", s.to_csv()));
        check(&format!("k1, k2, k3, k4, k5, k6, homerow, {}", s.to_csv()));
    }
}
