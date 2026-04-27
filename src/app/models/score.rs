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
    /// Serialize as a CSV row (no header).
    pub fn to_csv(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.effort,
            self.left_effort,
            self.left_count,
            self.right_effort,
            self.right_count,
            self.switches,
        )
    }

    /// CSV header matching [`to_csv`] column order.
    pub fn csv_header() -> &'static str {
        "effort,left_effort,left_count,right_effort,right_count,switches"
    }
}

impl std::fmt::Display for ScoreResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "effort: {:.4} | left: {:.4} ({}) | right: {:.4} ({}) | switches: {}",
            self.effort,
            self.left_effort,
            self.left_count,
            self.right_effort,
            self.right_count,
            self.switches,
        )
    }
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
