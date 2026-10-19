# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Launch preparation

- Clarify reference scoring and its authority boundary with core4. ([#38](https://github.com/wrfcoin/forecast-score/pull/38))
- Document the comparison between standalone and core4 scoring implementations. ([#39](https://github.com/wrfcoin/forecast-score/pull/39))
- Add the canonical public-testnet disclaimer. ([#40](https://github.com/wrfcoin/forecast-score/pull/40))


### Added
- Anti-gaming scoring helpers: `ScoringConfig`, `AntiGamingScore`, lead-time validation, and baseline-aware scoring
- Murphy-style Brier decomposition via `decompose_brier_score()` and `SkillComponents`
- `brier_skill_score()` and `brier_skill_score_checked()` relative to climatology
- Optional `meteo-types` feature with `score_weather_observations()` and `From<SensorReferencePair> for Pair`

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
