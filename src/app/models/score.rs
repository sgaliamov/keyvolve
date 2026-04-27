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
