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

    /// Effort by left-hand column: inner → outer, 5 slots.
    pub left_column_effort: [f64; 5],

    /// Effort by right-hand column: inner → outer, 5 slots.
    pub right_column_effort: [f64; 5],

    /// Effort by left-hand row: top, home, bottom.
    pub left_row_effort: [f64; 3],

    /// Effort by right-hand row: top, home, bottom.
    pub right_row_effort: [f64; 3],
}

#[inline]
fn hand_index(slot: u8) -> usize {
    (slot / 15) as usize
}

#[inline]
fn column_index(slot: u8) -> usize {
    (slot % 5) as usize
}

#[inline]
fn row_index(slot: u8) -> usize {
    crate::models::slot_row(slot) as usize
}

impl ScoreResult {
    /// Build one press worth of score from a physical slot and its effort.
    pub(crate) fn press(slot: u8, effort: f64) -> Self {
        let hand = hand_index(slot);
        let column = column_index(slot);
        let row = row_index(slot);

        let mut score = ScoreResult {
            effort,
            left_count: (hand == 0) as u64,
            right_count: (hand == 1) as u64,
            left_effort: if hand == 0 { effort } else { 0.0 },
            right_effort: if hand == 1 { effort } else { 0.0 },
            ..Default::default()
        };

        if hand == 0 {
            score.left_column_effort[column] = effort;
            score.left_row_effort[row] = effort;
        } else {
            score.right_column_effort[column] = effort;
            score.right_row_effort[row] = effort;
        }

        score
    }

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

    /// Left hand column effort shares, inner → outer.
    pub fn left_column_effort_ratios(&self) -> [f64; 5] {
        self.left_column_effort
            .map(|v| crate::math::ratio(v, self.effort))
    }

    /// Right hand column effort shares, inner → outer.
    pub fn right_column_effort_ratios(&self) -> [f64; 5] {
        self.right_column_effort
            .map(|v| crate::math::ratio(v, self.effort))
    }

    /// Row effort shares for both hands plus total, in top → bottom order.
    pub fn row_effort_ratios(&self) -> [(f64, f64, f64); 3] {
        core::array::from_fn(|row| {
            let left = self.left_row_effort[row];
            let right = self.right_row_effort[row];
            (
                crate::math::ratio(left, self.effort),
                crate::math::ratio(right, self.effort),
                crate::math::ratio(left + right, self.effort),
            )
        })
    }

    /// Left/right balance for top, home, bottom rows.
    pub fn row_balances(&self) -> [f64; 3] {
        core::array::from_fn(|row| {
            crate::math::signed_imbalance_percent(
                self.left_row_effort[row],
                self.right_row_effort[row],
            )
        })
    }

    /// Home row left/right balance as signed percent: asymmetry in home row effort between hands.
    /// Range: (-∞, ∞). 0% = balanced. Positive = right-lean, negative = left-lean.
    pub fn home_row_balance(&self) -> f64 {
        crate::math::signed_imbalance_percent(self.left_row_effort[1], self.right_row_effort[1])
    }

    /// Top row total effort share: (left_top + right_top) / total_effort.
    /// Range: [0.0, 1.0]. 0 = unused, 1.0 = all effort on top row.
    pub fn top_row_ratio(&self) -> f64 {
        crate::math::ratio(
            self.left_row_effort[0] + self.right_row_effort[0],
            self.effort,
        )
    }

    /// Home row total effort share: (left_home + right_home) / total_effort.
    /// Range: [0.0, 1.0]. 0 = unused, 1.0 = all effort on home row.
    pub fn home_row_ratio(&self) -> f64 {
        crate::math::ratio(
            self.left_row_effort[1] + self.right_row_effort[1],
            self.effort,
        )
    }

    /// Bottom row total effort share: (left_bottom + right_bottom) / total_effort.
    /// Range: [0.0, 1.0]. 0 = unused, 1.0 = all effort on bottom row.
    pub fn bottom_row_ratio(&self) -> f64 {
        crate::math::ratio(
            self.left_row_effort[2] + self.right_row_effort[2],
            self.effort,
        )
    }

    // Column ratios: effort share per column (left: pinky→thumb, right: index→thumb)

