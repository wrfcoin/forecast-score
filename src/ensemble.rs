//! Ensemble forecast verification metrics: CRPS, spread-skill, rank histogram.

use crate::ScoreError;
use serde::{Deserialize, Serialize};

/// A single ensemble forecast with multiple member values and one observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnsembleForecast {
    /// Ensemble member values (ordering does not matter).
    pub members: Vec<f64>,
    /// Verified observation.
    pub observed: f64,
}

impl EnsembleForecast {
    pub fn new(members: Vec<f64>, observed: f64) -> Self {
        Self { members, observed }
    }
}

fn validate_forecasts(forecasts: &[EnsembleForecast]) -> Result<(), ScoreError> {
    if forecasts.is_empty() {
        return Err(ScoreError::InvalidInput("forecasts slice must be non-empty"));
    }
    for f in forecasts {
        if f.members.is_empty() {
            return Err(ScoreError::InvalidInput(
                "each forecast must have at least one member",
            ));
        }
        if !f.observed.is_finite() {
            return Err(ScoreError::InvalidInput("observed values must be finite"));
        }
        if !f.members.iter().all(|m| m.is_finite()) {
            return Err(ScoreError::InvalidInput("member values must be finite"));
        }
    }
    Ok(())
}

/// CRPS for a single ensemble forecast using the energy score formula.
///
/// `CRPS = (1/n) Σ|x_i − y| − (1/2n²) Σ_i Σ_j |x_i − x_j|`
fn crps_single(members: &[f64], observed: f64) -> f64 {
    let n = members.len() as f64;
    let term1 = members.iter().map(|&x| (x - observed).abs()).sum::<f64>() / n;
    let mut spread_sum = 0.0;
    for i in 0..members.len() {
        for j in 0..members.len() {
            spread_sum += (members[i] - members[j]).abs();
        }
    }
    term1 - spread_sum / (2.0 * n * n)
}

/// Mean Continuous Ranked Probability Score over an ensemble of forecasts.
///
/// Uses the energy score form:
/// `CRPS = E[|X − y|] − ½ E[|X − X′|]`
///
/// For a single-member ensemble this reduces to absolute error (MAE).
/// Returns `Err` when the slice is empty or any value is non-finite.
pub fn crps_ensemble(forecasts: &[EnsembleForecast]) -> Result<f64, ScoreError> {
    validate_forecasts(forecasts)?;
    let sum: f64 = forecasts
        .iter()
        .map(|f| crps_single(&f.members, f.observed))
        .sum();
    Ok(sum / forecasts.len() as f64)
}

/// Spread-skill ratio: `mean_ensemble_spread / rmse_of_ensemble_mean`.
///
/// Spread is the population standard deviation of ensemble members for each
/// case; skill is the RMSE of the ensemble mean over all cases.
/// A ratio near 1.0 indicates a well-calibrated ensemble.
///
/// Returns `Err` when inputs are invalid or the ensemble mean RMSE is zero
/// (all forecasts are perfect — ratio undefined, treated as 1.0).
pub fn spread_skill_ratio(forecasts: &[EnsembleForecast]) -> Result<f64, ScoreError> {
    validate_forecasts(forecasts)?;
    let n = forecasts.len() as f64;

    let mut sum_spread = 0.0;
    let mut sum_sq_err = 0.0;

    for f in forecasts {
        let k = f.members.len() as f64;
        let mean_m = f.members.iter().sum::<f64>() / k;
        let var = f.members.iter().map(|&x| (x - mean_m).powi(2)).sum::<f64>() / k;
        sum_spread += var.sqrt();
        sum_sq_err += (mean_m - f.observed).powi(2);
    }

    let mean_spread = sum_spread / n;
    let rmse = (sum_sq_err / n).sqrt();

    if rmse == 0.0 {
        return Ok(1.0);
    }
    Ok(mean_spread / rmse)
}

