use forecast_score::{score_against_baselines, ForecastAccuracyMetrics, Pair, ScoringConfig};

fn constant_forecast(predicted: f64) -> Vec<Pair> {
    // Synthetic constant window; default minimum sample size is still enforced.
    vec![Pair::new(predicted, 5.0); 30]
}

#[test]
fn perfect_persistence_tie_has_no_effective_skill() {
    let score = score_against_baselines(
        &constant_forecast(5.0),
        &[5.0; 30],
        &[7.0; 30],
        &ScoringConfig::default(),
    )
    .unwrap();
    assert!(score.persistence_detected);
    assert_eq!(score.effective_skill_score, 0.0);
    // Diagnostic legacy metrics keep their historical meaning and wire shape.
    assert_eq!(
        score.metrics,
        ForecastAccuracyMetrics::from_pairs(&constant_forecast(5.0))
    );
    assert_eq!(score.metrics.skill_score, 1.0);
}

#[test]
fn perfect_climatology_tie_has_no_effective_skill() {
    let score = score_against_baselines(
        &constant_forecast(5.0),
        &[7.0; 30],
        &[5.0; 30],
        &ScoringConfig::default(),
    )
    .unwrap();
    assert!(!score.persistence_detected);
    assert_eq!(score.effective_skill_score, 0.0);
}

#[test]
fn perfect_tie_with_both_baselines_has_no_effective_skill() {
    let score = score_against_baselines(
        &constant_forecast(5.0),
        &[5.0; 30],
        &[5.0; 30],
        &ScoringConfig::default(),
    )
    .unwrap();
    assert_eq!(score.effective_skill_score, 0.0);
}

#[test]
fn improvement_over_both_baselines_retains_known_skill() {
    // Forecast RMSE=1; persistence RMSE=2; climatology RMSE=4.
    let score = score_against_baselines(
        &constant_forecast(6.0),
        &[7.0; 30],
        &[9.0; 30],
        &ScoringConfig::default(),
    )
    .unwrap();
    assert_eq!(score.effective_skill_score, 0.5);
    let perfect = score_against_baselines(
        &constant_forecast(5.0),
        &[7.0; 30],
        &[9.0; 30],
        &ScoringConfig::default(),
    )
    .unwrap();
    assert_eq!(perfect.effective_skill_score, 1.0);
}

#[test]
fn worse_than_a_perfect_baseline_has_no_effective_skill() {
    let score = score_against_baselines(
        &constant_forecast(6.0),
        &[5.0; 30],
        &[7.0; 30],
        &ScoringConfig::default(),
    )
    .unwrap();
    assert_eq!(score.effective_skill_score, 0.0);
}
