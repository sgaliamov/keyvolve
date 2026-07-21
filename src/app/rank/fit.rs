use super::state::{Answer, START_DEV, START_RATING};

/// Elo points represented by one natural-log Bradley–Terry skill unit.
const ELO_SCALE: f64 = 173.717_792_761_300_73;
/// Weak zero-centred Gaussian prior; resolves disconnected/sparse histories.
const PRIOR_PRECISION: f64 = (ELO_SCALE / START_DEV) * (ELO_SCALE / START_DEV);
const MAX_ITERATIONS: usize = 30;
const TOLERANCE: f64 = 1e-9;

/// Deterministic Bradley–Terry maximum-a-posteriori fit.
pub(super) struct BradleyTerryFit {
    /// Fitted Elo-compatible ratings.
    pub ratings: Vec<f64>,
    /// Marginal posterior standard deviations in Elo points.
    pub deviations: Vec<f64>,
    /// Dense posterior covariance in squared Elo points, row-major.
    pub covariance: Vec<f64>,
}

/// Fit ratings and uncertainty from raw answers; answer order has no effect.
pub(super) fn fit_bradley_terry(
    answers: &[Answer],
    item_count: usize,
    initial_ratings: &[f64],
) -> BradleyTerryFit {
    if answers.is_empty() {
        return BradleyTerryFit {
            ratings: vec![START_RATING; item_count],
            deviations: vec![START_DEV; item_count],
            covariance: prior_covariance(item_count),
        };
    }

    let mut skills = (0..item_count)
        .map(|i| {
            (initial_ratings.get(i).copied().unwrap_or(START_RATING) - START_RATING) / ELO_SCALE
        })
        .collect::<Vec<_>>();

    for _ in 0..MAX_ITERATIONS {
        let (gradient, hessian) = derivatives(&skills, answers);
        let rhs = gradient.iter().map(|g| -g).collect::<Vec<_>>();
        let Some(cholesky) = cholesky(hessian, item_count) else {
            break;
        };
        let delta = solve_cholesky(&cholesky, &rhs, item_count);
        let largest = delta.iter().copied().map(f64::abs).fold(0.0, f64::max);
        if largest < TOLERANCE {
            break;
        }

        let before = objective(&skills, answers);
        let mut step = 1.0;
        while step > 1.0 / 1024.0 {
            let proposed = skills
                .iter()
                .zip(&delta)
                .map(|(skill, change)| skill + step * change)
                .collect::<Vec<_>>();
            if objective(&proposed, answers) < before {
                skills = proposed;
                break;
            }
            step *= 0.5;
        }
        if step <= 1.0 / 1024.0 {
            break;
        }
    }

    let (_, hessian) = derivatives(&skills, answers);
    let cholesky = cholesky(hessian, item_count).expect("positive Bradley–Terry posterior Hessian");
    let covariance = inverse(&cholesky, item_count)
        .into_iter()
        .map(|value| value * ELO_SCALE * ELO_SCALE)
        .collect::<Vec<_>>();
    let deviations = (0..item_count)
        .map(|i| covariance[i * item_count + i].max(0.0).sqrt())
        .collect();
    let ratings = skills
        .into_iter()
        .map(|skill| START_RATING + skill * ELO_SCALE)
        .collect();

    BradleyTerryFit {
        ratings,
        deviations,
        covariance,
    }
}

/// Bradley–Terry probability that rating `a` beats rating `b`.
pub(super) fn expected_score(a: f64, b: f64) -> f64 {
    logistic((a - b) / ELO_SCALE)
}

/// Expected Fisher information of comparing two uncertain items.
pub(super) fn information_score(a: f64, b: f64, difference_deviation: f64) -> f64 {
    let p = expected_score(a, b);
    p * (1.0 - p) * difference_deviation * difference_deviation / (ELO_SCALE * ELO_SCALE)
}

fn derivatives(skills: &[f64], answers: &[Answer]) -> (Vec<f64>, Vec<f64>) {
    let n = skills.len();
    let mut gradient = skills
        .iter()
        .map(|skill| PRIOR_PRECISION * skill)
        .collect::<Vec<_>>();
    let mut hessian = vec![0.0; n * n];
    for i in 0..n {
        hessian[i * n + i] = PRIOR_PRECISION;
    }

    for answer in answers {
        let p = logistic(skills[answer.a] - skills[answer.b]);
        let residual = p - answer.score;
        let weight = (p * (1.0 - p)).max(1e-12);
        gradient[answer.a] += residual;
        gradient[answer.b] -= residual;
        hessian[answer.a * n + answer.a] += weight;
        hessian[answer.b * n + answer.b] += weight;
        hessian[answer.a * n + answer.b] -= weight;
        hessian[answer.b * n + answer.a] -= weight;
    }
    (gradient, hessian)
}

