# Autopilot Task: Cloudflare Containers Runner Backup Implementation

Status: queued delegation packet; blocked from dispatch until the Autopilot
dispatch gate in `AGENTS.md` is satisfied.

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `autopilot`

Team: `team_openagents_core`

Project: team-level platform task. If dispatch requires a concrete project,
preflight must create or select the current OpenAgents runner-platform project
and update the launch payload before dispatch. Do not fabricate a project id.

Visibility: private or team-visible. This is platform infrastructure work, not
a public campaign.

Public route or observer link: none required. If the run creates an observer
link, keep it team/private unless the operator explicitly approves a public
summary.

## Dispatch Gate

Do not launch this task until the programmatic Autopilot runbook
recommendations are complete enough for reliable delegation:

- operator preflight exists and reports migration state, agent/project
  presence, provider health, SHC health, callback config, GitHub writeback
  readiness, and target branch state;
- reconnect-required provider states are caught before dispatch;
- SHC callback payload contracts and retry/backfill paths are covered;
- run continuation attaches to the same durable goal;
- public/team goal observation can show the current run without exposing
  private delivery mechanics;
- operator checklist or equivalent status summary exists for foreground
  supervision.

Source runbook:
`2026-06-04-programmatic-autopilot-operator-runbook.md`

This is an Autopilot-owned implementation task. The foreground coding agent
should only administer the packet/goal/run and repair Autopilot infrastructure
defects that block honest execution, reporting, continuation, or visibility.

## Objective

Implement the Cloudflare Containers runner backup audit fully in OpenAgents product surface.

Build a backend-neutral runner gateway that keeps SHC as the primary
low-to-medium trust runner, adds Cloudflare Containers as a backup and burst
runner lane, and keeps GCloud/GCP as the sensitive/reference/canonical fallback
lane.

The finished system must support:

- `cloudflare_container` as a first-class `RunnerBackend`;
- D1 storage and public projections for the new backend;
- backend-neutral dispatch selection;
- SHC adapter behavior preserved exactly;
- Cloudflare Containers Worker binding and Container class;
- a minimal fake Container runner path for staging/tests;
- a real OpenCode/Codex runner image path once the fake path is proven;
- provider-account grant resolution inside the runner, not in public Worker
  bodies;
- callback/event/artifact closeout through the existing OpenAgents product surface ledger;
- billing metadata that distinguishes product runner usage from Cloudflare
  platform costs;
- operator-only health, capacity, and failover visibility;
- tests and staging smoke evidence before any automatic failover is enabled.

## Current Starting Point

The audit to implement is tracked at:

- `../2026-06-04-cloudflare-containers-runner-backup-audit.md`

Current repo facts from that audit:

- `packages/sync-schema/src/index.ts` defines `RunnerBackend` as
  `shc_vm | gcloud_vm`.
- `workers/api/src/omni-runs.ts` defaults agent runs to `shc_vm` and
  `runnerId: "oa-shc-katy-01"`.
- `buildAppDeployAssignment` sets `primaryBackend: "shc_vm"` and
  `fallbackBackend: "gcloud_vm"`.
- `workers/api/src/omni/dispatch-service.ts` delegates directly to
  `dispatchAgentRunToShc` and `dispatchDeploymentToShc`.
- `workers/api/src/config.ts` has only `SHC_*` dispatch config and
  `SHC_DISPATCH_MODE = live | unconfigured`.
- `workers/api/src/omni-handlers.ts` accepts only `shc_vm | gcloud_vm` in
  request selectors and defaults launches to `shc_vm`.
- Fleet projection currently reports
  `routingPolicy: "shc_primary_gcloud_fallback"`.
- `workers/api/wrangler.jsonc` has no `containers` entry and no Container
  Durable Object binding.
- `workers/api/package.json` does not depend on `@cloudflare/containers`.
- `workers/api/src/billing.ts` uses product "computer usage" billing terms that
  predate real Cloudflare Containers.

The audit recommendation is:

```text
schema + adapter boundary
  -> fake Container runner
  -> staging smoke
  -> OpenCode image
  -> provider grant resolution
  -> artifact closeout
  -> operator-only live runs
  -> automatic SHC failover for low-to-medium trust tasks
```

## Relevant Repo Files

Primary audit and task guidance:

- `../2026-06-04-cloudflare-containers-runner-backup-audit.md`
- `2026-06-04-programmatic-autopilot-operator-runbook.md`
- `AGENTS.md`

