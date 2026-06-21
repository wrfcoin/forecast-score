//! Reliability diagnostics for probabilistic forecasts.

use serde::{Deserialize, Serialize};

use crate::{brier_score, brier_score_checked, ScoreError};

/// One bin of the reliability diagram: forecast probability vs. observed frequency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReliabilityBin {
    /// Mean forecast probability of all forecasts in this bin.
    pub mean_forecast_prob: f64,
    /// Observed event frequency in this bin.
    pub observed_frequency: f64,
    /// Number of forecasts in this bin.
    pub count: u64,
}

/// Full Murphy (1973) Brier score decomposition with per-bin reliability diagram data.
///
/// BS = Reliability − Resolution + Uncertainty
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrierDecomposition {
    /// Overall Brier score for this sample.
    pub brier_score: f64,
    /// Calibration term: mean squared deviation of forecast probabilities from observed
    /// frequencies within each bin. Lower = better calibrated.
    pub reliability: f64,
    /// Resolution term: mean squared deviation of per-bin observed frequencies from the
    /// overall base rate. Higher = more informative.
    pub resolution: f64,
    /// Uncertainty: base-rate variance `p̄(1 − p̄)`. Depends only on observations.
    pub uncertainty: f64,
    /// Brier Skill Score relative to climatology: `(Resolution − Reliability) / Uncertainty`.
    /// Negative means worse than always forecasting the base rate.
    pub brier_skill_score: f64,
    /// Per-bin data for reliability diagram visualisation, sorted by `mean_forecast_prob`.
    pub bins: Vec<ReliabilityBin>,
}

/// Compute the full Murphy (1973) Brier score decomposition.
///
/// Returns reliability, resolution, uncertainty, BSS, and per-bin data for
/// reliability diagram plots. `n_bins` equal-width probability bins are used
/// (typical values: 10 or 20).
///
/// Returns `Err` when inputs are malformed (length mismatch, non-finite values,
/// probabilities outside [0, 1], or `n_bins == 0`).
pub fn brier_decomposition(
    forecast_probs: &[f64],
    observed_outcomes: &[f64],
    n_bins: usize,
) -> Result<BrierDecomposition, ScoreError> {
    brier_score_checked(forecast_probs, observed_outcomes)?;
    if n_bins == 0 {
        return Err(ScoreError::InvalidInput("n_bins must be greater than zero"));
    }

    let bs = brier_score(forecast_probs, observed_outcomes);

    if forecast_probs.is_empty() {
        return Ok(BrierDecomposition {
            brier_score: 0.0,
            reliability: 0.0,
            resolution: 0.0,
            uncertainty: 0.0,
            brier_skill_score: 0.0,
            bins: vec![],
        });
    }

    #[derive(Clone, Default)]
    struct BinAcc {
        count: usize,
        sum_prob: f64,
        sum_outcome: f64,
    }

    let n = forecast_probs.len() as f64;
    let base_rate = observed_outcomes.iter().sum::<f64>() / n;
    let mut accs = vec![BinAcc::default(); n_bins];

    for (&prob, &outcome) in forecast_probs.iter().zip(observed_outcomes.iter()) {
        let idx = if prob >= 1.0 {
            n_bins - 1
        } else {
            ((prob * n_bins as f64).floor() as usize).min(n_bins - 1)
        };
        let a = &mut accs[idx];
        a.count += 1;
        a.sum_prob += prob;
        a.sum_outcome += outcome;
    }

    let mut reliability = 0.0;
    let mut resolution = 0.0;
    let mut bins: Vec<ReliabilityBin> = Vec::with_capacity(n_bins);

    for acc in accs.into_iter().filter(|a| a.count > 0) {
        let w = acc.count as f64 / n;
        let mean_p = acc.sum_prob / acc.count as f64;
        let obs_freq = acc.sum_outcome / acc.count as f64;
        reliability += w * (mean_p - obs_freq).powi(2);
        resolution += w * (obs_freq - base_rate).powi(2);
        bins.push(ReliabilityBin {
            mean_forecast_prob: mean_p,
            observed_frequency: obs_freq,
            count: acc.count as u64,
        });
    }
    bins.sort_by(|a, b| {
        a.mean_forecast_prob
            .partial_cmp(&b.mean_forecast_prob)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let uncertainty = base_rate * (1.0 - base_rate);
    let brier_skill_score = if uncertainty > 0.0 {
        (resolution - reliability) / uncertainty
    } else if reliability == 0.0 {
        1.0
    } else {
        0.0
    };

    Ok(BrierDecomposition {
        brier_score: bs,
        reliability,
        resolution,
        uncertainty,
        brier_skill_score,
        bins,
    })
}

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

    // --- BrierDecomposition tests ---

    #[test]
    fn brier_decomposition_rejects_zero_bins() {
        let err = brier_decomposition(&[0.5], &[1.0], 0).unwrap_err();
        assert_eq!(err, ScoreError::InvalidInput("n_bins must be greater than zero"));
    }

    #[test]
    fn brier_decomposition_empty() {
        let result = brier_decomposition(&[], &[], 10).unwrap();
        assert_eq!(result.brier_score, 0.0);
        assert!(result.bins.is_empty());
    }

    #[test]
    fn brier_decomposition_perfect_forecast() {
        // Forecaster always says 1.0 when event occurs, 0.0 otherwise
        let probs = vec![1.0, 1.0, 0.0, 0.0];
        let obs   = vec![1.0, 1.0, 0.0, 0.0];
        let d = brier_decomposition(&probs, &obs, 10).unwrap();
        assert!((d.brier_score).abs() < 1e-10);
        assert!((d.reliability).abs() < 1e-10);
        assert!(d.brier_skill_score > 0.0);
    }

    #[test]
    fn brier_decomposition_identity_bs() {
        // brier_score via decomposition must match direct brier_score()
        let probs = vec![0.1, 0.4, 0.6, 0.9];
        let obs   = vec![0.0, 0.0, 1.0, 1.0];
        let d = brier_decomposition(&probs, &obs, 10).unwrap();
        let direct = brier_score(&probs, &obs);
        // BS = Reliability - Resolution + Uncertainty
        let reconstructed = d.reliability - d.resolution + d.uncertainty;
        assert!((reconstructed - direct).abs() < 1e-10, "decomposition identity");
        assert!((d.brier_score - direct).abs() < 1e-10);
    }

    #[test]
    fn brier_decomposition_bins_sorted() {
        let probs: Vec<f64> = (0..=10).map(|i| i as f64 / 10.0).collect();
        let obs:   Vec<f64> = probs.iter().map(|p| if *p > 0.5 { 1.0 } else { 0.0 }).collect();
        let d = brier_decomposition(&probs, &obs, 5).unwrap();
        for w in d.bins.windows(2) {
            assert!(w[0].mean_forecast_prob <= w[1].mean_forecast_prob);
        }
    }
}
