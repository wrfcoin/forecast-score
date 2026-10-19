# Forecast scoring: two implementations, one consensus source

Source comparison dated 2026-10-19: standalone `0c244f7f54c1cad4d434429fa0112122e2bcfaa8`; core4 `dd1801a3d4e12b53c8a3b94f234f1294f421ef3a`.
This is a source comparison and decision proposal, not runtime or payout evidence.

## Which one should I use?

For WRFCoin receipt types, node integration or chain-facing scoring, start with
[core4/forecast-score](https://github.com/wrfcoin/core4/tree/main/forecast-score).
For standalone meteorological verification and reference experiments, use this
repository. Do not substitute one for the other based on the directory name or
version. The Cargo package names are actually different.

| Property | Standalone repository | core4 path crate |
| --- | --- | --- |
| Package / version | `forecast-score` / `0.1.0` | `wrfcoin-forecast-score` / `0.1.0` |
| Rust import | `forecast_score` | `wrfcoin_forecast_score` |
| Common APIs | `Pair`, MAE/RMSE/bias, categorical and probabilistic metrics, manifest/receipt and chain-binding types | Same families, but not an API or serialization parity guarantee |
| Baseline APIs | `score_against_baselines`, `ScoringConfig`, better-of-persistence/climatology effective skill; root `ReferenceStrategy`; checked persistence helper | `anti_gaming::ReferenceStrategy`, `ForecastAccuracyMetrics::from_pairs_with_reference`, checked persistence RMSE |
| Additional reference metrics | Ensemble CRPS/rank histograms, Brier decomposition, HSS/ETS/ROC, optional weather scoring | No ensemble, reference, reliability or weather_scoring modules at this pin |
| Features | default empty; `ts`, optional sibling `meteo-types` | `ts`; workspace dependency versions |
| Test source inventory | 150 literal `#[test]` attributes across src/tests; reference datasets and Criterion bench | 82 literal `#[test]` attributes; bincode receipt/manifest regression coverage beside blockchain definitions |

Attribute counts are static inventory, not executed test counts. They exclude
doctests and any generated cases. Current test results belong to their exact
source commit and feature set, not to the other implementation.

## Consumers and verification

At the core4 pin, `shared-protocol/Cargo.toml` has the nonoptional path dependency
`wrfcoin-forecast-score = { path = "../forecast-score" }`. The native node consumes
shared-protocol. `tools/forecast-proto/Cargo.toml` also selects the core4 path crate;
its skill implementation imports `wrfcoin_forecast_score`. Core4 does not select
this standalone repository for those paths.

The standalone repository's examples, tests and benchmark consume its own
`forecast_score` library. Its optional meteo-types feature needs a sibling checkout.
External downstream consumers have not been exhaustively inventoried; registry
usage and deployment identity are UNKNOWN here. Repository metadata is not a
dependency graph. Reproduce the source comparison using the two Cargo manifests,
`src/lib.rs`, `src/anti_gaming.rs`, `src/blockchain.rs`, the standalone `src/reference.rs`,
and the core4 consumer manifests named above.

The [older B3.1 authority note](core4-scoring-authority.md) describes its pinned
historical cut, where core4 had no anti_gaming module. Current core4 has that module:
do not apply the older absence statement to current main. Baseline helpers are
scorer-side code; their presence does not prove activation, oracle honesty or payout
validation. Receipt wire compatibility and feature-off behavior must be preserved
through any future consolidation.

## Decision options

| Option | Benefit | Required work / risk |
| --- | --- | --- |
| A — retain core4 as authority for chain-facing types/scoring; standalone remains explicitly reference-only (recommended for launch) | Matches current dependency graph, avoids launch-time code movement | Maintain cross-links and a drift record; port only reviewed helpers with consumer and wire tests |
| B — make the standalone package the common dependency | One reusable implementation | Owner decision, API/serialization reconciliation, versioned release, reproducible dependency pin and full core4 integration/replay evidence before changing consumers |
| C — retire standalone after useful reference APIs are ported | Fewer duplicated implementations | Inventory consumers, port retained APIs/tests, publish migration guidance, then owner-led deprecation/archive; no silent replacement |

This note recommends A for launch; it does not authorize a canonical-package
migration or deprecation. If C is chosen later, keep existing tags and migration
instructions available, announce the successor package and API differences, and
archive only after users have a documented path. Neither repository is renamed
or archived by this documentation PR.
