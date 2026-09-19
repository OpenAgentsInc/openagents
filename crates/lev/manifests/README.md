# Decision model manifests

One `decision_model_manifest.v1` per release, read by `lev-serve --manifest`
and checked by `lev-adapter-check <manifest.json>`.

A manifest is the document the four serving checks are keyed off. It names
the artifact and its digest, the base model the artifact is pinned to, the
System One input and output shapes, the estimator and its sample count, and
the committed calibration records each admitted family rests on. Without it,
those four checks lived in three binaries and answered to each other.

It also names the policy snapshot the release runs under, in
`policySnapshot`. Every check above is decided when the door starts;
`policySnapshot` is what a running door asks, and the window in it is what
stops a door that stops reaching the service. See
[`../policy/README.md`](../policy/README.md) and
[`../../docs/lev/revocation.md`](../../docs/lev/revocation.md).

## The releases here

| File | Release | Artifact | Admits |
| --- | --- | --- | --- |
| `lev-base-v1.json` | `lev-base@1` | none; the operating system ships the weights | `routing` |
| `lev-adapted-v1.json` | `lev-adapted@1` | `runs/lev-v1/lev.fmadapter`, the choice objective | nothing |
| `lev-adapted-v2.json` | `lev-adapted@2` | `runs/lev-band/levband.fmadapter`, choice and the certainty band | nothing |
| `lev-adapted-v3.json` | `lev-adapted@3` | `runs/lev-perm/levperm.fmadapter`, the band over three option-order permutations | nothing |

The three adapters are three versions of one decision model rather than three
models: same contract, same suite, same base, successive training runs. Before
this directory they were distinguished only by the run directory they were
written to, and `creatorDefined` is `{}` in all three packages, so a
calibration record fitted against one of them could not name which one.
`name@version` is the name it could not say, and it is what a door now
publishes as its adapter identity in `GET /v1/models` and what every row and
record carries.

## Why the adapted releases admit nothing

Every committed calibration map was fitted against the base model with no
adapter attached, so no map describes an adapted door. A family without a
measured `evalRef` does not admit — the rule `docs/kev/mesh-plan.md` set for
decision-model artifacts — so all three adapted releases serve typed answers
with uncalibrated frequencies and refuse `require_calibration`.

That is the rule working rather than a gap in these files. A map fitted on
the base sat on disk through two adapter runs that changed which families are
admitted at all, and the manifest is what makes that impossible to repeat:
the record is named, digested, and checked on every start and on every
`cargo test -p lev`.

## Every release names a policy, and it is not optional

All four point at [`../policy/current.json`](../policy/README.md), cache
their copy at `~/.lev/policy/current.json`, and accept a window of no more
than 24 hours. A release that could decline to name a policy source would
escape revocation by leaving a field out, so `Manifest::load` refuses a
document without the block and names the blank field.

The cache lives outside this repository on purpose. A cache is a client's
copy of somebody else's document, and a committed copy is one that stops
being fetched.

## The artifact is not in git

A `.fmadapter` package is 133 MB, which is why `artifact.path` is a
machine-local path and `artifact.sha256` is what decides whether the bytes
under it are the right ones. The packages live in
`~/code/lev-adapter-work/runs/`. A manifest for a package that is not on this
machine still loads, still checks its records, and reports the artifact as
unchecked.

## Writing one

Emit it rather than typing it — the digest is the field a person gets wrong:

```text
cd crates/lev/manifests
cargo run -p lev --bin lev-adapter-check -- \
    ~/code/lev-adapter-work/runs/lev-v1/lev.fmadapter \
    --emit 'lev-adapted@4' \
    --description '...' --created 2026-09-19 --os-build 25E246 \
    --families 'routing,severity,urgency' \
    --estimator l2 --samples 8 --seed-base 0 \
    --calibration ../calibration/lev-adapted \
    > lev-adapted-v4.json
```

Run it from this directory: the `evalRef` paths are written as passed and
resolve against the manifest's own location, so the document works wherever
the repository is checked out. A release with no artifact — a base model the
operating system ships — takes `--base <signature>` and no package path.
The `policySnapshot` block is written with the published defaults, and
`--policy-source`, `--policy-cache`, and `--window` change them.

Then check it:

```text
cargo run -p lev --bin lev-adapter-check -- crates/lev/manifests/lev-adapted-v4.json
```
