//! Forecast verification metrics for meteorological and probabilistic predictions.
//!
//! Provides standard verification scores used in numerical weather prediction (NWP),
//! prediction markets, and other forecasting domains. No dependencies beyond serde.
//!
//! # Metric Categories
//!
//! - **Continuous**: [`mae`], [`rmse`], [`bias`] — for temperature, wind speed, etc.
//! - **Categorical**: [`pod`], [`far`], [`csi`], [`categorical_scores`] — for binary events (rain/no-rain)
//! - **Probabilistic**: [`brier_score`], [`crps`] — for probability forecasts
//! - **Composite**: [`ForecastAccuracyMetrics`], [`CategoricalScores`] — bundled results

pub mod categorical;
pub mod continuous;
pub mod probabilistic;

use serde::{Deserialize, Serialize};

// Re-export metric functions at crate root.
pub use categorical::{categorical_scores, csi, far, pod, CategoricalScores};
pub use continuous::{bias, mae, rmse};
pub use probabilistic::{brier_score, crps};

/// A forecast–observation pair.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
pub struct ForecastAccuracyMetrics {
    pub mae: f64,
    pub rmse: f64,
    pub bias: f64,
    pub skill_score: f64,
}

impl ForecastAccuracyMetrics {
    /// Compute all continuous metrics from a set of pairs, with a composite skill score.
    ///
    /// `skill_score` is derived from Brier and CRPS if probabilistic inputs are available,
    /// otherwise it falls back to `1.0 - (rmse / reference_rmse)` clamped to [0, 1].
    pub fn from_pairs(pairs: &[Pair]) -> Self {
        let mae_val = mae(pairs);
        let rmse_val = rmse(pairs);
        let bias_val = bias(pairs);
        // Simple skill: 1 - normalized RMSE (using observation stddev as reference)
        let skill_score = if pairs.is_empty() {
            0.0
        } else {
            let mean_obs =
                pairs.iter().map(|p| p.observed).sum::<f64>() / pairs.len() as f64;
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