Runtime and dispatch:

- `../../workers/api/src/omni-runs.ts`
- `../../workers/api/src/omni-runs.test.ts`
- `../../workers/api/src/omni/dispatch-service.ts`
- `../../workers/api/src/omni/assignments.ts`
- `../../workers/api/src/omni/errors.ts`
- `../../workers/api/src/omni/runner-events.ts`
- `../../workers/api/src/omni-handlers.ts`
- `../../workers/api/src/provider-accounts.ts`
- `../../workers/api/src/provider-account-service.ts`
- `../../workers/api/src/provider-account-domain.ts`
- `../../workers/api/src/billing.ts`
- `../../workers/api/src/config.ts`
- `../../workers/api/src/config.test.ts`
- `../../workers/api/src/bindings.ts`
- `../../workers/api/src/runtime.ts`
- `../../workers/api/wrangler.jsonc`
- `../../workers/api/package.json`

Shared schemas and migrations:

- `../../packages/sync-schema/src/index.ts`
- `../../packages/sync-schema/src/runner-event.ts`
- `../../workers/api/migrations/0010_omni_agent_runs_and_deployments.sql`
- `../../workers/api/migrations/0019_agent_runtime_modes.sql`

Operator and architecture guardrails:

- `../2026-06-02-shc-agent-deployment-runbook.md`
- `../2026-06-02-provider-account-implementation-notes.md`
- `../2026-06-02-cloudflare-only-openagents-sync-audit.md`
- `../2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- `../../INVARIANTS.md`
- `../../AGENTS.md`

## External References

Use current official Cloudflare docs before implementing config, limits,
pricing, or API details:

- Containers overview:
  <https://developers.cloudflare.com/containers/>
- Get started:
  <https://developers.cloudflare.com/containers/get-started/>
- Container class:
  <https://developers.cloudflare.com/containers/container-class/>
- Scaling and routing:
  <https://developers.cloudflare.com/containers/platform-details/scaling-and-routing/>
- Limits and instance types:
  <https://developers.cloudflare.com/containers/platform-details/limits/>
- Pricing:
  <https://developers.cloudflare.com/containers/pricing/>
- SSH:
  <https://developers.cloudflare.com/containers/ssh/>
- Wrangler container commands:
  <https://developers.cloudflare.com/workers/wrangler/commands/containers/>
- Wrangler container configuration:
  <https://developers.cloudflare.com/workers/wrangler/configuration/#containers>

When repo code, Wrangler schema, and Cloudflare docs disagree, trust current
Cloudflare docs for platform behavior and document the discrepancy.

## Production And Private Links

Safe production links:

- `https://openagents.com`
- `https://openagents.com/admin`

Do not include:

- provider tokens;
- callback tokens;
- OAuth material;
- local secret paths;
- raw runner payloads;
- SHC/GCloud private credentials;
- Cloudflare API tokens.

## Commit Input For Dispatch

Before dispatch, commit and push this task packet. The Autopilot launch input
must reference the pushed commit SHA that contains this file.

Suggested commit message for this delegation packet:

```text
docs: add Cloudflare Containers runner task packet
```

Launch input fields:

```json
{
  "repository": "OpenAgentsInc/openagents",
  "baseRef": "main",
  "taskSpecPath": "docs/autopilot-tasks/2026-06-04-cloudflare-containers-runner-backup-implementation.md",
  "agentId": "autopilot",
  "projectId": null,
  "teamId": "team_openagents_core",
  "visibility": "team",
  "goal": "Implement the Cloudflare Containers runner backup audit fully: add a backend-neutral runner gateway, preserve SHC, add Cloudflare Containers as a backup and burst lane, keep GCloud as the sensitive/reference fallback, and prove the path with tests and staging smoke evidence.",
  "delivery": "commit_or_pull_request_with_tests_and_staging_smoke_notes"
}
```

If preflight resolves a concrete runner-platform project, replace
`"projectId": null` with that project id before launch.

## Autopilot Work Plan

### Slice 1: Reconfirm Platform Facts

- Re-read the audit and current Cloudflare Containers docs.
- Check `node_modules/wrangler/config-schema.json` for the active
  `containers` config shape.
- Check the installed `@cloudflare/workers-types` and Wrangler version before
  adding binding types.
- Record any doc/schema mismatch in the implementation notes.

Do not start code from stale platform assumptions.

### Slice 2: Add Backend Schema And D1 Support

