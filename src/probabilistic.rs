//! Probabilistic verification metrics: Brier Score, CRPS.

/// Brier Score for probability forecasts.
///
/// BS = (1/n) * sum((forecast_probability - observed_outcome)^2)
///
/// - `forecast_probs`: forecast probabilities in [0, 1]
/// - `observed_outcomes`: binary outcomes (0.0 or 1.0)
///
/// Perfect score = 0.0, worst = 1.0.
/// Returns 0.0 for empty input.
///
/// Ported from `backend/server/services/forecast/forecast-verification-service.ts` lines 441–460.
pub fn brier_score(forecast_probs: &[f64], observed_outcomes: &[f64]) -> f64 {
    let n = forecast_probs.len().min(observed_outcomes.len());
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = forecast_probs
        .iter()
        .zip(observed_outcomes.iter())
        .take(n)
        .map(|(&f, &o)| (f - o).powi(2))
        .sum();
    sum / n as f64
}

/// Continuous Ranked Probability Score (simplified).
///
/// For deterministic forecasts, CRPS reduces to the mean absolute error.
/// This implementation uses the deterministic approximation:
///
/// CRPS ≈ (1/n) * sum(|forecast - observed|)
///
/// clamped to [0, 1] for scoring purposes (as in the WRFCoin backend).
///
/// Ported from `backend/server/services/forecast/forecast-verification-service.ts` lines 462–475.
pub fn crps(forecast_values: &[f64], observed_values: &[f64]) -> f64 {
    let n = forecast_values.len().min(observed_values.len());
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = forecast_values
        .iter()
        .zip(observed_values.iter())
        .take(n)
        .map(|(&f, &o)| (f - o).abs())
        .sum();
    let raw = sum / n as f64;
    raw.clamp(0.0, 1.0)
}

/// Composite skill score from Brier and CRPS.
///
/// skill = ((1 - brier_score) + (1 - crps_score)) / 2
///
/// Ported from `backend/server/services/forecast/forecast-verification-service.ts` line 474.
pub fn composite_skill(brier: f64, crps_val: f64) -> f64 {
    ((1.0 - brier) + (1.0 - crps_val)) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brier_perfect() {
        let probs = vec![1.0, 0.0, 1.0];
        let outcomes = vec![1.0, 0.0, 1.0];
        assert!((brier_score(&probs, &outcomes)).abs() < 1e-10);
    }

    #[test]
    fn brier_worst() {
        let probs = vec![0.0, 1.0];
        let outcomes = vec![1.0, 0.0];
        assert!((brier_score(&probs, &outcomes) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn brier_moderate() {
        // 50% confidence on all events that happened
        let probs = vec![0.5, 0.5];
        let outcomes = vec![1.0, 1.0];
        // BS = mean((0.5-1)^2) = 0.25
        assert!((brier_score(&probs, &outcomes) - 0.25).abs() < 1e-10);
    }

    #[test]
    fn brier_empty() {
        assert_eq!(brier_score(&[], &[]), 0.0);
    }

    #[test]
    fn crps_perfect() {
        let f = vec![10.0, 20.0, 30.0];
        let o = vec![10.0, 20.0, 30.0];
        assert!((crps(&f, &o)).abs() < 1e-10);
    }

    #[test]
    fn crps_clamped() {
        // Large errors get clamped to 1.0
        let f = vec![100.0];
        let o = vec![0.0];
        assert_eq!(crps(&f, &o), 1.0);
    }

    #[test]
    fn composite_skill_perfect() {
        assert!((composite_skill(0.0, 0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn composite_skill_moderate() {
        // brier=0.25, crps=0.3 → skill = ((1-0.25) + (1-0.3))/2 = (0.75+0.7)/2 = 0.725
        assert!((composite_skill(0.25, 0.3) - 0.725).abs() < 1e-10);
    }
}