fn objective(skills: &[f64], answers: &[Answer]) -> f64 {
    let prior = skills
        .iter()
        .map(|skill| 0.5 * PRIOR_PRECISION * skill * skill)
        .sum::<f64>();
    answers.iter().fold(prior, |loss, answer| {
        let difference = skills[answer.a] - skills[answer.b];
        loss + softplus(difference) - answer.score * difference
    })
}

fn logistic(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}

fn softplus(value: f64) -> f64 {
    if value > 0.0 {
        value + (-value).exp().ln_1p()
    } else {
        value.exp().ln_1p()
    }
}

fn cholesky(mut matrix: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for row in 0..n {
        for col in 0..=row {
            let mut value = matrix[row * n + col];
            for k in 0..col {
                value -= matrix[row * n + k] * matrix[col * n + k];
            }
            if row == col {
                if value <= 0.0 || !value.is_finite() {
                    return None;
                }
                matrix[row * n + col] = value.sqrt();
            } else {
                matrix[row * n + col] = value / matrix[col * n + col];
            }
        }
        for col in row + 1..n {
            matrix[row * n + col] = 0.0;
        }
    }
    Some(matrix)
}

fn solve_cholesky(cholesky: &[f64], rhs: &[f64], n: usize) -> Vec<f64> {
    let mut solution = rhs.to_vec();
    for row in 0..n {
        for col in 0..row {
            solution[row] -= cholesky[row * n + col] * solution[col];
        }
        solution[row] /= cholesky[row * n + row];
    }
    for row in (0..n).rev() {
        for col in row + 1..n {
            solution[row] -= cholesky[col * n + row] * solution[col];
        }
        solution[row] /= cholesky[row * n + row];
    }
    solution
}

fn inverse(cholesky: &[f64], n: usize) -> Vec<f64> {
    let mut inverse = vec![0.0; n * n];
    for column in 0..n {
        let mut unit = vec![0.0; n];
        unit[column] = 1.0;
        let solution = solve_cholesky(cholesky, &unit, n);
        for row in 0..n {
            inverse[row * n + column] = solution[row];
        }
    }
    inverse
}

fn prior_covariance(n: usize) -> Vec<f64> {
    let mut covariance = vec![0.0; n * n];
    for i in 0..n {
        covariance[i * n + i] = START_DEV * START_DEV;
    }
    covariance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(a: usize, b: usize, score: f64) -> Answer {
        Answer {
            a,
            b,
            score,
            prev_a: (START_RATING, START_DEV, 0),
            prev_b: (START_RATING, START_DEV, 0),
            prev_pending_a: 0,
            prev_pending_b: 0,
        }
    }

    #[test]
    fn fit_is_independent_of_answer_order() {
        let answers = vec![answer(0, 1, 1.0), answer(1, 2, 1.0), answer(0, 2, 1.0)];
        let mut reversed = answers.clone();
        reversed.reverse();
        let a = fit_bradley_terry(&answers, 3, &[]);
        let b = fit_bradley_terry(&reversed, 3, &[]);
        for (x, y) in a.ratings.iter().zip(b.ratings) {
            assert!((x - y).abs() < 1e-8);
        }
    }

    #[test]
    fn fit_is_independent_of_warm_start() {
        let answers = (0..20)
            .flat_map(|_| [answer(0, 1, 1.0), answer(1, 2, 1.0)])
            .collect::<Vec<_>>();
        let centered = fit_bradley_terry(&answers, 3, &[]);
        let extreme = fit_bradley_terry(&answers, 3, &[10_000.0, -5_000.0, 4_000.0]);
        for (a, b) in centered.ratings.iter().zip(extreme.ratings) {
            assert!((a - b).abs() < 1e-8);
        }
    }

    #[test]
    fn fit_recovers_order_and_reduces_uncertainty() {
        let answers = (0..20)
            .flat_map(|_| [answer(0, 1, 1.0), answer(1, 2, 1.0)])
            .collect::<Vec<_>>();
        let fit = fit_bradley_terry(&answers, 3, &[]);
        assert!(fit.ratings[0] > fit.ratings[1]);
        assert!(fit.ratings[1] > fit.ratings[2]);
        assert!(
            fit.deviations
                .iter()
                .all(|&deviation| deviation < START_DEV)
        );
    }

    #[test]
    fn equal_ties_keep_equal_ratings() {
        let fit = fit_bradley_terry(&[answer(0, 1, 0.5)], 2, &[]);
        assert!((fit.ratings[0] - fit.ratings[1]).abs() < 1e-8);
    }
}