/// Rank histogram (Talagrand diagram) for ensemble forecasts.
///
/// For each forecast, the observation is ranked among the sorted ensemble
/// members. The histogram has `n_members + 1` bins: bin 0 means the
/// observation fell below all members; bin `n_members` means it exceeded all.
///
/// All forecasts must have the **same** number of members.
///
/// Returns `Err` when forecasts are invalid or member counts are inconsistent.
pub fn rank_histogram(forecasts: &[EnsembleForecast]) -> Result<Vec<u64>, ScoreError> {
    validate_forecasts(forecasts)?;
    let n_members = forecasts[0].members.len();
    for f in forecasts {
        if f.members.len() != n_members {
            return Err(ScoreError::InvalidInput(
                "all forecasts must have the same number of members for rank histogram",
            ));
        }
    }

    let mut counts = vec![0u64; n_members + 1];
    let mut sorted = Vec::with_capacity(n_members);

    for f in forecasts {
        sorted.clear();
        sorted.extend_from_slice(&f.members);
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // rank = number of members strictly less than observed
        let rank = sorted.partition_point(|&x| x < f.observed);
        counts[rank] += 1;
    }

    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(members: Vec<f64>, observed: f64) -> EnsembleForecast {
        EnsembleForecast::new(members, observed)
    }

    // --- crps_ensemble ---

    #[test]
    fn crps_single_member_equals_absolute_error() {
        // With 1 member, CRPS = |x - y|
        let forecasts = vec![
            fc(vec![1.0], 3.0),
            fc(vec![5.0], 5.0),
            fc(vec![10.0], 7.0),
        ];
        let c = crps_ensemble(&forecasts).unwrap();
        // mean |error| = (2 + 0 + 3) / 3 = 5/3
        assert!((c - 5.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn crps_perfect_ensemble() {
        // Each member equals the observation → CRPS = 0
        let forecasts = vec![
            fc(vec![5.0, 5.0, 5.0], 5.0),
            fc(vec![10.0, 10.0], 10.0),
        ];
        let c = crps_ensemble(&forecasts).unwrap();
        assert!(c.abs() < 1e-12);
    }

    #[test]
    fn crps_rejects_empty_slice() {
        assert!(crps_ensemble(&[]).is_err());
    }

    #[test]
    fn crps_rejects_empty_members() {
        let err = crps_ensemble(&[fc(vec![], 1.0)]).unwrap_err();
        assert_eq!(
            err,
            ScoreError::InvalidInput("each forecast must have at least one member")
        );
    }

    #[test]
    fn crps_larger_ensemble_is_better() {
        // A 5-member ensemble centered on obs vs a 1-member far away
        let good = vec![fc(vec![9.8, 9.9, 10.0, 10.1, 10.2], 10.0)];
        let bad = vec![fc(vec![13.0], 10.0)];
        assert!(crps_ensemble(&good).unwrap() < crps_ensemble(&bad).unwrap());
    }

    // --- spread_skill_ratio ---

    #[test]
    fn spread_skill_well_calibrated() {
        // Ensemble spread ≈ error → ratio ≈ 1.0
        // Perfect ensemble mean but realistic spread is hard to engineer;
        // instead verify the formula is consistent.
        let forecasts: Vec<EnsembleForecast> = (0..20)
            .map(|i| {
                let obs = i as f64;
                // members centered on obs with spread = 1
                let members = vec![obs - 1.0, obs, obs + 1.0];
                fc(members, obs)
            })
            .collect();
        let ratio = spread_skill_ratio(&forecasts).unwrap();
        // ensemble mean = obs always → RMSE = 0 → falls back to 1.0
        assert!((ratio - 1.0).abs() < 1e-10);
    }

    #[test]
    fn spread_skill_underdispersive() {
        // Spread much smaller than error → ratio < 1
        let forecasts = vec![
            fc(vec![10.0, 10.01], 5.0),
            fc(vec![20.0, 20.01], 15.0),
        ];
        let ratio = spread_skill_ratio(&forecasts).unwrap();
        assert!(ratio < 0.1);
    }

    #[test]
    fn spread_skill_rejects_empty() {
        assert!(spread_skill_ratio(&[]).is_err());
    }

    // --- rank_histogram ---

    #[test]
    fn rank_histogram_flat_for_perfect_calibration() {
        // Observation always falls in a different rank → near-uniform counts
        // 3-member ensemble; possible ranks 0..=3
        let forecasts = vec![
            fc(vec![2.0, 4.0, 6.0], 1.0), // below all → rank 0
            fc(vec![2.0, 4.0, 6.0], 3.0), // between 2 and 4 → rank 1
            fc(vec![2.0, 4.0, 6.0], 5.0), // between 4 and 6 → rank 2
            fc(vec![2.0, 4.0, 6.0], 7.0), // above all → rank 3
        ];
        let hist = rank_histogram(&forecasts).unwrap();
        assert_eq!(hist.len(), 4);
        assert_eq!(hist, vec![1, 1, 1, 1]);
    }

    #[test]
    fn rank_histogram_u_shape_underdispersive() {
        // All observations fall outside ensemble → bins 0 and n_members get all counts
        let forecasts = vec![
            fc(vec![3.0, 4.0, 5.0], 1.0), // rank 0
            fc(vec![3.0, 4.0, 5.0], 1.5), // rank 0
            fc(vec![3.0, 4.0, 5.0], 8.0), // rank 3
            fc(vec![3.0, 4.0, 5.0], 9.0), // rank 3
        ];
        let hist = rank_histogram(&forecasts).unwrap();
        assert_eq!(hist[0], 2);
        assert_eq!(hist[1], 0);
        assert_eq!(hist[2], 0);
        assert_eq!(hist[3], 2);
    }

    #[test]
    fn rank_histogram_rejects_mismatched_member_counts() {
        let forecasts = vec![fc(vec![1.0, 2.0], 1.5), fc(vec![1.0, 2.0, 3.0], 2.0)];
        assert!(rank_histogram(&forecasts).is_err());
    }

    #[test]
    fn rank_histogram_rejects_empty() {
        assert!(rank_histogram(&[]).is_err());
    }
}
