//! Probabilistic verification metrics: Brier Score, CRPS.

use crate::ScoreError;

fn ensure_equal_len(lhs_len: usize, rhs_len: usize) -> Result<(), ScoreError> {
    if lhs_len != rhs_len {
        return Err(ScoreError::LengthMismatch {
            expected: lhs_len,
            got: rhs_len,
        });
    }
    Ok(())
}

fn validate_brier_inputs(
    forecast_probs: &[f64],
    observed_outcomes: &[f64],
) -> Result<(), ScoreError> {
    if !forecast_probs
        .iter()
        .all(|p| p.is_finite() && (0.0..=1.0).contains(p))
    {
        return Err(ScoreError::InvalidInput(
            "forecast probabilities must be finite and in [0, 1]",
        ));
    }
    if !observed_outcomes
        .iter()
        .all(|o| o.is_finite() && (*o == 0.0 || *o == 1.0))
    {
        return Err(ScoreError::InvalidInput(
            "observed outcomes must be finite 0.0 or 1.0 values",
        ));
    }
    Ok(())
}

fn validate_crps_inputs(
    forecast_values: &[f64],
    observed_values: &[f64],
) -> Result<(), ScoreError> {
    if !forecast_values.iter().all(|v| v.is_finite()) {
        return Err(ScoreError::InvalidInput("forecast values must be finite"));
    }
    if !observed_values.iter().all(|v| v.is_finite()) {
        return Err(ScoreError::InvalidInput("observed values must be finite"));
    }
    Ok(())
}

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
    brier_score_checked(forecast_probs, observed_outcomes)
        .expect("brier_score preconditions violated")
}

/// Checked Brier Score variant.
///
/// Returns an error when:
/// - input slices have different lengths ([`ScoreError::LengthMismatch`])
/// - forecast probabilities are outside [0, 1] or non-finite ([`ScoreError::InvalidInput`])
/// - observed outcomes are not binary (0.0 or 1.0) or non-finite ([`ScoreError::InvalidInput`])
pub fn brier_score_checked(
    forecast_probs: &[f64],
    observed_outcomes: &[f64],
) -> Result<f64, ScoreError> {
    ensure_equal_len(forecast_probs.len(), observed_outcomes.len())?;
    validate_brier_inputs(forecast_probs, observed_outcomes)?;

    if forecast_probs.is_empty() {
        return Ok(0.0);
    }

    let sum: f64 = forecast_probs
        .iter()
        .zip(observed_outcomes.iter())
        .map(|(&f, &o)| (f - o).powi(2))
        .sum();
    Ok(sum / forecast_probs.len() as f64)
}

/// Continuous Ranked Probability Score (deterministic approximation).
///
/// For deterministic forecasts, CRPS reduces to the mean absolute error.
/// This implementation is unclamped and suitable for any numeric domain
/// (temperature in K, wind in m/s, etc.).
///
/// CRPS ≈ (1/n) * sum(|forecast - observed|)
///
/// See also [`crps_normalized`] for a version that maps scores to [0, 1].
///
/// Ported from `backend/server/services/forecast/forecast-verification-service.ts` lines 462–475.
pub fn crps(forecast_values: &[f64], observed_values: &[f64]) -> f64 {
    crps_checked(forecast_values, observed_values).expect("crps preconditions violated")
}

/// Checked CRPS variant.
///
/// Returns an error when:
/// - input slices have different lengths ([`ScoreError::LengthMismatch`])
/// - any forecast/observation value is non-finite ([`ScoreError::InvalidInput`])
pub fn crps_checked(
    forecast_values: &[f64],
    observed_values: &[f64],
) -> Result<f64, ScoreError> {
    ensure_equal_len(forecast_values.len(), observed_values.len())?;
    validate_crps_inputs(forecast_values, observed_values)?;

    if forecast_values.is_empty() {
        return Ok(0.0);
    }
    let sum: f64 = forecast_values
        .iter()
        .zip(observed_values.iter())
        .map(|(&f, &o)| (f - o).abs())
        .sum();
    Ok(sum / forecast_values.len() as f64)
}

/// Normalised CRPS: mean of `min(|forecast - observed| / threshold, 1.0)`.
///
/// Maps the unclamped MAE to [0, 1] by dividing each absolute error by `threshold`
/// and clamping the result. Suitable for use with [`composite_skill`], which expects
/// both inputs in [0, 1].
///
/// `threshold` must be finite and positive (e.g., the expected maximum error for the domain).
pub fn crps_normalized(
    forecast_values: &[f64],
    observed_values: &[f64],
    threshold: f64,
) -> f64 {
    crps_normalized_checked(forecast_values, observed_values, threshold)
        .expect("crps_normalized preconditions violated")
}

