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

The first split manifest is
`fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json`. It locks
the Stage 0/1 task selector version, task order ref, retained fixtures,
validation split, frozen holdout split, local smoke fixtures, public-safe task
refs, scorer/verifier refs, and allowed claim state for the first Probe GEPA
campaign.

The first executable Stage 0 smoke is implemented by
`build_probe_gepa_stage0_smoke_campaign` in `crates/benchmark-cloud`. It uses
five retained Terminal-Bench fixtures, four text-bundle candidates, and twenty
metric-call records. The example runner is
`cargo run -p benchmark-cloud --example probe_gepa_stage0_smoke`.

The first Stage 1 retained-failure sprint is implemented by
`build_probe_gepa_stage1_retained_sprint`. It uses the same public split
manifest, seven retained fixtures, eight Pylon assignment refs, ten text-bundle
candidates, and 210 metric-call records. The example runner is
`cargo run -p benchmark-cloud --example probe_gepa_stage1_retained_sprint`.

The first selected SHC validation sweep is implemented by
`build_probe_gepa_validation_sweep`. It uses validation tasks only, compares the
current Probe champion, the GEPA candidate, and a baseline backend route, and
records cost, duration, verifier result, artifact availability, candidate hash,
and Probe commit for every rollout. The example runner is
`cargo run -p benchmark-cloud --example probe_gepa_validation_sweep`.

## Evidence Splits

Benchmark records use explicit split labels:

- `retained`
- `validation`
- `holdout`
- `live`

Retained and validation evidence can drive GEPA candidate selection and product
learning. They are not public benchmark claims. Holdout and live evidence still
need explicit public claim gates before projection.

Stage 0/1 manifests use `allowed_claim_state: "none"`. They are optimizer and
validation inputs, not public-score authority.

## Probe Runner Lane

`benchmark-cloud` also defines the public Probe runner adapter contract used by
the Terminal-Bench lane. It builds a normalized `probe.benchmark_assignment.v1`
JSON payload, records the Probe command invocation as `probe benchmark run
--assignment-json -`, and emits the required artifact set:

- `result.json`
- `events.jsonl`
- `metadata.json`
- `artifact_manifest.json`
- `proof_bundle.json`
- `resource_usage_receipt.json`
- `probe-run-record.json`
- `probe-closeout.json`

The fake runner path covers pass, timeout, and error outcomes. It preserves
artifact, proof, and resource records for successful and failed outcomes and
keeps Probe-selected Blueprint signature refs plus the Probe tool-menu ref in
the proof bundle. Assignment refs are public account/grant refs only; raw
credentials and private traces are rejected before invocation.

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

The Stage 0 smoke campaign can project only
`public_status.probe_gepa.measured_retained_smoke.v1`. It explicitly records no
LoRA, no model training, no public leaderboard claim, and no automatic
promotion.

The Stage 1 retained sprint can project only retained-evidence summary status.
Its selected candidate may enter `optimizer_accepted` or `rejected`, but not an
active production state.

The validation sweep can project only `validation measured only`. It must not
claim that Probe beats Terminal-Bench. A candidate may move to shadow only when
Omega and Blueprint gate refs are present.

## Verification

Run:

```sh
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```
