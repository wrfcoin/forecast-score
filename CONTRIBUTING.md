# Contributing

Read the [README](README.md) for this repository's purpose and technical details.
Follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Build and test

Install stable Rust and Python 3. CI validates the default-feature build with:

```sh
python3 -m unittest discover -s scripts/tests
python3 scripts/test_default_features.py
```

The helper runs Cargo tests and compiles the Criterion targets. The optional meteo-types feature needs a sibling checkout; see the README.

## Branch and pull request flow

Fork the repository and create a focused branch from current `main`.
Add tests for behavior changes and run the checks above before submitting a pull request.
Describe the change, its motivation, and validation results; use `Refs #N` to link the issue.
Keep unrelated changes separate and wait for maintainer review before merging.
Report vulnerabilities through [SECURITY.md](SECURITY.md).