- Add `cloudflare_container` to `RunnerBackend`.
- Update request selector parsing in `workers/api/src/omni-handlers.ts`.
- Add a D1 migration for:
  - `agent_runs.backend`;
  - `deployments.primary_backend`;
  - `deployments.fallback_backend`.
- Preserve existing `shc_vm` and `gcloud_vm` rows.
- Do not reintroduce `local_fake` as a public backend.
- Update public run/deploy projections and tests.

Expected tests:

- schema decode accepts `cloudflare_container`;
- invalid backends still fail;
- migration accepts the new backend and preserves existing rows;
- launch selector can choose `cloudflare_container`.

### Slice 3: Introduce Runner Gateway Boundary

- Replace SHC-only dispatch coupling with a backend-neutral gateway interface.
- Keep SHC adapter behavior byte-for-byte compatible where possible.
- Add adapter-level typed errors for unavailable, unconfigured, rejected,
  malformed, timeout, transport, and capacity-exhausted cases.
- Keep `DispatchResult` stable unless a schema change is required.
- Add a placeholder or config-aware GCloud adapter boundary without pretending
  GCloud live dispatch is implemented if it is not.

Expected tests:

- `shc_vm` still dispatches through SHC;
- SHC request bodies match existing tests;
- unconfigured Container backend fails with a typed dispatch error;
- dispatch failure events still append to run timelines;
- deployment dispatch preserves primary/fallback semantics.

### Slice 4: Add Cloudflare Containers Binding Behind A Disabled Flag

- Add `@cloudflare/containers` to `workers/api/package.json`.
- Add a Worker-owned `OpenAgentsRunnerContainer` class.
- Add `containers` config and a `RUNNER_CONTAINER` Durable Object binding in
  `workers/api/wrangler.jsonc`.
- Add migration tags only after verifying deployed Durable Object history.
- Add config fields:
  - `RUNNER_DISPATCH_POLICY`;
  - `CLOUDFLARE_CONTAINER_DISPATCH_MODE`;
  - optional image tag/digest metadata.
- Keep the default mode `unconfigured`.
- Route binding access through the existing typed runtime capability boundary.

Expected tests:

- minimal env remains valid with Containers unconfigured;
- malformed Container dispatch mode fails;
- live Container mode requires the required binding/config;
- no production route directly reads raw `env.RUNNER_CONTAINER` outside the
  runtime boundary.

### Slice 5: Build Minimal Fake Container Runner

- Add a minimal Dockerfile and runner control server that can:
  - receive a sanitized assignment;
  - emit lifecycle events;
  - emit a fake `result.md` artifact manifest;
  - return a stable external id;
  - handle cancel.
- Use explicit run IDs with `getContainer(env.RUNNER_CONTAINER, runId)`.
- Do not use `getRandom` for stateful runs.
- Keep the fake runner intentionally small before adding OpenCode/Codex.

Expected tests:

- Worker dispatch reaches the fake Container adapter in local/fake tests;
- events normalize through `OmniRunnerEventService`;
- duplicate events are idempotent;
- artifact refs are recorded before completion;
- no credential-shaped material reaches D1 or public projection.

### Slice 6: Add Real OpenCode/Codex Image Path

- Add Bun/Node, Git, any required GitHub tooling, OpenCode/Codex runtime, and a
  small control server to the image.
- Reuse the SHC control contract where possible.
- Support health/readiness, start, cancel, callbacks, and closeout.
- Handle SIGTERM gracefully and upload/callback required artifacts before
  `sleepAfter` stops the instance.
- Do not bake credentials into the image or layers.

Expected tests or smoke:

- staging launch clones a test repo;
- produces `result.md`;
- produces `github-writeback.json` when writeback is requested;
- sends lifecycle events;
- scrubs provider auth material;
- stops/sleeps cleanly.

### Slice 7: Preserve Provider Account Secret Boundary

- Keep browser issue path returning only public grant metadata.
- Keep assignments carrying `authGrantRef`, not raw provider auth.
- Resolve provider grants from inside the runner using the existing service
  route and a narrowly scoped service actor credential.
- Materialize `OPENCODE_AUTH_CONTENT` or isolated auth JSON only inside the
  container.
- Scrub all auth material after closeout.
- Keep existing provider secret scanners on every event/artifact projection.

Expected tests:

- Worker dispatch bodies do not contain OpenCode auth JSON;
- runner event payloads containing credential-shaped values are rejected;
- revoked provider auth still maps to reconnect-required health;
- service credential names do not appear in public events.

