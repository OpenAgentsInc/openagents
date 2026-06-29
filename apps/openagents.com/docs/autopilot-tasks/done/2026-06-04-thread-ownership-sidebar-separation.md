# Autopilot Task: Thread Ownership Sidebar Separation

Status: complete; merged

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `Artanis` / `agent_artanis`, or another write-capable private
OpenAgents product surface implementation agent selected by preflight.

Team: OpenAgents core / `team_openagents_core`

Project: OpenAgents product surface workroom navigation and thread ownership clarity. Resolve the
concrete project ID through operator preflight before dispatch; do not invent
one in the runner prompt.

Visibility: private/operator-visible during implementation. The resulting
sidebar behavior ships to authenticated users after tests and production smoke
pass.

Public route or observer link: existing authenticated product routes:

- `/`
- `/t/:threadId`
- `/teams/:teamRef/chat`
- `/teams/:teamRef/projects/:projectRef/chat` if project workrooms are enabled

## Dispatch Attempt

2026-06-04 operator preflight passed for `chris@openagents.com`,
`team_openagents_core`, `project_artanis`, and `agent_artanis`: migrations,
team/project agent metadata, ChatGPT/Codex provider account, GitHub writeback,
SHC control, runner callback config, and callback lag were all `ok`.

The previous Stripe durable goal in the same Artanis project scope had already
merged and was archived through the operator goal `clear` action before
dispatch so this task would not inherit the old Stripe objective.

New private Autopilot run:

- run: `7c6c23ad-49b4-4c82-beec-86af742b3840`
- goal: `agent_goal_3f2787399c5743d48fadfe54597aff60`
- branch: `openagents/autopilot-thread-ownership-sidebar`
- PR title: `Separate sidebar thread ownership`
- commit referenced by launch: `e5e40a30de86975361ff8dbac9a9c34899999da5`

Run `7c6c23ad-49b4-4c82-beec-86af742b3840` reached local patch activity but
timed out before pushing a branch or opening a PR. No PR or remote branch was
present for `openagents/autopilot-thread-ownership-sidebar` after the timeout.

The operator launched a narrower retry with a longer runner timeout:

- retry run: `60fcd25d-2301-4662-9d2d-35214ab8add4`
- retry goal: `agent_goal_35a62714daae4e20aa1e19904b3ed9b1`
- branch: `openagents/autopilot-thread-ownership-sidebar`
- PR title: `Separate sidebar thread ownership`
- retry focus: typed ownership metadata, `My threads` / `Team threads`, focused
  projection/view tests, commit, branch push, PR, and `result.md`.

Retry run `60fcd25d-2301-4662-9d2d-35214ab8add4` completed and opened PR #56:

- PR: `https://github.com/OpenAgentsInc/openagents/pull/56`
- Autopilot commit: `4f5e5d52e60b5e9f1ffa0abd7921cb4eae555a81`
- foreground review fix: `9023c01b9812aaa2361d4e2cc361e1956c66e109`
- merge commit: `43dc89da63a8b2387a57afde5b890a7a9c5c553d`

Foreground review found and fixed one cross-scope snapshot issue before merge:
workspace mission snapshots and team mission snapshots now replace only their
owned mission group while preserving the other section, unless the incoming
snapshot explicitly carries rows for that other ownership group.

Merged verification on `main`:

```text
bun run --cwd apps/web test src/page/loggedIn/update.test.ts src/page/loggedIn/sync/projection.test.ts src/page/loggedIn/view.scene.test.ts
bun run --cwd workers/api test src/omni-runs.test.ts
bun run --cwd apps/web typecheck
bun run --cwd workers/api typecheck
bun run check:deploy
```

All listed checks passed after merge. No production deployment was performed in
this operator pass.

## Dispatch Gate

Do not launch this task until the programmatic Autopilot runbook gate is
satisfied:

- operator preflight reports migrations, project/agent presence, provider
  health, SHC health, callback config, and GitHub writeback readiness;
- reconnect-required provider states are caught before dispatch;
- SHC callback payload contracts and retry/backfill paths are covered;
- run continuation attaches to the same durable goal;
- private goal/run observation can show current progress without exposing
  private delivery mechanics.

Source runbook:
`2026-06-04-programmatic-autopilot-operator-runbook.md`

Thread-sidebar-specific preflight must also confirm:

- the browser bundle and Worker are on current `main`;
- the sidebar status-dot mapping and newest-first sort from commits
  `499cc3a8` and `54142b8e` are present;
- the current product policy for project workrooms is known before deciding
  whether project threads are visible, hidden, or nested under team context;
- no UI copy introduces implementation mechanics, sync internals, SHC, gcloud,
  callback, provider, or grant language.

This is an Autopilot-owned product implementation task. The foreground coding
agent should only administer the goal/run and repair Autopilot infrastructure
defects that block honest execution or reporting.

## Objective

Make the sidebar clearly distinguish personal threads from team-owned threads.

