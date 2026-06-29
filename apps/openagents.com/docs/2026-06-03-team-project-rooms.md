# Team Project Rooms

Autopilot team rooms can now contain project-scoped chat rooms. The first
production project is `Artanis` under `OpenAgents Core Team`.

## Routes

- Team chat: `/teams/:teamSlug/chat`
- Project chat: `/teams/:teamSlug/projects/:projectSlug/chat`
- Team file: `/teams/:teamSlug/files/:fileId`

Project chats use the same logged-in app shell, sidebar, composer, file upload,
and Autopilot launch flow as team chat. They differ only by the extra
`project_id` scope.

## Data Model

`workers/api/migrations/0023_team_projects.sql` adds:

- `team_projects`, keyed by stable project id and scoped to a team.
- `team_chat_messages.project_id`, nullable for the normal team room.
- `agent_runs.project_id`, nullable for personal or team-level runs.

The seeded production row is:

```text
project_artanis | team_openagents_core | artanis | Artanis
```

## Chat Scoping

Team room messages are still stored in `team_chat_messages` with
`project_id IS NULL`.

Project room messages are stored in the same table with `project_id` set. API
reads filter by the route:

- `/api/teams/:teamId/chat/messages` returns only non-project team messages.
- `/api/teams/:teamId/projects/:projectId/chat/messages` returns only that
  project's messages.

OpenAgents Sync keeps separate in-memory buckets for each room:

- `teamId`
- `teamId:project:projectId`

That lets new project messages appear immediately without leaking into the
parent team room.

## Files

Project chat uploads use the room thread id:

```text
team:{teamId}:project:{projectId}:chat
```

Normal team chat uploads keep using:

```text
team:{teamId}:chat
```

File detail pages remain team-scoped and shareable to team members. Message
references link back to the correct team or project chat route based on the
referencing message's `project_id`.

## Autopilot Context

Project chat Autopilot launches include the parent team and project ids in the
server-side work order context, but the user-facing chat keeps showing the
normal message and run status. The project id is also stored on the generated
`agent_runs` row so the sidebar can keep project-originated threads grouped
with the correct room.

## 2026-06-03 Operator API Smoke

The operator launch path now preserves project scope for API-triggered runs.
`POST /api/omni/operator/agent-runs` accepts `teamId` and `projectId`, stores
both on `agent_runs`, and returns:

- `statusUrl`: admin-token readable `/api/omni/operator/agent-runs/:runId`
- `browserStatusUrl`: browser-session readable `/api/omni/agent-runs/:runId`
- `streamUrl`: the browser event stream URL

This matters because operator smoke tests use the admin API token, while normal
users still use browser session cookies.

Live Artanis project smoke evidence:

```text
run: cf44c410-3f0a-40a1-a3f6-4086091bc28a
teamId: team_openagents_core
projectId: project_artanis
status: completed
externalRunId: shc-codex:oa-shc-katy-01:cf44c410-3f0a-40a1-a3f6-4086091bc28a
events: 123
token usage rows: 15
token total: 174,674
branch: openagents/artanis-api-smoke-2
```

The provider account was healthy and available before launch, and SHC returned
OpenCode/Codex token events during the run. The remaining product work is to
make project-scoped runs first-class in the project chat UI rather than only
launching them through the operator API.

## 2026-06-04 Operator Project Chat API

`/api/omni/operator/team-chat/messages` now lets operator smokes exercise the
same team/project chat code path that the browser uses:

- `GET` lists visible room messages for a target user, team, and optional
  project.
- `POST` inserts the durable team/project message, scopes file context, launches
  Autopilot for `autopilot_intent`, publishes OpenAgents Sync patches, and
  returns the same project-scoped payload as the browser route.
- The route requires the admin API token, then resolves the target user with the
  existing operator selector fields such as `email`.

This is deliberately separate from `/api/omni/operator/agent-runs`: raw
operator runs prove SHC dispatch, while operator team-chat messages prove the
actual room workflow that users see.

## 2026-06-04 Artanis Answer-Back Verification

