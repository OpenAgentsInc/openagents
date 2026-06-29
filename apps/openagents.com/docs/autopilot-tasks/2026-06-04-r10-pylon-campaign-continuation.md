# Autopilot Task: R10 Pylon Campaign Continuation

Status: dispatched; first OpenAgents product surface public-surface slice merged, continuation needed

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `Artanis` / `agent_artanis`

Public route: `https://openagents.com/artanis`

## Dispatch Attempt

2026-06-04 operator preflight passed for `chris@openagents.com`,
`team_openagents_core`, `project_artanis`, and `agent_artanis`: migrations,
team/project agent metadata, ChatGPT/Codex provider account, GitHub writeback,
SHC control, runner callback config, and callback lag were all `ok`.

New public Autopilot run:

- run: `f5467248-2703-4938-bbe4-b3e992182ea1`
- goal: `agent_goal_c990333ba75e47a399cc487a52fd59c2`
- branch: `openagents/autopilot-r10-pylon-campaign`
- PR title: `Continue R10 Pylon campaign`
- commit referenced by launch: `ae7912549301df1a0df78353d47f64196ad6faf6`
- initial implementation slice: OpenAgents product surface-visible public Artanis
  goal/activity/Pylon stats improvements, safe receipts, stale/unavailable
  stats behavior, public projection safety, and typed blockers for unavailable
  Pylon integration homes.

Run `f5467248-2703-4938-bbe4-b3e992182ea1` ended with SHC status `canceled`
after opening PR #57. The branch was still coherent and locally verified, so
the foreground operator reviewed and merged the PR:

- PR: `https://github.com/OpenAgentsInc/openagents/pull/57`
- Autopilot commit: `8496fc9e3b7302d8401f55a732a4c90bacac5267`
- merge commit: `948304c2a095b6a38472dbec10cd835f06508c42`

Merged first-slice behavior:

- public goal activity now carries sanitized public-safe `commitRefs`,
  `artifactRefs`, and `receiptRefs`;
- unsafe control-plane/provider/callback/secret-shaped refs are filtered out of
  public projection;
- `/artanis` shows current run ID and public-safe refs in activity rows;
- stale Nexus stats timestamps are treated as unavailable rather than live.

Merged verification on `main`:

```text
bun run --cwd workers/api test src/public-pylon-stats.test.ts src/agent-goal-routes.test.ts
bun run --cwd apps/web test src/docs-blog-route.test.ts src/page/loggedOut/page/login.scene.test.ts
bun run --cwd workers/api typecheck
bun run --cwd apps/web typecheck
bun run check:deploy
```

All listed checks passed. No production deployment was performed in this
operator pass.

Next continuation should keep the same public Artanis goal when possible and
move beyond public projection polish into the actual Pylon release/integration
work: identify the current Pylon implementation home, add or document the next
release artifact, route a bounded inference or fine-tuning work slice toward
Pylons, and project accepted-work / Bitcoin-accounting receipts safely.

## Dispatch Gate

Do not launch this task until the programmatic Autopilot runbook recommendations
are complete enough for reliable delegation:

- operator preflight exists and reports migrations, project/agent presence,
  provider health, SHC health, callback config, and GitHub writeback readiness;
- reconnect-required provider states are caught before dispatch;
- SHC callback payload contracts and retry/backfill paths are covered;
- run continuation attaches to the same durable goal;
- public/team goal observation can show the current run without exposing
  private delivery mechanics.

Source runbook:
`2026-06-04-programmatic-autopilot-operator-runbook.md`

This is an Autopilot-owned product task. The foreground coding agent should only
administer the goal/run and repair Autopilot infrastructure defects that block
honest execution or reporting.

## Objective

Continue the R10 Pylon campaign by having Artanis publicly drive the next Pylon
release:

- release the next version of Pylon;
- connect Pylon more deeply to the current OpenAgents product surface `openagents.com` codebase;
- send increasing inference and fine-tuning work to the live Pylon wave;
- use the new Bitcoin infrastructure as the settlement/accounting layer for
  accepted Pylon work;
- keep the public Artanis route useful as a livestream/referral proof page even
  while the work is still in progress.

The public narrative should be simple: work that used to wait on direct
foreground implementation is now being built by Autopilot, with Artanis pursuing
a visible durable goal and the Pylon network stats shown beside it.

## Current OpenAgents product surface Starting Point

The foreground implementation already added the public Pylon proof surface:

- `/artanis` maps to `PublicAgentRoute({ agentRef: 'artanis' })`.
- `/agents/artanis` remains the canonical public-agent route.
- `GET /api/public/pylon-stats` now projects OpenAgents product surface-owned Pylon API
  registration and heartbeat state for Pylon v0.2.5+ clients.
- The public page renders campaign objective, current public goal when present,
  Nexus connection, Pylon counters, recent Pylon rows, and sanitized activity.

Relevant OpenAgents product surface files:

