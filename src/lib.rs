//! Forecast verification metrics for meteorological and probabilistic predictions.
//!
//! Provides standard verification scores used in numerical weather prediction (NWP),
//! prediction markets, and other forecasting domains. No dependencies beyond serde.
//!
//! # Metric Categories
//!
//! - **Continuous**: [`mae`], [`rmse`], [`bias`] — for temperature, wind speed, etc.
//! - **Categorical**: [`pod`], [`far`], [`csi`], [`categorical_scores`] — for binary events (rain/no-rain)
//! - **Probabilistic**: [`brier_score`], [`crps`], [`crps_normalized`] — for probability forecasts
//! - **Composite**: [`ForecastAccuracyMetrics`], [`CategoricalScores`] — bundled results

pub mod anti_gaming;
pub mod blockchain;
pub mod categorical;
pub mod continuous;
pub mod probabilistic;
pub mod reliability;
#[cfg(feature = "meteo-types")]
pub mod weather_scoring;

// Re-export blockchain types at crate root.
pub use blockchain::{
    ForecastArtifactManifest, ForecastManifestValidationError, ForecastMetricsValidationError,
    ForecastReceiptValidationError, ForecastVerificationReceipt,
};

use serde::{Deserialize, Serialize};

// Re-export metric functions at crate root.
pub use anti_gaming::{
    score_against_baselines, score_pairs_with_config, score_pairs_with_metadata,
    validate_lead_time, AntiGamingScore, ScoringConfig,
};
pub use categorical::{
    auc_from_roc, categorical_scores, csi, ets, far, frequency_bias, hss, pod, roc_curve,
    CategoricalScores, RocPoint,
};
pub use continuous::{bias, mae, rmse};
pub use probabilistic::{
    brier_score, brier_score_checked, brier_skill_score, brier_skill_score_checked, crps,
    crps_checked, crps_normalized, crps_normalized_checked,
};
pub use reliability::{
    brier_decomposition, decompose_brier_score, BrierDecomposition, ReliabilityBin, SkillComponents,
};
#[cfg(feature = "meteo-types")]
pub use weather_scoring::{score_weather_observations, WeatherVariableScores};

/// Errors returned by fallible scoring functions.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ScoreError {
    /// Input slices have different lengths.
    LengthMismatch { expected: usize, got: usize },
    /// An input value is outside the valid domain.
    InvalidInput(&'static str),
}

impl core::fmt::Display for ScoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ScoreError::LengthMismatch { expected, got } => {
                write!(f, "length mismatch: expected {expected}, got {got}")
            }
            ScoreError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

impl std::error::Error for ScoreError {}

/// A forecast–observation pair.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "bindings/"))]
pub struct Pair {
    pub predicted: f64,
    pub observed: f64,
}

impl Pair {
    pub fn new(predicted: f64, observed: f64) -> Self {
        Self {
            predicted,
            observed,
        }
    }
}

/// Reference forecast strategies for skill score computation.
///
/// Controls how `from_pairs_with_reference` selects the baseline RMSE that
/// represents "no skill". A skill score of 0 means the forecast performs as
/// well as this baseline; 1 means perfect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReferenceStrategy {
    /// Population standard deviation of observations in the sample (same as
    /// `from_pairs()`). Works well for large heterogeneous datasets; can be
    /// gamed by submitting during low-variability windows.
    SampleStdDev,
    /// Caller-supplied climatological RMSE (e.g., computed from years of
    /// history for this location and time-of-year). Prevents gaming via
    /// homogeneous short windows.
    FixedClimatology { reference_rmse: f64 },
    /// Persistence baseline: previous observation as forecast. Skill > 0
    /// means the model beats "do nothing". Falls back to 0 skill when fewer
    /// than two observations are available.
    Persistence,
}

/// RMSE of the persistence forecast over a time-ordered observation series.
///
/// `error[i] = observed[i] - observed[i-1]` for i ≥ 1.
/// Returns 0.0 when fewer than two observations are supplied.
pub fn persistence_rmse(observations: &[f64]) -> f64 {
    if observations.len() < 2 {
        return 0.0;
    }
    let sse: f64 = observations.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    (sse / (observations.len() - 1) as f64).sqrt()
}

/// Accuracy metrics from forecast verification.
///
/// Compatible with `ForecastAccuracyMetrics` in WRFCoin's shared-protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "bindings/"))]
pub struct ForecastAccuracyMetrics {
    pub mae: f64,
    pub rmse: f64,
    pub bias: f64,
    pub skill_score: f64,
}

