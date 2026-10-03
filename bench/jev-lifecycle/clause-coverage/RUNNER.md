# Run the advisory clause review

This runner coordinates the frozen policy in [protocol.md](protocol.md). It
prepares the original twelve slots, then executes one reviewed registration.
Preparation makes no network calls. The preparer and its policy remain unchanged.

## Prepare

Use Python 3.13 or newer. Run after the original panel has ended and every
launched native run has a bound receipt confirming `execution_closed=true`.
A stopped panel can be prepared only when all launched executions have closed;
unlaunched slots remain explicit skips.

```sh
python3.13 run_coverage_review.py \
  --harness /path/to/openagents \
  --repo /path/to/source-object-repository \
  --evidence /path/to/collected-native-pilot \
  --output /path/to/new-review-output
```

The output directory must not exist. The harness contains the frozen imported
modules. The source repository supplies immutable Git blobs. The evidence
collection must preserve this layout:

```text
plan/plan.json
plan/<run-uuid>.json
inputs/<task-id>/task.json
inputs/<task-id>/index.json
runs/<run-uuid>/pilot.json
runs/<run-uuid>/native/result.json
runs/<run-uuid>/native/candidate-manifest.json
runs/<run-uuid>/native/candidate.tar.gz
runs/<run-uuid>/native/changes.json
panel/panel.json
```

Each task file uses the original `{"tasks": [one public task]}` wrapper. The
runner resolves collected files by this layout and verifies their exact hashes;
remote absolute paths in the original configs are not used for local reads.
It reads only schedule, source, candidate, and closure fields from execution
receipts. Acceptance results, private checks, model identities, and conversation
contents do not enter the requests.

Each slot gets `prepared/NN/preparation.json`. Ready slots also get the exact
serialized `request.json`; skipped or invalid slots retain their reasons.
`registration.json` binds the original schedule and ended panel, every consumed
evidence artifact, every preparation and request, the frozen preparer and import
closure, the runner and its tests, and the fixed execution limits. Retain its
SHA-256 outside this output before admitting calls. `preparing.json` records
preparation progress; an interrupted preparation is not resumable.

## Execute once

Review the registration and prepared evidence first. Supply its exact digest:

```sh
python3.13 run_coverage_review.py \
  --harness /path/to/openagents \
  --repo /path/to/source-object-repository \
  --evidence /path/to/collected-native-pilot \
  --output /path/to/existing-review-output \
  --live --registration-sha256 REVIEWED_REGISTRATION_SHA256
```

The live path validates the existing registration and artifacts. It does not
prepare requests again. `--repo` remains a required common CLI argument; live
execution uses the already materialized request bytes. The existing gateway
client obtains credentials through its existing environment contract; the
runner adds no credential flags or credential artifacts.

An exclusive `live.claim` prevents any rerun, including after a crash. There is
no resume or retry path. Do not delete the claim to repeat a call. A failed setup
retains an execution receipt when the process can still write files. An abrupt
process kill or filesystem failure can leave only the claim and last durable
checkpoint; account for that uncertainty before doing any separate experiment.

Calls follow original panel order, skipping only non-ready preparations. The
limits are twelve application calls, a 30-second socket timeout, and a
90-second outer deadline per call. New calls stop when known cost reaches
$0.05 or a call has unknown accounting. The cost limit is an observed admission
threshold: an admitted call can cross it, and that charge remains recorded.
There are no repairs, replacement cases, or modifications to the native panel.

The runner verifies the imported module closure and exact request again before
each call. It retains launch intent, the gateway request/response/receipt, and a
scrubbed per-call outcome under `calls/NN/`. Interrupted calls recover any known
charge and attempt count from the gateway checkpoint. Missing accounting stays
unknown; an absent response is not a zero-cost observation.

`execution.json` retains all twelve slots, total observed known cost, whether
accounting and attempt counts are complete, stop reason, and call-phase elapsed
time. Preparation time stays in the registration and each preparation. Exact
source omissions and typed answers remain in their respective artifacts. No
exception message, response header, authorization value, or raw environment is
copied into orchestration errors; those record exception types only.

## Verify without calls

From the harness worktree, run:

```sh
python3.13 -m unittest discover \
  -s /path/to/clause-coverage -p 'test_*coverage_review.py' -v
```

The seven preparer tests and twelve runner tests use synthetic sources and
injected gateway responses. They cover complete evidence, mandatory overflow,
privacy, twelve-slot retention, closure and identity barriers, cost stops,
unknown charges, interrupted receipts, zero-call setup failures, and refusal to
resume. These tests make no model calls and do not inspect actual pilot patches.

Interpret the answers using the original protocol: advisory requirement
coverage is not executable acceptance, and a primary-check pass is not a
complete correctness label.
