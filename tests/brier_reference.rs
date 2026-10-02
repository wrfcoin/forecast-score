use forecast_score::{brier_decomposition, decompose_brier_score};

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn decomposition_preserves_raw_score_when_distinct_probabilities_share_a_bin() {
    for bins in [1, 2, 10] {
        let d = brier_decomposition(&[0.1, 0.2], &[0.0, 1.0], bins).unwrap();
        near(d.brier_score, 0.325);
        near(d.reliability - d.resolution + d.uncertainty, 0.325);
        near(d.brier_skill_score, -0.3);
        let legacy = decompose_brier_score(&[0.1, 0.2], &[0.0, 1.0], bins).unwrap();
        near(legacy.reliability, d.reliability);
        near(legacy.resolution, d.resolution);
        near(legacy.brier_skill_score, -0.3);
    }
}

#[test]
fn tampere_published_decomposition_is_independent_of_diagram_bins() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("reference_data/tampere_24h.json")).unwrap();
    let mut probabilities = Vec::new();
    let mut outcomes = Vec::new();
    for row in fixture["rows"].as_array().unwrap() {
        let count = row[1].as_u64().unwrap();
        let events = row[2].as_u64().unwrap();
        for i in 0..count {
            probabilities.push(row[0].as_f64().unwrap());
            outcomes.push(if i < events { 1.0 } else { 0.0 });
        }
    }
    assert_eq!(probabilities.len(), 346);
    for bins in [1, 2, 10, 11, 20] {
        let d = brier_decomposition(&probabilities, &outcomes, bins).unwrap();
        for (actual, key) in [
            (d.brier_score, "brier_score"),
            (d.reliability, "reliability"),
            (d.resolution, "resolution"),
            (d.uncertainty, "uncertainty"),
            (d.brier_skill_score, "brier_skill_score"),
        ] {
            // Published values are rounded to four or five decimal places.
            assert!(
                (actual - fixture["published"][key].as_f64().unwrap()).abs() < 5e-5,
                "{key}: {actual}"
            );
        }
        near(d.brier_score, d.reliability - d.resolution + d.uncertainty);
        near(d.brier_skill_score, 1.0 - d.brier_score / d.uncertainty);
        assert_eq!(d.bins.iter().map(|b| b.count).sum::<u64>(), 346);
        assert!(d.bins.len() <= bins);
    }
}

#[test]
fn diagram_keeps_equal_width_binning_while_components_use_exact_categories() {
    let d = brier_decomposition(&[0.1, 0.2], &[0.0, 1.0], 1).unwrap();
    assert_eq!(d.bins.len(), 1);
    assert_eq!(d.bins[0].count, 2);
    near(d.bins[0].mean_forecast_prob, 0.15);
    near(d.bins[0].observed_frequency, 0.5);
    near(d.reliability, 0.325);
    near(d.resolution, 0.25);
}

#[test]
fn binary_and_probability_validation_is_preserved() {
    for (p, o) in [(0.5, 0.2), (f64::NAN, 1.0), (1.1, 0.0)] {
        assert!(brier_decomposition(&[p], &[o], 10).is_err());
        assert!(decompose_brier_score(&[p], &[o], 10).is_err());
    }
}