- `../2026-06-04-openai-codex-goal-implementation-audit.md`
- `../2026-06-03-team-project-rooms.md`
- `../../apps/web/src/route.ts`
- `../../apps/web/src/page/loggedOut/page/publicAgent.ts`
- `../../apps/web/src/page/loggedOut/update.ts`
- `../../apps/web/src/page/loggedOut/model.ts`
- `../../workers/api/src/public-pylon-stats.ts`
- `../../workers/api/src/public-pylon-stats-routes.ts`
- `../../workers/api/src/public-pylon-stats.test.ts`

Historical Laravel reference files:

- `../../../deprecated/openagents.com/routes/web.php`
- `../../../deprecated/openagents.com/app/Services/NexusStats.php`
- `../../../deprecated/openagents.com/resources/js/pages/stats.tsx`

Production/public links:

- `https://openagents.com/artanis`
- `https://openagents.com/agents/artanis`
- `https://openagents.com/api/public/pylon-stats`
- `https://nexus.openagents.com/api/stats`

## Commit Input For Dispatch

Before dispatch, commit and push this task spec and the current OpenAgents product surface public
Pylon surface changes. The Autopilot launch input should reference the commit
that contains this file.

Suggested commit message for this delegation packet:

```text
docs: add R10 Pylon campaign Autopilot task
```

Launch input fields:

```json
{
  "repository": "OpenAgentsInc/openagents",
  "baseRef": "main",
  "taskSpecPath": "docs/autopilot-tasks/2026-06-04-r10-pylon-campaign-continuation.md",
  "agentId": "agent_artanis",
  "projectId": "project_artanis",
  "teamId": "team_openagents_core",
  "visibility": "public",
  "goal": "Release the next version of Pylon, connect it deeply to OpenAgents product surface, and route more inference and fine-tuning work to the live Pylon wave using the new Bitcoin infrastructure.",
  "delivery": "commit_or_pull_request_with_tests_and_public_artifacts"
}
```

Do not include provider tokens, callback tokens, OAuth material, local secret
paths, or private runner prompts in the launch payload.

## Autopilot Work Plan

1. Confirm the public Artanis goal exists, is attached to `agent_artanis`, and
   has public visibility. If the fallback campaign copy is still showing because
   no durable public goal exists, create or update the durable goal first.
2. Audit the current OpenAgents product surface public Pylon stats implementation and compare it to
   the old Laravel `/stats` behavior. Preserve the old public counters unless a
   counter is no longer present in Nexus.
3. Make `/artanis` the public campaign page for the run: current goal, current
   run/continuation, Nexus connection, Pylon stats, sanitized activity, and
   receipts.
4. Add or complete the live public sync path for Artanis goal/activity/Pylon
   stats if the snapshot-only path is still current at dispatch time.
5. Identify the current Pylon implementation home and the OpenAgents product surface integration
   points. Do not guess across repositories; if a required repo or branch is
   unavailable, record a typed blocker instead of fabricating the integration.
6. Implement the next Pylon release work in bounded slices:
   - OpenAgents product surface-facing registration/status surface;
   - workload assignment path for inference and fine-tuning;
   - accepted-work accounting and receipt projection;
   - Bitcoin settlement/accounting integration;
   - public artifact/receipt links suitable for the `/artanis` audience.
7. Keep public projection safe. Public routes may show statuses, counts,
   summaries, commits, receipts, artifacts, and sanitized run events. They must
   not expose SHC callback tokens, provider refs, hidden steering prompts, raw
   runner payloads, private repository contents, raw shell output, `payloadJson`,
   or credentials.
8. Produce a commit or pull request with tests, deployment notes, and a public
   Artanis activity summary.

## Acceptance Criteria

- `https://openagents.com/artanis` is an anonymous public page and does not
  enter the private product shell.
- The page shows the durable public Artanis goal, not only fallback copy, once
  the goal has been created.
- Pylon stats show feed state, source refs, last refresh, Pylons online,
  registered Pylons, wallet-ready Pylons, assignment-ready Pylons, seen-24h
  count, minimum client version, and recent Pylon rows where available. Online
  and readiness counts do not prove accepted work, paid work, payout, or
  settlement.
- Unavailable OpenAgents product surface public Pylon stats are shown as unavailable rather than
  live.
- Public Artanis activity has no private control-plane material.
- The run records a clear current-run/continuation relationship under the same
  durable goal.
- Tests cover the public route, Pylon stats normalization, unavailable/stale
  stats behavior, public projection safety, and any new Pylon/OpenAgents product surface integration
  path.
- The final artifact includes commit/PR link, test output, and deployment or
  deployment-blocker notes.

## Suggested Public Run Summary

```text
Artanis is releasing the next Pylon version and wiring Pylon deeper into OpenAgents product surface.
The public page shows the live Nexus/Pylon network state, the current goal, and
sanitized progress as the agent moves inference and fine-tuning work toward the
Pylon network with Bitcoin-backed accounting.
```
