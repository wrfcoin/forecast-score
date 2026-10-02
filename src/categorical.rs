//! Categorical (binary event) verification metrics: POD, FAR, CSI.
//!
//! These metrics evaluate yes/no forecasts (e.g., "will it rain?") using
//! a contingency table of hits, misses, and false alarms.

use crate::{Pair, ScoreError};
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
    /// Defaults permit reading historical JSON; zero means unavailable there.
    #[serde(default)]
    pub hss: f64,
    #[serde(default)]
    pub ets: f64,
    #[serde(default)]
    pub frequency_bias: f64,
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
        hss: hss(hits, misses, false_alarms, correct_negatives),
        ets: ets(hits, misses, false_alarms, correct_negatives),
        frequency_bias: frequency_bias(hits, misses, false_alarms),
    }
}

/// Heidke Skill Score (HSS).
///
/// Measures improvement over random chance. Unlike CSI, HSS accounts for
/// correct negatives, making it well-suited for rare events (e.g., severe weather).
///
/// HSS = 2(H·CN − M·FA) / ((H+M)(M+CN) + (H+FA)(FA+CN))
///
/// Returns 0.0 when the denominator is zero (degenerate contingency table).
pub fn hss(hits: u64, misses: u64, false_alarms: u64, correct_negatives: u64) -> f64 {
    let h = hits as f64;
    let m = misses as f64;
    let fa = false_alarms as f64;
    let cn = correct_negatives as f64;
    let numer = 2.0 * (h * cn - m * fa);
    let denom = (h + m) * (m + cn) + (h + fa) * (fa + cn);
    if denom == 0.0 {
        return 0.0;
    }
    numer / denom
}

/// Equitable Threat Score (Gilbert Skill Score / ETS).
///
/// Like CSI but corrected for hits expected by chance. Widely used by NCEP
/// for precipitation verification.
///
/// hits_random = (hits + misses)(hits + false_alarms) / n
/// ETS = (hits − hits_random) / (hits + misses + false_alarms − hits_random)
///
/// Returns 0.0 when the denominator is zero.
pub fn ets(hits: u64, misses: u64, false_alarms: u64, correct_negatives: u64) -> f64 {
    let n = (hits + misses + false_alarms + correct_negatives) as f64;
    if n == 0.0 {
        return 0.0;
    }
    let h = hits as f64;
    let m = misses as f64;
    let fa = false_alarms as f64;
    let hits_random = (h + m) * (h + fa) / n;
    let denom = h + m + fa - hits_random;
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }
    (h - hits_random) / denom
}

/// Frequency Bias.
///
/// Ratio of forecast frequency to observed frequency.
/// Bias > 1 → overforecasting; Bias < 1 → underforecasting; 1.0 = unbiased.
///
/// Returns 1.0 when there are no observed events (bias undefined).
pub fn frequency_bias(hits: u64, misses: u64, false_alarms: u64) -> f64 {
    let observed_freq = (hits + misses) as f64;
    if observed_freq == 0.0 {
        return 1.0;
    }
    (hits + false_alarms) as f64 / observed_freq
}

/// One point on a ROC curve: (false positive rate, true positive rate) at a threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RocPoint {
    pub threshold: f64,
    pub false_positive_rate: f64,
    pub true_positive_rate: f64,
}

/// Compute a probability ROC curve with binary observed outcomes (0 or 1).
///
/// Only the forecast decision threshold varies; observed classes stay constant.
/// Points are sorted by descending threshold (ascending FPR and TPR), preserving
/// vertical segments for trapezoidal AUC. Include thresholds above 1 and at 0
/// to cover both endpoints. No endpoints are added implicitly.
///
/// Panics on invalid probabilities/outcomes or non-finite thresholds. For
/// continuous variables and a fallible interface, use
/// [`roc_curve_with_observed_threshold`].
pub fn roc_curve(pairs: &[Pair], thresholds: Vec<f64>) -> Vec<RocPoint> {
    let probabilities: Vec<_> = pairs.iter().map(|p| p.predicted).collect();
    let outcomes: Vec<_> = pairs.iter().map(|p| p.observed).collect();
    crate::brier_score_checked(&probabilities, &outcomes)
        .expect("roc_curve requires finite probabilities and binary outcomes");
    roc_curve_with_observed_threshold(pairs, thresholds, 0.5)
        .expect("roc_curve requires finite thresholds")
}

