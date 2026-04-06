# Forge Hosted GCP Dogfood Runbook

This runbook is the first honest operator path for running Forge on a hosted
Probe lane inside our own GCP footprint. It is intentionally boring:
Autopilot stays the local operator shell, Probe owns hosted runtime truth, and
the operator records closeout and bookkeeping state above that runtime instead
of inventing a fake hosted control plane in the app.

## Goal

Run one hosted coding closeout and one hosted bookkeeping rehearsal on GCP,
then leave behind enough linked evidence to explain:

- what session ran
- which packs were routed and actually mounted
- what delivery and review state closed the run
- what bounty, campaign, promotion, and settlement objects were tied to it
- what recovery steps or defects still required operator intervention

## Current Hosting Assumption

- Autopilot desktop is the control surface.
- Probe owns the hosted session, mounted refs, execution-host identity, and
  hosted receipts.
- The first hosted lane runs in our own GCP project.
- Thin stateless edges can stay flexible, but hosted Probe workers should
  default to boring private compute in `us-central1`.
- Storage should stay equally boring: object storage plus a DB-backed control
  store.
- Queueing can stay DB-backed first; Pub/Sub is optional follow-on, not a
  prerequisite for the first dogfood lane.

## What Is Automated Now

- Probe-backed shared sessions, evidence bundles, delivery receipts, campaign
  objects, promotion ledgers, bounty contracts, bounty claims, and settlement
  receipts all persist in the Autopilot shell.
- The internal team can now mirror those app-owned collaboration objects into
  a shared Forge state file by setting `OPENAGENTS_FORGE_SHARED_STATE_PATH` on
  each operator desktop to the same internal path.
- Once multiple desktops point at that path, operators can list and attach to
  the same hosted Forge session with:
  - `/hosted sessions`
  - `/hosted attach shared <shared-session-id>`
  - `/hosted attach probe <probe-session-id>`
- Once attached, operators can coordinate one active human controller at a
  time with:
  - `/handoff status`
  - `/handoff request <summary>`
  - `/handoff accept <summary>`
  - `/handoff take <summary>`
  - `/handoff note <summary>`
- The shared-session shell now shows the current controller, the participant
  roster, any pending handoff request, and recent collaboration events above
  the hosted Probe runtime transcript.
- Scoped pack routing projects app-owned pack choices into typed Probe mounts.
- Hosted Probe receipts for auth, checkout, worker ownership, cost, and cleanup
  can be projected into the shared session when Probe reports them.
- Operators can run a hosted launch preflight and export the result with:
  - `/hosted preflight [path]`
- Operators can record one hosted coding audit bundle and one hosted
  bookkeeping audit bundle with:
  - `/hosted coding <environment-summary>`
  - `/hosted bookkeeping <environment-summary>`
  - `/hosted note <coding|bookkeeping> <summary>`
  - `/hosted recovery <coding|bookkeeping> <summary>`
  - `/hosted defect <coding|bookkeeping> <summary>`
  - `/hosted export <coding|bookkeeping> [path]`
  - `/hosted status`
- A repo-owned end-to-end harness now exists:
  - `cargo run -p autopilot-desktop --bin autopilot-forge-hosted-dogfood -- ...`
  - it runs the hosted preflight, starts the hosted Probe session, drives one
    patch turn plus one read-back turn, projects hosted receipts back into the
    shell, records evidence and bookkeeping objects, and exports deterministic
    Markdown and JSON outputs.

## What Is Still Manual

- Reading raw GCP logs when the shell cards are insufficient.
- Recording operator recovery and defect notes through `/hosted ...` commands.

## What Remains Unsafe

- Hosted receipt truth is still snapshot-oriented. Restart, orphan cleanup, and
  operator takeover drills are not yet exposed as a typed event history.
- Shell-only hosted proof turns emit a retained session summary, but they do
  not currently emit an accepted patch summary artifact.

## Preconditions

- `openagents` includes the hosted coding and bookkeeping bundle commits:
  - `d0f011642` for hosted coding closeout bundles
  - `cde7213c4` for hosted bookkeeping rehearsal bundles
- Probe hosted receipts are available to the desktop lane.
- If multiple operator desktops need to see the same app-owned Forge objects,
  they all point `OPENAGENTS_FORGE_SHARED_STATE_PATH` at the same internal
  shared document path.
- The second operator then uses `/hosted sessions` to find the live shared
  session and `/hosted attach ...` to bind a local thread before the normal
  Probe `LoadSession` path hydrates the transcript and runtime projection.
- The operator has:
  - a local `gh` install with repo access
  - GCP access to the target project
  - the repo and branch to exercise
  - the knowledge packs or runbooks to mount

## First Proven Run

The first live run on April 2, 2026 used:

- local harness:
  - `cargo run -p autopilot-desktop --bin autopilot-forge-hosted-dogfood -- --address 127.0.0.1:17777 --remote-cwd /var/lib/probe-hosted/hosted/workspaces/forge-openagents-main/openagents --local-workspace-root /Users/christopherdavid/work/openagents --output-dir /private/tmp/forge-hosted-proof-20260402g --worker-baseline forge-openagents-main --repo-secret-ref github-public-https`
- final hosted Probe session:
  - `sess_1775150159726_14012_184`
