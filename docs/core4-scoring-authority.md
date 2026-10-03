# Scoring authority at the B3.1 source cut

Refs [infra#1238](https://github.com/wrfcoin/infra/issues/1238),
[core4#3244](https://github.com/wrfcoin/core4/issues/3244).

This is a source and design map, dated 2026-10-03. It changes documentation only.
B3.1 source pin: `fe8b3191b6346ae17a5517b9ba21bdb33bab8c42`.
Standalone source pin: `c7e35c3c2f951e4178b1b7eaedc3ffe1472f7688`.
Source inclusion is distinct from observed node execution or deployment.

## Which crate the node links

At the B3.1 pin, run from a pristine local core4 worktree:

```bash
cargo metadata --offline --locked --format-version 1
```

The complete package graph contains `wrfcoin-forecast-score` 0.1.0 with
`source: null` and `manifest_path: <core4-worktree>/forecast-score/Cargo.toml`.
`wrfcoin-shared-protocol` depends on it through a nonoptional path dependency,
`../forecast-score` ([manifest line 80](https://github.com/wrfcoin/core4/blob/fe8b3191b6346ae17a5517b9ba21bdb33bab8c42/shared-protocol/Cargo.toml#L80)).
The native node uses shared-protocol; the graph has no package from this
standalone repository. Identical crate family names and versions are insufficient
provenance: the manifest path and Git pin select the implementation.

## Observed drift and reward trust boundary

| Boundary | Pinned source | Meaning |
| --- | --- | --- |
| Deployed fork's metric | [core4 forecast-score/src/lib.rs:100](https://github.com/wrfcoin/core4/blob/fe8b3191b6346ae17a5517b9ba21bdb33bab8c42/forecast-score/src/lib.rs#L100) | Diagnostic skill uses observed-window population standard deviation, with a perfect constant-window forecast returning 1.0. No anti_gaming module exists at this pin. |
| Standalone baseline helper | [src/anti_gaming.rs:72](https://github.com/wrfcoin/forecast-score/blob/c7e35c3c2f951e4178b1b7eaedc3ffe1472f7688/src/anti_gaming.rs#L72), [line 163](https://github.com/wrfcoin/forecast-score/blob/c7e35c3c2f951e4178b1b7eaedc3ffe1472f7688/src/anti_gaming.rs#L163) | Effective skill measures improvement over the better persistence/climatology baseline. A zero-error baseline earns zero effective skill even for a perfect tie. Legacy diagnostic metrics retain their definition. |
| Valid signed receipt | [core4 proof_of_publishing.rs:5283](https://github.com/wrfcoin/core4/blob/fe8b3191b6346ae17a5517b9ba21bdb33bab8c42/shared-protocol/src/consensus/proof_of_publishing.rs#L5283) | Reward derivation validates receipt, signature, oracle authority, manifest digest and commit ID, then consumes receipt.metrics.skill_score and truncates its clamped value to basis points. It does not recompute persistence/climatology observations from the receipt. |
| Missing/invalid receipt | [core4 proof_of_publishing.rs:5312](https://github.com/wrfcoin/core4/blob/fe8b3191b6346ae17a5517b9ba21bdb33bab8c42/shared-protocol/src/consensus/proof_of_publishing.rs#L5312) | A configured forecast verifier can supply an outcome; otherwise the baseline fallback applies. This does not import standalone anti-gaming helpers. |
| Payout use | [core4 proof_of_publishing.rs:5333](https://github.com/wrfcoin/core4/blob/fe8b3191b6346ae17a5517b9ba21bdb33bab8c42/shared-protocol/src/consensus/proof_of_publishing.rs#L5333) | Forecast skill feeds reward calculation; changing consumed values requires payout-rule review. |

A library port alone is inert. An oracle/tool changing the signed score can
change rewards even while consensus still trusts the authorized receipt.
Neither a standalone `[Testnet]` label nor its passing tests proves protection
in B3.1. There is no new runtime scoring claim in this note.

## Safe owner-approved route

The 2026-10-02 19:55Z owner ruling selected option A for core4#3244:
minimal forecast-prototype plumbing with a **NEW UNSCHEDULED**
`forecast_skill_baseline` feature, dormant until separately reviewed.
[SOL-H's core4#3435](https://github.com/wrfcoin/core4/pull/3435) implements that
separate work. At this audit its head was
`bd881eea88d5ef0acf453651fe53220951df3494`, still open; it is not the B3.1 pin.
Its proposed strategy is per-variable persistence-relative RMSE, rather than
this repository's better-of-two baseline helper. Do not infer full helper parity.

SOL-P stops before any core4 code port. Retain legacy feature-off scores,
wire bytes, manifest/receipt schemas and replay roots; require independent
copy/tie/improvement controls and signed caller evidence in the owning story.
The proposed caller strategy alone does not enforce an honest oracle's claimed
baseline on every node. Node-side enforcement needs a separate design,
authorization and activation review.

Owner question for a future release: retain the approved scorer-only dormant
scope (recommended for the current tranche), or commission node-side evidence
and baseline enforcement as a separately scheduled consensus story? Option A
is already authorized; this note neither repeats an approval request nor chooses
an activation height. No consensus source, feature schedule or node is changed.
