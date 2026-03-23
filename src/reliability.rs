//! Reliability diagnostics for probabilistic forecasts.

use serde::{Deserialize, Serialize};

use crate::{brier_score_checked, ScoreError};

/// Diagnostic decomposition of forecast skill into calibration and informativeness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "bindings/"))]
pub struct SkillComponents {
    /// Calibration error: lower is better.
    pub reliability: f64,
    /// Ability to separate different outcomes: higher is better.
    pub resolution: f64,
    /// Decisiveness relative to climatology, normalized to `[0, 1]`.
    pub sharpness: f64,
    /// Skill relative to climatology. May be negative when worse than baseline.
    pub brier_skill_score: f64,
}

/// Murphy-style decomposition of the Brier score using equal-width probability bins.
pub fn decompose_brier_score(
    forecast_probs: &[f64],
    observed_outcomes: &[f64],
    bins: usize,
) -> Result<SkillComponents, ScoreError> {
    brier_score_checked(forecast_probs, observed_outcomes)?;
    if bins == 0 {
        return Err(ScoreError::InvalidInput("bins must be greater than zero"));
    }
    if forecast_probs.is_empty() {
        return Ok(SkillComponents::default());
    }

    #[derive(Clone, Copy, Default)]
    struct BinStats {
        count: usize,
        sum_prob: f64,
        sum_outcome: f64,
    }

    let sample_count = forecast_probs.len() as f64;
    let climatology_rate = observed_outcomes.iter().sum::<f64>() / sample_count;
    let mut stats = vec![BinStats::default(); bins];

    for (&prob, &outcome) in forecast_probs.iter().zip(observed_outcomes.iter()) {
        let index = if prob >= 1.0 {
            bins - 1
        } else {
            ((prob * bins as f64).floor() as usize).min(bins - 1)
        };
        let bin = &mut stats[index];
        bin.count += 1;
        bin.sum_prob += prob;
        bin.sum_outcome += outcome;
    }

    let mut reliability = 0.0;
    let mut resolution = 0.0;

    for bin in stats.into_iter().filter(|bin| bin.count > 0) {
        let weight = bin.count as f64 / sample_count;
        let mean_prob = bin.sum_prob / bin.count as f64;
        let observed_rate = bin.sum_outcome / bin.count as f64;
        reliability += weight * (mean_prob - observed_rate).powi(2);
        resolution += weight * (observed_rate - climatology_rate).powi(2);
    }

    let uncertainty = climatology_rate * (1.0 - climatology_rate);
    let brier_skill_score = if uncertainty > 0.0 {
        (resolution - reliability) / uncertainty
    } else if reliability == 0.0 {
        1.0
    } else {
        0.0
    };

    let max_spread = climatology_rate.max(1.0 - climatology_rate);
    let sharpness = if max_spread > 0.0 {
        forecast_probs
            .iter()
            .map(|prob| (prob - climatology_rate).abs())
            .sum::<f64>()
            / sample_count
            / max_spread
    } else {
        0.0
    };

    Ok(SkillComponents {
        reliability,
        resolution,
        sharpness: sharpness.clamp(0.0, 1.0),
        brier_skill_score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposition_rejects_zero_bins() {
        let err = decompose_brier_score(&[0.5], &[1.0], 0).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput("bins must be greater than zero")
        );
    }

    #[test]
    fn decomposition_returns_zeroes_for_empty_input() {
        let result = decompose_brier_score(&[], &[], 10).unwrap();
        assert_eq!(result, SkillComponents::default());
    }

    #[test]
    fn decomposition_rewards_calibrated_forecasts() {
        let result =
            decompose_brier_score(&[0.1, 0.2, 0.8, 0.9], &[0.0, 0.0, 1.0, 1.0], 4).unwrap();
        assert!(result.reliability < 0.05);
        assert!(result.resolution > 0.0);
        assert!(result.sharpness > 0.5);
        assert!(result.brier_skill_score > 0.0);
    }

    #[test]
    fn decomposition_penalizes_miscalibrated_forecasts() {
        let result =
            decompose_brier_score(&[0.9, 0.9, 0.1, 0.1], &[0.0, 0.0, 1.0, 1.0], 4).unwrap();
        assert!(result.reliability > result.resolution);
        assert!(result.brier_skill_score <= 0.0);
    }
}
