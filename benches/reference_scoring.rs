#[path = "../tests/reference_support/mod.rs"]
mod reference_support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use forecast_score::{bias, brier_score, categorical_scores, crps, crps_ensemble, mae, rmse};

fn scoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("reference_scoring");
    for (name, data) in reference_support::probabilities() {
        let pairs = data.pairs();
        let forecasts: Vec<_> = pairs.iter().map(|p| p.predicted).collect();
        let observations: Vec<_> = pairs.iter().map(|p| p.observed).collect();
        assert!(data.expected["brier"].as_f64().unwrap().is_finite());
        for (metric, score) in [
            ("mae", mae as fn(&[_]) -> f64),
            ("rmse", rmse),
            ("bias", bias),
        ] {
            group.bench_function(BenchmarkId::new(metric, name), |b| {
                b.iter(|| score(black_box(&pairs)))
            });
        }
        group.bench_function(BenchmarkId::new("brier", name), |b| {
            b.iter(|| brier_score(black_box(&forecasts), black_box(&observations)))
        });
        group.bench_function(BenchmarkId::new("crps", name), |b| {
            b.iter(|| crps(black_box(&forecasts), black_box(&observations)))
        });
        group.bench_function(BenchmarkId::new("categorical", name), |b| {
            b.iter(|| categorical_scores(black_box(&pairs), black_box(0.5)))
        });
    }
    let data = reference_support::analytic();
    let pairs = data.pairs();
    assert!(data.expected["crps_ensemble"].as_f64().unwrap().is_finite());
    group.bench_function("ensemble_crps/analytic", |b| {
        b.iter(|| crps_ensemble(black_box(&data.ensembles)).unwrap())
    });
    group.bench_function("rmse/analytic", |b| b.iter(|| rmse(black_box(&pairs))));
    group.finish();
}

criterion_group!(benches, scoring);
criterion_main!(benches);
