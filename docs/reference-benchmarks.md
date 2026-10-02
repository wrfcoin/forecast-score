# Small reference suite

`cargo test --test reference_benchmarks` verifies three checked-in datasets.
`cargo bench --bench reference_scoring` times the same data, parsed before the
timed loops. `-- --test` is a smoke run rather than a performance measurement.
The inputs are under 3 KiB combined; no weather API, bulk data or station
coordinates are involved. Formula-derived answers use absolute tolerance 1e-10.

The Tampere 24h and 48h probability-count tables come from Beth Ebert's
[2007 ECMWF workshop](https://www.ecmwf.int/sites/default/files/elibrary/2007/15489-verification-probability-forecasts.pdf),
slides 10/12 (counts), 18 (rounded 24h Brier score), 24/27 (contingencies).
Each has 346 cases, encoded as 11 aggregate rows. They are the same probability
categories used by the Brier and ROC known-answer controls. Row order after
expansion is synthetic; it is not a chronological weather sequence and must
not be used for persistence or temporal inference.

Scalar expectations were calculated independently using rational arithmetic:
for each probability p, count c and event count e, absolute error sums
`(c-e)*p + e*(1-p)`, squared error sums `(c-e)*p*p + e*(1-p)*(1-p)`;
divide by total count, taking the square root for RMSE. Bias sums `c*p-e`.
The continuous metrics here describe probability-versus-binary pairs, with
dimensionless units; they are not temperature verification results. Printed
four-decimal Brier values have rounding tolerance 5e-5.

`verification_analytic.json` is explicitly synthetic, using ensemble members
[2,4,6] and observations [1,3,5,7]. Its CRPS answers follow the piecewise CDF
integral in Ebert's [2005 ECMWF ensemble workshop](https://www.ecmwf.int/sites/default/files/elibrary/2005/15865-verification-ensembles.pdf),
slide 25. The outer observations yield 19/9 and the inner ones 7/9, so the mean
is 13/9. The integration test independently integrates CDF intervals, rather
than repeating the production pairwise energy algorithm. Ensemble means are
4: MAE=2, RMSE=sqrt(5), bias=0. Population spread is sqrt(8/3), giving ratio
sqrt(8/15). Ranks are [1,1,1,1]. These are analytic consequences of published
definitions, not invented measurements or a claim of statistical calibration.

Criterion benchmarks cover MAE, RMSE, bias, raw Brier, deterministic CRPS and
categorical counts on both published tables, and ensemble CRPS plus RMSE on
the analytic cases. Performance varies by machine and load; there is no
wall-clock pass/fail cutoff. CI runs the reference tests and compiles benches.
The repository's existing optional path dependency needs meteo-types checked
out alongside forecast-score even when that feature is disabled.

Hosted CI uses `scripts/test_default_features.py`: a validated temporary Cargo
manifest omits the unused private sibling dependency, with unchanged source,
tests, benchmark targets and all other dependencies. It refuses a nonempty
default feature list or another feature referencing meteo-types. This avoids
cross-repository credentials. This job proves the default build only; optional
meteo-types integration requires the real sibling and its own feature gate.
Local evidence also uses the original manifest with a real sibling checkout.