    /// Left column 1 (pinky) effort share: left_column_effort[0] / total_effort.
    /// Range: [0.0, 1.0]. 0 = unused, 1.0 = all effort on left pinky.
    pub fn left_c1_ratio(&self) -> f64 {
        crate::math::ratio(self.left_column_effort[0], self.effort)
    }

    /// Left column 2 (ring) effort share.
    pub fn left_c2_ratio(&self) -> f64 {
        crate::math::ratio(self.left_column_effort[1], self.effort)
    }

    /// Left column 3 (middle) effort share.
    pub fn left_c3_ratio(&self) -> f64 {
        crate::math::ratio(self.left_column_effort[2], self.effort)
    }

    /// Left column 4 (index) effort share.
    pub fn left_c4_ratio(&self) -> f64 {
        crate::math::ratio(self.left_column_effort[3], self.effort)
    }

    /// Left column 5 (thumb) effort share.
    pub fn left_c5_ratio(&self) -> f64 {
        crate::math::ratio(self.left_column_effort[4], self.effort)
    }

    /// Right column 1 (index) effort share: right_column_effort[0] / total_effort.
    /// Range: [0.0, 1.0]. 0 = unused, 1.0 = all effort on right index.
    pub fn right_c1_ratio(&self) -> f64 {
        crate::math::ratio(self.right_column_effort[0], self.effort)
    }

    /// Right column 2 (middle) effort share.
    pub fn right_c2_ratio(&self) -> f64 {
        crate::math::ratio(self.right_column_effort[1], self.effort)
    }

    /// Right column 3 (ring) effort share.
    pub fn right_c3_ratio(&self) -> f64 {
        crate::math::ratio(self.right_column_effort[2], self.effort)
    }

    /// Right column 4 (pinky) effort share.
    pub fn right_c4_ratio(&self) -> f64 {
        crate::math::ratio(self.right_column_effort[3], self.effort)
    }

    /// Right column 5 (thumb) effort share.
    pub fn right_c5_ratio(&self) -> f64 {
        crate::math::ratio(self.right_column_effort[4], self.effort)
    }

