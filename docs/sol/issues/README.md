# Sol checked-in issue sources

- Class: index
- Status: current classification index
- Dispatch: no; use live GitHub issues and the claim ledger
- Owner: Sol roadmap

This directory preserves bounded issue-body sources, acceptance contracts,
closure evidence, and non-revival tombstones for the reliable Desktop/mobile
program. It is not a second live issue database.

## Current work authority

Before selecting or claiming work, read:

1. [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md) for current program authority
   and dependency order;
2. [open `roadmap:sol` issues](https://github.com/OpenAgentsInc/openagents/issues?q=is%3Aissue%20state%3Aopen%20label%3Aroadmap%3Asol)
   for the live operational set and acceptance state;
3. [`../CLAIM_PROTOCOL.md`](../CLAIM_PROTOCOL.md) and the live issue comments
   for ownership and collision avoidance;
4. current code, tests, deployments, and receipts for proof state.

Do not infer that a checked-in file is open because it is present here. Do not
infer closure from prose alone. Refresh the live issue and its current claim.

## Source classes in this directory

### Program and client source records

- [`app-program.md`](./app-program.md)
- [`app-desktop.md`](./app-desktop.md)
- [`app-mobile.md`](./app-mobile.md)
- [`fc-cloud-codex.md`](./fc-cloud-codex.md)
- [`fc-4-hybrid-cloud.md`](./fc-4-hybrid-cloud.md)
- [`native-streamed-conversation-handoff.md`](./native-streamed-conversation-handoff.md)
- [`conversation-fault-convergence.md`](./conversation-fault-convergence.md)

These files preserve scope and acceptance language for major program records.
Their open/closed state and next action are intentionally not copied here.

### Closed proof and implementation sources

Examples include:

- [`desktop-codex-subagent-history.md`](./desktop-codex-subagent-history.md)
- [`desktop-codex-trace-acceptance.md`](./desktop-codex-trace-acceptance.md)
- [`desktop-effect-scope-topology.md`](./desktop-effect-scope-topology.md)
- [`desktop-runtime-gateway.md`](./desktop-runtime-gateway.md)
- [`desktop-sync-host.md`](./desktop-sync-host.md)
- [`mobile-sync-host.md`](./mobile-sync-host.md)
- [`local-first-identity.md`](./local-first-identity.md)
- [`fc-1-run-contract.md`](./fc-1-run-contract.md)
- [`fc-2-local-executor.md`](./fc-2-local-executor.md)
- [`fc-3-supervision.md`](./fc-3-supervision.md)
- [`fc-5-dogfood.md`](./fc-5-dogfood.md)

They are historical issue sources or receipts, not ready work. The cleanup
program will classify the full directory and move closed sources to an indexed
closed path only after inbound links are inventoried.

### Closed non-revival tombstones

- [`app-web-consolidation.md`](./app-web-consolidation.md)
- [`app-forum.md`](./app-forum.md)
- [`app-landing.md`](./app-landing.md)
- [`sarah-presentation.md`](./sarah-presentation.md)
- [`role-programs-and-colleagues.md`](./role-programs-and-colleagues.md)
- [`glass-ui-and-sarah-mobile.md`](./glass-ui-and-sarah-mobile.md)
- [`blueprint-correction.md`](./blueprint-correction.md)

These preserve explicit closed/not-planned boundaries. They are not dormant
epics. A real security, privacy, accessibility, data-loss, outage, or supported
surface defect receives a new bounded issue under its actual owner.

## CUT graph and receipts

The dated
[`CUT-01–CUT-27 plan`](../2026-07-11-openagents-coding-cutover-issue-plan.md)
preserves dependency and acceptance design. It does not define the current
queue. Use each live GitHub issue and its dedicated receipt for final state.
Completed CUT, AC, EP, owner-acceptance, deployment, emulator, and failure
records remain evidence even when their bodies contain earlier intermediate
rungs.

## Reconciliation rule

- Keep live state in GitHub and one canonical roadmap projection.
- Keep durable acceptance and negative boundaries in contracts or checked-in
  sources.
- Keep proof in immutable, indexed receipts with an obvious final disposition.
- Do not update every historical source merely to mirror an issue transition.
- Do not rewrite a historical receipt into current proof.
- If a checked-in source and live body materially disagree on still-open
  acceptance, reconcile the owning contract and issue before dispatch.

The ordered cleanup and retirement rules are in
[`../2026-07-12-documentation-cleanup-audit-and-retirement-plan.md`](../2026-07-12-documentation-cleanup-audit-and-retirement-plan.md).
