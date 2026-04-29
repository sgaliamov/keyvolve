/// Full breakdown of a scoring pass over a word or corpus.
#[derive(Debug, Clone, Default)]
pub struct ScoreResult {
    /// Total weighted effort, with balance factor applied at corpus level.
    pub effort: f64,

    /// Number of consecutive same-hand bigrams on the left.
    pub left_count: u32,

    /// Number of consecutive same-hand bigrams on the right.
    pub right_count: u32,

    /// Number of hand switches.
    pub switches: u32,

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

    /// Serialize as a CSV row (no header).
    pub fn to_csv(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{}",
            self.effort,
            self.left_effort,
            self.left_effort_ratio(),
            self.left_count,
            self.left_count_ratio(),
            self.right_effort,
            self.right_effort_ratio(),
            self.right_count,
            self.right_count_ratio(),
            self.switches,
        )
    }

    /// CSV header matching [`to_csv`] column order.
    pub fn csv_header() -> &'static str {
        "effort,left_effort,left_effort_ratio,left_count,left_count_ratio,right_effort,right_effort_ratio,right_count,right_count_ratio,switches"
    }
}

impl std::fmt::Display for ScoreResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "effort: {:.2} | left: {:.2} ({}, {:.1}%) | right: {:.2} ({}, {:.1}%) | switches: {}",
            self.effort,
            self.left_effort,
            self.left_count,
            self.left_effort_ratio() * 100.0,
            self.right_effort,
            self.right_count,
            self.right_effort_ratio() * 100.0,
            self.switches,
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
            left_count: self.left_count + other.left_count,
            right_count: self.right_count + other.right_count,
            switches: self.switches + other.switches,
            left_effort: self.left_effort + other.left_effort,
            right_effort: self.right_effort + other.right_effort,
        }
    }
}
