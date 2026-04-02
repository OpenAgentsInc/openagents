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

## What Is Still Manual

- Reading raw GCP logs when the shell cards are insufficient.
- Recording operator recovery and defect notes through `/hosted ...` commands.

## What Remains Unsafe

- Hosted receipt truth is still snapshot-oriented. Restart, orphan cleanup, and
  operator takeover drills are not yet exposed as a typed event history.

## Preconditions

- `openagents` includes the hosted coding and bookkeeping bundle commits:
  - `d0f011642` for hosted coding closeout bundles
  - `cde7213c4` for hosted bookkeeping rehearsal bundles
- Probe hosted receipts are available to the desktop lane.
- The operator has:
  - a local `gh` install with repo access
  - GCP access to the target project
  - the repo and branch to exercise
  - the knowledge packs or runbooks to mount

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

- typed hosted recovery event history instead of snapshot-only receipts
  - tracked in `probe#98`
