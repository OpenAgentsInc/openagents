# Autopilot Task: Effect-Driven Chat Demo Page

Status: complete; moved to `docs/autopilot-tasks/done`

Completion evidence:

- `/demo` and `/demo/order` routes landed with deterministic fixture data.
- demo playback is Schema-backed, Effect-driven, resettable, and covered by
  update/playback tests.
- the demo uses nested logged-in workroom state/components for project chat,
  thread, files, and file detail views.
- playback controls include replay/pause and spacebar play/pause handling.
- the customer-order recording flow and 15-second workroom flow are covered by
  focused tests and have been deployed in foreground sessions.

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `Artanis` / `agent_artanis`, or another write-capable private
OpenAgents product surface implementation agent selected by preflight.

Team: OpenAgents core / `team_openagents_core`

Project: OpenAgents product surface demo and workroom presentation. Resolve the concrete project ID
through operator preflight before dispatch; do not invent one in the runner
prompt.

Resolved dispatch project: `project_artanis`

Dispatch run:

- run ID: `0a7dc4b6-47a6-4a6b-af3a-32c778c9109f`
- goal ID: `agent_goal_72bc3a6b810b4beaa5701d36cf592f82`
- task spec commit:
  `4d33431d7bf82dc70d2d760e37d18a4d05588c60`
- status: failed before implementation because the selected ChatGPT/Codex
  provider account was invalidated by OpenAI
- callback state: no callback lag at cursor `20`

Resume requirement: reconnect the target user's ChatGPT/Codex provider account
in Settings, then run the operator checklist again for `team_openagents_core`,
`project_artanis`, and the run ID above. If the checklist reports provider
health `ok`, continue the durable goal or launch the next run attached to
`agent_goal_72bc3a6b810b4beaa5701d36cf592f82`.

Visibility: private/operator-visible until the demo route is intentionally
enabled for recording. The implementation may expose a safe `/demo` route, but
it must not publish private provider, runner, repository, or billing state.

Public route or observer link: proposed local/staging route is `/demo`.
Production route is `https://openagents.com/demo` only after the route is
explicitly approved for public recording.

## Dispatch Gate

Do not launch this task until the programmatic Autopilot runbook
recommendations are complete enough for reliable delegation:

- operator preflight exists and reports migrations, project/agent presence,
  provider health, SHC health, callback config, and GitHub writeback readiness;
- reconnect-required provider states are caught before dispatch;
- SHC callback payload contracts and retry/backfill paths are covered;
- run continuation attaches to the same durable goal;
- private goal/run observation can show current progress without exposing
  private delivery mechanics.

Source runbook:
`2026-06-04-programmatic-autopilot-operator-runbook.md`

Demo-specific preflight must also report:

- whether `/demo` is local-only, staging-only, admin-gated production, or public
  production for a specific recording window;
- the demo route does not call real Autopilot launch, provider-account, Stripe,
  GitHub writeback, SHC, sync-stream, R2, or billing APIs;
- the demo route is not linked from normal product navigation unless the
  operator explicitly asks for that;
- project workrooms are not globally enabled just to make the demo pass;
- the scripted demo data contains no secrets, real provider refs, callback
  tokens, runner payloads, private branch logs, or private file contents;
- the 15-second playback is deterministic in tests and resettable by reloading
  `/demo`.

This is an Autopilot-owned implementation task. The foreground coding agent
should only administer the goal/run and repair Autopilot infrastructure defects
that block honest execution or reporting.

## Objective

Build a recordable demo page that shows the core OpenAgents product surface workroom journey over
roughly 15 seconds using the same logged-in workroom components and message
flow that the real app uses.

The page should demonstrate:

- the chat composer starting an Autopilot run;
- a team/project room receiving the user invocation;
- the compact parent-room Autopilot run card becoming active;
- run events arriving over the same sync-shaped data path used by the app;
- navigation into the full `/t/:threadId` run/workroom view;
- side-panel context, run diagnostics, artifacts, and uploaded files;
- navigation to team files and a file detail page with message references;
- a completed answer appearing back in the project/team room.

The demo must be driven by Effect. That means the scenario, playback schedule,
fixtures, and emitted events should be modeled as Schema data and Effect
services/layers. It must not be a DOM animation, a screenshot, or a parallel
mock UI that bypasses Foldkit update semantics.