/// ROC for continuous forecasts with a single, constant observed event cutoff.
///
/// An event is `observed >= observed_threshold` at every forecast cutoff. This
/// supports precipitation amounts as well as probabilities with binary labels.
/// Returns one point per forecast threshold in descending threshold order.
/// Empty inputs or absent observed classes retain the zero-rate convention.
pub fn roc_curve_with_observed_threshold(
    pairs: &[Pair],
    mut thresholds: Vec<f64>,
    observed_threshold: f64,
) -> Result<Vec<RocPoint>, ScoreError> {
    if !observed_threshold.is_finite()
        || !thresholds.iter().all(|t| t.is_finite())
        || !pairs
            .iter()
            .all(|p| p.predicted.is_finite() && p.observed.is_finite())
    {
        return Err(ScoreError::InvalidInput(
            "ROC inputs and thresholds must be finite",
        ));
    }
    thresholds.sort_by(|a, b| b.total_cmp(a));
    let points: Vec<RocPoint> = thresholds
        .into_iter()
        .map(|t| {
            let mut hits = 0;
            let mut misses = 0;
            let mut false_alarms = 0;
            let mut correct_negatives = 0;
            for p in pairs {
                match (p.predicted >= t, p.observed >= observed_threshold) {
                    (true, true) => hits += 1,
                    (false, true) => misses += 1,
                    (true, false) => false_alarms += 1,
                    (false, false) => correct_negatives += 1,
                }
            }
            let tpr = pod(hits, misses);
            let total_neg = false_alarms + correct_negatives;
            let fpr = if total_neg == 0 {
                0.0
            } else {
                false_alarms as f64 / total_neg as f64
            };
            RocPoint {
                threshold: t,
                false_positive_rate: fpr,
                true_positive_rate: tpr,
            }
        })
        .collect();
    Ok(points)
}

/// Compute the Area Under the ROC Curve (AUC) via the trapezoidal rule.
///
/// Expects points sorted by ascending false_positive_rate (as returned by `roc_curve`).
/// Returns a value in [0, 1]; 0.5 = no skill, 1.0 = perfect discrimination.
pub fn auc_from_roc(points: &[RocPoint]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut area = 0.0;
    for window in points.windows(2) {
        let dx = window[1].false_positive_rate - window[0].false_positive_rate;
        let avg_y = (window[0].true_positive_rate + window[1].true_positive_rate) / 2.0;
        area += dx * avg_y;
    }
    area.abs()
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

    #[test]
    fn hss_perfect() {
        // perfect forecast: all hits and correct negatives, zero misses/false_alarms
        let score = hss(10, 0, 0, 10);
        assert!((score - 1.0).abs() < 1e-10);
    }

    #[test]
    fn hss_no_skill() {
        // random-chance level — numerator zero when H·CN == M·FA
        // 2*2 == 2*2 → HSS = 0
        let score = hss(2, 2, 2, 2);
        assert!((score).abs() < 1e-10);
    }

    #[test]
    fn hss_degenerate() {
        assert_eq!(hss(0, 0, 0, 0), 0.0);
    }

    #[test]
    fn ets_perfect() {
        // h=10,m=0,fa=0,cn=10: hits_random = 10*10/20 = 5; ETS = (10-5)/(10+0+0-5)=1
        let score = ets(10, 0, 0, 10);
        assert!((score - 1.0).abs() < 1e-10);
    }

    #[test]
    fn ets_degenerate() {
        assert_eq!(ets(0, 0, 0, 0), 0.0);
    }

    #[test]
    fn frequency_bias_unbiased() {
        // 5 hits, 5 misses, 5 false alarms → bias = 10/10 = 1
        assert!((frequency_bias(5, 5, 5) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn frequency_bias_overforecast() {
        // 5 hits, 0 misses, 5 false alarms → bias = 10/5 = 2
        assert!((frequency_bias(5, 0, 5) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn roc_and_auc_perfect() {
        // Perfect binary classifier: all positives score 1.0, all negatives 0.0
        let pairs: Vec<Pair> = (0..5)
            .map(|_| Pair::new(1.0, 1.0))
            .chain((0..5).map(|_| Pair::new(0.0, 0.0)))
            .collect();
        let thresholds = vec![0.5];
        let curve = roc_curve(&pairs, thresholds);
        assert_eq!(curve.len(), 1);
        assert!((curve[0].true_positive_rate - 1.0).abs() < 1e-10);
        assert!((curve[0].false_positive_rate - 0.0).abs() < 1e-10);
    }

    #[test]
    fn auc_from_roc_unit_square() {
        // Perfect ROC passes through (0,1): AUC should be 1.0 with anchor points
        let points = vec![
            RocPoint {
                threshold: 1.0,
                false_positive_rate: 0.0,
                true_positive_rate: 0.0,
            },
            RocPoint {
                threshold: 0.5,
                false_positive_rate: 0.0,
                true_positive_rate: 1.0,
            },
            RocPoint {
                threshold: 0.0,
                false_positive_rate: 1.0,
                true_positive_rate: 1.0,
            },
        ];
        let auc = auc_from_roc(&points);
        assert!((auc - 1.0).abs() < 1e-10);
    }

    #[test]
    fn auc_empty() {
        assert_eq!(auc_from_roc(&[]), 0.0);
        let single = vec![RocPoint {
            threshold: 0.5,
            false_positive_rate: 0.5,
            true_positive_rate: 0.5,
        }];
        assert_eq!(auc_from_roc(&single), 0.0);
    }
}
