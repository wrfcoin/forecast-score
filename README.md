# forecast-score

> **Non-authoritative for WRFCoin consensus.** B3.1 compiles the separate
> `wrfcoin-forecast-score` path crate inside core4. A merge here does not change
> node scoring or rewards. See the [pinned authority and drift note](docs/core4-scoring-authority.md)
> before interpreting standalone anti-gaming results as chain behavior.

Forecast verification metrics for meteorological and probabilistic predictions.

`forecast-score` provides the standard verification scores used in numerical weather prediction
(NWP), prediction markets, and other forecasting domains. Ported from WRFCoin's TypeScript
implementation (~750 LOC) into a dependency-free Rust crate.

**Minimal dependencies** -- only `serde` for serialization. No runtime, no async.

## Metric categories

| Category | Functions | Use case |
|----------|-----------|----------|
| **Continuous** | `mae`, `rmse`, `bias` | Temperature, wind speed, pressure -- any numeric forecast |
| **Categorical** | `pod`, `far`, `csi`, `categorical_scores` | Binary events: rain/no-rain, frost/no-frost |
| **Probabilistic** | `brier_score`, `crps`, `crps_normalized`, `brier_score_checked`, `crps_checked`, `crps_normalized_checked` | Probability forecasts, ensemble predictions |
| **Composite** | `ForecastAccuracyMetrics`, `CategoricalScores` | Bundled results for scoring pipelines |

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
forecast-score = "0.1"
```

Optional `meteo-types` integration stays off by default:

```toml
[dependencies]
forecast-score = { version = "0.1", features = ["meteo-types"] }
```

### Continuous metrics

```rust
use forecast_score::{Pair, mae, rmse, bias};

let pairs = vec![
    Pair::new(12.0, 10.0),  // predicted 12, observed 10
    Pair::new(18.0, 20.0),  // predicted 18, observed 20
];

assert!((mae(&pairs) - 2.0).abs() < 1e-10);   // mean absolute error
assert!((rmse(&pairs) - 2.0).abs() < 1e-10);   // root mean squared error
assert!((bias(&pairs) - 0.0).abs() < 1e-10);   // errors cancel out
```

### Categorical metrics (contingency table)

```rust
use forecast_score::{pod, far, csi, categorical_scores, Pair};

// Direct computation from contingency counts
let hits = 10;
let misses = 2;
let false_alarms = 3;

assert!((pod(hits, misses) - 0.8333).abs() < 0.001);  // probability of detection
assert!((far(hits, false_alarms) - 0.2308).abs() < 0.001); // false alarm ratio
assert!((csi(hits, misses, false_alarms) - 0.6667).abs() < 0.001); // critical success index

// Or compute from forecast-observation pairs at a threshold
let pairs = vec![
    Pair::new(0.5, 0.3),  // both above 0.01 -> hit
    Pair::new(0.0, 0.2),  // miss
    Pair::new(0.3, 0.0),  // false alarm
    Pair::new(0.0, 0.0),  // correct negative
];
let scores = categorical_scores(&pairs, 0.01);
assert_eq!(scores.hits, 1);
assert_eq!(scores.misses, 1);
```

### Probabilistic metrics

```rust
use forecast_score::{brier_score_checked, crps_checked};

// Brier Score: measures accuracy of probability forecasts
// Perfect = 0.0, worst = 1.0
let probs = vec![0.9, 0.1, 0.8];
let outcomes = vec![1.0, 0.0, 1.0];  // binary: event happened or not
let bs = brier_score_checked(&probs, &outcomes).unwrap();  // close to 0 = good

// CRPS (deterministic approximation): reduces to MAE for deterministic forecasts
let forecasts = vec![10.0, 20.0, 30.0];
let observed = vec![10.0, 20.0, 30.0];
assert!((crps_checked(&forecasts, &observed).unwrap()).abs() < 1e-10);  // perfect
```

### Composite skill score

```rust
use forecast_score::{Pair, ForecastAccuracyMetrics};

let pairs = vec![
    Pair::new(22.0, 20.0),
    Pair::new(18.0, 20.0),
    Pair::new(20.5, 20.0),
];

let metrics = ForecastAccuracyMetrics::from_pairs(&pairs);
assert!(metrics.validate().is_ok());
println!("MAE={:.2}, RMSE={:.2}, Bias={:.2}, Skill={:.3}",
    metrics.mae, metrics.rmse, metrics.bias, metrics.skill_score);
```

### Anti-gaming scoring

```rust
use forecast_score::{score_against_baselines, Pair, ScoringConfig};

let forecast = vec![
    Pair::new(20.0, 19.5),
    Pair::new(21.0, 21.2),
    Pair::new(19.8, 20.0),
];
let persistence = vec![19.5, 19.5, 21.2];
let climatology = vec![18.0, 18.0, 18.0];

let score = score_against_baselines(&forecast, &persistence, &climatology, &ScoringConfig {
    min_sample_size: 3,
    ..ScoringConfig::default()
}).unwrap();

assert!(score.effective_skill_score >= 0.0);
```

### Probabilistic diagnostics

```rust
use forecast_score::{brier_skill_score_checked, decompose_brier_score};

let probs = vec![0.2, 0.8, 0.7, 0.1];
let outcomes = vec![0.0, 1.0, 1.0, 0.0];

