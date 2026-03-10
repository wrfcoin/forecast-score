# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-09

### Added
- Continuous metrics: `mae`, `rmse`, `bias` (scaffold)
- Categorical metrics: `pod`, `far`, `csi`, `categorical_scores` (scaffold)
- Probabilistic metrics: `brier_score`, `crps`, `crps_normalized` (scaffold)
- `ForecastAccuracyMetrics::from_pairs()` composite scorer with skill score (scaffold)
- Checked variants for probabilistic metrics returning `Result<f64, ScoreError>` (#7)
- `ForecastAccuracyMetrics::validate()` with `#[must_use]` (#7)
- `CategoricalScores` bundle type (#7)
- `ts-rs` TypeScript binding generation behind `ts` feature flag (#9)
- `#[non_exhaustive]` on `ScoreError` for forward compatibility
- `repository` field in Cargo.toml
- LICENSE-MIT and LICENSE-APACHE files

### Breaking Changes
- `#[non_exhaustive]` added to `ScoreError` — downstream `match` arms require a wildcard