The current sidebar has one `Threads` list whose rows can come from personal
runs, team runs, and project-scoped runs. That makes it unclear whether a run
belongs to the individual user, the OpenAgents core team, or a project room.
Completed runs can also move or disappear when project filters and sync
snapshots reproject the same row.

Implement a product model where:

- personal runs appear under `My threads`;
- team runs appear under `Team threads`;
- project-scoped runs appear as team-owned project work, not as ambiguous
  personal work;
- the active `/t/:threadId` row remains visible and highlighted even if it is
  project-scoped or no longer newest;
- each section sorts by most recent activity descending;
- status dots only indicate run state, never ownership.

Recommended UX direction:

- Keep the existing compact dark sidebar.
- Replace the single `Threads` section with two collapsible sections:
  `My threads` and `Team threads`.
- If project workrooms are disabled, still keep the active project-scoped
  thread visible under `Team threads` so the user can tell what they are
  viewing.
- If project workrooms are enabled, group or annotate project-scoped rows
  within `Team threads` using the existing small `detail` text, for example
  the repo or project slug. Do not add explanatory paragraphs.
- Preserve `Team rooms`, `Projects`, and `Files` navigation behavior unless a
  local change is required to make ownership consistent.

Do not add a new dashboard, broad navigation redesign, or marketing copy. This
task is only about thread ownership clarity in the workroom sidebar.

## Current Starting Point

Recent foreground work already changed several adjacent behaviors:

- `499cc3a8 Sort sidebar threads by latest activity`
  - mission rows carry `updatedAt`;
  - thread rows sort newest-first;
  - queued rows are neutral gray;
  - failed/canceled rows are red instead of blue queued.
- `54142b8e Keep current thread visible in sidebar`
  - the current `/t/:threadId` row remains visible even when project-scoped;
  - the active thread row has a yellow border/background distinct from the
    status dot.

Those fixes should be preserved.

The current ownership model is still too coarse:

- `apps/web/src/page/loggedIn/model.ts` models `SidebarSessionSection` and
  `SidebarSessionItem`, but sidebar session rows do not have an explicit
  owner/scope tag such as `personal`, `team`, or `project`.
- `apps/web/src/page/loggedIn/sync/projection.ts` projects active run responses
  into sidebar mission rows.
- `workers/api/src/omni-runs.ts` projects Worker-side agent runs into `missions`
  sync rows, but the mission projection should be audited to make sure it
  includes enough ownership metadata for the browser to section rows
  deterministically.
- `apps/web/src/page/loggedIn/sync/transitions.ts` applies workspace and team
  sync snapshots and patches into the sidebar.
- `apps/web/src/page/loggedIn/view.ts` maps `SidebarModel` into
  `Ui.WorkroomSidebarSessionSection` for rendering.
- `apps/web/src/ui/workroom.ts` renders compact collapsible sidebar sections
  and active row styling.
- `apps/web/src/product-policy.ts` currently disables project workrooms by
  default and owns the project mission visibility policy.

Known live confusion that this task must resolve:

- a completed project-scoped thread could show as green, be opened, and then no
  longer be obviously present in the thread list;
- users cannot tell whether a thread is personal work or OpenAgents core team
  work;
- color alone is being read as ownership or queue state, which is wrong.

## Relevant Repo Files

Planning and runbooks:

