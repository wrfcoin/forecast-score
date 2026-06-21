use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// Generate pairs of (forecast, observation) for benchmarking
fn gen_pairs(n: usize) -> (Vec<f64>, Vec<f64>) {
    let forecasts: Vec<f64> = (0..n).map(|i| (i as f64 * 0.1).sin() * 10.0 + 20.0).collect();
    let observations: Vec<f64> = (0..n).map(|i| (i as f64 * 0.1).cos() * 10.0 + 20.0).collect();
    (forecasts, observations)
}

fn gen_categorical(n: usize) -> (Vec<usize>, Vec<usize>, usize) {
    let forecast: Vec<usize> = (0..n).map(|i| i % 3).collect();
    let observed: Vec<usize> = (0..n).map(|i| (i + 1) % 3).collect();
    (forecast, observed, 3)
}

fn bench_continuous(c: &mut Criterion) {
    use forecast_score::continuous;

    let mut group = c.benchmark_group("continuous");
    for n in [100usize, 1_000, 10_000] {
        let (forecasts, observations) = gen_pairs(n);

        group.bench_with_input(BenchmarkId::new("mae", n), &n, |b, _| {
            b.iter(|| continuous::mae(black_box(&forecasts), black_box(&observations)).unwrap())
        });

        group.bench_with_input(BenchmarkId::new("rmse", n), &n, |b, _| {
            b.iter(|| continuous::rmse(black_box(&forecasts), black_box(&observations)).unwrap())
        });

        group.bench_with_input(BenchmarkId::new("bias", n), &n, |b, _| {
            b.iter(|| continuous::bias(black_box(&forecasts), black_box(&observations)).unwrap())
        });

        group.bench_with_input(BenchmarkId::new("pearson_correlation", n), &n, |b, _| {
            b.iter(|| {
                continuous::pearson_correlation(black_box(&forecasts), black_box(&observations))
                    .unwrap()
            })
        });
    }
    group.finish();
}

fn bench_categorical(c: &mut Criterion) {
    use forecast_score::categorical;

    let mut group = c.benchmark_group("categorical");
    for n in [100usize, 1_000, 10_000] {
        let (forecast, observed, k) = gen_categorical(n);

        group.bench_with_input(BenchmarkId::new("accuracy", n), &n, |b, _| {
            b.iter(|| {
                categorical::accuracy(black_box(&forecast), black_box(&observed), black_box(k))
                    .unwrap()
            })
        });

        group.bench_with_input(BenchmarkId::new("heidke_skill_score", n), &n, |b, _| {
            b.iter(|| {
                categorical::heidke_skill_score(
                    black_box(&forecast),
                    black_box(&observed),
                    black_box(k),
                )
                .unwrap()
            })
        });
    }
    group.finish();
}

fn bench_probabilistic(c: &mut Criterion) {
    use forecast_score::probabilistic;

    let mut group = c.benchmark_group("probabilistic");
    for n in [100usize, 1_000, 10_000] {
        let (_, observations) = gen_pairs(n);
        // Ensemble of 10 members
        let ensemble: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..10).map(|j| (i + j) as f64 * 0.05 + 15.0).collect())
            .collect();

        group.bench_with_input(BenchmarkId::new("crps", n), &n, |b, _| {
            b.iter(|| {
                probabilistic::crps(black_box(&ensemble), black_box(&observations)).unwrap()
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_continuous, bench_categorical, bench_probabilistic);
criterion_main!(benches);
