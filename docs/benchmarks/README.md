# benchmark-cloud contracts

Date: 2026-06-08

This folder documents the public `benchmark-cloud` contract layer now owned by
the `openagents` monorepo. The private Cloud repo may remain source material,
but public benchmark manifests, split definitions, artifact/proof records, and
Probe closeout imports should land here.

## Contract Package

The Rust contract crate is `crates/benchmark-cloud`.

It defines:

- `BenchmarkTask`
- `BenchmarkResult`
- `BenchmarkEvent`
- `BenchmarkArtifactManifest`
- `BenchmarkProofBundle`
- `openagents.resource_usage_receipt.v1`
- `BenchmarkSplitManifest`
- `BenchmarkRunManifest`
- `ScorerVerifierRef`
- `NoCheatMetadata`
- `BenchmarkRedactionState`

The first fixture is
`fixtures/benchmarks/terminal_bench_probe_contract_smoke.json`. It proves the
contract can represent a Terminal-Bench 2 through Harbor retained result and
import Probe closeout refs without committing task text, hidden verifier
content, private repo refs, raw logs, wallet/payment material, or private
Harbor traces.

## Evidence Splits

Benchmark records use explicit split labels:

- `retained`
- `validation`
- `holdout`
- `live`

Retained and validation evidence can drive GEPA candidate selection and product
learning. They are not public benchmark claims. Holdout and live evidence still
need explicit public claim gates before projection.

## Failure Records

Failed, timed-out, policy-blocked, and errored runs must still carry artifact
manifest refs and proof bundle refs. They also require failure classification.
If resource usage is unavailable, the result must carry an explicit
`resource_unavailable_reason`.

## Public Claim Boundary

`BenchmarkResult` can record public claim metadata, but it cannot upgrade a
claim on its own. The `public_claim_upgrade_authority` flag must remain false.
Any non-`none` public claim level requires `live` evidence and at least one
external release gate ref. Retained, validation, and holdout records must not
claim that Probe beats Terminal-Bench.

## Verification

Run:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```
