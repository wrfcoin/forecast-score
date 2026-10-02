mod reference_support;

use forecast_score::{
    bias, brier_score, categorical_scores, crps, crps_ensemble, mae, rank_histogram, rmse,
    spread_skill_ratio,
};

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn published_probability_tables_have_known_scalar_and_contingency_answers() {
    for (_, data) in reference_support::probabilities() {
        let pairs = data.pairs();
        assert_eq!(pairs.len(), 346);
        let forecasts: Vec<_> = pairs.iter().map(|p| p.predicted).collect();
        let observations: Vec<_> = pairs.iter().map(|p| p.observed).collect();
        for (key, actual) in [
            ("mae", mae(&pairs)),
            ("rmse", rmse(&pairs)),
            ("bias", bias(&pairs)),
            ("brier", brier_score(&forecasts, &observations)),
        ] {
            near(actual, data.expected[key].as_f64().unwrap());
        }
        near(
            crps(&forecasts, &observations),
            data.expected["mae"].as_f64().unwrap(),
        );
        let scores = categorical_scores(&pairs, 0.5);
        for (actual, expected) in [
            scores.hits,
            scores.misses,
            scores.false_alarms,
            scores.correct_negatives,
        ]
        .into_iter()
        .zip(data.expected["contingency"].as_array().unwrap())
        {
            assert_eq!(actual, expected.as_u64().unwrap());
        }
    }
}

#[test]
fn analytic_ensembles_match_independent_piecewise_cdf_integrals() {
    let data = reference_support::analytic();
    let pairs = data.pairs();
    for (key, actual) in [
        ("mae", mae(&pairs)),
        ("rmse", rmse(&pairs)),
        ("bias", bias(&pairs)),
        ("crps_ensemble", crps_ensemble(&data.ensembles).unwrap()),
        ("spread_skill", spread_skill_ratio(&data.ensembles).unwrap()),
    ] {
        near(actual, data.expected[key].as_f64().unwrap());
    }
    let expected: Vec<u64> = serde_json::from_value(data.expected["ranks"].clone()).unwrap();
    assert_eq!(rank_histogram(&data.ensembles).unwrap(), expected);
    // Independent CDF integration, not the pairwise energy formula in production.
    for f in &data.ensembles {
        let mut knots = f.members.clone();
        knots.push(f.observed);
        knots.sort_by(f64::total_cmp);
        let integral: f64 = knots
            .windows(2)
            .map(|w| {
                let midpoint = (w[0] + w[1]) / 2.0;
                let cdf = f.members.iter().filter(|&&m| m <= midpoint).count() as f64
                    / f.members.len() as f64;
                let observed = if midpoint >= f.observed { 1.0 } else { 0.0 };
                (w[1] - w[0]) * (cdf - observed).powi(2)
            })
            .sum();
        near(crps_ensemble(std::slice::from_ref(f)).unwrap(), integral);
    }
}

#[test]
fn ensemble_direction_and_spread_known_answers() {
    use forecast_score::EnsembleForecast;
    let cases =
        [1.0, 1.0, 3.0].map(|observed| EnsembleForecast::new(vec![2.0, 4.0, 6.0], observed));
    assert_eq!(rank_histogram(&cases).unwrap(), vec![2, 1, 0, 0]);
    let equal_spread_and_error = [EnsembleForecast::new(vec![-1.0, 1.0], 1.0)];
    near(spread_skill_ratio(&equal_spread_and_error).unwrap(), 1.0);
    let singleton = [EnsembleForecast::new(vec![4.0], 1.0)];
    near(crps_ensemble(&singleton).unwrap(), 3.0);
}

#[test]
fn all_ensemble_metrics_reject_invalid_cases() {
    use forecast_score::EnsembleForecast;
    for cases in [
        vec![],
        vec![EnsembleForecast::new(vec![], 0.0)],
        vec![EnsembleForecast::new(vec![f64::NAN], 0.0)],
        vec![EnsembleForecast::new(vec![0.0], f64::INFINITY)],
    ] {
        assert!(crps_ensemble(&cases).is_err());
        assert!(spread_skill_ratio(&cases).is_err());
        assert!(rank_histogram(&cases).is_err());
    }
}
