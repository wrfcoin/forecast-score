//! Continuous verification metrics: MAE, RMSE, Bias.

use crate::Pair;

/// Mean Absolute Error.
///
/// MAE = (1/n) * sum(|predicted - observed|)
///
/// Returns 0.0 for empty input.
pub fn mae(pairs: &[Pair]) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    let sum: f64 = pairs.iter().map(|p| (p.predicted - p.observed).abs()).sum();
    sum / pairs.len() as f64
}

/// Root Mean Squared Error.
///
/// RMSE = sqrt((1/n) * sum((predicted - observed)^2))
///
/// Returns 0.0 for empty input.
pub fn rmse(pairs: &[Pair]) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = pairs
        .iter()
        .map(|p| (p.predicted - p.observed).powi(2))
        .sum();
    (sum_sq / pairs.len() as f64).sqrt()
}

/// Mean Bias (mean signed error).
///
/// Bias = (1/n) * sum(predicted - observed)
///
/// Positive bias means over-prediction, negative means under-prediction.
/// Returns 0.0 for empty input.
pub fn bias(pairs: &[Pair]) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    let sum: f64 = pairs.iter().map(|p| p.predicted - p.observed).sum();
    sum / pairs.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(data: &[(f64, f64)]) -> Vec<Pair> {
        data.iter().map(|&(p, o)| Pair::new(p, o)).collect()
    }

    #[test]
    fn empty_input() {
        assert_eq!(mae(&[]), 0.0);
        assert_eq!(rmse(&[]), 0.0);
        assert_eq!(bias(&[]), 0.0);
    }

    #[test]
    fn perfect_forecast() {
        let p = pairs(&[(10.0, 10.0), (20.0, 20.0), (30.0, 30.0)]);
        assert_eq!(mae(&p), 0.0);
        assert_eq!(rmse(&p), 0.0);
        assert_eq!(bias(&p), 0.0);
    }

    #[test]
    fn constant_error() {
        // All predictions 2.0 above observed
        let p = pairs(&[(12.0, 10.0), (22.0, 20.0), (32.0, 30.0)]);
        assert!((mae(&p) - 2.0).abs() < 1e-10);
        assert!((rmse(&p) - 2.0).abs() < 1e-10);
        assert!((bias(&p) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn mixed_errors() {
        let p = pairs(&[(12.0, 10.0), (18.0, 20.0)]); // +2, -2
        assert!((mae(&p) - 2.0).abs() < 1e-10);
        assert!((rmse(&p) - 2.0).abs() < 1e-10);
        assert!((bias(&p) - 0.0).abs() < 1e-10); // errors cancel
    }

    #[test]
    fn rmse_penalizes_outliers() {
        // One big error vs many small errors
        let big = pairs(&[(10.0, 0.0)]); // error = 10
        let small = pairs(&[(1.0, 0.0), (1.0, 0.0), (1.0, 0.0)]); // errors = 1,1,1
        // MAE: big=10, small=1  |  RMSE: big=10, small=1
        // But if we combine: one big + three small
        let mixed = pairs(&[(10.0, 0.0), (1.0, 0.0), (1.0, 0.0), (1.0, 0.0)]);
        let mixed_mae = mae(&mixed); // (10+1+1+1)/4 = 3.25
        let mixed_rmse = rmse(&mixed); // sqrt((100+1+1+1)/4) = sqrt(25.75) ≈ 5.07
        assert!(mixed_rmse > mixed_mae); // RMSE always >= MAE
        assert!((mixed_mae - 3.25).abs() < 1e-10);
        // Verify RMSE > MAE relationship
        assert!(rmse(&big) >= mae(&big));
        assert!(rmse(&small) >= mae(&small));
    }
}
