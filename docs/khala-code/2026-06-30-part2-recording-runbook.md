# Khala Code Part 2 Recording Runbook

Date: 2026-06-30

Status: operator runbook for the minimum part-two recording slice tracked by
#7755. This is a demo/proof path, not a public benchmark claim, product-promise
change, runtime promotion, payout path, or automatic GEPA admission.

## Goal

Record the part-two follow-up to transcript 245:

1. show Khala Code can steer the Codex Fleet;
2. show the old `0/1 available` `codex_spawn` dead-end no longer appears;
3. show the deterministic `khala.fleet.delegate` module path;
4. feed a Mutalisk candidate into the no-UI Gym bridge and show the
   admission-ready Action Submission proposal ref.

This runbook covers the minimum #7755 recording slice: #7752 deterministic
delegation smoke, #7753 token-rate status, Mutalisk #10 candidate emission, and
#7754 no-UI Gym/admission ingest.

## Preflight Smoke

Run this before recording. It uses a fake Pylon runner, so it does not touch
real Codex credentials, spend tokens, or call the network. It fails if the
desktop path regresses to the old opaque `0/1 available` dead-end.

```sh
cd /Users/christopherdavid/work/openagents
bun clients/khala-code-desktop/scripts/part2-delegation-smoke.ts
```

Expected output:

```text
Part 2 delegation smoke: PASS
assignmentRef=assignment.public.codex_agent_task.part2_demo
pylonRef=pylon.local.part2
delegate=khala.fleet.delegate status=completed
- ensure_pylon: satisfied
- advertise_capacity: satisfied
- select_account: satisfied
- prepare_work: recovered
- dispatch: satisfied
- verify_closeout: satisfied
```

If this fails, do not record the live flow. Fix the deterministic desktop seam
first.

## Live Setup

Use a clean current `main` checkout for the app and Pylon.

```sh
cd /Users/christopherdavid/work/openagents
git status --short --branch
git pull --ff-only origin main

export OPENAGENTS_REPO_ROOT="$PWD"
export OPENAGENTS_PYLON_APP_PATH="$OPENAGENTS_REPO_ROOT/apps/pylon"
export PYLON_HOME="${PYLON_HOME:-$HOME/.openagents/pylon}"
```

Do not run Codex login against the default `~/.codex` home. If accounts need to
be connected, use:

```sh
khala fleet connect
khala fleet status
```

## Start Khala Code

```sh
bun run --cwd clients/khala-code-desktop dev
```

If recording without the native window, use the preview bridge:

```sh
KHALA_CODE_DESKTOP_OPEN_WINDOW=0 \
KHALA_CODE_DESKTOP_PREVIEW_PORT=50121 \
bun clients/khala-code-desktop/src/bun/index.ts
```

## Recording Prompt

In Khala Code, first ask:

```text
what is the status of the fleet?
```

Then run the transcript-245 style smoke:

```text
Test delegating a piece of work to one Codex worker, targeting one open issue, and only do analysis. Do not change code.
```

For a fixture-only smoke, the model should call `codex_spawn` without repo pins.
For real repository work, give explicit repo, commit, and verifier pins.

## Expected Fleet Status Output

The first status prompt should call `codex_fleet_status`. For recording, the
important shape is:

```text
Pylon: online (pylon...)
Codex capacity: 4/5 available
Codex accounts: 1 total, 1 ready
Token rate: exact 42 tokens/min completed window across 3 exact row(s); active-adjusted 342 tokens/min; in-flight 600 token(s)
- codex-2: ready, slots 4/5 available, busy 1, queued 0
Active assignment markers: 1
- assignment.public... elapsed=2m00s account=account.pylon.codex... tokens=exact 600, 300 tokens/min, kind=exact
Server assignment token rows: 1
- assignment.public... elapsed=2m00s tokens=exact 600, 300 tokens/min, kind=exact
Active Codex exec processes: 1
```

If an assignment is active but exact `token_usage_events` rows have not landed
yet, the honest status is:

```text
Token rate: pending exact token rows
- assignment.public... tokens=pending exact rows
```

If no APM/proof source is available, the status should say `not_measured`. An
exact `0 tokens/min` is only recordable when the status also shows exact row
evidence, such as `across 1 exact row(s)`.

## Expected `codex_spawn` Output

The model-visible tool output should include the deterministic path before the
slot result:

```text
Khala fleet delegate: khala.fleet.delegate (completed)
- ensure_pylon: ...
- advertise_capacity: ...
- select_account: ...
- prepare_work: ...
- dispatch: ...
- verify_closeout: ...
Codex spawn: accepted 1/1 via pylon...
- slot 1 codex-...: accepted
  assignment: assignment.public...
  assignment run: completed
  closeout: accepted
  proof: ... verified tokens across ... row(s)
```

The important on-camera claim is narrow: Khala Code no longer asks the user to
manually discover Pylon/Codex capacity config. The deterministic bundle performs
the handoff and makes the module path visible.

## Mutalisk Candidate And Gym Bridge

Run the Mutalisk offline demo from a clean Mutalisk checkout:

```sh
cd /Users/christopherdavid/work/mutalisk
uv run mutalisk-optimize demo khala-fleet-delegation \
  --dataset fixtures/khala_fleet_delegation_demo.json \
  --max-metric-calls 8 \
  --emit-openagents-summary out/khala-fleet-delegation-summary.json
```

Expected shape:

```text
Mutalisk Khala fleet delegation demo: PASS
mode:                 offline_gepa_no_lm_no_network
signature:            khala.fleet.delegation
metricName:           khala.fleet.delegation
metricValueBps:       10000
candidateManifestRef: candidate_manifest.khala.fleet.delegation...
candidateRef:         candidate.khala.fleet.delegation...
candidateArtifact:    out/candidates/...
openagentsSummary:    out/khala-fleet-delegation-summary.json
OpenAgents no-UI bridge:
  bun clients/khala-code-desktop/scripts/part2-gepa-manifest-bridge.ts --summary out/khala-fleet-delegation-summary.json --out out/khala-gepa-bridge-proof.json
```