## Current OpenAgents product surface Starting Point

OpenAgents product surface currently has no `/demo` route.

Important current route facts:

- `apps/web/src/route.ts` owns browser route schemas. It includes personal
  root chat, team chat, project chat, files, file detail, thread, billing,
  usage, settings, docs, blog, and public agent routes.
- `/` is the authenticated personal chat product surface. `workers/api/src/index.ts`
  currently returns `notFound()` for exact `/chat`, while unmatched non-API
  routes fall through to the app shell.
- `apps/web/src/routing/startup.ts` decides whether a browser route is logged
  out, logged in, redirected, invite-gated, or onboarding-gated.
- `apps/web/src/product-policy.ts` disables project workrooms by default, but
  route/view/model code already contains project-room support.
- `workers/api/src/worker-routes.ts` serves the app shell for non-API routes
  after exact routes, route groups, `/t/:threadId`, `/api/*`, and `/assets/*`.

The relevant existing browser flow is:

- `apps/web/src/page/loggedIn/model.ts` defines Schema-backed state for
  `chatMessages`, `chatRun`, `teamChatMessagesByTeam`, `threadFilesByScope`,
  `threadFileDetailsById`, `agentGoalPanel`, `sidebar`, and sync state.
- `apps/web/src/page/loggedIn/page/chat.ts` renders the real workroom using
  `Ui.workroomTimeline`, `Ui.workroomComposer`, `Ui.workroomFilePanel`,
  project-agent metadata rows, run rows, artifacts, uploaded files, goal dock,
  and run metadata dialog.
- `apps/web/src/page/loggedIn/view.ts` renders the shared `Ui.workroomShell`,
  desktop sidebar, mobile sidebar, route main area, chat route, team room,
  project room, thread route, team files, and file detail pages.
- `apps/web/src/page/loggedIn/team-chat/transitions.ts` detects exact
  `@autopilot` messages, posts team/project chat messages, and projects launch
  responses into `chatRun`, sidebar missions, and sync snapshot commands.
- `apps/web/src/page/loggedIn/runs/transitions.ts` handles personal launch,
  `/t/:threadId` entry, launch success, fetch success, sync run projection,
  metadata dialog state, and sidebar mission projection.
- `apps/web/src/page/loggedIn/sync/transitions.ts` applies
  `SucceededLoadSyncSnapshot`, `ReceivedSyncPatch`, cursor gaps, team sync
  collections, agent-run collections, and agent-goal collections.
- `apps/web/src/subscriptions.ts` already uses Effect streams for sync
  WebSocket payloads and active run polling derived from model state.

Relevant implementation docs:

- `../2026-06-03-logged-in-sidebar-consolidation.md`
- `../2026-06-03-team-room-shared-history-autopilot-audit.md`
- `../2026-06-03-team-project-rooms.md`
- `../2026-06-03-thread-message-archive-support.md`
- `../2026-06-04-effect-foldkit-codebase-audit.md`
- `../2026-06-04-openagents-broader-effect-refactor-audit.md`
- `../2026-06-04-openagents-zero-tech-debt-caller-inventory.md`

Relevant repo files:

- `../../DESIGN.md`
- `../../apps/web/src/route.ts`
- `../../apps/web/src/routing/startup.ts`
- `../../apps/web/src/main.ts`
- `../../apps/web/src/model.ts`
- `../../apps/web/src/message.ts`
- `../../apps/web/src/update.ts`
- `../../apps/web/src/view.ts`
- `../../apps/web/src/subscriptions.ts`
- `../../apps/web/src/product-policy.ts`
- `../../apps/web/src/page/loggedIn/model.ts`
- `../../apps/web/src/page/loggedIn/message.ts`
- `../../apps/web/src/page/loggedIn/update.ts`
- `../../apps/web/src/page/loggedIn/view.ts`
- `../../apps/web/src/page/loggedIn/page/chat.ts`
- `../../apps/web/src/page/loggedIn/page/files.ts`
- `../../apps/web/src/page/loggedIn/chatState.ts`
- `../../apps/web/src/page/loggedIn/runs/transitions.ts`
- `../../apps/web/src/page/loggedIn/runs/commands.ts`
- `../../apps/web/src/page/loggedIn/team-chat/transitions.ts`
- `../../apps/web/src/page/loggedIn/team-chat/commands.ts`
- `../../apps/web/src/page/loggedIn/sync/transitions.ts`
- `../../apps/web/src/page/loggedIn/sync/projection.ts`
- `../../apps/web/src/page/loggedIn/thread-files/transitions.ts`
- `../../apps/web/src/page/loggedIn/thread-files/commands.ts`
- `../../apps/web/src/page/loggedIn/goals/view.ts`
- `../../apps/web/src/ui/workroom.ts`
- `../../packages/sync-schema/src/index.ts`
- `../../packages/sync-client/src/index.ts`
- `../../workers/api/src/index.ts`
- `../../workers/api/src/worker-routes.ts`
- `../../workers/api/src/team-chat.ts`
- `../../workers/api/src/team-chat-routes.ts`
- `../../workers/api/src/thread-file-routes.ts`