- final exported outputs:
  - `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-summary.md`
  - `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-coding-audit.md`
  - `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-bookkeeping-audit.md`
  - `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-preflight.md`

What that run proved:

- hosted Probe created and preserved a prepared-baseline session on GCP
- the shell projected hosted auth, checkout, worker, cost, and cleanup receipts
- project-scoped docs and runbook packs mounted successfully as `forge-pack-1`
  and `forge-pack-2`
- the coding closeout and bookkeeping rehearsal exported deterministically from
  the shell
- the resulting shared session closed as:
  - delivery `merged`
  - evidence `complete`
  - bounty `admitted`
  - campaign `scoped`
  - promotion `promoted`
  - settlement `recorded`

Lessons from the proving run:

- hosted Codex turns need a looser watchdog posture than the local detached
  default, so the hosted worker now runs with a `180000ms` stall budget and
  `300000ms` timeout budget
- the harness must use `tool_choice = auto` for patch and read-back turns, or
  the model never gets a clean stop condition after the tool call
- project-scoped pack routing must include the thread project id when building
  the mounted-ref plan
- the shell must treat accepted patch summary artifacts as optional for
  shell-only proof turns

## Step 1: Prepare The Session

1. Open the target repo in Autopilot and start a Probe-backed thread.
2. Author or import the packs needed for the run:
   - `/pack docs <title> <path> [path ...]`
   - `/pack runbook <title> <path> [path ...]`
   - `/pack retained [title]`
   - `/pack patch [title]`
3. Route the packs into the next session start:
   - `/pack route auto <pack-id> [pack-id ...]`
   - `/pack route status`
4. Confirm the run is intended for the hosted GCP lane and not the local daemon
   path.
5. Run the hosted preflight and do not launch if it reports blockers:
   - `/hosted preflight [path]`

## Step 2: Start The Hosted Coding Run

1. Start the hosted Probe session.
2. Immediately record the hosted coding bundle:
   - `/hosted coding gcp us-central1 <purpose>`
3. Export the coding bundle when you need a durable closeout artifact:
   - `/hosted export coding [path]`
4. Confirm the shared-session card shows:
   - remote session ownership
   - workspace identity
   - routed pack ids
   - mounted pack ids and any unsupported routes
   - hosted Probe receipts as they arrive

## Step 3: Drive Coding Closeout

Use the normal Probe-backed thread for the coding task, then close it out in
the shell:

- capture reviewer evidence:
  - `/evidence verify <label> <passed|failed|running> [reference]`
  - `/evidence log <label> <reference>`
  - `/evidence preview <label> <reference>`
  - `/evidence screenshot <label> <reference>`
- record delivery:
  - `/deliver pr [base-branch] [pr-url]`
  - `/deliver review <commented|approved|changes_requested> <reviewer-label> [summary]`
  - `/deliver merge <reviewer-label> [summary]`
  - `/deliver status`
- record settlement:
  - `/settle merge <reviewer-label> [summary]`

When the operator has to recover from a hosted failure, log it immediately:

- `/hosted recovery coding <summary>`
- `/hosted defect coding <summary>`
- `/hosted export coding [path]`

## Step 4: Run The Bookkeeping Rehearsal

After the coding closeout exists, rehearse the bookkeeping layer against the
same shared session:

- open and fund the bounty envelope:
  - `/bounty open <merge|metric> <title>`
  - `/bounty credit <participant-label> <basis-points>`
  - `/bounty claim <claimant-label> [summary]`
  - `/bounty advance <admitted|completed|canceled|disputed> [summary]`
  - `/bounty status`
- record the retained-case shell:
  - `/campaign open <title>`
  - `/campaign goal <summary>`
  - `/campaign scope <summary>`
  - `/campaign candidate <probe_summary|accepted_patch|evidence_bundle|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign verify <evidence_bundle|delivery_receipt|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign status`
- record rollout posture:
  - `/promote shadow <probe_summary|accepted_patch|evidence_bundle|psionic_eval|psionic_compare> <reference> <actor-label> [summary]`
  - `/promote promote <actor-label> [summary]`
  - `/promote status`
- record the bookkeeping bundle:
  - `/hosted bookkeeping gcp us-central1 <purpose>`
  - `/hosted note bookkeeping <summary>`
  - `/hosted recovery bookkeeping <summary>`
  - `/hosted defect bookkeeping <summary>`
  - `/hosted export bookkeeping [path]`

## Step 5: Recovery Drills

At minimum, the first dogfood lane should exercise:

1. hosted worker restart
2. orphan cleanup after an interrupted run
3. operator takeover or reattach

Current rule: if Probe does not emit a typed event for the drill, record the
outcome manually through `/hosted recovery ...` or `/hosted defect ...` and
link the raw infrastructure evidence in the audit bundle.

## Step 6: Closeout Checklist

Before ending the dogfood run, verify the shell shows all of the following:

- shared session id
- active hosted coding audit bundle
- active hosted bookkeeping audit bundle
- evidence bundle
- delivery receipt
- settlement receipt
- campaign
- promotion ledger
- bounty contract
- bounty claim
- mounted pack projection
- hosted Probe receipts for auth, checkout, worker, cost, and cleanup

## First Follow-On Gaps

- no standing follow-on issue remains from the first hosted dogfood batch
- the next honest backlog should come from the next hosted GCP run and its
  recorded audit defects rather than from more speculative infrastructure docs