Then copy the summary JSON into the OpenAgents checkout or point the bridge at
the Mutalisk output path directly:

```sh
cd /Users/christopherdavid/work/openagents
bun clients/khala-code-desktop/scripts/part2-gepa-manifest-bridge.ts \
  --summary /Users/christopherdavid/work/mutalisk/out/khala-fleet-delegation-summary.json \
  --out out/khala-gepa-bridge-proof.json
```

Expected shape:

```text
OpenAgents Mutalisk Gym bridge: PASS
runRef=gym.run.khala_code_delegation_gepa...
jobRef=gym.job.mutalisk_khala_delegation...
stage=completed
candidateManifestRef=candidate_manifest.khala.fleet.delegation...
candidateRef=candidate.khala.fleet.delegation...
metricValueBps=10000
admissionDecision=gated_proposal_ready
decisionGrade=false
actionSubmissionProposalRef=action_submission.khala_fleet_delegation...
proof=out/khala-gepa-bridge-proof.json
```

The bridge is a no-UI backend proof. It schema-decodes Mutalisk's public-safe
`psionic.probe_gepa_candidate_manifest.v1` summary, creates the typed
`openagents.gym.mutalisk_khala_delegation_job.v0` /
`openagents.gym.mutalisk_khala_delegation_summary.v0` records in the
in-memory demo store, emits `openagents.gym.run_progress.v1` snapshots for
`queued -> running -> summary_ingested -> admission_projected -> completed` or
`blocked`, converts the summary into the
`probe-gepa-standing-optimization-loop` input, and calls
`projectKhalaFleetDelegationCandidateAdmission`. It does not import Mutalisk
Python, DSPy, or GEPA runtime code; it does not auto-promote or approve the
candidate; and `decisionGrade` remains `false` until real held-out/live evidence
satisfies the Gym gates.

## Load The Gym Pane

The Gym pane starts empty by design. For a deterministic UI smoke without a live
Mutalisk run, use the preview bridge and opt into the public fixture:

```sh
cd /Users/christopherdavid/work/openagents
KHALA_CODE_DESKTOP_OPEN_WINDOW=0 \
KHALA_CODE_DESKTOP_PREVIEW_PORT=50121 \
bun clients/khala-code-desktop/src/bun/index.ts
```

Then open:

```text
http://127.0.0.1:50121/?gymProof=fixture&view=gym
```

Expected UI shape:

```text
Gym
Read-only
Mutalisk bridge proof
Loaded
metricValueBps: 10000 bps
admissionDecision: gated_proposal_ready
decisionGrade: false
candidate refs: manifest... candidate... module...
Action Submission proposal refs: action_submission.proposal...
read-only Arbiter-style graph with evidence-backed links
```

To load a generated bridge proof instead of the built-in fixture, copy the proof
JSON and load it from the preview or native Web Inspector console:

```sh
cd /Users/christopherdavid/work/openagents
pbcopy < out/khala-gepa-bridge-proof.json
```

```js
khalaCodeDesktop.loadGymProof(await navigator.clipboard.readText())
```

For quick local toggles:

```js
khalaCodeDesktop.loadGymDemoProof()
khalaCodeDesktop.clearGymProof()
khalaCodeDesktop.gymState()
```

Missing proof data should continue to show `No Gym proof loaded.` Blocked proof
data should show a blocked badge, blocker refs, and blocked graph links rather
than an admission-ready proposal.

## Failure Triage

- `codex_spawn_failed: No Pylon Codex assignment capacity is available right now`
  means the old failure shape regressed. Run the preflight smoke and inspect the
  `advertise_capacity` step.
- `blocker.public.khala_fleet_delegate.connect_account_required` means no ready
  isolated Codex account is linked. Use `khala fleet connect`.
- `blocker.public.pylon_dispatch.duplicate_active_assignment` is usually
  transient; a fresh heartbeat and one retry should recover through the
  deterministic dispatch fallback.
- `blocker.public.pylon_dispatch.stale_heartbeat` should recover through a fresh
  `advertise_capacity` step. If it does not, restart the local Pylon from the
  same clean checkout.

## Verification Commands

Before recording, the focused checks are:

```sh
bun clients/khala-code-desktop/scripts/part2-delegation-smoke.ts
bun clients/khala-code-desktop/scripts/part2-gepa-manifest-bridge.ts --summary /Users/christopherdavid/work/mutalisk/out/khala-fleet-delegation-summary.json --out out/khala-gepa-bridge-proof.json
bun clients/khala-code-desktop/scripts/part2-gepa-manifest-bridge.ts --summary /Users/christopherdavid/work/mutalisk/out/khala-fleet-delegation-summary.json --api-base https://openagents.com --operator-token-env OPENAGENTS_OPERATOR_BEARER_TOKEN --out out/khala-gepa-worker-proof.json
bun run typecheck:khala-code-desktop
bun test clients/khala-code-desktop/tests/gym-proof-loader.test.ts clients/khala-code-desktop/tests/gym-graph-renderer.test.ts
bun test clients/khala-code-desktop/tests/khala-codex-fleet-tools.test.ts
bun run --cwd apps/openagents.com/workers/api test -- src/inference/gym/mutalisk-khala-delegation-bridge.test.ts src/inference/gym/mutalisk-khala-delegation-routes.test.ts src/worker-exact-routes.test.ts src/probe-gepa-standing-optimization-loop.test.ts
bun run --cwd apps/openagents.com/workers/api typecheck
git diff --check
```
