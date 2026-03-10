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

pub mod categorical;
pub mod continuous;
pub mod probabilistic;

use serde::{Deserialize, Serialize};

// Re-export metric functions at crate root.
pub use categorical::{categorical_scores, csi, far, pod, CategoricalScores};
pub use continuous::{bias, mae, rmse};
pub use probabilistic::{
    brier_score, brier_score_checked, crps, crps_checked, crps_normalized,
    crps_normalized_checked,
};

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
            let mean_obs =
                pairs.iter().map(|p| p.observed).sum::<f64>() / pairs.len() as f64;
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
        let metrics = ForecastAccuracyMetrics::from_pairs(&pairs(&[
            (0.0, 0.0),
            (1.0, 2.0),
            (5.0, 4.0),
        ]));
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
        let e = ScoreError::LengthMismatch { expected: 3, got: 2 };
        assert_eq!(e.to_string(), "length mismatch: expected 3, got 2");
        let e2 = ScoreError::InvalidInput("bad value");
        assert_eq!(e2.to_string(), "invalid input: bad value");
    }
}