### Slice 8: Artifact, Billing, And Operator Surfaces

- Treat Container disk as ephemeral.
- Require artifact closeout before terminal success.
- Store artifact blobs in R2 and artifact refs in D1.
- Separate product runner usage from Cloudflare platform cost metadata.
- Add backend metadata to billing ledger entries.
- Add operator-visible backend health/capacity/cold-start/cost estimates.
- Update fleet routing policy to:
  `shc_primary_cloudflare_container_backup_gcloud_reference`.

Expected tests:

- missing required artifacts fail or block the run;
- billing records backend metadata for `cloudflare_container`;
- exhausted credits cancel or stop the active backend;
- fleet projection reports Container availability without leaking private
  dispatch mechanics.

### Slice 9: Failover Policy And Staging Rollout

- Enable `cloudflare_container` only for operator-selected runs first.
- Add health and capacity routing after manual operator launches pass.
- Fail over SHC to Containers only for low-to-medium trust workloads.
- Keep GCloud for sensitive/reference/canonical reruns.
- Add staging smoke notes and a production enablement checklist.

Do not silently route customer-sensitive, wallet-bearing, or broad-cloud
credential work to Containers.

## Safety Rules

- Do not print, commit, log, or project provider auth payloads, callback
  tokens, Cloudflare API tokens, OAuth material, private keys, or local secret
  paths.
- Do not store raw OpenCode auth JSON in D1, public sync, issue comments, or
  tracked docs.
- Do not pass raw provider auth from the Worker to the container start request.
- Do not introduce ad hoc string routing for user intent, retrieval, or tool
  selection.
- Do not weaken runtime policy, provider-account redaction, public projection,
  billing gates, or callback validation to make the Container path pass.
- Do not bypass the zero-tech-debt guardrails. If a temporary adapter facade is
  required, update `../2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
  with caller evidence, deletion condition, and guardrail.
- If adding or changing a runtime policy invariant, update `../../INVARIANTS.md`
  in the same change and add the corresponding regression test or explicit
  model-boundary note.
- Keep product UI copy away from Container, SHC, grant, callback, and dispatch
  mechanics unless the surface is operator-only.

## Acceptance Criteria

Code and schema:

- `cloudflare_container` is a first-class backend in shared schemas, D1, route
  selectors, stored rows, and public-safe projections.
- A backend-neutral runner gateway dispatches through SHC, Cloudflare
  Containers, or GCloud adapter boundaries.
- SHC behavior and existing tests remain compatible.
- Cloudflare Containers config, binding, Container class, and runner image path
  are present and default to disabled/unconfigured until explicitly enabled.

Security:

- No raw provider auth or callback token appears in Worker dispatch bodies,
  D1, public sync, test snapshots, logs, docs, or artifacts.
- Provider grant resolution happens runner-side through the existing service
  contract.
- Artifact closeout happens before terminal success.
- Container backend is not eligible for sensitive/reference workloads unless a
  later trust-tier policy explicitly allows it.

Tests:

- Run and report `bun run typecheck`.
- Run and report `bun run test`.
- Run focused Worker tests for config, schema, dispatch, runner events,
  provider-account redaction, billing, and migrations.
- Run any new container adapter tests.
- If `bun run check:deploy` is relevant after Worker route/config changes, run
  and report it.

Staging smoke:

- Deploy or dry-run the Worker with the Container config in a non-production
  environment.
- Launch a fake Container run.
- Verify lifecycle events, artifact refs, billing metadata, and sync
  projection.
- Launch a real OpenCode/Codex Container run only after fake runner smoke
  passes.
- Record Container instance id/external id only as safe metadata.

Delivery:

- Produce a commit or pull request with implementation, tests, and staging
  notes.
- Include a concise operator run summary.
- Include any blockers as typed platform blockers, not vague "could not run"
  notes.
- Do not deploy automatic failover to production until operator-only Container
  runs have passed and the operator explicitly approves the rollout.

## Suggested Team-Visible Run Summary

```text
Implemented the Cloudflare Containers runner backup path for OpenAgents product surface: added the
cloudflare_container backend, introduced a backend-neutral runner gateway,
preserved SHC behavior, added the disabled-by-default Container binding and
runner path, protected provider-account secrets, and verified callback,
artifact, billing, and staging smoke behavior. GCloud remains the
sensitive/reference fallback.
```
