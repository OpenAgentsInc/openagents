# Autopilot Task: Programmatic Autopilot Operator Runbook

Status: active operator runbook packet; API implementation is committed,
pushed, deployed, and production-smoked. Use this packet as the current
programmatic dispatch and recovery contract.

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: foreground coding agent acting as Autopilot operator.

Team: `team_openagents_core`

Project: platform-level Autopilot operations. Use a concrete project only when
the delegated product task needs one.

Visibility: private or team-visible by default. Public visibility is only for
tasks whose product objective is explicitly observer-visible.

Public route or observer link: task-specific. Team/shared run links use
`/t/:runId`; public goal snapshots use the public goal APIs.

## Purpose

This packet is the canonical runbook for future coding-agent sessions that
need to spin up, monitor, recover, and continue Autopilot coding runs
programmatically.

The intended end state is that the foreground coding agent acts as an
Autopilot operator. Autopilot owns product implementation. The foreground agent
owns goal/run setup, preflight, provider readiness, SHC control readiness,
callback ingestion, continuation, sync visibility, and platform repairs that
block honest execution.

The historical audit remains at:

- `../2026-06-04-programmatic-autopilot-work-runbook-audit.md`

Use this task packet for launch mechanics. Use the audit for background on the
experiment and failure history.

## Dispatch Gate

Do not dispatch new product implementation packets until these conditions are
true in the pushed and deployed Worker:

- operator preflight reports D1 migration state, target user, team/project
  presence, project agent metadata, provider health, SHC health, callback
  config, GitHub writeback readiness, and target/current run where relevant;
- reconnect-required provider states return a typed
  `provider_reconnect_required` response before run creation;
- the Worker accepts the canonical SHC callback dialects listed below and tests
  them;
- operator callback retry can backfill pending SHC events by run ID without
  manually reconstructing callback JSON or using the runner callback token;
- operator continuation can queue a follow-up turn for an active SHC run or
  request a policy-gated continuation for a stopped run attached to a durable
  goal;
- the team/public observer surfaces show owner attribution and stable sidebar
  visibility without exposing private delivery mechanics;
- `scripts/autopilot-operator-checklist.mjs` or an equivalent command prints
  current readiness, callback lag, and the next safe action.

## Current Implemented Operator Surfaces

The current foreground session is adding these surfaces:

- `GET` or `POST /api/operator/autopilot/preflight`
- `GET` or `POST /api/omni/operator/autopilot/preflight`
- `GET` or `POST /api/omni/operator/autopilot/checklist`
- `GET` or `POST /api/omni/operator/agent-runs/:runId/checklist`
- `POST /api/omni/operator/agent-runs/:runId/callbacks/retry`
- `POST /api/omni/operator/agent-runs/:runId/continue`
- `scripts/autopilot-operator-checklist.mjs`

The current foreground session has already verified the focused SHC helper and
callback normalization tests, API typecheck, and zero-debt architecture guard.
Full deploy checks passed, the operator implementation was committed and
pushed, and production was deployed as Worker version
`1104923f-9754-4457-9da1-99e33eb9e16c`. Production checklist smoke passed for
the ImageGen target run `11a4ff12-601b-48f3-b596-34f947bfc4bb` with no callback
lag. The run is completed and attached to durable goal
`agent_goal_c964d70720954a989b99916e1e4ebcdf`; the next safe operator action is
goal continuation or launch of the next task packet attached to that goal.

## Operator Checklist Command

Use the script from the repo root after deployment:

```sh
OPENAGENTS_ADMIN_API_TOKEN=... \
  node scripts/autopilot-operator-checklist.mjs \
  --email chris@openagents.com \
  --teamId team_openagents_core \
  --projectId project_imagegen_support \
  --runId 11a4ff12-601b-48f3-b596-34f947bfc4bb
```

Use `--json` when another tool needs machine-readable output.

The command must not print provider tokens, callback tokens, OAuth material,
local secret paths, raw runner payloads, private prompts, or shell transcripts.

## Programmatic Flow

1. Write or locate a task packet under `docs/autopilot-tasks/`.
2. Commit and push the packet and any prerequisite specs.
3. Run the operator checklist for the target user/team/project/run.
4. Resolve any blocked checks before creating or continuing a run.
5. Create or update the durable goal.
6. Launch the run or queue a continuation using the programmatic API.
7. Monitor Cloudflare-backed goal/run state first.
8. Use SHC host inspection only for diagnosing control/runner failures.
9. Retry callbacks by run ID if callback lag appears.
10. Continue the same durable goal instead of creating unrelated duplicate
    runs.
11. Commit, push, and deploy only Autopilot platform fixes from the foreground
    session.
12. Leave product implementation to the Autopilot runner unless the user
    explicitly switches back to direct foreground implementation.

## Canonical Callback Contract

The Worker callback receiver must support:

- canonical `openagents.runner_event.v1` payloads;
- SHC job-event envelopes with top-level sequence/source/type/summary and
  optional `dataJson`;
- sparse SHC control events where Cloudflare supplies fallback sequence and
  current timestamp;
- tool/status events with richer detail under `dataJson`;
- redacted runner events where credential-shaped material is dropped before D1
  persistence.

Credential-shaped material includes OAuth refresh/access tokens, auth JSON,
provider account material, GitHub tokens, and other secret-like fields. Do not
store or print those payloads in docs, issue comments, logs, fixtures, or public
artifacts.

Regression coverage:

- `../../workers/api/src/omni-services.test.ts`
- `../../workers/api/src/omni-runs.test.ts`

## Continuation Rules

If a run is `queued`, `running`, or `waiting_for_input`, queue a follow-up turn
against SHC. Do not launch a second unrelated run.

If a run has stopped and has a durable `goalId`, request a policy-gated goal
continuation. The new run must remain attached to the same goal and carry the
same product objective unless the user explicitly edits the goal.

If a stopped run has no durable `goalId`, record a typed blocker. Do not invent
a goal relationship after the fact without an explicit operator/user decision.

## Observer Rules

Public or team observers may see:

- objective;
- current run ID;
- status;
- budget and usage counters;
- sanitized activity summaries;
- artifact, commit, PR, deployment, and receipt links that are safe to show.

Observers must not see:

- callback tokens;
- provider refs or OAuth material;
- hidden steering;
- raw runner payloads;
- private repo contents;
- raw shell logs;
- `payloadJson` fields;
- secrets or secret-shaped diagnostics.

## Autopilot Work Plan For Future Operator Improvements

1. Promote the current preflight/checklist endpoints to the only supported
   dispatch path for foreground agents.
2. Add a polished browser operator view over the same readiness data.
3. Add a dedicated public goal observer page if the existing public goal APIs
   plus `/t/:runId` route are not enough for observer-visible campaign work.
4. Add durable callback retry receipts so operators can see each backfill
   attempt and outcome without inspecting SHC files.
5. Add a "dispatch packet" API that accepts `taskSpecPath`, validates the
   packet exists at the pushed commit SHA, runs preflight, and creates or
   continues the durable goal in one typed operation.

## Acceptance Criteria

- Future coding agents can determine readiness with one command.
- Provider reconnect states are typed before launch.
- Callback lag can be repaired through the Worker.
- Continuation preserves durable goal identity.
- Team/public observer surfaces preserve owner attribution and sidebar
  visibility.
- Task packets can be dispatched without hidden chat context.
- Secrets and private delivery mechanics remain out of packets and observer
  surfaces.
