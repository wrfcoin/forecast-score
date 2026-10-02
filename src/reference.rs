//! Checked reference baselines; sample-relative metrics retain their legacy shape.

use crate::{ForecastAccuracyMetrics, Pair, ReferenceStrategy, ScoreError};

/// Persistence error over adjacent, time-ordered observations, with >=2 samples.
pub fn persistence_rmse_checked(observations: &[f64]) -> Result<f64, ScoreError> {
    if observations.len() < 2 {
        return Err(ScoreError::InvalidInput(
            "persistence needs two observations",
        ));
    }
    if observations.iter().any(|v| !v.is_finite()) {
        return Err(ScoreError::InvalidInput("observations must be finite"));
    }
    let squared_error: f64 = observations.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    let error = (squared_error / (observations.len() - 1) as f64).sqrt();
    if !error.is_finite() {
        return Err(ScoreError::InvalidInput("persistence error overflow"));
    }
    Ok(error)
}

impl ForecastAccuracyMetrics {
    /// Checked baseline scoring. For persistence, the first pair seeds history;
    /// compare forecast and baseline errors over pairs[1..]. Continuous metrics
    /// still summarize the full window. A zero-error persistence tie has zero
    /// skill. Requires finite metrics and a positive finite climatological RMSE.
    pub fn from_pairs_with_reference_checked(
        pairs: &[Pair],
        reference: ReferenceStrategy,
    ) -> Result<Self, ScoreError> {
        if pairs
            .iter()
            .any(|p| !p.predicted.is_finite() || !p.observed.is_finite())
        {
            return Err(ScoreError::InvalidInput("pairs must be finite"));
        }
        let mut metrics = Self::from_pairs(pairs);
        metrics
            .validate()
            .map_err(|_| ScoreError::InvalidInput("metrics overflow"))?;
        let (error, baseline) = match reference {
            ReferenceStrategy::SampleStdDev => return Ok(metrics),
            ReferenceStrategy::FixedClimatology { reference_rmse } => {
                if !reference_rmse.is_finite() || reference_rmse <= 0.0 {
                    return Err(ScoreError::InvalidInput(
                        "climatological RMSE must be positive and finite",
                    ));
                }
                (metrics.rmse, reference_rmse)
            }
            ReferenceStrategy::Persistence => {
                let observations: Vec<_> = pairs.iter().map(|p| p.observed).collect();
                let baseline = persistence_rmse_checked(&observations)?;
                (crate::rmse(&pairs[1..]), baseline)
            }
        };
        if !error.is_finite() {
            return Err(ScoreError::InvalidInput("forecast error overflow"));
        }
        metrics.skill_score = if pairs.is_empty() || baseline == 0.0 {
            0.0
        } else {
            (1.0 - error / baseline).clamp(0.0, 1.0)
        };
        Ok(metrics)
    }
}