    /// Format a 5-column ratio list for display.
    fn format_ratio_list(values: [f64; 5]) -> String {
        values
            .into_iter()
            .map(|value| format!("{:05.2}%", value * 100.0))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Format a row triplet as left/right/total effort ratios.
    fn format_row_triplet(value: (f64, f64, f64)) -> String {
        format!(
            "{:05.2}%, {:05.2}%, {:05.2}%",
            value.0 * 100.0,
            value.1 * 100.0,
            value.2 * 100.0,
        )
    }

    /// Format row balances in top → bottom order.
    fn format_balance_list(values: [f64; 3]) -> String {
        values
            .into_iter()
            .map(|value| format!("{value:+06.2}%"))
            .collect::<Vec<_>>()
            .join(", ")
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
            left_column_effort: self.right_column_effort,
            right_column_effort: self.left_column_effort,
            left_row_effort: self.right_row_effort,
            right_row_effort: self.left_row_effort,
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

    /// Parse a ratio column written as `12.34%` or legacy raw `12.34`.
    fn parse_ratio(value: &str) -> Option<f64> {
        let trimmed = value.trim();
        let percent = trimmed.ends_with('%');
        let raw = trimmed.trim_end_matches('%').parse::<f64>().ok()?;
        Some(if percent { raw / 100.0 } else { raw })
    }

    /// Parse a bucket column from either raw effort or percent of total effort.
    fn parse_bucket(value: Option<&&str>, effort: f64) -> f64 {
        let Some(value) = value else {
            return 0.0;
        };
        let trimmed = value.trim();
        if trimmed.ends_with('%') {
            Self::parse_ratio(trimmed).unwrap_or(0.0) * effort
        } else {
            trimmed.parse::<f64>().unwrap_or(0.0)
        }
    }

    /// Serialize as a CSV row (no header).
    pub fn to_csv(&self) -> String {
        [
            format!("{:.6}", self.fitness),
            format!("{:.2}%", self.row_switch_ratio() * 100.0),
            Self::format_imbalance(self.row_switch_imbalance()),
            format!("{:.2}%", self.hand_switch_ratio() * 100.0),
            Self::format_imbalance(self.hands_imbalance()),
            format!("{:.2}", self.effort),
            Self::format_imbalance(self.efforts_imbalance()),
            Self::format_imbalance(self.roll_imbalance()),
            format!("{:.2}", self.mean_streak()),
            Self::format_imbalance(self.streak_imbalance()),
            format!("{:.2}", self.left_streak()),
            format!("{:.2}", self.right_streak()),
            format!("{:.2}%", self.left_c1_ratio() * 100.0),
            format!("{:.2}%", self.left_c2_ratio() * 100.0),
            format!("{:.2}%", self.left_c3_ratio() * 100.0),
            format!("{:.2}%", self.left_c4_ratio() * 100.0),
            format!("{:.2}%", self.left_c5_ratio() * 100.0),
            format!("{:.2}%", self.right_c1_ratio() * 100.0),
            format!("{:.2}%", self.right_c2_ratio() * 100.0),
            format!("{:.2}%", self.right_c3_ratio() * 100.0),
            format!("{:.2}%", self.right_c4_ratio() * 100.0),
            format!("{:.2}%", self.right_c5_ratio() * 100.0),
            format!("{:.2}%", self.top_row_ratio() * 100.0),
            format!("{:.2}%", self.home_row_ratio() * 100.0),
            format!("{:.2}%", self.bottom_row_ratio() * 100.0),
            Self::format_imbalance(self.row_balances()[0]),
            Self::format_imbalance(self.home_row_balance()),
            Self::format_imbalance(self.row_balances()[2]),
            // Remaining columns
            format!("{:.2}%", self.left_effort_ratio() * 100.0),
            format!("{:.2}%", self.right_effort_ratio() * 100.0),
            format!("{:.2}%", self.left_count_ratio() * 100.0),
            format!("{:.2}%", self.right_count_ratio() * 100.0),
            format!("{:.2}", self.left_effort),
            format!("{:.2}", self.right_effort),
            self.left_count.to_string(),
            self.right_count.to_string(),
            self.hand_switches.to_string(),
            self.left_row_switch_cost.to_string(),
            self.right_row_switch_cost.to_string(),
            self.left_rolls.to_string(),
            self.right_rolls.to_string(),
            format!("{:.2}%", self.row_effort_ratios()[0].0 * 100.0),
            format!("{:.2}%", self.row_effort_ratios()[1].0 * 100.0),
            format!("{:.2}%", self.row_effort_ratios()[2].0 * 100.0),
            format!("{:.2}%", self.row_effort_ratios()[0].1 * 100.0),
            format!("{:.2}%", self.row_effort_ratios()[1].1 * 100.0),
            format!("{:.2}%", self.row_effort_ratios()[2].1 * 100.0),
        ]
        .join(",")
    }

    /// CSV header matching [`to_csv`] column order.
    pub fn csv_header() -> &'static str {
        "fitness,row_switch_ratio,row_switch_imbalance,hand_switch_ratio,hands_imbalance,effort,efforts_imbalance,roll_imbalance,mean_streak,streak_imbalance,left_streak,right_streak,left_c1_ratio,left_c2_ratio,left_c3_ratio,left_c4_ratio,left_c5_ratio,right_c1_ratio,right_c2_ratio,right_c3_ratio,right_c4_ratio,right_c5_ratio,top_row_ratio,home_row_ratio,bottom_row_ratio,left_row_balance,home_row_balance,bottom_row_balance,left_effort_ratio,right_effort_ratio,left_count_ratio,right_count_ratio,left_effort,right_effort,left_count,right_count,hand_switches,left_row_switch_cost,right_row_switch_cost,left_rolls,right_rolls,left_top_row_ratio,left_home_row_ratio,left_bottom_row_ratio,right_top_row_ratio,right_home_row_ratio,right_bottom_row_ratio"
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
        let effort = c.get(5)?.parse::<f64>().ok()?;
        Some(ScoreResult {
            fitness: c.first()?.parse().ok()?,
            effort,
            left_effort: c.get(32)?.parse().ok()?,
            right_effort: c.get(33)?.parse().ok()?,
            left_count: c.get(34)?.parse().ok()?,
            right_count: c.get(35)?.parse().ok()?,
            hand_switches: c.get(36)?.parse().ok()?,
            left_row_switch_cost: c.get(37)?.parse().ok()?,
            right_row_switch_cost: c.get(38)?.parse().ok()?,
            left_rolls: c.get(39)?.parse().ok()?,
            right_rolls: c.get(40)?.parse().ok()?,
            left_column_effort: [
                Self::parse_bucket(c.get(12), effort),
                Self::parse_bucket(c.get(13), effort),
                Self::parse_bucket(c.get(14), effort),
                Self::parse_bucket(c.get(15), effort),
                Self::parse_bucket(c.get(16), effort),
            ],
            right_column_effort: [
                Self::parse_bucket(c.get(17), effort),
                Self::parse_bucket(c.get(18), effort),
                Self::parse_bucket(c.get(19), effort),
                Self::parse_bucket(c.get(20), effort),
                Self::parse_bucket(c.get(21), effort),
            ],
            left_row_effort: [
                Self::parse_bucket(c.get(41), effort),
                Self::parse_bucket(c.get(42), effort),
                Self::parse_bucket(c.get(43), effort),
            ],
            right_row_effort: [
                Self::parse_bucket(c.get(44), effort),
                Self::parse_bucket(c.get(45), effort),
                Self::parse_bucket(c.get(46), effort),
            ],
        })
    }
}

impl std::fmt::Display for ScoreResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "φ {:.4} | ↕ {:05.2}% | ⇄ {:05.2}% | ⟳Δ {:+06.2}% | Δ {:+06.2}% | εΔ {:+06.2}% | ↕↔ {:+06.2}% | →Δ {:+06.2}% | → {:.2} | ε {:.2}",
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
        )?;
        writeln!(
            f,
            "cL [{}] | cR [{}]",
            Self::format_ratio_list(self.left_column_effort_ratios()),
            Self::format_ratio_list(self.right_column_effort_ratios()),
        )?;
        writeln!(
            f,
            "rT [{}] | rH [{}] | rB [{}]",
            Self::format_row_triplet(self.row_effort_ratios()[0]),
            Self::format_row_triplet(self.row_effort_ratios()[1]),
            Self::format_row_triplet(self.row_effort_ratios()[2]),
        )?;
        write!(
            f,
            "bal [{}]",
            Self::format_balance_list(self.row_balances()),
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
            left_column_effort: core::array::from_fn(|i| {
                self.left_column_effort[i] + other.left_column_effort[i]
            }),
            right_column_effort: core::array::from_fn(|i| {
                self.right_column_effort[i] + other.right_column_effort[i]
            }),
            left_row_effort: core::array::from_fn(|i| {
                self.left_row_effort[i] + other.left_row_effort[i]
            }),
            right_row_effort: core::array::from_fn(|i| {
                self.right_row_effort[i] + other.right_row_effort[i]
            }),
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
            left_column_effort: self.left_column_effort.map(|v| v * f),
            right_column_effort: self.right_column_effort.map(|v| v * f),
            left_row_effort: self.left_row_effort.map(|v| v * f),
            right_row_effort: self.right_row_effort.map(|v| v * f),
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
    fn press_tracks_columns_and_rows() {
        let left = ScoreResult::press(3, 2.5);
        let right = ScoreResult::press(18, 1.5);

        assert_eq!(left.left_count, 1);
        assert_eq!(left.left_column_effort[3], 2.5);
        assert_eq!(left.left_row_effort[0], 2.5);

        assert_eq!(right.right_count, 1);
        assert_eq!(right.right_column_effort[3], 1.5);
        assert_eq!(right.right_row_effort[0], 1.5);
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
            ..Default::default()
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

    #[test]
    fn to_csv_roundtrips_new_buckets() {
        let s =
            ScoreResult::press(0, 1.0) + ScoreResult::press(3, 2.0) + ScoreResult::press(18, 3.0);
        let csv = format!("k1,k2,k3,k4,k5,k6,{}", s.to_csv());
        let parsed = ScoreResult::from_csv(&csv).unwrap();

        assert!(ScoreResult::csv_header().contains("left_c1_ratio"));
        assert!(ScoreResult::csv_header().contains("left_row_balance"));
        assert_eq!(parsed.effort, s.effort);
        assert_eq!(parsed.left_count, s.left_count);
        assert_eq!(parsed.right_count, s.right_count);
    }
}
