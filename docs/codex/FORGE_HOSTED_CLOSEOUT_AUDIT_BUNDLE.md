# Forge Hosted Closeout Audit Bundle

Date: `2026-04-02`

This is the first checked-in closeout bundle for the hosted Forge dogfood lane.
It is intentionally honest about the current state: the app can now persist the
right operator objects, but the concrete hosted run ids still live in the
shell-local projection unless the operator copies them out manually.

## Source Commits

- `d0f011642` `autopilot: add hosted coding audit bundles`
- `cde7213c4` `autopilot: add hosted bookkeeping rehearsal bundles`

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

## What Worked

- the shared session now carries hosted session location plus mounted-pack truth
  without hiding that state inside transcript prose
- hosted Probe receipts project auth, checkout, worker, cost, and cleanup
  ownership into the same shell the operator already uses for evidence and
  delivery
- the coding closeout and bookkeeping rehearsal are now represented as separate
  hosted audit bundles, which makes it possible to tell runtime failures apart
  from bookkeeping gaps
- bookkeeping rehearsal bundles now link the campaign, promotion, bounty,
  claim, and settlement objects that were tied to the hosted run

## Manual Steps Still Required

- verify GCP project, worker baseline, and secrets before launch
- inspect raw infra logs outside the shell when receipt cards are incomplete
- record recovery and defect notes manually through `/hosted ...`
- copy concrete bundle and session ids out of the local shell when a durable
  checked-in audit needs them

## Recovery And Cleanup Notes

Current operator expectation for the first lane:

- treat worker restart, orphan cleanup, and operator takeover as required
  drills, not optional nice-to-haves
- if the runtime does not expose a typed receipt for a recovery step, capture
  the step manually with `/hosted recovery ...`
- if the recovery outcome is ambiguous, record `/hosted defect ...` before
  closing the run

## Known Defects Filed From This Audit

- automated export of hosted closeout bundles does not exist yet
- the hosted lane still lacks a hard preflight for project, secret, and worker
  readiness
- hosted receipt truth is snapshot-only and does not expose restart or cleanup
  history as a typed event stream

## Follow-On Issue Links

- `openagents#4104` Export hosted closeout audit bundles from shell-local Forge
  state
- `openagents#4105` Add hosted GCP preflight and hard launch blockers for Forge
  dogfood sessions
- `probe#98` Emit hosted restart, cleanup, and operator-takeover history in
  Probe receipts

## Honest Conclusion

Forge now has enough object and projection truth to run a hosted dogfood lane
without pretending that the runtime or bookkeeping state lives somewhere else.
The next gap is operational hardening: automatic audit export, preflight
checks, and typed hosted recovery history.
