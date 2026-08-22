//! Fundamental numeric helpers shared across the crate. Pure functions on
//! primitives only — no domain types — so every "do the arithmetic safely"
//! pattern has exactly one implementation.

/// Safe division: `0.0` when `total == 0.0` instead of `NaN`/`inf`.
pub fn ratio(value: f64, total: f64) -> f64 {
    if total == 0.0 { 0.0 } else { value / total }
}

/// Directional imbalance as a percent: how far the `left/right` ratio strays
/// from parity. `0%` when balanced or when `right == 0`. Negative = left-heavy,
/// positive = right-heavy.
pub fn signed_imbalance_percent(left: f64, right: f64) -> f64 {
    if right == 0.0 {
        0.0
    } else {
        (left / right - 1.0) * 100.0
    }
}

/// Average run length from presses and same-run continuations. Every press
/// starts a run or continues one; continuations are exactly `rolls`, so
/// `runs = count − rolls` and `streak = count / runs`. `0.0` for an unused hand.
pub fn streak(count: u64, rolls: u64) -> f64 {
    match count.saturating_sub(rolls) {
        0 => 0.0,
        runs => count as f64 / runs as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_guards_zero_total() {
        assert_eq!(ratio(5.0, 0.0), 0.0);
        assert_eq!(ratio(3.0, 6.0), 0.5);
    }

    #[test]
    fn signed_imbalance_percent_guards_zero_right() {
        assert_eq!(signed_imbalance_percent(4.0, 0.0), 0.0);
    }

    #[test]
    fn signed_imbalance_percent_measures_skew() {
        assert_eq!(signed_imbalance_percent(5.0, 5.0), 0.0);
        // 6/3 - 1 = 1 -> 100% (left-heavy, positive).
        assert!((signed_imbalance_percent(6.0, 3.0) - 100.0).abs() < 1e-9);
        // 3/6 - 1 = -0.5 -> -50% (right-heavy, negative).
        assert!((signed_imbalance_percent(3.0, 6.0) - (-50.0)).abs() < 1e-9);
    }

    #[test]
    fn streak_averages_run_lengths() {
        assert_eq!(streak(8, 6), 4.0);
        assert_eq!(streak(2, 0), 1.0);
        assert_eq!(streak(0, 0), 0.0);
    }
}