let bss = brier_skill_score_checked(&probs, &outcomes, 0.5).unwrap();
let components = decompose_brier_score(&probs, &outcomes, 10).unwrap();

assert!(bss >= 0.0);
assert!(components.sharpness >= 0.0);
```

### `meteo-types` weather scoring

```rust
#[cfg(feature = "meteo-types")]
{
    use forecast_score::score_weather_observations;
    use meteo_types::WeatherObservation;

    let forecast = vec![WeatherObservation {
        temperature_c: Some(20.0),
        ..WeatherObservation::default()
    }];
    let observed = vec![WeatherObservation {
        temperature_c: Some(19.5),
        ..WeatherObservation::default()
    }];

    let scores = score_weather_observations(&forecast, &observed).unwrap();
    assert!(scores.temperature.is_some());
}
```

## Metric reference

### Continuous

| Metric | Formula | Interpretation |
|--------|---------|---------------|
| MAE | `(1/n) * sum(\|predicted - observed\|)` | Average magnitude of errors. Always >= 0. |
| RMSE | `sqrt((1/n) * sum((predicted - observed)^2))` | Penalizes large errors more than MAE. Always >= MAE. |
| Bias | `(1/n) * sum(predicted - observed)` | Positive = over-prediction, negative = under-prediction. |

### Categorical

| Metric | Formula | Range |
|--------|---------|-------|
| POD (Probability of Detection) | `hits / (hits + misses)` | [0, 1], 1 = perfect |
| FAR (False Alarm Ratio) | `false_alarms / (hits + false_alarms)` | [0, 1], 0 = perfect |
| CSI (Critical Success Index) | `hits / (hits + misses + false_alarms)` | [0, 1], 1 = perfect |

### Probabilistic

| Metric | Formula | Range |
|--------|---------|-------|
| Brier Score | `(1/n) * sum((probability - outcome)^2)` | [0, 1], 0 = perfect |
| CRPS (deterministic approximation) | `(1/n) * sum(\|forecast - observed\|)` | [0, +inf), 0 = perfect |

## WRFCoin context

Receipt signing and verification require the same explicit `ChainBinding`:
construct it with `ChainBinding::new(chain_id, network_id)` or
`ChainBinding::from_env()`, then use `signing_message_with(&binding)` and
`has_valid_signature_with(&binding)`. Environment construction requires both
`WRFCOIN_CHAIN_ID` and `WRFCOIN_NETWORK_ID`; unset or blank values are errors
that name the variable. For example, use `777` / `testnet` for that test chain,
or explicitly select `ChainBinding::MAINNET` for `wrfcoin-main` / `mainnet`.
Bindings are captured at construction, not reread during signing. The deprecated
`signing_message()` and `has_valid_signature()` wrappers are mainnet-only
compatibility APIs; migrate callers to the explicit methods.

This standalone crate was extracted from the [WRFCoin](https://github.com/wrfcoin) blockchain platform
and includes additional reference metrics. Its `ForecastAccuracyMetrics` shape is related to the
receipt type, but the node compiles the separate core4 crate. Standalone helpers and tests do not
establish the score used by an authorized oracle, consensus payout behavior or deployed inclusion.

See the [Rust crate audit table](https://github.com/wrfcoin/core4/blob/main/docs/plans/wrfcoin_rust_crate_audit_table.md)
for the full extraction plan and related crates (`meteo-types`, `wrf-primitives`, `reward-engine`, etc.).

Tracking issue: [wrfcoin/core4#544](https://github.com/wrfcoin/core4/issues/544)

## Error handling

Checked variants (`brier_score_checked`, `crps_checked`, `crps_normalized_checked`) return
`Result<f64, ScoreError>` with structured errors instead of panicking:

```rust
use forecast_score::{brier_score_checked, ScoreError};

match brier_score_checked(&[0.5, 0.8], &[1.0]) {
    Err(ScoreError::LengthMismatch { expected, got }) => {
        eprintln!("length mismatch: expected {expected}, got {got}");
    }
    Err(ScoreError::InvalidInput(msg)) => eprintln!("invalid input: {msg}"),
    Ok(bs) => println!("Brier Score={bs:.3}"),
}
```

## Input contracts

- `brier_score()` and `crps()` require equal-length slices and finite values; panics on violation.
- `brier_score()` additionally requires probabilities in [0, 1] and binary outcomes (0.0 or 1.0).
- `brier_skill_score()` additionally requires a climatology rate in [0, 1].
- `crps_normalized()` additionally requires a positive finite `threshold`.
- For explicit error handling instead of panics, use the `_checked` variants.

## Compatibility notes

- `ForecastAccuracyMetrics` remains `mae`, `rmse`, `bias`, and `skill_score` only.
- Anti-gaming helpers are additive APIs and do not change receipt or binding shapes.
- The `meteo-types` dependency is feature-gated so the default build remains lightweight.

## Reference tests and benchmarks

Run `cargo test --test reference_benchmarks` for the small known-answer suite
and `cargo bench --bench reference_scoring` for Criterion timings on the same
inputs. [Dataset provenance, tolerances and analytic derivations](docs/reference-benchmarks.md)
are checked in; tests do not fetch weather data. Use `-- --test` for a bench
smoke run. The optional meteo-types path dependency requires a sibling checkout.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
