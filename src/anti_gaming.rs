//! Anti-gaming helpers for reward-sensitive forecast scoring.

use serde::{Deserialize, Serialize};

use crate::{reliability::SkillComponents, ForecastAccuracyMetrics, Pair, ScoreError};

/// Tuning parameters for anti-gaming checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "bindings/"))]
pub struct ScoringConfig {
    pub min_sample_size: usize,
    pub persistence_threshold: f64,
    pub min_lead_time_secs: u64,
    pub late_submission_penalty: f64,
    pub sharpness_floor: f64,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            min_sample_size: 30,
            persistence_threshold: 0.1,
            min_lead_time_secs: 3_600,
            late_submission_penalty: 0.0,
            sharpness_floor: 0.05,
        }
    }
}

/// Rich scoring output that preserves the legacy metrics shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "bindings/"))]
pub struct AntiGamingScore {
    pub metrics: ForecastAccuracyMetrics,
    pub components: Option<SkillComponents>,
    pub persistence_detected: bool,
    pub sample_size_ok: bool,
    pub lead_time_ok: bool,
    pub effective_skill_score: f64,
}

fn validate_config(cfg: &ScoringConfig) -> Result<(), ScoreError> {
    if !cfg.persistence_threshold.is_finite() || cfg.persistence_threshold < 0.0 {
        return Err(ScoreError::InvalidInput(
            "persistence_threshold must be finite and non-negative",
        ));
    }
    if !cfg.late_submission_penalty.is_finite()
        || !(0.0..=1.0).contains(&cfg.late_submission_penalty)
    {
        return Err(ScoreError::InvalidInput(
            "late_submission_penalty must be finite and in [0, 1]",
        ));
    }
    if !cfg.sharpness_floor.is_finite() || cfg.sharpness_floor < 0.0 {
        return Err(ScoreError::InvalidInput(
            "sharpness_floor must be finite and non-negative",
        ));
    }
    Ok(())
}

fn require_sample_size(len: usize, cfg: &ScoringConfig) -> Result<(), ScoreError> {
    if len < cfg.min_sample_size {
        return Err(ScoreError::InvalidInput("insufficient sample size"));
    }
    Ok(())
}

fn skill_from_reference(forecast_rmse: f64, reference_rmse: f64) -> f64 {
    if reference_rmse > 0.0 {
        (1.0 - forecast_rmse / reference_rmse).clamp(0.0, 1.0)
    } else {
        // A perfect baseline cannot be beaten; even a perfect forecast is a tie.
        0.0
    }
}

fn pairs_from_baseline(observed: &[f64], baseline: &[f64]) -> Vec<Pair> {
    observed
        .iter()
        .zip(baseline.iter())
        .map(|(&obs, &pred)| Pair::new(pred, obs))
        .collect()
}

/// Validate that a submission has enough lead time before the valid timestamp.
pub fn validate_lead_time(submitted_at_unix: u64, valid_at_unix: u64, cfg: &ScoringConfig) -> bool {
    valid_at_unix
        .saturating_sub(submitted_at_unix)
        .ge(&cfg.min_lead_time_secs)
}

/// Score a basic set of forecast/observation pairs with sample-size enforcement.
pub fn score_pairs_with_config(
    pairs: &[Pair],
    cfg: &ScoringConfig,
) -> Result<AntiGamingScore, ScoreError> {
    validate_config(cfg)?;
    require_sample_size(pairs.len(), cfg)?;

    let metrics = ForecastAccuracyMetrics::from_pairs(pairs);
    Ok(AntiGamingScore {
        effective_skill_score: metrics.skill_score,
        metrics,
        components: None,
        persistence_detected: false,
        sample_size_ok: true,
        lead_time_ok: true,
    })
}

/// Score pairs while enforcing submission lead time.
pub fn score_pairs_with_metadata(
    pairs: &[Pair],
    submitted_at_unix: u64,
    valid_at_unix: u64,
    cfg: &ScoringConfig,
) -> Result<AntiGamingScore, ScoreError> {
    let mut score = score_pairs_with_config(pairs, cfg)?;
    if !validate_lead_time(submitted_at_unix, valid_at_unix, cfg) {
        score.lead_time_ok = false;
        score.effective_skill_score = cfg.late_submission_penalty;
    }
    Ok(score)
}

