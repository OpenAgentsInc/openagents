# FC-4 hybrid client acceptance receipt

Date: 2026-07-13  
Issue: [#8636](https://github.com/OpenAgentsInc/openagents/issues/8636)  
Production revision: `openagents-monolith-00117-kdg`  
Result: closed

## Accepted boundary

One production FleetRun completed two real work units through one claim
registry: one owner-local Codex unit and one managed-cloud Codex Agent Computer
unit. The owner used OpenAgents mobile's **Your Codex** account-selection and
device-link flow to make the server-managed Codex account available before the
run. That is the mobile management action for the managed unit; mobile did not
author placement, claims, or terminal truth. Android and Desktop then consumed
the same authenticated, read-only server projection and rendered the same run,
unit, target, claim, assignment, outcome, and closeout refs.

This receipt does not claim portable-session movement, arbitrary provider
adapters, owner-managed remote hosts, or client-owned Fleet authority.

## Production run

- run: `fleet_run.sarah.f566771758bbe0ab5fc5`
- terminal state: `completed`
- terminal sequence: `96`
- owner-local unit: `unit.fc4.owner_local.acceptance.202607131047`
  - target/outcome: `owner_local` / `accepted`
  - claim: `fleet_run.sarah.f566771758bbe0ab5fc5.claim.unit.fc4.owner_local.acceptance.202607131047.1783939623734.0`
  - public account hash: `account.pylon.codex.f88a4773edd26cae162ceb2f`
  - assignment: `assignment.public.khala_coding.chatcmpl_c9db1507f52a44468b43545317e10c8e`
  - artifact: `artifact.pylon.codex_agent_task.patch.7c1f592ec9cb36a10ff1aa4e`
  - verification: `proof.pylon.codex_agent_task.test.fcf8dc5d3377f1fa10703ebc`
  - closeout: `assignment.closeout.summary.e2d06ebe9e9e8d74`
  - usage: exact served-token evidence was retained at
    `event.inference.served-tokens.pylon-codex.21e4110b85f1e525f26ea55231050aa0`
- managed-cloud unit: `unit.fc4.managed_cloud.acceptance.202607131047`
  - target/outcome: `managed_cloud` / `accepted`
  - claim: `fleet_run.sarah.f566771758bbe0ab5fc5.claim.unit.fc4.managed_cloud.acceptance.202607131047.1783939623734.1`
  - public account hash: `account.pylon.codex.8c4cc8341ca620288165a2a6`
  - assignment: `assignment.pylon.managed_cloud.de6892b92ed6ab448336f908`
  - artifact: `artifact.sha256.a10834aa89a19162495b3d0f094f4591dc4322533ced8aaa4aea2b7c69bbef06`
  - closeout: `closeout.agent_computer.execution.78c8fe0d47bc9b1efd382d81`
  - usage: explicit `not_measured`; no token counts were fabricated

Both work claims were distinct and released. The local orchestration audit
reported two completed tasks, zero undelivered outbox rows, exactly two
`work_terminal` records and one `run_terminal` record. The workspace lease is
`cleaned` with receipt
`receipt.pylon.workspace_cleanup.c1701d6ee32abdd005f33c9b`; only the reusable
prepared baseline remains. There was no duplicate claim, provider/target
substitution, or residual per-run workspace.

## Same-ref client proof

The production `GET /api/sarah/fleet-runs` projection returned HTTP 200 under
the owner's real encrypted Desktop session and reported
`privateMaterialExcluded: true`. The authenticated response carried the exact
run and both exact unit projections above. Credentials, raw provider account
refs, prompts, command output, workspace paths, and private Agent Computer
topology were absent.

Current-source Android rendered the run first in the Fleet drawer and exposed
both units with their exact target/outcome, claim, assignment, and closeout
refs. A Developer ID-signed current-source Desktop package rendered those same
refs from the same production authority. The Desktop acceptance launch bounded
local Codex/Claude history roots to small fixtures to avoid an unrelated large-
history Electron worker crash; it retained the real encrypted owner session,
production API, and Fleet projection. Desktop also fetched server authority
before the optional local-Pylon projection, so local runtime unavailability
cannot hide a valid server FleetRun.

The completed run is immutable, so Desktop's accepted action was to reopen and
resume observation/reconciliation of its authority projection, not to mutate or
re-dispatch a terminal unit. Mobile's accepted management action was the named
server Codex account selection/link that admitted the managed target. This is
the issue's bounded “starts or manages” alternative while preserving its stated
rule that clients do not own placement or claim authority.

## Landed implementation and verification

- `fdd0507382`: shared public-safe FleetRun projection, authenticated server
  list route, and Desktop/mobile consumers
- `94a3c5763c`: cancelled history and mobile Fleet drawer rows
- `fd1fcffc42`: legacy timestamp normalization
- `9c289eccf3`: cancelled authority history remains safe
- `fff2c4c722`: Desktop prefers server Fleet authority before optional local
  Pylon projection
- `935f9ce47f`: JSON-normalized context-bridge Fleet projection

Focused verification passed:

- 58 shared, server, mobile, and Desktop projection tests
- 33 Desktop Fleet workspace tests / 134 expectations
- Desktop typecheck and renderer build
- Developer ID-signed macOS packaging and exact-ref UI inspection
- current-source Android emulator exact-ref UI inspection

The retained follow-on boundary is portability: moving a live graph between
targets, owner-managed enrollment, additional managed-provider adapters, and
any-host mobile control remain PORT-03 through PORT-08.