Local references:

- `../../../projects/repos/foldkit/examples/`
- `../../../projects/repos/foldkit/packages/foldkit/src/`
- `../../../projects/repos/effect-cf/`
- Effect guidance loaded through `effect-solutions show basics
services-and-layers data-modeling error-handling testing`

Safe routes or links that may appear in the demo:

- `/demo`
- `/demo/t/pylon-release-demo`
- `/demo/teams/openagents-core-team/projects/artanis/chat`
- `/demo/teams/openagents-core-team/files`
- `/demo/teams/openagents-core-team/files/file_pylon_release_plan`

The exact prefixed route set is an implementation decision, but keep the demo
under a demo namespace or make it otherwise impossible for recording fixtures
to collide with real product routes.

Do not include real OpenAI, Stripe, GitHub, SHC, Cloudflare, callback, session,
or billing secrets in the packet, implementation, fixtures, screenshots, logs,
or run output.

## Core Demo Journey

Use one coherent story, not a sampler page. The recommended 15-second journey:

| Time  | Visible State                                                                                                                                                                     |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0.0s  | `/demo` opens into an OpenAgents workroom shell with the OpenAgents Core Team, Artanis project, a visible project side panel, and an uploaded file named `pylon-release-plan.md`. |
| 1.0s  | The composer fills with `@autopilot prepare the Pylon release briefing from the attached plan`.                                                                                   |
| 1.8s  | The prompt submits. A project-room user message appears and the compact `Autopilot run` card enters queued/running state.                                                         |
| 3.0s  | Sync-shaped run events arrive: accepted, dispatched to `oa-shc-katy-01`, repository checkout, and file context loaded.                                                            |
| 5.0s  | The run card updates with runtime/backend, event count, tool-call count, and token count. The side panel shows run status, tokens, `pylon-release-plan.md`, and `result.md`.      |
| 6.5s  | The demo navigates to the full thread/workroom view for `/t/pylon-release-demo`, showing the same run as a full event timeline.                                                   |
| 8.5s  | More events arrive: shell command completed, artifact written, and a concise assistant result. The metadata button opens or becomes visibly available.                            |
| 10.5s | The demo navigates back to the Artanis project room. The parent room now contains the completed answer-back message and the run card says completed.                              |
| 12.0s | The demo navigates to team files. The file table shows `pylon-release-plan.md` and `result.md`.                                                                                   |
| 13.5s | The demo opens `pylon-release-plan.md` detail. The reference list shows the invoking message and Autopilot answer that used the file.                                             |
| 15.0s | The demo settles on the file detail or returns to the project room, ready for the screen recording to stop.                                                                       |

The route may include playback controls if useful for recording, but avoid
visible copy explaining implementation mechanics. Acceptable controls are
short operator-facing labels such as `Replay`, `Pause`, and `15s`.

## Commit Input For Dispatch

Before dispatch, commit and push this task spec. The Autopilot launch input
must reference the commit that contains this file.

Suggested commit message for this delegation packet:

```text
docs: add Effect chat demo task packet
```

Launch input fields:

```json
{
  "repository": "OpenAgentsInc/openagents",
  "baseRef": "main",
  "taskSpecPath": "docs/autopilot-tasks/2026-06-04-effect-driven-chat-demo-page.md",
  "agentId": "agent_artanis",
  "teamId": "team_openagents_core",
  "projectId": "<preflight-resolved-openagents-demo-project-id>",
  "visibility": "private",
  "goal": "Implement a deterministic Effect-driven /demo route that reuses the real OpenAgents product surface logged-in workroom components and shows a 15-second chat-to-agent-to-thread-to-files journey with Schema-backed demo data.",
  "delivery": "commit_or_pull_request_with_tests_and_recording_notes"
}
```