/// Score a forecast against persistence and climatological baselines.
///
/// Effective skill measures improvement over the better baseline. A zero-error
/// baseline gives zero effective skill, including a perfect forecast tie. The
/// legacy diagnostic metrics retain their sample-relative skill definition.
pub fn score_against_baselines(
    forecast: &[Pair],
    persistence: &[f64],
    climatology: &[f64],
    cfg: &ScoringConfig,
) -> Result<AntiGamingScore, ScoreError> {
    validate_config(cfg)?;
    require_sample_size(forecast.len(), cfg)?;

    if forecast.len() != persistence.len() {
        return Err(ScoreError::LengthMismatch {
            expected: forecast.len(),
            got: persistence.len(),
        });
    }
    if forecast.len() != climatology.len() {
        return Err(ScoreError::LengthMismatch {
            expected: forecast.len(),
            got: climatology.len(),
        });
    }

    let metrics = ForecastAccuracyMetrics::from_pairs(forecast);
    let observed: Vec<f64> = forecast.iter().map(|pair| pair.observed).collect();
    let persistence_pairs = pairs_from_baseline(&observed, persistence);
    let climatology_pairs = pairs_from_baseline(&observed, climatology);
    let persistence_metrics = ForecastAccuracyMetrics::from_pairs(&persistence_pairs);
    let climatology_metrics = ForecastAccuracyMetrics::from_pairs(&climatology_pairs);
    let baseline_rmse = persistence_metrics.rmse.min(climatology_metrics.rmse);

    let persistence_detected =
        forecast
            .iter()
            .zip(persistence.iter())
            .all(|(pair, persistence_value)| {
                (pair.predicted - persistence_value).abs() <= cfg.persistence_threshold
            });

    Ok(AntiGamingScore {
        effective_skill_score: skill_from_reference(metrics.rmse, baseline_rmse),
        metrics,
        components: None,
        persistence_detected,
        sample_size_ok: true,
        lead_time_ok: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(data: &[(f64, f64)]) -> Vec<Pair> {
        data.iter()
            .map(|&(predicted, observed)| Pair::new(predicted, observed))
            .collect()
    }

    #[test]
    fn scoring_config_defaults_match_plan() {
        let cfg = ScoringConfig::default();
        assert_eq!(cfg.min_sample_size, 30);
        assert_eq!(cfg.persistence_threshold, 0.1);
        assert_eq!(cfg.min_lead_time_secs, 3_600);
        assert_eq!(cfg.late_submission_penalty, 0.0);
        assert_eq!(cfg.sharpness_floor, 0.05);
    }

    #[test]
    fn score_pairs_rejects_insufficient_sample_size() {
        let cfg = ScoringConfig::default();
        let err = score_pairs_with_config(&pairs(&[(1.0, 1.0)]), &cfg).unwrap_err();
        assert_eq!(err, ScoreError::InvalidInput("insufficient sample size"));
    }

    #[test]
    fn lead_time_validation_accepts_early_submission() {
        let cfg = ScoringConfig::default();
        assert!(validate_lead_time(100, 4_000, &cfg));
        assert!(!validate_lead_time(3_800, 4_000, &cfg));
    }

    #[test]
    fn score_pairs_with_metadata_penalizes_late_submission() {
        let cfg = ScoringConfig {
            min_sample_size: 2,
            late_submission_penalty: 0.15,
            ..ScoringConfig::default()
        };
        let score =
            score_pairs_with_metadata(&pairs(&[(1.0, 1.0), (2.0, 2.0)]), 900, 1_000, &cfg).unwrap();
        assert!(!score.lead_time_ok);
        assert_eq!(score.effective_skill_score, 0.15);
    }

    #[test]
    fn score_against_baselines_detects_persistence_and_zeroes_skill_when_not_better() {
        let cfg = ScoringConfig {
            min_sample_size: 3,
            persistence_threshold: 0.05,
            ..ScoringConfig::default()
        };
        let forecast = pairs(&[(10.0, 10.0), (12.0, 13.0), (14.0, 12.0)]);
        let persistence = [10.0, 12.0, 14.0];
        let climatology = [11.0, 11.0, 11.0];
        let score = score_against_baselines(&forecast, &persistence, &climatology, &cfg).unwrap();
        assert!(score.persistence_detected);
        assert_eq!(score.effective_skill_score, 0.0);
    }

    #[test]
    fn score_against_baselines_rewards_forecast_that_beats_both_baselines() {
        let cfg = ScoringConfig {
            min_sample_size: 3,
            ..ScoringConfig::default()
        };
        let forecast = pairs(&[(10.0, 10.0), (12.0, 12.1), (13.8, 14.0)]);
        let persistence = [10.0, 15.0, 16.0];
        let climatology = [8.0, 8.0, 8.0];
        let score = score_against_baselines(&forecast, &persistence, &climatology, &cfg).unwrap();
        assert!(!score.persistence_detected);
        assert!(score.effective_skill_score > 0.0);
        assert!(score.effective_skill_score <= 1.0);
    }
}
