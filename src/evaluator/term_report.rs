/// One metric's diagnostic row: what it reads, what it wants, what it costs.
#[derive(Debug, Clone, Copy)]
pub struct TermReport {
    /// Metric name as printed in the CSV.
    pub name: &'static str,

    /// Measured value, percent units.
    pub value: f64,

    /// Configured limit or target point.
    pub goal: f64,

    /// Normalized deviation; `1.0` at the accepted edge.
    pub deviation: f64,

    /// Penalty contribution: `weight · deviation^sharpness`.
    pub cost: f64,

    /// Marginal cost per percentage point: `weight · sharpness · deviation^(s−1) / norm`.
    /// The term's pull in the tug-of-war — a metric rests off goal when its pressure is
    /// lower than what the opposing terms (and raw effort) gain from the same move.
    pub pressure: f64,
}