/// Checked normalised CRPS variant.
///
/// Returns an error when:
/// - input slices have different lengths ([`ScoreError::LengthMismatch`])
/// - any forecast/observation value is non-finite ([`ScoreError::InvalidInput`])
/// - `threshold` is not finite or not positive ([`ScoreError::InvalidInput`])
pub fn crps_normalized_checked(
    forecast_values: &[f64],
    observed_values: &[f64],
    threshold: f64,
) -> Result<f64, ScoreError> {
    ensure_equal_len(forecast_values.len(), observed_values.len())?;
    validate_crps_inputs(forecast_values, observed_values)?;
    if !threshold.is_finite() || threshold <= 0.0 {
        return Err(ScoreError::InvalidInput(
            "threshold must be finite and positive",
        ));
    }

    if forecast_values.is_empty() {
        return Ok(0.0);
    }
    let sum: f64 = forecast_values
        .iter()
        .zip(observed_values.iter())
        .map(|(&f, &o)| ((f - o).abs() / threshold).min(1.0))
        .sum();
    Ok(sum / forecast_values.len() as f64)
}

/// Composite skill score from Brier and normalised CRPS.
///
/// skill = ((1 - brier_score) + (1 - crps_normalized)) / 2
///
/// Both inputs are expected to be in [0, 1]. Use [`crps_normalized`] (not [`crps`])
/// to produce the CRPS argument.
///
/// Ported from `backend/server/services/forecast/forecast-verification-service.ts` line 474.
pub fn composite_skill(brier: f64, crps_val: f64) -> f64 {
    ((1.0 - brier) + (1.0 - crps_val)) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScoreError;

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
    fn brier_checked_rejects_length_mismatch() {
        let err = brier_score_checked(&[0.2, 0.4], &[0.0]).unwrap_err();
        assert_eq!(err, ScoreError::LengthMismatch { expected: 2, got: 1 });
    }

    #[test]
    fn brier_checked_rejects_invalid_probabilities() {
        let err = brier_score_checked(&[1.2], &[1.0]).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput(
                "forecast probabilities must be finite and in [0, 1]"
            )
        );
    }

    #[test]
    fn brier_checked_rejects_nan_probability() {
        let err = brier_score_checked(&[f64::NAN], &[1.0]).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput(
                "forecast probabilities must be finite and in [0, 1]"
            )
        );
    }

    #[test]
    fn brier_checked_rejects_negative_probability() {
        let err = brier_score_checked(&[-0.1], &[0.0]).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput(
                "forecast probabilities must be finite and in [0, 1]"
            )
        );
    }

    #[test]
    fn brier_checked_rejects_invalid_outcomes() {
        let err = brier_score_checked(&[0.2], &[0.3]).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput(
                "observed outcomes must be finite 0.0 or 1.0 values"
            )
        );
    }

    #[test]
    fn crps_perfect() {
        let f = vec![10.0, 20.0, 30.0];
        let o = vec![10.0, 20.0, 30.0];
        assert!((crps(&f, &o)).abs() < 1e-10);
    }

    #[test]
    fn crps_unclamped_large_domain() {
        let f = vec![100.0];
        let o = vec![0.0];
        assert_eq!(crps(&f, &o), 100.0);
    }

    #[test]
    fn crps_checked_rejects_length_mismatch() {
        let err = crps_checked(&[1.0, 2.0], &[1.0]).unwrap_err();
        assert_eq!(err, ScoreError::LengthMismatch { expected: 2, got: 1 });
    }

    #[test]
    fn crps_checked_rejects_non_finite_values() {
        let err = crps_checked(&[1.0, f64::NAN], &[1.0, 2.0]).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput("forecast values must be finite")
        );
    }

    // ── crps_normalized ───────────────────────────────────────────────────────

    #[test]
    fn crps_normalized_basic() {
        // error = 0.5, threshold = 1.0 → normalized = 0.5
        let f = vec![1.5, 2.5];
        let o = vec![1.0, 2.0];
        assert!((crps_normalized(&f, &o, 1.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn crps_normalized_large_error_clamped() {
        // error = 100.0, threshold = 1.0 → clamped to 1.0
        let f = vec![100.0];
        let o = vec![0.0];
        assert_eq!(crps_normalized(&f, &o, 1.0), 1.0);
    }

    #[test]
    fn crps_normalized_custom_threshold() {
        // error = 5.0, threshold = 10.0 → 0.5
        let f = vec![15.0];
        let o = vec![10.0];
        assert!((crps_normalized(&f, &o, 10.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn crps_normalized_empty() {
        assert_eq!(crps_normalized(&[], &[], 1.0), 0.0);
    }

    #[test]
    fn crps_normalized_checked_rejects_length_mismatch() {
        let err = crps_normalized_checked(&[1.0, 2.0], &[1.0], 1.0).unwrap_err();
        assert_eq!(err, ScoreError::LengthMismatch { expected: 2, got: 1 });
    }

    #[test]
    fn crps_normalized_checked_rejects_bad_threshold() {
        let err = crps_normalized_checked(&[1.0], &[1.0], -1.0).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput("threshold must be finite and positive")
        );
        let err2 = crps_normalized_checked(&[1.0], &[1.0], f64::NAN).unwrap_err();
        assert_eq!(
            err2,
            ScoreError::InvalidInput("threshold must be finite and positive")
        );
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
