//! Categorical (binary event) verification metrics: POD, FAR, CSI.
//!
//! These metrics evaluate yes/no forecasts (e.g., "will it rain?") using
//! a contingency table of hits, misses, and false alarms.

use crate::Pair;
use serde::{Deserialize, Serialize};

/// Probability of Detection (hit rate).
///
/// POD = hits / (hits + misses)
///
/// Returns 0.0 if there are no observed events (hits + misses = 0).
#[inline]
pub fn pod(hits: u64, misses: u64) -> f64 {
    let denom = hits + misses;
    if denom == 0 {
        return 0.0;
    }
    hits as f64 / denom as f64
}

/// False Alarm Ratio.
///
/// FAR = false_alarms / (hits + false_alarms)
///
/// Returns 0.0 if there are no forecast events (hits + false_alarms = 0).
#[inline]
pub fn far(hits: u64, false_alarms: u64) -> f64 {
    let denom = hits + false_alarms;
    if denom == 0 {
        return 0.0;
    }
    false_alarms as f64 / denom as f64
}

/// Critical Success Index (Threat Score).
///
/// CSI = hits / (hits + misses + false_alarms)
///
/// Returns 0.0 if there are no events (all three = 0).
#[inline]
pub fn csi(hits: u64, misses: u64, false_alarms: u64) -> f64 {
    let denom = hits + misses + false_alarms;
    if denom == 0 {
        return 0.0;
    }
    hits as f64 / denom as f64
}

/// Bundle of categorical scores for a single threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "bindings/"))]
pub struct CategoricalScores {
    pub threshold: f64,
    pub hits: u64,
    pub misses: u64,
    pub false_alarms: u64,
    pub correct_negatives: u64,
    pub pod: f64,
    pub far: f64,
    pub csi: f64,
}

/// Compute categorical scores for a set of continuous forecast–observation pairs
/// at a given threshold.
///
/// A "hit" occurs when both predicted >= threshold and observed >= threshold.
/// Ported from `backend/server/services/forecast/forecast-verification-service.ts` lines 409–439.
pub fn categorical_scores(pairs: &[Pair], threshold: f64) -> CategoricalScores {
    let mut hits = 0u64;
    let mut misses = 0u64;
    let mut false_alarms = 0u64;
    let mut correct_negatives = 0u64;

    for p in pairs {
        let forecast_yes = p.predicted >= threshold;
        let observed_yes = p.observed >= threshold;
        match (forecast_yes, observed_yes) {
            (true, true) => hits += 1,
            (false, true) => misses += 1,
            (true, false) => false_alarms += 1,
            (false, false) => correct_negatives += 1,
        }
    }

    CategoricalScores {
        threshold,
        hits,
        misses,
        false_alarms,
        correct_negatives,
        pod: pod(hits, misses),
        far: far(hits, false_alarms),
        csi: csi(hits, misses, false_alarms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_detection() {
        assert_eq!(pod(10, 0), 1.0);
        assert_eq!(far(10, 0), 0.0);
        assert_eq!(csi(10, 0, 0), 1.0);
    }

    #[test]
    fn no_detection() {
        assert_eq!(pod(0, 10), 0.0);
        assert_eq!(csi(0, 10, 0), 0.0);
    }

    #[test]
    fn all_false_alarms() {
        assert_eq!(far(0, 10), 1.0);
        assert_eq!(csi(0, 0, 10), 0.0);
    }

    #[test]
    fn empty_contingency() {
        assert_eq!(pod(0, 0), 0.0);
        assert_eq!(far(0, 0), 0.0);
        assert_eq!(csi(0, 0, 0), 0.0);
    }

    #[test]
    fn categorical_scores_precipitation() {
        let pairs = vec![
            Pair::new(0.5, 0.3), // both above 0.01" threshold
            Pair::new(0.0, 0.2), // miss
            Pair::new(0.3, 0.0), // false alarm
            Pair::new(0.0, 0.0), // correct negative
        ];
        let scores = categorical_scores(&pairs, 0.01);
        assert_eq!(scores.hits, 1);
        assert_eq!(scores.misses, 1);
        assert_eq!(scores.false_alarms, 1);
        assert_eq!(scores.correct_negatives, 1);
        assert!((scores.pod - 0.5).abs() < 1e-10);
        assert!((scores.far - 0.5).abs() < 1e-10);
        assert!((scores.csi - 1.0 / 3.0).abs() < 1e-10);
    }
}