- `2026-06-04-programmatic-autopilot-operator-runbook.md`
- `../2026-06-03-logged-in-sidebar-consolidation.md`
- `../2026-06-03-team-room-shared-history-autopilot-audit.md`
- `../2026-06-03-team-project-rooms.md`
- `../2026-06-04-effect-foldkit-codebase-audit.md`
- `../2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- `../../AGENTS.md`
- `../../DESIGN.md`

Browser sidebar and route model:

- `../../apps/web/src/product-policy.ts`
- `../../apps/web/src/route.ts`
- `../../apps/web/src/routing/startup.ts`
- `../../apps/web/src/page/loggedIn/model.ts`
- `../../apps/web/src/page/loggedIn/view.ts`
- `../../apps/web/src/page/loggedIn/page/chat.ts`
- `../../apps/web/src/page/loggedIn/runs/transitions.ts`
- `../../apps/web/src/page/loggedIn/team-chat/transitions.ts`
- `../../apps/web/src/page/loggedIn/sync/transitions.ts`
- `../../apps/web/src/page/loggedIn/sync/projection.ts`
- `../../apps/web/src/page/loggedIn/view.scene.test.ts`
- `../../apps/web/src/page/loggedIn/update.test.ts`
- `../../apps/web/src/ui/workroom.ts`
- `../../apps/web/src/ui/primitives.ts`

Worker and sync projection:

- `../../workers/api/src/omni-runs.ts`
- `../../workers/api/src/omni-handlers.ts`
- `../../workers/api/src/sync-routes.ts`
- `../../workers/api/src/team-chat.ts`
- `../../workers/api/src/team-chat-routes.ts`
- `../../packages/sync-schema/src/index.ts`
- `../../packages/sync-client/src/index.ts`
- `../../packages/sync-worker/src/index.ts`

## Implementation Direction

Use explicit ownership data instead of inferring from labels, colors, or route
strings.

Recommended model shape:

- add an ownership field to sidebar mission/session items, for example
  `owner: 'personal' | 'team' | 'project'`;
- carry `teamId`, `projectId`, and `ownerUserId` or equivalent nullable fields
  through sync mission projections where available;
- derive section membership from typed ownership fields:
  - no team ID: `My threads`;
  - team ID and no project ID: `Team threads`;
  - team ID and project ID: `Team threads`, with project detail retained;
- keep active-route visibility as a separate rule from section filtering;
- keep status as a separate enum from ownership.

If the existing sync schema cannot carry these fields without a shared schema
change, update the shared schema and both Worker/browser projections in the
same commit. Do not add ad hoc JSON field probing across multiple files when a
typed boundary is available.

## Autopilot Work Plan

1. Trace current thread-row data flow from Worker `agent_runs` rows through
   sync `missions` rows, browser snapshot/patch ingestion, sidebar model, and
   `workroomSidebar`.
2. Add explicit ownership metadata at the earliest stable projection boundary.
3. Split the sidebar model into `My threads` and `Team threads` sections while
   preserving newest-first sort inside each section.
4. Preserve current active-thread behavior:
   - active row stays visible;
   - active row is visibly highlighted;
   - active state is independent of status dot color.
5. Preserve current project-workroom policy:
   - do not globally enable project workrooms to make this task pass;
   - when project workrooms are disabled, show only active project-scoped
     threads if necessary for context;
   - when enabled, show project-scoped rows under team ownership.
6. Add tests for personal/team/project row separation, active project-thread
   visibility, newest-first ordering inside each section, and status/ownership
   independence.
7. Run focused tests, typecheck, full deploy gate, then deploy with the
   canonical OpenAgents product surface deploy command only if implementation changes are ready to
   ship.

## Product Copy Rules

Allowed sidebar labels:

- `My threads`
- `Team threads`
- `Team rooms`
- `Projects`
- `Files`

Allowed row details:

- repository name;
- project name or slug;
- short run status such as `running`, `completed`, `failed`, or `queued`;
- small owner/project metadata only if it fits the compact sidebar.

Disallowed user-facing copy:

- implementation mechanics such as sync, SHC, callback, dispatch, provider
  grants, runner IDs, gcloud, or internal route policy;
- explanatory paragraphs in the sidebar;
- color legends in the main product chrome.

## Safety Rules

- Do not expose private repo contents, runner payloads, provider refs,
  callback tokens, OAuth material, GitHub write grants, or raw sync payloads in
  UI copy, tests, docs, screenshots, or logs.
- Do not weaken backend authorization to make team/project rows appear.
- Do not trust browser-only filtering for team/project access. If ownership
  metadata changes affect access boundaries, update Worker authorization and
  tests in the same change.
- Do not reintroduce `/chat` as a route or redirect alias.
- Do not re-enable project workrooms globally unless the operator explicitly
  asks for that product change.

## Acceptance Criteria

- The sidebar no longer has one ambiguous `Threads` section for mixed
  ownership.
- Personal runs appear under `My threads`.
- Team and project runs appear under `Team threads`.
- The active thread row remains visible and highlighted when viewing
  `/t/:threadId`.
- Completed team/project rows show completed status without implying they are
  personal work.
- Newest thread activity appears first within each section.
- Long titles still truncate without pushing section carets or widening the
  sidebar.
- Scene tests cover the visible section labels and active-row highlight.
- Update/projection tests cover snapshot and patch ingestion for personal,
  team, and project rows.
- Worker or shared-schema tests cover any added mission ownership fields.
- `bun run check:deploy` passes.
- If deployed, production smoke checks include:
  - `/` authenticated workroom shell;
  - a known personal thread;
  - a known team/project thread;
  - row visibility and active highlight after opening `/t/:threadId`.

## Commit Input For Dispatch

Before dispatch, commit and push this task spec. The Autopilot launch input
must reference the commit that contains this file.

Suggested commit message for this delegation packet:

```text
docs: add thread ownership sidebar task packet
```

Launch input fields:

```json
{
  "repository": "OpenAgentsInc/openagents",
  "baseRef": "main",
  "taskSpecPath": "docs/autopilot-tasks/2026-06-04-thread-ownership-sidebar-separation.md",
  "agentId": "agent_artanis",
  "teamId": "team_openagents_core",
  "projectId": "<preflight-resolved-openagents-workroom-navigation-project-id>",
  "visibility": "private",
  "goal": "Separate OpenAgents product surface sidebar thread lists into clear personal and team-owned areas, preserving active thread visibility, newest-first ordering, project policy, and compact workroom design.",
  "delivery": "commit_or_pull_request_with_tests_and_deployment_notes"
}
```

## Suggested Private Run Summary

```text
OpenAgents product surface workroom sidebar now separates personal and team-owned threads, keeps the
current thread visible and highlighted, preserves newest-first ordering, and
keeps run status separate from ownership.
```
