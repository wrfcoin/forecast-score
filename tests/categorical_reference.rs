use forecast_score::{auc_from_roc, categorical_scores, ets, frequency_bias, hss, roc_curve, Pair};

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn roc_keeps_observed_classes_constant_at_zero_probability_threshold() {
    let pairs = [Pair::new(0.0, 0.0), Pair::new(1.0, 1.0)];
    let curve = roc_curve(&pairs, vec![0.0, 0.5, 1.1]);
    assert_eq!(curve.len(), 3);
    near(curve[2].false_positive_rate, 1.0);
    near(auc_from_roc(&curve), 1.0);
}

#[test]
fn random_discriminator_has_half_auc() {
    let pairs = [Pair::new(0.5, 0.0), Pair::new(0.5, 1.0)];
    near(auc_from_roc(&roc_curve(&pairs, vec![0.0, 0.5, 1.1])), 0.5);
}

#[test]
fn tampere_published_roc_rows_and_auc() {
    // Ebert 2007 ECMWF workshop, slides 10 and 24; only 11 aggregate rows.
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("reference_data/tampere_roc.json")).unwrap();
    let mut pairs = Vec::new();
    for row in fixture["rows"].as_array().unwrap() {
        let count = row[1].as_u64().unwrap();
        let events = row[2].as_u64().unwrap();
        for i in 0..count {
            pairs.push(Pair::new(
                row[0].as_f64().unwrap(),
                if i < events { 1.0 } else { 0.0 },
            ));
        }
    }
    let thresholds = (0..=11).map(|i| i as f64 / 10.0).collect();
    let curve = roc_curve(&pairs, thresholds);
    for (i, row) in fixture["contingency"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let hits = row[0].as_u64().unwrap();
        let misses = row[1].as_u64().unwrap();
        let fa = row[2].as_u64().unwrap();
        let cn = row[3].as_u64().unwrap();
        let point = curve
            .iter()
            .find(|p| p.threshold == i as f64 / 10.0)
            .unwrap();
        near(
            point.true_positive_rate,
            hits as f64 / (hits + misses) as f64,
        );
        near(point.false_positive_rate, fa as f64 / (fa + cn) as f64);
    }
    near(
        auc_from_roc(&curve),
        fixture["exact_auc_from_table"].as_f64().unwrap(),
    );
    assert!((auc_from_roc(&curve) - fixture["published_auc"].as_f64().unwrap()).abs() < 0.005);
    near(hss(65, 16, 61, 204), 0.4797500488185901);
    near(ets(65, 16, 61, 204), 0.31557313877613935);
    near(frequency_bias(65, 16, 61), 1.5555555555555556);
    for w in curve.windows(2) {
        assert!(w[0].threshold >= w[1].threshold);
        assert!(w[0].false_positive_rate <= w[1].false_positive_rate);
        assert!(w[0].true_positive_rate <= w[1].true_positive_rate);
    }
}

#[test]
fn categorical_serialization_exposes_skill_metrics() {
    let pairs = [Pair::new(1.0, 1.0), Pair::new(0.0, 0.0)];
    let value = serde_json::to_value(categorical_scores(&pairs, 0.5)).unwrap();
    for key in ["hss", "ets", "frequency_bias"] {
        assert_eq!(value[key].as_f64(), Some(1.0));
    }
    let historical = r#"{"threshold":0.5,"hits":1,"misses":0,"false_alarms":0,"correct_negatives":1,"pod":1.0,"far":0.0,"csi":1.0}"#;
    let decoded: forecast_score::CategoricalScores = serde_json::from_str(historical).unwrap();
    let roundtrip = serde_json::to_value(decoded).unwrap();
    assert_eq!(roundtrip["hss"].as_f64(), Some(0.0));
}

#[test]
fn continuous_roc_uses_one_observed_event_cutoff() {
    let pairs = [Pair::new(0.0, 0.1), Pair::new(2.0, 1.1)];
    let curve = forecast_score::roc_curve_with_observed_threshold(&pairs, vec![0.0, 1.0, 3.0], 1.0)
        .unwrap();
    near(auc_from_roc(&curve), 1.0);
    for bad in [f64::NAN, f64::INFINITY] {
        assert!(forecast_score::roc_curve_with_observed_threshold(&pairs, vec![bad], 1.0).is_err());
        assert!(forecast_score::roc_curve_with_observed_threshold(&pairs, vec![1.0], bad).is_err());
        assert!(forecast_score::roc_curve_with_observed_threshold(
            &[Pair::new(bad, 0.0)],
            vec![1.0],
            1.0
        )
        .is_err());
    }
}

#[test]
fn degenerate_roc_rates_are_finite_and_use_zero_for_absent_classes() {
    for observed in [0.0, 1.0] {
        let curve = roc_curve(&[Pair::new(0.5, observed)], vec![0.0, 1.1]);
        assert!(curve
            .iter()
            .all(|p| p.true_positive_rate.is_finite() && p.false_positive_rate.is_finite()));
        near(auc_from_roc(&curve), 0.0);
    }
    assert_eq!(roc_curve(&[], vec![]), vec![]);
}