Do not include provider tokens, callback tokens, OAuth material, local secret
paths, session cookies, raw runner payloads, or private runner prompts in the
launch payload.

## Autopilot Work Plan

1. Read the referenced docs and app files. Confirm the current route, startup,
   model, update, subscription, and workroom component ownership before
   editing.
2. Design a demo architecture that nests or wraps the existing logged-in
   workroom model rather than reimplementing chat UI. Prefer a new
   `apps/web/src/page/demo/` module that owns demo state while rendering the
   existing `LoggedIn.view` or existing workroom component families.
3. Add Schema models for demo ids, scenario metadata, cue timings, playback
   state, route targets, and fixture records. Keep long-lived demo state as
   tagged unions, not nullable flags.
4. Add `DemoScenarioService` as a `Context.Service` with a live fixture layer
   and test layer. The service should return decoded `AuthBootstrap`,
   `AgentGoalResponse`, `TeamChatMessagesResponse`, `TeamChatPostResponse`,
   `AgentRunLaunchResponse` or `AgentRunDetailResponse`, `SyncSnapshot`,
   `SyncPatch`, `ThreadFilesResponse`, and `ThreadFileDetailResponse` values.
5. Add `DemoPlaybackService` or equivalent as a service returning an
   `Effect.Stream` of timed demo cues. Use Effect `Clock`, `Duration`, and
   named `Effect.fn` methods. Tests should be able to drive time
   deterministically.
6. Add a demo route such as `DemoRoute` for `/demo`. If the implementation
   needs prefixed child routes, add them explicitly under `/demo/...` and map
   them to the underlying logged-in route semantics without leaking fixtures
   into real `/t`, `/teams`, or `/files` product routes.
7. Update startup routing so `/demo` can load without a real authenticated
   session while staying outside normal logged-out marketing surfaces. Do not
   use `/demo` as a back door to real authenticated APIs.
8. Add a demo model wrapper. It should contain the nested logged-in model,
   playback state, and scenario identity. The nested model should be created by
   the same `LoggedIn.init(...)` shape with a demo `AuthBootstrap`, team, and
   Artanis project.
9. Add parent/root messages for demo playback, for example `GotDemoMessage` and
   demo cue messages. Demo cues should translate into existing
   `LoggedIn.Message` values such as `UpdatedChatComposer`,
   `SubmittedChatComposer`, `SucceededPostTeamChatMessage`,
   `SucceededLoadSyncSnapshot`, `ReceivedSyncPatch`,
   `SucceededLoadThreadFiles`, and `SucceededLoadThreadFileDetail`.
10. When a demo cue calls `LoggedIn.update`, do not let returned production
    commands call real APIs. Either filter those commands in the demo wrapper
    and replace them with deterministic demo cues, or route them through
    demo-only command implementations backed by the demo services. Document the
    chosen boundary in code and tests.
11. Represent run progression through the same sync-shaped collections used by
    `sync/transitions.ts`: `missions`, `agent_goals`, `team_chat_messages`,
    `agent_runs`, `agent_run_events`, and `thread_files`.
12. Reuse `Ui.workroomShell`, `workroomSidebar`, `workroomTimeline`,
    `workroomComposer`, `workroomFilePanel`, `workroomMetadataDialog`,
    `applicationDetailScreen`, `tableList`, and existing icon catalog entries.
    Do not add a separate card-heavy marketing or explainer page.
13. Keep `/demo` visually consistent with `DESIGN.md`: dark-only, compact,
    mono-first, operational panes, short labels, no decorative gradients, no
    emoji, no public implementation narration.
14. Add an explicit replay/reset path. Reloading `/demo` should always restart
    the same 15-second scenario from cue zero. If pause/replay controls are
    added, they should be demo state messages, not direct timer mutation.
15. Ensure the scenario works on a normal desktop recording viewport. It should
    avoid text overlap, clipped buttons, unstable layout jumps, and oversized
    card shells.