impl ForecastAccuracyMetrics {
    /// Compute all continuous metrics from a set of pairs, with a composite skill score.
    ///
    /// The skill score is defined as `1.0 - (rmse / ref_rmse)`, clamped to `[0, 1]`,
    /// where `ref_rmse` is the **population standard deviation of the observed values**
    /// (equivalent to the RMSE of the climatological mean forecast).
    ///
    /// - A skill score of `1.0` means perfect forecasts (RMSE = 0).
    /// - A skill score of `0.0` means the forecast is no better than climatology.
    /// - Negative skill (RMSE > ref_rmse) is clamped to `0.0`.
    ///
    /// When all observations are identical (`ref_rmse = 0`), skill is `1.0` only
    /// if RMSE is also zero, otherwise `0.0`.
    pub fn from_pairs(pairs: &[Pair]) -> Self {
        let mae_val = mae(pairs);
        let rmse_val = rmse(pairs);
        let bias_val = bias(pairs);
        let skill_score = if pairs.is_empty() {
            0.0
        } else {
            let mean_obs = pairs.iter().map(|p| p.observed).sum::<f64>() / pairs.len() as f64;
            // ref_rmse = population stddev of observations = RMSE of climatological mean
            let variance = pairs
                .iter()
                .map(|p| (p.observed - mean_obs).powi(2))
                .sum::<f64>()
                / pairs.len() as f64;
            let ref_rmse = variance.sqrt();
            if ref_rmse > 0.0 {
                (1.0 - rmse_val / ref_rmse).clamp(0.0, 1.0)
            } else {
                // All observations identical — perfect if RMSE is 0
                if rmse_val == 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
        };
        Self {
            mae: mae_val,
            rmse: rmse_val,
            bias: bias_val,
            skill_score,
        }
    }

    /// Compute metrics against a chosen reference baseline.
    ///
    /// All continuous metrics (MAE, RMSE, bias) are the same as `from_pairs()`.
    /// Only `skill_score` changes: `1.0 − (rmse / reference_rmse)`, clamped to
    /// `[0, 1]`. See [`ReferenceStrategy`] for baseline options.
    pub fn from_pairs_with_reference(pairs: &[Pair], reference: ReferenceStrategy) -> Self {
        let mae_val = mae(pairs);
        let rmse_val = rmse(pairs);
        let bias_val = bias(pairs);
        let skill_score = if pairs.is_empty() {
            0.0
        } else {
            let ref_rmse = match reference {
                ReferenceStrategy::SampleStdDev => {
                    let n = pairs.len() as f64;
                    let mean_obs = pairs.iter().map(|p| p.observed).sum::<f64>() / n;
                    let var = pairs
                        .iter()
                        .map(|p| (p.observed - mean_obs).powi(2))
                        .sum::<f64>()
                        / n;
                    var.sqrt()
                }
                ReferenceStrategy::FixedClimatology { reference_rmse } => reference_rmse,
                ReferenceStrategy::Persistence => {
                    let obs: Vec<f64> = pairs.iter().map(|p| p.observed).collect();
                    persistence_rmse(&obs)
                }
            };
            if ref_rmse > 0.0 {
                (1.0 - rmse_val / ref_rmse).clamp(0.0, 1.0)
            } else if rmse_val == 0.0 {
                1.0
            } else {
                0.0
            }
        };
        Self { mae: mae_val, rmse: rmse_val, bias: bias_val, skill_score }
    }

    /// Validate that all fields are finite and within expected ranges.
    #[must_use = "validation errors must be handled"]
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.mae.is_finite() || self.mae < 0.0 {
            return Err("mae must be finite and non-negative");
        }
        if !self.rmse.is_finite() || self.rmse < 0.0 {
            return Err("rmse must be finite and non-negative");
        }
        if !self.bias.is_finite() {
            return Err("bias must be finite");
        }
        if !self.skill_score.is_finite() || !(0.0..=1.0).contains(&self.skill_score) {
            return Err("skill_score must be in [0, 1]");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(data: &[(f64, f64)]) -> Vec<Pair> {
        data.iter().map(|&(p, o)| Pair::new(p, o)).collect()
    }

    #[test]
    fn from_pairs_empty() {
        let metrics = ForecastAccuracyMetrics::from_pairs(&[]);
        assert_eq!(metrics.mae, 0.0);
        assert_eq!(metrics.rmse, 0.0);
        assert_eq!(metrics.bias, 0.0);
        assert_eq!(metrics.skill_score, 0.0);
    }

    #[test]
    fn from_pairs_perfect_forecast() {
        let metrics = ForecastAccuracyMetrics::from_pairs(&pairs(&[
            (10.0, 10.0),
            (20.0, 20.0),
            (30.0, 30.0),
        ]));
        assert_eq!(metrics.mae, 0.0);
        assert_eq!(metrics.rmse, 0.0);
        assert_eq!(metrics.bias, 0.0);
        assert_eq!(metrics.skill_score, 1.0);
    }

    #[test]
    fn from_pairs_identical_observations_nonzero_error() {
        let metrics = ForecastAccuracyMetrics::from_pairs(&pairs(&[(5.0, 5.0), (6.0, 5.0)]));
        assert_eq!(metrics.skill_score, 0.0);
    }

    /// All observations identical, forecast also identical → ref_rmse=0, RMSE=0 → skill=1.0.
    #[test]
    fn from_pairs_identical_obs_perfect_forecast() {
        let metrics =
            ForecastAccuracyMetrics::from_pairs(&pairs(&[(5.0, 5.0), (5.0, 5.0), (5.0, 5.0)]));
        assert_eq!(metrics.rmse, 0.0);
        assert_eq!(metrics.skill_score, 1.0);
    }

    /// Climatological mean forecast: RMSE = ref_rmse → skill = 0.
    #[test]
    fn from_pairs_skill_zero_climatology() {
        // obs=[0.0, 1.0], mean=0.5, ref_rmse=0.5
        // predict climatological mean 0.5 for both → RMSE=0.5, skill=0.0
        let metrics = ForecastAccuracyMetrics::from_pairs(&pairs(&[(0.5, 0.0), (0.5, 1.0)]));
        assert!((metrics.rmse - 0.5).abs() < 1e-12);
        assert!((metrics.skill_score - 0.0).abs() < 1e-12);
    }

    #[test]
    fn from_pairs_mixed_errors() {
        let metrics =
            ForecastAccuracyMetrics::from_pairs(&pairs(&[(0.0, 0.0), (1.0, 2.0), (5.0, 4.0)]));
        assert!((metrics.mae - (2.0 / 3.0)).abs() < 1e-12);
        assert!((metrics.rmse - (2.0f64 / 3.0).sqrt()).abs() < 1e-12);
        assert_eq!(metrics.bias, 0.0);
        assert!((metrics.skill_score - 0.5).abs() < 1e-12);
    }

    #[test]
    fn from_pairs_single_pair() {
        let metrics = ForecastAccuracyMetrics::from_pairs(&pairs(&[(8.0, 10.0)]));
        assert_eq!(metrics.mae, 2.0);
        assert_eq!(metrics.rmse, 2.0);
        assert_eq!(metrics.bias, -2.0);
        assert_eq!(metrics.skill_score, 0.0);
    }

    #[test]
    fn validate_accepts_valid_metrics() {
        let metrics = ForecastAccuracyMetrics {
            mae: 0.1,
            rmse: 0.2,
            bias: -0.1,
            skill_score: 0.7,
        };
        assert_eq!(metrics.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_negative_mae() {
        let metrics = ForecastAccuracyMetrics {
            mae: -0.1,
            rmse: 0.2,
            bias: 0.0,
            skill_score: 0.7,
        };
        assert_eq!(
            metrics.validate(),
            Err("mae must be finite and non-negative")
        );
    }

    #[test]
    fn validate_rejects_negative_rmse() {
        let metrics = ForecastAccuracyMetrics {
            mae: 0.1,
            rmse: -0.2,
            bias: 0.0,
            skill_score: 0.7,
        };
        assert_eq!(
            metrics.validate(),
            Err("rmse must be finite and non-negative")
        );
    }

    #[test]
    fn validate_rejects_non_finite_bias() {
        let metrics = ForecastAccuracyMetrics {
            mae: 0.1,
            rmse: 0.2,
            bias: f64::NAN,
            skill_score: 0.7,
        };
        assert_eq!(metrics.validate(), Err("bias must be finite"));
    }

    #[test]
    fn validate_rejects_skill_out_of_range() {
        let metrics = ForecastAccuracyMetrics {
            mae: 0.1,
            rmse: 0.2,
            bias: 0.0,
            skill_score: 1.5,
        };
        assert_eq!(metrics.validate(), Err("skill_score must be in [0, 1]"));
    }

    #[test]
    fn validate_rejects_non_finite_skill() {
        let metrics = ForecastAccuracyMetrics {
            mae: 0.1,
            rmse: 0.2,
            bias: 0.0,
            skill_score: f64::INFINITY,
        };
        assert_eq!(metrics.validate(), Err("skill_score must be in [0, 1]"));
    }

    #[test]
    fn score_error_display() {
        let e = ScoreError::LengthMismatch {
            expected: 3,
            got: 2,
        };
        assert_eq!(e.to_string(), "length mismatch: expected 3, got 2");
        let e2 = ScoreError::InvalidInput("bad value");
        assert_eq!(e2.to_string(), "invalid input: bad value");
    }

    // --- ReferenceStrategy / persistence_rmse ---

    #[test]
    fn sample_stddev_matches_from_pairs() {
        let data = pairs(&[(1.0, 0.0), (2.0, 1.0), (3.0, 3.0)]);
        let a = ForecastAccuracyMetrics::from_pairs(&data);
        let b = ForecastAccuracyMetrics::from_pairs_with_reference(
            &data,
            ReferenceStrategy::SampleStdDev,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn fixed_climatology_uses_supplied_rmse() {
        // obs=[0, 1], forecast=[0.5, 0.5] → RMSE=0.5
        // with ref_rmse=1.0: skill = 1 - 0.5/1.0 = 0.5
        let data = pairs(&[(0.5, 0.0), (0.5, 1.0)]);
        let m = ForecastAccuracyMetrics::from_pairs_with_reference(
            &data,
            ReferenceStrategy::FixedClimatology { reference_rmse: 1.0 },
        );
        assert!((m.skill_score - 0.5).abs() < 1e-12);
    }

    #[test]
    fn fixed_climatology_perfect_forecast_skill_one() {
        let data = pairs(&[(1.0, 1.0), (2.0, 2.0)]);
        let m = ForecastAccuracyMetrics::from_pairs_with_reference(
            &data,
            ReferenceStrategy::FixedClimatology { reference_rmse: 2.0 },
        );
        assert_eq!(m.skill_score, 1.0);
    }

    #[test]
    fn persistence_skill_beats_persistence() {
        // obs=[10, 11, 12, 13] — perfect linear trend
        // persistence RMSE = 1.0 (each step differs by 1)
        // perfect forecast: RMSE = 0 → skill = 1.0
        let data: Vec<Pair> = [10.0_f64, 11.0, 12.0, 13.0]
            .iter()
            .map(|&o| Pair::new(o, o))
            .collect();
        let m = ForecastAccuracyMetrics::from_pairs_with_reference(
            &data,
            ReferenceStrategy::Persistence,
        );
        assert_eq!(m.skill_score, 1.0);
    }

    #[test]
    fn persistence_skill_is_zero_at_baseline() {
        // obs=[10, 11, 12, 13]: each step +1 → persistence_rmse = 1.0
        // Pairs are exactly the persistence forecast (predicted = prev obs):
        //   t1: (10→11), t2: (11→12), t3: (12→13) → RMSE = 1.0
        // skill = 1 - 1.0/1.0 = 0.0
        let data = vec![
            Pair::new(10.0, 11.0),
            Pair::new(11.0, 12.0),
            Pair::new(12.0, 13.0),
        ];
        let m = ForecastAccuracyMetrics::from_pairs_with_reference(
            &data,
            ReferenceStrategy::Persistence,
        );
        // ref = persistence_rmse of observations [11,12,13] extracted from pairs
        // persistence_rmse([11,12,13]) = sqrt((1+1)/2) = 1.0
        // forecast RMSE = sqrt((1+1+1)/3) = 1.0 → skill = 0.0
        assert!((m.skill_score).abs() < 1e-10);
    }

    #[test]
    fn persistence_rmse_basic() {
        // [10, 11, 12, 13]: each diff = 1 → RMSE = 1.0
        assert!((persistence_rmse(&[10.0, 11.0, 12.0, 13.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn persistence_rmse_single_returns_zero() {
        assert_eq!(persistence_rmse(&[5.0]), 0.0);
        assert_eq!(persistence_rmse(&[]), 0.0);
    }

    #[test]
    fn persistence_rmse_two_points() {
        // diff = 3.0 → RMSE = 3.0
        assert!((persistence_rmse(&[0.0, 3.0]) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn skill_clamped_to_zero_when_rmse_exceeds_reference() {
        // forecast RMSE > reference_rmse → raw skill < 0 → clamped to 0
        let data = pairs(&[(0.0, 10.0), (0.0, 10.0)]); // RMSE = 10
        let m = ForecastAccuracyMetrics::from_pairs_with_reference(
            &data,
            ReferenceStrategy::FixedClimatology { reference_rmse: 1.0 },
        );
        assert_eq!(m.skill_score, 0.0);
    }
}
