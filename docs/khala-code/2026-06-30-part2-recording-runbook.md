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
4. later in the sequence, feed a Mutalisk candidate into the no-UI Gym bridge.

This runbook starts with the #7752 deterministic delegation smoke. #7753,
Mutalisk #10, and #7754 will extend it with token-rate status, candidate
emission, and Gym/admission ingest.

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

Before closing #7752, the focused checks are:

```sh
bun clients/khala-code-desktop/scripts/part2-delegation-smoke.ts
bun test clients/khala-code-desktop/tests/khala-codex-fleet-tools.test.ts
git diff --check
```