16. Add docs or implementation notes near the demo module explaining the
    fixture/service boundary, how to update the scenario safely, and how to
    record it locally or in staging.

## Safety Rules

- Do not call live Autopilot launch, provider account, SHC, GitHub writeback,
  Stripe, billing, sync-stream, R2, D1, or admin APIs from `/demo`.
- Do not put real provider refs, secret refs, session cookies, callback tokens,
  raw runner payloads, private branch logs, private repository content, or
  private file contents in demo fixtures.
- Do not make project workrooms globally visible just for the demo. Scope any
  project-room rendering exception to the demo route or ship it only when the
  normal product gate is intentionally changed.
- Do not duplicate the workroom UI as a static mock. If the demo needs a new
  wrapper, it should compose existing models, messages, update functions, and
  UI primitives.
- Do not mutate the DOM directly for playback. Demo time should produce
  messages or route changes, and the view should follow from model state.
- Do not introduce `setTimeout`, `Date.now`, `Math.random`, ad hoc global
  timers, or local storage as the demo engine. Use Effect Clock/Duration,
  deterministic IDs, and explicit fixture records.
- Do not add raw `Effect.runPromise` bridges below the app entry/test
  boundary. Demo services should be normal services/layers.
- Do not add string/regex user-intent routing for the demo beyond bounded
  scripted fixtures. The demo prompt is a fixture, not a new intent selector.
- Do not expose `/demo` from public navigation or marketing copy unless the
  operator explicitly approves it after reviewing the route.

## Acceptance Criteria

Implementation:

- `/demo` opens a deterministic demo workroom using safe fixture data with no
  auth/session requirement.
- The demo uses existing workroom UI components and a nested or wrapped
  logged-in model rather than a parallel static mock.
- The 15-second journey shows project chat, Autopilot invocation, compact run
  card, run event progression, thread/workroom navigation, side-panel
  files/artifacts, team files, file detail, and answer-back.
- Demo playback emits Schema-backed messages/cues through Effect services or
  Effect streams.
- Demo data is shaped like real app data: `AuthBootstrap`, team/project
  metadata, `TeamChatPostResponse`, `AgentRunDetailResponse`,
  `SyncSnapshot`/`SyncPatch`, `ThreadFilesResponse`, and
  `ThreadFileDetailResponse`.
- Production app routes and API commands still behave exactly as before.
- Reloading `/demo` restarts the playback from the first cue.

Architecture:

- Demo scenario, playback, and fixtures live behind `Context.Service` contracts
  and `Layer` implementations.
- Service methods expose typed `Effect<Success, TaggedError, Requirements>`
  contracts and use `Effect.fn` for named effects.
- Expected demo decode/playback failures are tagged errors, not thrown strings.
- Long-lived demo model state is Schema-backed and uses tagged state.
- Returned production commands are not allowed to escape from the demo wrapper
  and call real APIs.

Tests and checks:

- Route/startup tests cover `/demo` for logged-out and authenticated visitors.
- Update tests prove demo cues produce the expected nested logged-in model
  changes without executing live API commands.
- Subscription or playback tests use deterministic time to step through the
  15-second sequence.
- Scene tests cover the major visible states: initial project room, active run
  card, full thread, team files, and file detail references.
- Existing logged-in route tests continue passing, especially root route,
  `/chat` policy, project route gating, team files, file detail, and thread
  route tests.
- Run `bun run typecheck:web`, `bun run test:web`, and
  `bun run check:deploy`, or document exact blockers if the Autopilot
  environment cannot complete the full deploy check.
- If a browser automation tool is available, capture at least one desktop
  screenshot around the 6-8 second mark and one around the final state to
  verify framing and lack of overlap.

Delivery:

- Produce a commit or pull request with the demo route, demo services, tests,
  and recording notes.
- Include clear notes about whether the route is local/staging/admin/public and
  how to toggle or remove it.
- Do not deploy a public `/demo` route until the operator reviews the exact
  visible fixture data.

## Suggested Private Run Summary

```text
Implemented an Effect-driven /demo route for a 15-second OpenAgents workroom
recording. The demo reuses the real logged-in workroom components, drives a
project chat to Autopilot run to thread to files journey through Schema-backed
demo fixtures and Effect playback services, and keeps all data synthetic and
isolated from production APIs.
```
