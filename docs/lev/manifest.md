# The decision model manifest

**Status:** written for the four releases that exist, and the serving checks
read it. The documents are in
[`crates/lev/manifests/`](../../crates/lev/manifests/README.md); the schema is
`crates/lev/src/manifest.rs`.

## What it fixes

Serving an adapter took four checks in three binaries, and none of them was
keyed off a single document:

- `lev-adapter-check` read the package and confirmed the rank and the base
  signature.
- `lev-serve` pinned the package at startup and exited on a mismatch.
- The per-family admission rule read a calibration record and asked whether
  the gate had admitted it.
- The estimator and the sample count were command-line flags.

Nothing tied those together, and nothing recorded them as one artifact
identity. The cost is already on the record. A calibration map fitted against
the base model survived two adapter runs without anything noticing, which is
the fault [`calibration.md`](calibration.md) and the
[calibration directory](../../crates/lev/calibration/README.md) now describe
at length. Issue #9369 fixed it for evaluation rows by giving every row a
door identity; artifacts kept the hole, because there was no artifact
identity to record.

## The document

`openagents.lev.decision_model_manifest.v1`, one file per release:

| Field | Holds |
| --- | --- |
| `name`, `version` | The release, as `name@version`. The model id a door reports and the adapter identity a record stores. |
| `artifact` | Path, `sha256` and `sizeBytes` of `adapter_weights.bin`, `metadataSha256`, `format`, `loraRank`, and the package's own `adapterIdentifier`. Absent when the operating system ships the weights. |
| `base` | The base model signature the artifact is pinned to, the operating system build it has been checked under, and the runtime that holds it. |
| `interface` | The System One question types with their bounds, the answer fields each returns, the response extensions, and the families this model was trained and measured on. |
| `estimator` | Which estimator draws the raw signal, over how many draws, from which seed block. |
| `evalRef` | One entry per measured family: the committed calibration record, its digest, its suite and partition, the gate that judged it, its `admitted` flag, and its verdict. |

Two of those are worth their own paragraph.

**`version` is not bookkeeping.** Three adapters exist and were distinguished
only by the run directory they were written to — `lev-v1`, `lev-band`,
`lev-perm` — with `creatorDefined` empty in all three packages. A calibration
record fitted against one of them could not name which one. `lev-adapted@2`
is the name that was missing, and `GET /v1/models` now publishes it where the
adapter's filesystem path used to go. A path is machine-local; three packages
that differ only by run directory are three paths and one name.

**`interface` is checked, not quoted.** A manifest declares the contract, and
`Interface::check` compares that declaration against the contract this build
implements. A document describing an interface the code no longer serves
grants nothing, so it refuses rather than being believed.

## The admission rule

[`docs/kev/mesh-plan.md`](../kev/mesh-plan.md) drafted this schema for
decision-model artifacts and set the rule that **a row without a measured
`evalRef` does not admit.** That rule is kept here exactly:

- A family with no `evalRef` entry serves no probability.
- A family whose entry the gate refused serves no probability.
- A record in the calibration directory that the manifest does not name does
  not serve, however well it matches the runtime. Dropping a file into a
  directory is not a measurement.
- A record whose digest or verdict no longer matches the entry does not
  serve, which is the stale map caught as an artifact-level fault.
- A map fitted under a different estimator or sample count than the release
  serves does not serve either. A map fitted on eight draws describes an
  eight-draw signal.

All four adapted releases therefore admit nothing today, because every
committed map was fitted against the base model with no adapter attached.
`lev-base@1` admits `routing` and names the refusals for `severity` and
`urgency` with the gate's own words.

## The checks, keyed off it

```text
cargo run -p lev --bin lev-adapter-check -- crates/lev/manifests/lev-base-v1.json
```

reads the document and checks every claim in it: the artifact's digest and
size against the package on disk, the rank and identifier the package
declares, the manifest's pin against the package's own, the declared
interface against the contract, each calibration record against its recorded
digest and verdict, and the base signature against the device when one is
reachable. It names the field that failed and exits 1.

```text
cargo run -p lev --features serve --bin lev-serve -- \
    --manifest crates/lev/manifests/lev-base-v1.json \
    --calibration crates/lev/calibration/lev-base
```

runs the same checks before the port is bound, then takes the adapter, the
estimator, and the sample count from the document. `--adapter`, `--samples`,
and `--seed-base` are refused beside `--manifest`: one document decides, or
none does. Those flags still start a door for an unreleased run, and that
door admits nothing, because without a manifest there is no `evalRef` to
have.

`cargo test -p lev --test manifests` checks every committed manifest against
every committed record on each run, and checks the artifact when the package
is on the machine.

## Where the next two issues attach

- A **behavioral admission floor** (#9389) is a stricter reading of the same
  rule: digest, then base signature, then the isolation probe, then a
  per-family admitted record, with anything short of that serving the typed
  answer and refusing `uncalibrated`. The probe's result joins the
  calibration record in `evalRef`; the document does not need reshaping.
- **Revocation** (#9390) is a freshness window and a revoked flag on the
  release. The base-signature treadmill is a standing revocation event — an
  operating system update replaces the base and invalidates every adapter and
  every map fitted against it — and `base.signature` is the field a
  revocation would name.

Neither is built here.

## What is deliberately absent

Listing separate from release, a catalog, publishing scopes, a namespace,
upload idempotency, signatures. There is one adapter lane, one owner, and one
machine, and every one of those mechanisms exists to coordinate more than
that. See
[`../decision-models/research/2026-09-19-capability-sockets.md`](../decision-models/research/2026-09-19-capability-sockets.md)
for the review that reached this conclusion, including the measured reason
not to carry the rest of that toolchain.
