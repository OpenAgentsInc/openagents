# Forge Hosted Closeout Audit Bundle

Date: `2026-04-02`

This is the first checked-in closeout bundle for the hosted Forge dogfood lane.
It now corresponds to a real successful hosted GCP run, not just the local
shell shape around a hypothetical hosted lane.

## Successful Run Identity

- final hosted session:
  - `sess_1775150159726_14012_184`
- operator endpoint:
  - `127.0.0.1:17777`
- remote workspace:
  - `/var/lib/probe-hosted/hosted/workspaces/forge-openagents-main/openagents`
- exported output directory:
  - `/private/tmp/forge-hosted-proof-20260402g`

## Hosted Environment

- GCP project:
  - `openagentsgemini`
- region / zone:
  - `us-central1` / `us-central1-a`
- VPC / subnet:
  - `oa-lightning` / `oa-lightning-us-central1`
- Probe VM:
  - `probe-hosted-forge-1`
- Probe service:
  - `probe-hosted.service`
- Probe home:
  - `/var/lib/probe-hosted`
- prepared baseline:
  - `forge-openagents-main`

## Run Scope

- one hosted coding closeout rehearsal
- one hosted bookkeeping rehearsal on the same shared session
- GCP-hosted Probe runtime with local Autopilot operator control

## Artifact Map

### Coding Closeout Artifact Set

- shared session
- Probe remote session projection
- evidence bundle
- delivery receipt
- settlement receipt
- hosted coding audit bundle

### Bookkeeping Artifact Set

- campaign
- promotion ledger
- bounty contract
- bounty claim
- hosted bookkeeping audit bundle

### Exported Files

- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-summary.md`
- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-summary.json`
- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-preflight.md`
- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-preflight.json`
- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-coding-audit.md`
- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-coding-audit.json`
- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-bookkeeping-audit.md`
- `/private/tmp/forge-hosted-proof-20260402g/forge-hosted-bookkeeping-audit.json`

## What Worked

- hosted Probe created a prepared-baseline session and completed both the proof
  patch turn and the read-back turn
- the shared session projected mounted-pack truth for:
  - `forge-pack-1` `OpenAgents repo docs`
  - `forge-pack-2` `Forge hosted dogfood runbook`
- the shell exported a deterministic hosted preflight, coding audit, bookkeeping
  audit, and summary bundle
- hosted Probe receipts projected auth, checkout, worker, cost, and cleanup
  ownership into the same shell the operator already uses for evidence,
  delivery, and bookkeeping
- the shared session closed with:
  - delivery `merged`
  - evidence `complete`
  - bounty `admitted`
  - campaign `scoped`
  - promotion `promoted`
  - settlement `recorded`

## Failures Found While Proving The Lane

1. The first hosted patch turn hit the harness tool-loop cap before the model
   finished.
2. The original hosted worker inherited a 30-second detached watchdog stall
   budget, which was too aggressive for remote Codex-backed turns.
3. Forcing `tool_choice = named("shell")` caused the model to keep calling
   shell instead of terminating cleanly after the proof write succeeded.
4. The first successful hosted shell run still projected zero mounted packs
   because the harness built the route plan without the thread project id.
5. The shell originally treated accepted patch summary packs as mandatory even
   though the proof turn used `shell`, which only emits a retained session
   summary today.

## What Changed To Make The Live Run Pass

- Probe hosted TCP now accepts an explicit watchdog policy and the deployed GCP
  worker runs with `--watchdog-stall-ms 180000 --watchdog-timeout-ms 300000`.
- The hosted harness now uses `ProbeToolChoice::Auto` for patch and read-back
  turns with explicit stop instructions.
- The hosted harness now builds the knowledge-mount plan with the thread
  project id, so project-scoped docs and runbook packs are mounted honestly.
- The hosted harness now treats accepted patch summary artifacts as optional
  for shell-only proof turns and records that omission explicitly in the
  summary.

## Manual Steps Still Required

- inspect raw infra logs outside the shell when receipt cards are incomplete
- record recovery and defect notes manually through `/hosted ...`

## Recovery And Cleanup Notes

Current operator expectation for the first lane:

- treat worker restart, orphan cleanup, and operator takeover as required
  drills, not optional nice-to-haves
- if the runtime does not expose a typed receipt for a recovery step, capture
  the step manually with `/hosted recovery ...`
- if the recovery outcome is ambiguous, record `/hosted defect ...` before
  closing the run

The live run also confirmed two remaining operational truths:

- cleanup still records as `pending`; the workspace is marked managed hosted
  state but there is no teardown hook yet
- hosted receipt history is still snapshot-oriented rather than a typed restart
  and takeover event log

## Honest Conclusion

Forge now has enough object and projection truth to run one real hosted GCP
closeout lane end to end without pretending that runtime or bookkeeping state
live somewhere else. The next honest work is repeatability and operational
hardening: a second hosted run, restart and takeover drills, and teardown truth
rather than more speculative object design.
