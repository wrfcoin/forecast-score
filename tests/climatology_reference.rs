use forecast_score::{ForecastAccuracyMetrics as Metrics, Pair, ReferenceStrategy as Reference};

#[test]
fn persistence_copy_and_perfect_future_use_the_same_window() {
    let copy = [
        Pair::new(99.0, 0.0),
        Pair::new(0.0, 4.0),
        Pair::new(4.0, 5.0),
        Pair::new(5.0, 9.0),
    ];
    assert_eq!(
        Metrics::from_pairs_with_reference(&copy, Reference::Persistence).skill_score,
        0.0
    );
    let perfect = [
        Pair::new(99.0, 0.0),
        Pair::new(4.0, 4.0),
        Pair::new(5.0, 5.0),
        Pair::new(9.0, 9.0),
    ];
    assert_eq!(
        Metrics::from_pairs_with_reference(&perfect, Reference::Persistence).skill_score,
        1.0
    );
}

#[test]
fn constant_and_short_persistence_add_no_information() {
    for data in [
        vec![Pair::new(5.0, 5.0)],
        vec![Pair::new(5.0, 5.0), Pair::new(5.0, 5.0)],
    ] {
        assert_eq!(
            Metrics::from_pairs_with_reference(&data, Reference::Persistence).skill_score,
            0.0
        );
    }
}

#[test]
fn independent_climatology_changes_skill_without_changing_continuous_metrics() {
    let data = [Pair::new(0.5, 0.0), Pair::new(0.5, 1.0)];
    let legacy = Metrics::from_pairs(&data);
    let reference = Metrics::from_pairs_with_reference(
        &data,
        Reference::FixedClimatology {
            reference_rmse: 1.0,
        },
    );
    assert_eq!(reference.skill_score, 0.5);
    assert_eq!(
        (reference.mae, reference.rmse, reference.bias),
        (legacy.mae, legacy.rmse, legacy.bias)
    );
    assert_eq!(
        serde_json::to_vec(&legacy).unwrap(),
        serde_json::to_vec(&Metrics::from_pairs_with_reference(
            &data,
            Reference::SampleStdDev
        ))
        .unwrap()
    );
}

#[test]
fn checked_scoring_rejects_invalid_baselines_history_and_overflow() {
    use forecast_score::persistence_rmse_checked;
    for observations in [
        vec![],
        vec![1.0],
        vec![f64::NAN, 1.0],
        vec![-f64::MAX, f64::MAX],
    ] {
        assert!(persistence_rmse_checked(&observations).is_err());
    }
    let data = [Pair::new(1.0, 1.0), Pair::new(2.0, 2.0)];
    for error in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(Metrics::from_pairs_with_reference_checked(
            &data,
            Reference::FixedClimatology {
                reference_rmse: error
            }
        )
        .is_err());
    }
    assert!(
        Metrics::from_pairs_with_reference_checked(&data[..1], Reference::Persistence).is_err()
    );
    for value in [f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(Metrics::from_pairs_with_reference_checked(
            &[Pair::new(value, 0.0), Pair::new(0.0, 1.0)],
            Reference::SampleStdDev
        )
        .is_err());
    }
    assert_eq!(persistence_rmse_checked(&[0.0, 3.0]).unwrap(), 3.0);
    let checked =
        Metrics::from_pairs_with_reference_checked(&data, Reference::SampleStdDev).unwrap();
    assert_eq!(
        serde_json::to_vec(&checked).unwrap(),
        serde_json::to_vec(&Metrics::from_pairs(&data)).unwrap()
    );
}

#[test]
fn published_small_forecast_window_uses_an_independent_reference() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("reference_data/climatology_window.json")).unwrap();
    let pairs: Vec<_> = fixture["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Pair::new(p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
        .collect();
    let score = Metrics::from_pairs_with_reference_checked(
        &pairs,
        Reference::FixedClimatology {
            reference_rmse: fixture["reference_rmse"].as_f64().unwrap(),
        },
    )
    .unwrap();
    assert!((score.skill_score - fixture["expected_skill"].as_f64().unwrap()).abs() < 1e-10);
    assert_eq!(
        Metrics::from_pairs_with_reference(&pairs, Reference::SampleStdDev).skill_score,
        0.0
    );
}
