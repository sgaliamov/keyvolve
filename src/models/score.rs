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

    /// Left share of same-hand effort.
    pub fn left_effort_ratio(&self) -> f64 {
        ratio(self.left_effort, self.left_effort + self.right_effort)
    }

    /// Right share of same-hand effort.
    pub fn right_effort_ratio(&self) -> f64 {
        ratio(self.right_effort, self.left_effort + self.right_effort)
    }

    /// Share of hand switches among all bigram transitions.
    pub fn switch_ratio(&self) -> f64 {
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

    /// Serialize as a CSV row (no header).
    pub fn to_csv(&self) -> String {
        format!(
            "{:.4}, {:.2}, {:.2}, {:.2}, {:.2}%, {}, {:.2}%, {:.2}, {:.2}%, {}, {:.2}%, {}, {:.2}%, {}, {:.2}%",
            self.fitness,
            if self.right_count == 0 {
                0.0
            } else {
                self.left_count as f64 / self.right_count as f64
            },
            self.effort,
            self.left_effort,
            self.left_effort_ratio() * 100.0,
            self.left_count,
            self.left_count_ratio() * 100.0,
            self.right_effort,
            self.right_effort_ratio() * 100.0,
            self.right_count,
            self.right_count_ratio() * 100.0,
            self.bigram_switches,
            self.switch_ratio() * 100.0,
            self.row_switch_cost,
            self.row_switch_ratio() * 100.0
        )
    }

    /// CSV header matching [`to_csv`] column order.
    pub fn csv_header() -> &'static str {
        "fitness, count_ratio, effort, left_effort, left_effort_ratio, left_count, left_count_ratio, right_effort, right_effort_ratio, right_count, right_count_ratio, bigram_switches, switch_ratio, row_switch_cost, row_switch_ratio"
    }
}

impl std::fmt::Display for ScoreResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "φ {:.4} | L/R {:.2} | ε {:.2} | L {:.2} ({}, {:.1}%) | R {:.2} ({}, {:.1}%) | ⇄ {} ({:.2}%) | ↕ {} ({:.2}%)",
            self.fitness,
            if self.right_count == 0 {
                0.0
            } else {
                self.left_count as f64 / self.right_count as f64
            },
            self.effort,
            self.left_effort,
            self.left_count,
            self.left_effort_ratio() * 100.0,
            self.right_effort,
            self.right_count,
            self.right_effort_ratio() * 100.0,
            self.bigram_switches,
            self.switch_ratio() * 100.0,
            self.row_switch_cost,
            self.row_switch_ratio() * 100.0,
        )
    }
}

/// Safe ratio helper.
fn ratio(value: f64, total: f64) -> f64 {
    if total == 0.0 { 0.0 } else { value / total }
}

impl std::ops::Add for ScoreResult {
    type Output = Self;

    fn add(self, other: ScoreResult) -> Self {
        ScoreResult {
            effort: self.effort + other.effort,
            fitness: self.fitness + other.fitness,
            left_count: self.left_count + other.left_count,
            right_count: self.right_count + other.right_count,
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
            bigram_switches: self.bigram_switches * n,
            row_switch_cost: self.row_switch_cost * n,
            left_effort: self.left_effort * f,
            right_effort: self.right_effort * f,
        }
    }
}