The first project-chat smoke through the new operator route proved project
scope and SHC dispatch, but exposed a bad answer-back source. The parent room
answer was selected from a late assistant progress note instead of the useful
run result. A second smoke proved lifecycle cleanup was filtered, but still
showed that progress text could win when the runner did not expose a final
assistant answer.

The Worker answer-back resolver now:

1. Looks for a completed `result.md` artifact event.
2. Reads `result.md` from the pushed GitHub branch using the user's server-side
   GitHub write connection token when the repository is private.
3. Falls back to assistant text events only when the artifact cannot be read.
4. Continues to ignore lifecycle, cleanup, receipt, and generic completion
   events.

Live production evidence:

```text
run: 62fac3fa-56e1-4aee-b672-51999f3dacf2
teamId: team_openagents_core
projectId: project_artanis
threadId: d02fdda4-6362-4ec5-ab82-6462db1b73ec
status: completed
events: 103
branch: openagents/artanis-auth-artifact-answerback-check
result.md: Authenticated artifact reads now drive the Artanis project chat answer.
visible answer: Authenticated artifact reads now drive the Artanis project chat answer.
```

The previous stale smoke answer was repaired to the corresponding `result.md`
sentence and a `team:team_openagents_core` sync `put` was published so connected
clients receive the corrected message.

## 2026-06-04 Project Agent Projection And Run Cards

`workers/api/migrations/0024_project_agent_metadata.sql` adds a first Artanis
agent projection to `team_projects.metadata_json`. The Worker reads that
metadata into the authenticated session bootstrap, and the Foldkit project room
renders it in the side panel only for project rooms that have agent data.

The current Artanis projection is intentionally compact:

```text
Agent: Artanis
Status: active
Scope: project
Runtime: Autopilot
Backend: SHC
Repo: openagents
Focus: Pylon
```

Team rooms without files, active runs, artifacts, diagnostics, or project-agent
metadata still render without the right panel.

Parent room Autopilot cards now use a compact run summary instead of generic
child-thread language. On launch, the parent `team_chat_messages.metadata_json`
stores `runSummary` with status, runtime, backend, repository, event count,
tool-call count, token total, duration, and `updatedAt`. When a completed run
posts its answer back, the Worker updates the parent row with the final summary
and publishes a `team:<teamId>` sync `put`, so refreshed or connected clients
can show the run card without loading the child thread first.

The card remains linked to `/t/:runId`, but the visible copy is now status and
stats such as:

```text
Autopilot run
opencode_codex on shc_vm
Succeeded in 3m 20s
42 events
7 tool calls
31556 tokens
```

The parent room no longer shows `linked child thread` or `Open full Autopilot
thread.` fallback copy.

## 2026-06-04 Public Artanis Pylon Campaign Surface

Artanis now has a public campaign/proof route for the Pylon release plan:

```text
https://openagents.com/artanis
https://openagents.com/agents/artanis
```

Both routes stay public and load outside the authenticated product shell. The
short `/artanis` route is the livestream/referral-friendly link; the
`/agents/artanis` route remains the canonical public-agent shape.

The public page loads two snapshots:

- the sanitized public Artanis durable goal projection for `agent_artanis`;
- the public Pylon stats snapshot from `GET /api/public/pylon-stats`, which
  now projects OpenAgents product surface-owned Pylon API registration and heartbeat state for
  Pylon v0.2.5+ clients.

The visible campaign objective is to release the next Pylon version, connect it
more deeply to OpenAgents product surface, route more inference and fine-tuning work to the live
Pylon wave, and use the new Bitcoin infrastructure as the work settlement
layer. The stats panel shows feed state, Pylons online, registered Pylons,
wallet-ready Pylons, assignment-ready Pylons, 24-hour seen count, minimum
client version, resource/client-version breakdowns, and compact recent-Pylon
rows. These online stats are not accepted-work, payout, or settlement evidence;
Nexus/Pylon receipt refs remain the separate settlement proof path.

The route is intentionally a public proof surface, not an operator control
plane. It must not expose SHC callback tokens, provider refs, hidden steering,
raw runner payloads, private repository contents, raw shell output,
`payloadJson`, or unredacted credentials.
