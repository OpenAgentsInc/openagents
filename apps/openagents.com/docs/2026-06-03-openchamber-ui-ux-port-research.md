# OpenChamber UI/UX Port Research

Date: 2026-06-03

Status: Research report for porting OpenChamber-style chat/workroom UX into
`openagents`, updated after the first sidebar consolidation pass landed.

Scope: This report studies the local `projects/repos/openchamber` and
`projects/repos/opencode` checkouts as reference material. It focuses on chat
structure, layout, runtime interaction with OpenCode, and concrete lessons for
the Foldkit/Effect OpenAgents Autopilot surface.

## Executive Summary

OpenChamber is not just a chat UI. It is a dense operational workroom around an
OpenCode server. The core product shape is a three-part workspace:

1. A session and project rail for navigating agent runs.
2. A central turn-based chat timeline with rich tool output, permissions,
   questions, diffs, todos, and status.
3. A right operational panel for Git, files, context, diffs, terminal, and
   other run-adjacent state.

The most important OpenChamber lesson for OpenAgents product surface is that the chat transcript is
the coordination surface, not the whole application. Chat messages, tool calls,
file changes, reviews, approvals, and runtime state are projected into one
workroom. The side panels are not secondary pages; they are live projections of
the same agent session.

OpenAgents product surface should not port OpenChamber's React, Zustand, Electron, or direct local
filesystem model. OpenAgents product surface should port the interaction model and event shape into
Foldkit/Effect:

- Treat agent runtime updates as typed events.
- Project events into turn records, activity groups, blockers, status rows, and
  context panels.
- Keep the runtime connector behind an authoritative OpenAgents Cloud/BFF
  boundary.
- Preserve dark operational density, compact controls, and first-class review
  affordances.

The current OpenAgents product surface chat model is still mostly string-message oriented. Before a
live OpenCode-style integration, OpenAgents product surface should introduce a typed workroom model
with message parts, tool parts, permission requests, question requests, status,
todo progress, changed-file summaries, and context panel state.

## 2026-06-03 Implementation Update

OpenAgents product surface has now completed the first narrow UI port from this research: the
logged-in left navigation and the chat/session rail were consolidated into one
OpenChamber-style workroom sidebar. The first pass landed in the Foldkit
browser app. A follow-up corrected production ownership so the logged-in
surface is now served by `apps/web`, while the Worker provides auth, API
routes, cookie refresh, and the `/api/auth/session` bootstrap payload.

Implemented work:

- GitHub issue: `OpenAgentsInc/openagents#8`
- Commit: `95de11cc fix: consolidate logged-in workroom sidebar`
- Follow-up implementation note:
  `docs/2026-06-03-logged-in-sidebar-consolidation.md`
- UI primitive: `apps/web/src/ui/index.ts` now exposes `workroomSidebar`,
  `workroomMobileSidebar`, and route wrapper helpers.
- Model contract: `apps/web/src/page/loggedIn/model.ts` now carries a
  schema-backed sidebar model with primary navigation, session sections, status
  and attention metadata, and footer rows.
- Thread sidebar follow-up: `/api/auth/session` now includes recent
  Worker-owned Autopilot runs as compact Threads. `apps/web` renders them under
  Team Rooms and links them to `/t/:missionId`, matching OpenChamber's
  session-list expectation without fabricating local threads. Thread clicks now
  hydrate `/api/omni/agent-runs/:missionId`, reconstruct the saved user turn
  from the persisted run goal, and render the saved SHC/OpenCode event
  timeline. New thread IDs are UUIDs; older `agent_run_<32 hex>` rows are
  exposed through UUID route aliases.
- Route ownership: `apps/web/src/page/loggedIn/view.ts` now owns the shared
  logged-in shell for Chat, Dashboard, Settings, and NotFound.
- Chat ownership: `apps/web/src/page/loggedIn/page/chat.ts` no longer renders a
  separate left rail; it owns the center timeline/composer, right review/context
  panel.
- Team-room routing: Team Rooms now deep link through clean app routes such as
  `/teams/openagents-core-team/chat` instead of sharing `/chat`; the production
  chat surface no longer renders the GitHub writeback smoke fixture as default
  state.
- Production shell: `workers/api/src/index.ts` now serves the Vite app shell
  for product routes instead of rendering authenticated product HTML in the
  Worker. `apps/web/src/main.ts` loads the authenticated session/team bootstrap
  from `/api/auth/session`.
- Worker test coverage: `workers/api/src/admin-access.test.ts` now asserts
  product routes return the app shell instead of Worker-rendered UI.
- Chat runtime follow-up:
  - `apps/web/src/page/loggedIn/update.ts` now submits chat composer turns to
    `/api/omni/agent-runs` with `runnerBackend: shc_vm`.
  - `apps/web/src/subscriptions.ts` polls `/api/omni/agent-runs/:runId` for
    real run details while chat-like routes are active.
  - `apps/web/src/page/loggedIn/page/chat.ts` renders source, runtime, backend,
    runner, repository, event, and token metadata from Worker/SHC events instead
    of hardcoded assistant prose.
  - `workers/api/src/omni-runs.ts` no longer returns fake dispatch results when
    live SHC dispatch is not configured.
  - The shared `RunnerBackend` schema and current Worker request parser no
    longer accept `local_fake`; old migration text remains historical schema
    state only.
- SHC/OpenCode projection follow-up:
  - `apps/web/src/page/loggedIn/page/chat.ts` now projects the noisy raw runner
    stream into OpenCode-style message parts: lifecycle, thinking, shell/tool
    execution, file/artifact changes, and final assistant text. Raw run and
    event payloads are no longer dumped into the transcript; they are available
    through the right-panel run diagnostics `i` dialog.
  - Chat submit now preserves the browser UX contract of Enter to submit and
    Shift+Enter for a newline.
  - `workers/api/src/omni-runs.ts` now passes repository clone/ref metadata to
    SHC and requires `github-writeback.json` whenever a GitHub work order is
    present.
  - The SHC workroom/control deployment now clones the target GitHub repository
    into the workroom, keeps runner-only notes out of the checkout, installs
    Bun and GitHub CLI on `oa-shc-katy-01`, streams Codex stdout/stderr JSON
    into `openagents-runner-events.jsonl` while the process is still running,
    and mirrors those events into the Worker/D1 callback path.
  - The event normalizer now understands current Codex/OpenCode JSON shapes
    such as `agent_message`, `command_execution`, `file_change`,
    `message.part.updated`, and `session.next.shell.*`, while continuing to
    omit high-volume text/tool delta records from durable public projection.
  - Live smoke `585f5094-9d8f-4a09-af78-f563320c00f4` was launched through
    `/api/omni/operator/agent-runs` as the target user. It checked out
    `OpenAgentsInc/openagents@main`, ran `bun --version`, committed and
    pushed a smoke receipt branch, wrote `result.md` and
    `github-writeback.json`, streamed 96 events into D1, and completed. The
    temporary smoke branch was deleted after verification.
- OpenAgents Sync follow-up:
  - `packages/sync-worker` and `workers/api` now expose D1-backed sync
    snapshots and `SyncRoomDurableObject` WebSocket replay for workspace,
    thread, and agent-run scopes.
  - `apps/web` keeps the sidebar thread list sync-backed through
    `workspace:{userId}`, opens `thread:{threadId}` streams on thread routes,
    and opens `agent-run:{runId}` streams while an active thread is visible.
  - Thread clicks now load thread snapshots first and only fall back to the
    compatibility detail API when sync has no data yet.
  - Active run polling is now fallback behavior after sync stream failure or
    closure, not the primary live transport.
  - Sync `agent_runs` and `agent_run_events` collections are converted into the
    existing OpenCode-style timeline parts, preserving readable chat ordering
    while keeping raw metadata in the run diagnostics dialog.
- OpenCode/Vortex motion follow-up:
  - OpenAgents product surface now ports the lightweight Vortex `opencode-motion` vocabulary from
    `vortex/components/autopilot/opencode-motion.tsx` and
    `vortex/app/globals.css` into `apps/web/src/styles.css` using the same
    `oa-status-morph`, `oa-odometer-number`, `oa-text-reveal`,
    `oa-pane-open`, and `oa-progress-strip` class names.
  - `apps/web/src/ui/index.ts` applies those animations through the shared
    Foldkit UI registry rather than page-level raw classes: workroom shells,
    route panes, the consolidated sidebar, nav/session rows, top-bar status,
    timeline text/tool/diff/file parts, the active run progress strip,
    composer, right-side context panel, key-value rows, and metadata dialog now
    use the motion primitives.
  - Reduced-motion users get the same static layout with animation disabled by
    `prefers-reduced-motion`.
- Concise sidebar and file upload follow-up:
  - The logged-in sidebar now removes repeated account/credit/team/token footer
    rows and strips noisy `live`, `ops`, account, token, and balance metadata
    from primary navigation. Billing and Usage remain as pages, but the rail is
    closer to OpenChamber's dense session list.
  - The sidebar section formerly labeled `Missions` is now labeled `Threads`,
    matching the operator mental model for saved chat/run transcripts.
  - The chat right panel no longer repeats static room/scope/transcript
    metadata. It shows only active run diagnostics, uploaded thread files, and
    captured artifacts.
  - `workers/api/migrations/0020_thread_files.sql` adds D1 metadata for
    personal/team-scoped thread uploads, with R2 object storage under the
    existing `ARTIFACTS` binding.
  - `/api/thread-files` supports listing and multipart uploads. Personal chat
    uploads are scoped to the current user; team chat uploads require active
    team membership and are readable by the team.
  - `/api/teams/:teamId/files` lists team-scoped files, and
    `/teams/:teamRef/files` exposes a logged-in Team Files page.
  - `workers/api/migrations/0021_thread_file_pages.sql` adds
    per-file download toggles and a `thread_file_message_refs` index so a file
    can show where team members or Autopilot referenced it.
  - `/api/thread-files/:fileId` now returns an authorized file detail payload
    with download state, manage capability, and message references. Team file
    detail pages live at `/teams/:teamRef/files/:fileId`; personal file detail
    pages live at `/files/:fileId`.
  - File tables now link to the first-party file detail route instead of raw
    downloads. Raw download remains a page action and is disabled when the file
    owner/team manager toggles download access off.
  - Team Autopilot intent rendering now splits the user message from the
    linked run card. The user text remains right-aligned, while the
    OpenCode-style tool card sits as a separate chronological Autopilot row
    below it.
  - Timeline animation keys now stay stable across status/detail updates, so
    existing run headers update in place while newly arrived rows animate in.
  - SHC closeout/writeback prompt text now marks artifact and GitHub delivery
    steps as private operational instructions. Agents should perform them
    quietly and keep user-visible chat focused on the requested work.
- SHC runtime follow-up:
  - OpenAgents product surface now records explicit runner modes instead of the ambiguous old
    `opencode` runtime label. New runs default to `opencode_codex`, meaning
    SHC launches OpenCode while feeding it the already-materialized Codex
    connected-account auth cache. Raw `codex` remains an explicit alternate
    runtime for fallback/debug runs.
  - `workers/api` sends `agentRuntime` to SHC control, D1 runtime constraints
    were widened to `opencode_codex | codex`, and existing `opencode` rows are
    migrated to `opencode_codex`.
  - This aligns the OpenChamber/OpenCode recommendation with the actual
    producer path: OpenCode is now the default agent process, while OpenAgents
    remains the authoritative projection, sync, billing, and UI state system.
- Settings and billing follow-up:
  - Account routes now switch the left rail into a dedicated settings sidebar
    with a Back to App action. `/settings`, `/settings/:section`, `/billing`,
    and `/usage` share that settings rail instead of mixing account controls
    into the workroom thread/sidebar list.
  - Settings sections expose the browser-confirmed GitHub identity, team and
    member bootstrap data, repository lane, and the run-launch provider grant
    model without pretending that detailed server-side grant inventories are
    browser-owned.
  - Billing now presents usage-only credit billing: balance, rates, payment
    status, add-credit packages, coupons, active metered runs, and ledger rows.
- Failure projection follow-up:
  - Failed SHC/OpenCode/Codex events now extract actual payload details from
    `error`, `reason`, `message`, `stderr`, exit status, and nested event
    fields. The timeline no longer renders a bare "Codex reported a failure
    event" line when a specific runner error is available.
  - If a runner fails without sending any error payload, the UI explicitly says
    that the runner omitted error detail and includes event type/source/sequence
    so the failure is still inspectable.

This confirms one of the report's core recommendations: the left rail should be
a single product/workroom navigation surface, while the right review/files panel
remains a separate live context pane. It also confirms the runtime authority
recommendation: the browser does not fake an assistant stream and does not talk
to OpenCode directly; it asks the OpenAgents Worker to launch SHC work and
renders the Worker-ingested run/event projection.

The remaining work from this report is now narrower:

- replace simple sidebar room rows with typed workroom/session data;
- expand timeline parts beyond string text/tool/diff/file records;
- continue replacing compatibility detail fetch fallbacks with fully
  authoritative sync snapshots as every scope has durable projection coverage;
- keep timeline and right-panel state projected from the same normalized event
  stream as live data arrives.

## Repositories Inspected

OpenChamber local reference:

- `projects/repos/openchamber`
- Remote: `https://github.com/openchamber/openchamber.git`

OpenCode local reference:

- `projects/repos/opencode`
- Remote: `https://github.com/anomalyco/opencode.git`

OpenAgents product surface target:

- `openagents`

Important OpenChamber source paths inspected:

- `packages/ui/src/components/layout/MainLayout.tsx`
- `packages/ui/src/components/views/ChatView.tsx`
- `packages/ui/src/components/chat/ChatContainer.tsx`
- `packages/ui/src/components/chat/MessageList.tsx`
- `packages/ui/src/components/chat/TurnItem.tsx`
- `packages/ui/src/components/chat/TurnAssistantBlock.tsx`
- `packages/ui/src/components/chat/TurnActivity.tsx`
- `packages/ui/src/components/chat/ChatInput.tsx`
- `packages/ui/src/components/chat/StatusRow.tsx`
- `packages/ui/src/components/chat/ModelControls.tsx`
- `packages/ui/src/components/chat/message/parts/DOCUMENTATION.md`
- `packages/ui/src/components/session/sidebar/DOCUMENTATION.md`
- `packages/ui/src/sync/DOCUMENTATION.md`
- `packages/ui/src/sync/event-pipeline.ts`
- `packages/ui/src/sync/event-reducer.ts`
- `packages/ui/src/sync/session-actions.ts`
- `packages/ui/src/sync/input-store.ts`
- `packages/ui/src/sync/session-ui-store.ts`
- `packages/ui/src/lib/opencode/client.ts`
- `packages/ui/src/lib/opencode/runtime-fetch.ts`
- `packages/ui/src/lib/opencode/runtime-url.ts`
- `packages/web/server/index.js`
- `packages/web/server/lib/opencode/DOCUMENTATION.md`
- `packages/web/server/lib/opencode/lifecycle.js`
- `packages/web/server/lib/opencode/proxy.js`
- `packages/web/server/lib/opencode/watcher.js`
- `packages/web/server/lib/opencode/session-runtime.js`
- `packages/web/server/lib/fs/DOCUMENTATION.md`

Important OpenCode source paths inspected:

- `packages/sdk/js/src/server.ts`
- `packages/sdk/js/src/client.ts`
- `packages/sdk/js/src/gen/sdk.gen.ts`
- `packages/sdk/js/src/gen/types.gen.ts`
- `packages/opencode/src/server/server.ts`
- `packages/opencode/src/server/event.ts`
- `packages/opencode/src/session/message-v2.ts`
- `packages/opencode/src/session/status.ts`
- `packages/opencode/src/session/todo.ts`
- `packages/opencode/src/permission/index.ts`
- `packages/opencode/src/question/index.ts`

Important OpenAgents product surface target paths inspected:

- `DESIGN.md`
- `INVARIANTS.md`
- `apps/web/README.md`
- `apps/web/src/main.ts`
- `apps/web/src/model.ts`
- `apps/web/src/message.ts`
- `apps/web/src/view.ts`
- `apps/web/src/page/loggedIn/page/chat.ts`
- `apps/web/src/ui/index.ts`

OpenChamber screenshots inspected:

- `docs/references/chat_example.png`
- `docs/references/tool_output_example.png`
- `docs/references/pwa_chat_example.png`
- `docs/references/pwa_diff_example.png`
- `docs/references/diff_example.png`
- `docs/references/settings_example.png`

## What OpenChamber Is

OpenChamber is a multi-runtime UI for OpenCode. It has web, desktop, and VS Code
surfaces, but the same product idea appears across them: keep the agent run,
project filesystem, Git state, review state, and user input in one coordinated
workspace.

The OpenChamber `AGENTS.md` describes the intended boundary clearly:

- Official OpenCode traffic goes through `@opencode-ai/sdk`.
- OpenChamber-owned runtime capabilities go through local runtime APIs,
  `runtimeFetch`, and runtime URL helpers.
- The desktop app boots the web server in the same Node process and loads the
  loopback UI. It does not launch a separate OpenChamber sidecar subprocess.
- Backend and domain logic live under `packages/web/server/*`.
- Electron owns the desktop shell and security boundary.
- VS Code parity lives under `packages/vscode/*`.

The product implication is important for OpenAgents product surface. OpenChamber separates "agent
runtime protocol" from "application runtime". OpenCode is the agent server and
event source. OpenChamber is the shell that starts, proxies, observes, and
projects that runtime into a usable workroom.

OpenAgents product surface should keep that separation. OpenCode-style events can be a useful runner
adapter, but OpenAgents product surface's authoritative product state should remain in OpenAgents
Cloud, the BFF, or whatever Source Authority controls the run.

## Visual Structure

### Desktop Shell

The primary desktop layout is a compact three-column workroom:

1. Left rail: project and session navigation.
2. Center: current chat/workroom timeline.
3. Right panel: Git, files, context, diff, and run-adjacent tools.

In the `chat_example.png` screenshot, the left rail shows recent sessions,
projects, worktrees, selected sessions, and bottom icon controls. The center has
a top context bar, the chat timeline, changed-file summary, and composer. The
right panel shows runtime controls, tabs, branch state, changed files, commit
message, and commit/sync actions.

This is the strongest UI pattern to port. OpenAgents product surface should avoid treating chat as a
single full-width page with detached tools. The better model is a persistent
workroom where navigation, transcript, and operational context stay visible
together.

### Left Session Rail

OpenChamber's session sidebar is a single multi-project tree. Its
documentation describes the intended hierarchy:

- Recent sessions at the top.
- Projects below recent sessions.
- Worktree and archived groups under projects.
- Sessions under project/worktree groupings.
- Project headers own root sessions.
- Selected rows are text-first and compact rather than large filled cards.
- Archived sections are collapsed by default.
- Dates and metadata are inline and compact.

This gives users repeated-run ergonomics. They can scan many agent sessions
without leaving the workroom.

OpenAgents product surface should port the information architecture, not necessarily the exact
component tree. A Foldkit model should represent:

- Project or workspace groups.
- Session rows.
- Active session id.
- Run status summaries.
- Needs-attention counts.
- Archived/collapsed state.
- Worktree or branch metadata when available.

For OpenAgents product surface's OpenAgents context, the grouping may eventually be program, account,
workspace, workroom, or Source Authority rather than a raw local directory.
Still, OpenChamber proves that users need a dense run selector next to the
timeline.

### Center Workroom Timeline

The center is turn-based rather than message-list-only. OpenChamber projects a
chat run into user turns, assistant output, tool activity, permissions,
questions, todos, diffs, and status rows.

Key components:

- `ChatContainer.tsx` owns the current session frame, status, permissions,
  questions, messages, read-only state, hydrating state, and input.
- `MessageList.tsx` uses virtualized rows and turn records.
- `TurnItem.tsx` renders a user turn and the assistant/runtime material below
  it.
- `TurnAssistantBlock.tsx` and `TurnActivity.tsx` render assistant content and
  progressive activity groups.
- `StatusRow.tsx` renders active todos, run progress, and abort affordances.
- `ChatInput.tsx` owns the bottom composer and mode controls.

The screenshots show several useful patterns:

- User turns can become sticky headers while assistant activity scrolls below.
- Tool calls are collapsed into concise activity rows when they are routine.
- Important tools expand into file paths, diffs, command output, syntax blocks,
  and stats.
- Changed files appear as both timeline artifacts and right-panel state.
- Permissions and questions are treated as blocking UI, not as plain text.
- The composer remains bottom-stable and carries attachments, mode controls,
  and pending-change awareness.

OpenAgents product surface should shift from "message body string" to "turn projection". This is the
single most important model change.

### Right Operational Panel

OpenChamber's right panel is a live context pane. It is not just a settings
sidebar. In the screenshots it carries:

- Git branch state.
- Changed files.
- Commit/update/pull request tabs.
- Commit message generation.
- File and diff tabs.
- Context controls.
- Runtime and local connection indicators.

The `diff_example.png` screenshot shows that the right panel can widen into a
substantial diff/editor surface while the chat remains visible. This is the
right instinct for review-heavy Autopilot workflows. The agent transcript and
review artifact should stay connected.

OpenAgents product surface should use the right panel for the current run's authoritative context:

- Files changed.
- Review/approval state.
- Acceptance evidence.
- Receipts and projection summaries.
- GitHub PR or issue state when relevant.
- Runtime logs or command output when relevant.
- Source Authority status.

For OpenAgents product surface, Git may not always be the dominant tab. In OpenAgents workflows, the
right panel may prioritize review, approval, program policy, or receipts over
raw local Git status. The layout pattern still ports cleanly.

### Bottom Composer And Docks

OpenChamber's composer is dense and multimodal. It supports:

- Text prompts.
- Slash commands.
- Shell mode.
- Model/provider/agent controls.
- Permission or edit-mode controls.
- Attached files.
- GitHub issue/PR context.
- Voice mode.
- Queued messages.
- Pending workspace changes.
- Drag/drop and paste attachment flows.

This is more than a textarea. It is a command dock for the current workroom.

OpenAgents product surface should preserve that direction while simplifying the first implementation.
A useful initial OpenAgents product surface composer model should include:

- Prompt text.
- Selected agent or runtime.
- Permission/editing mode.
- Attachment references.
- Command mode.
- Pending blocker state.
- Submit availability.
- Active run abort/resume state.

Do not expose all OpenChamber settings immediately. Port the stable underlying
states first.

### Mobile And PWA Collapse

The `pwa_chat_example.png` screenshot shows that OpenChamber does not attempt to
keep three visible columns on mobile. It collapses to:

- A compact top icon rail.
- A focused timeline.
- Concise activity rows.
- Bottom composer.
- Drawer/sheet access to side panels.
- A small live task/status overlay.

The lesson is that the workroom state survives the viewport change, but the
layout is projected differently. OpenAgents product surface should model the workroom first and let
desktop/mobile views render that model differently.

For OpenAgents product surface, mobile should not be a separate chat-only product. It should expose
the same session, blockers, approvals, and artifacts through compact panels.

### Settings And Operator Controls

OpenChamber's settings screenshot exposes important chat rendering controls:

- Chat render mode: sorted or live.
- Message transport: automatic, WebSocket, or SSE.
- Default-open tools such as bash or edit.
- Markdown versus plain user rendering.
- Mermaid rendering.
- Diff layout and diff view mode.
- Reasoning trace visibility.
- Sticky user header.
- Wide chat layout.

These settings reveal real product concerns:

- Streaming order can differ from final sorted order.
- Transport behavior matters enough to expose when debugging.
- Tool output expansion has user preference and density implications.
- Diffs need responsive layout choices.
- Reasoning visibility should be controlled deliberately.

OpenAgents product surface should not copy this entire settings surface first. It should encode the
same choices as typed state, then expose only the controls that fit the
OpenAgents operator model.

## Chat Rendering Model

OpenChamber's chat rendering system has a more mature model than a normal chat
app.

Its message-parts documentation divides parts into categories:

- Static grouped tools rendered by `StaticToolRow` and `ProgressiveGroup.tsx`.
- Expandable tools rendered by `ToolPart.tsx`.
- Shared tool presentation through `toolPresentation.tsx`.
- Static tools such as reads, searches, and fetches.
- Expandable tools such as bash, edit, write, question, and task.
- Reasoning and justification blocks.

The practical UX rule is:

- Routine activity should be compact.
- Review-relevant output should be expandable.
- File edits and diffs should be first-class.
- Runtime blockers should become explicit cards or docks.

OpenAgents product surface should model timeline parts with a discriminated union. A useful target
shape:

```text
TimelinePart =
  | TextPart
  | ReasoningPart
  | StaticToolPart
  | ExpandableToolPart
  | DiffPart
  | FilePart
  | PermissionRequestPart
  | QuestionRequestPart
  | TodoStatusPart
  | RuntimeStatusPart
```

The current OpenAgents product surface `ChatMessage` shape is too flat for this. A string `body` plus
message `status` cannot represent:

- Streaming tool deltas.
- Command output.
- A permission question that needs action.
- A diff that should open in the right panel.
- A file list that should synchronize with a changed-files strip.
- A todo status update.
- A run that is busy, cooling down, idle, or needs attention.

OpenAgents product surface should keep the current run metadata projection as a narrow foundation,
but the next model iteration should introduce structured parts and turn records.

## Event And Store Architecture

OpenChamber uses several stores rather than one giant state object. The
documentation describes the split:

- Directory-scoped child stores for live session/message/part/permission/
  question status.
- A global sessions cache for active, archived, and known session lists.
- Session action helpers that mutate the global list only after SDK calls
  succeed.
- UI stores for ephemeral selection, input state, abort prompts, pending
  changes, and view preferences.
- Feature caches for Git, PRs, project state, and terminal state.

The important rule from OpenChamber's sync documentation is that event reducers
should only clone the fields touched by an event. Streaming deltas are frequent;
spreading whole session records during every delta creates avoidable churn.

OpenChamber handles these event families:

- `session.created`
- `session.updated`
- `session.deleted`
- `session.diff`
- `session.status`
- `todo.updated`
- `message.updated`
- `message.removed`
- `message.part.updated`
- `message.part.removed`
- `message.part.delta`
- `vcs.branch.updated`
- `permission.asked`
- `permission.replied`
- `question.asked`
- `question.replied`
- `question.rejected`
- `lsp.updated`

OpenAgents product surface should translate that into Foldkit/Effect language:

- External runtime updates become typed messages.
- The Foldkit update function applies small, targeted state transitions.
- Side effects live in commands/effects, not in rendering code.
- State is split by feature and update frequency.
- The view receives already-projected workroom state.

A useful OpenAgents product surface model split:

```text
LoggedInModel
  route
  workroom
    sessionRail
    timeline
    composer
    rightPanel
    runtimeConnection
    blockers
    settings
```

Where:

```text
TimelineModel
  activeSessionId
  turns
  activeStatus
  pendingQuestions
  pendingPermissions
  changedFiles
  renderMode
```

And:

```text
RuntimeConnectionModel
  status: disconnected | connecting | connected | degraded | reconnecting
  transport: auto | websocket | sse
  lastEventAt
  backoff
  diagnostics
```

The projection layer should turn raw runtime events into stable UI records. This
keeps the UI registry simple and keeps runtime protocol details out of visual
components.

## How OpenChamber Interacts With OpenCode

OpenChamber talks to OpenCode through the official JavaScript SDK and its own
runtime bridge.

### Client Boundary

`packages/ui/src/lib/opencode/client.ts` wraps the generated SDK client from
`@opencode-ai/sdk/v2`. It adds:

- Runtime URL resolution.
- Runtime-aware fetch behavior.
- Directory-scoped clients.
- Retry and provider-circuit behavior.
- File MIME normalization.
- HEIC conversion.
- Session, message, command, status, shell, fork, revert, and abort helpers.

Representative operations include:

- `listSessions`
- `createSession`
- `getSession`
- `getSessionMessages`
- `getSessionTodos`
- `sendMessage`
- `sendCommand`
- `abortSession`
- `shellSession`
- `revertSession`
- `unrevertSession`
- `forkSession`
- `getSessionStatus`

`sendMessage` ultimately uses OpenCode's async prompt endpoint. That matters for
OpenAgents product surface: user submit is not just local state mutation. It is a command sent to a
runtime, and the visible transcript is then reconciled through runtime events.

### Runtime URL And Fetch

OpenChamber does not hardcode a single server URL in component code. It uses:

- `runtime-url.ts` to resolve the active runtime endpoint.
- `runtime-fetch.ts` to rewrite `/api`, `/auth`, and `/health` traffic to the
  active runtime.
- Runtime auth headers and URL tokens for assets, SSE, and WebSocket paths.

For OpenAgents product surface, this reinforces a boundary principle: UI code should not directly
know the physical runtime. It should talk to an OpenAgents product surface BFF or runtime connector
that owns credentials, endpoint selection, and authorization.

### Server Boundary

The OpenChamber web server composes OpenCode lifecycle, proxying, filesystem
APIs, terminal, push, settings, tunnels, and session runtime logic.

Important modules:

- `lifecycle.js` starts, restarts, waits for, and health-checks OpenCode.
- `proxy.js` gates and proxies `/api` traffic to the current OpenCode target.
- `watcher.js` subscribes to OpenCode global events.
- `session-runtime.js` derives status, activity, attention, and synthetic
  session events from OpenCode event payloads.
- `index.js` composes all server systems and forwards OpenCode/global events to
  connected UI clients.

OpenChamber can start a managed OpenCode server, or it can connect to an
external server with environment settings such as:

- `OPENCODE_HOST`
- `OPENCODE_PORT`
- `OPENCODE_SKIP_START=true`

The lifecycle module waits for OpenCode to print the listening URL, captures
diagnostics, and stores the active endpoint. The proxy module preserves SSE
behavior and readiness semantics while OpenCode starts or restarts.

OpenAgents product surface should not copy this local process model unless it grows a local desktop
surface. The Cloud-facing product should keep runtime start/connect/restart
behind a server-side adapter.

### OpenCode SDK And Events

The OpenCode SDK exposes a generated client with session and global endpoints.
The useful surfaces for OpenAgents product surface research are:

- Global events through `/global/event`.
- Session list/create/get/update/delete.
- Session status.
- Session messages.
- Session prompt and prompt-async submission.
- Session command and shell routes.
- Session fork.
- Session abort.
- Session todo.
- Session diff.
- Revert and unrevert.

OpenCode emits session/message/tool state as events. The key event families are:

- Server and global connection events.
- Session status events.
- Todo updated events.
- Message updated/removed events.
- Message part updated/removed/delta events.
- Permission asked/replied events.
- Question asked/replied/rejected events.

OpenCode itself is already eventful enough for a sophisticated UI. OpenChamber's
extra work is to normalize, coalesce, and project those events into UI-friendly
state.

### Event Pipeline

OpenChamber's event pipeline supports automatic, WebSocket, and SSE transport.
It:

- Opens one pipeline per mounted sync provider.
- Builds the event URL for global events.
- Coalesces high-frequency event keys such as session status, LSP updates, and
  message part deltas.
- Flushes near animation-frame cadence.
- Uses backpressure windows for hidden/offline states.
- Handles heartbeat timeouts and reconnect behavior.
- Normalizes OpenChamber synthetic events into session status/activity facts.

The user-facing settings expose transport selection because transport matters
for debugging streaming UIs.

OpenAgents product surface should make transport explicit in the model even if it starts with a
single implementation. A hidden stringly transport state would make reconnect,
debugging, and tests harder later.

## What OpenAgents product surface Should Learn And Port

### 1. Port The Workroom Frame

The strongest pattern is:

- Session rail on the left.
- Timeline in the center.
- Context/review panel on the right.
- Composer at the bottom.
- Optional terminal/log dock.

OpenAgents product surface already has `Ui.workroomSessionRail`, `Ui.workroomTopBar`,
`Ui.workroomActionDock`, `Ui.workroomPermissionDock`, and related workroom
registry functions. The next step is to make those components represent a
structured model instead of a mostly static seed page.

### 2. Port Turn-Based Projection

OpenChamber renders a run as turns, not only messages. OpenAgents product surface should define:

- User turn.
- Assistant response.
- Runtime activity group.
- Tool outputs.
- Diffs.
- Files.
- Permissions.
- Questions.
- Status/todo updates.

This will make Autopilot runs easier to review. A transcript that only stores
assistant text loses too much of the operational story.

### 3. Port Tool-First Rendering

The UI should make tools visible without making every tool call visually loud.
The rule:

- Read/search/fetch style activity can be compact.
- Bash/edit/write/diff/question/task output should be expandable.
- File edits and diffs should connect to the right context panel.
- Error and blocker tool states should get higher contrast.

This directly supports Autopilot review and approval flows.

### 4. Port Blocking Interaction Patterns

OpenChamber treats permissions and questions as first-class runtime requests.
They are not plain assistant text.

OpenAgents product surface should treat approvals, policy blocks, account decisions, operator
questions, and Source Authority prompts the same way:

- Show them in the timeline.
- Surface active blockers in a dock.
- Keep actions compact and explicit.
- Feed replies back as typed runtime commands.

This is a natural fit for OpenAgents acceptance and approval semantics.

### 5. Port Context Panel Coupling

The right panel should update from the same events that update the timeline.
For OpenAgents product surface, this means:

- A file change event updates both the timeline and changed-files panel.
- A review request updates both the timeline and approval dock.
- A receipt event updates both the timeline and receipt/context panel.
- A PR event updates both the timeline and GitHub panel.

This avoids the common failure mode where chat says one thing and side panels
show stale or unrelated state.

### 6. Port Session Hierarchy

OpenChamber's session rail makes many sessions manageable. OpenAgents product surface should port
the idea of session grouping and attention state:

- Active sessions.
- Recently touched sessions.
- Needs-attention sessions.
- Archived sessions.
- Program/workspace/account grouping.
- Run status and last update metadata.

The exact labels should be OpenAgents product surface-native, but the density is worth preserving.

### 7. Port Transport And Status Visibility

OpenChamber exposes live/busy/idle status and transport controls. OpenAgents product surface should
represent:

- Runtime connected/reconnecting/degraded.
- Active run busy/idle/waiting for input.
- Last event timestamp.
- Retry/backoff state.
- Transport selection.

This is especially important for Cloud and distributed runner workflows, where a
silent UI can be mistaken for an idle agent.

### 8. Port Mobile State, Not Mobile Layout

OpenChamber's mobile screenshot proves the state can survive layout collapse.
OpenAgents product surface should design desktop and mobile views from the same workroom model:

- Desktop: three columns.
- Tablet: rail or right panel collapses first.
- Mobile: timeline plus drawer sheets and bottom composer.

The model should not depend on column visibility.

## What OpenAgents product surface Should Not Port

OpenAgents product surface should avoid copying:

- OpenChamber's React component architecture.
- Zustand store layout.
- Electron process lifecycle.
- Direct local filesystem endpoints as browser-facing product authority.
- Direct local OpenCode auth assumptions.
- The exact settings surface before OpenAgents product surface has matching runtime needs.
- One-to-one visual palette decisions from OpenChamber screenshots.
- Raw user-facing OpenCode protocol details in primary UI.

The OpenChamber palette uses warm off-white text, dark backgrounds, muted
orange/brown accents, olive/blue status chips, and compact panels. OpenAgents product surface's
`DESIGN.md` already requires a dark Vortex/OpenAgents operational interface,
pure black foundations, warm off-white text, compact mono details, status rows,
panes, event tapes, and review surfaces. The port should use OpenAgents product surface's visual
language while adopting OpenChamber's layout and interaction density.

## OpenAgents product surface Target Architecture

### Current Gap

The current OpenAgents product surface web model now submits chat turns into real Autopilot run
records and projects basic SHC/OpenCode run metadata back into the timeline.
The existing workroom UI registry is promising, but the data model still does
not yet carry full OpenChamber-grade runtime information.

Current limitations:

- Message content is too string-oriented.
- Timeline parts are too limited.
- Tool output lacks enough state for review.
- Permission/question/status are not complete first-class timeline records.
- The right context panel is not yet a live projection of the same runtime
  event stream.
- Run updates are still poll-and-replace projections rather than a normalized
  event-reducer model.

These are acceptable for the current foundation, but they should be fixed
before the app grows beyond the current Worker-owned SHC run projection.

### Recommended Model Sketch

Recommended high-level shape:

```text
WorkroomModel
  sessionRail: WorkroomSessionRailModel
  timeline: WorkroomTimelineModel
  composer: WorkroomComposerModel
  rightPanel: WorkroomRightPanelModel
  runtimeConnection: RuntimeConnectionModel
  blockers: WorkroomBlockerModel
  settings: WorkroomSettingsModel
```

Recommended session rail:

```text
WorkroomSessionRailModel
  groups: ReadonlyArray<SessionGroup>
  activeSessionId: Option<SessionId>
  collapsedGroupIds: ReadonlySet<GroupId>
  attentionSessionIds: ReadonlySet<SessionId>
```

Recommended timeline:

```text
WorkroomTimelineModel
  sessionId: SessionId
  turns: ReadonlyArray<TimelineTurn>
  activeStatus: RuntimeRunStatus
  todos: ReadonlyArray<TodoItem>
  pendingPermissions: ReadonlyArray<PermissionRequest>
  pendingQuestions: ReadonlyArray<QuestionRequest>
  changedFiles: ReadonlyArray<ChangedFileSummary>
  renderMode: sorted | live
```

Recommended turn:

```text
TimelineTurn
  id: TurnId
  user: UserMessage
  assistant: ReadonlyArray<AssistantMessage>
  activity: ReadonlyArray<ActivityGroup>
  blockers: ReadonlyArray<BlockerRef>
  createdAt
  updatedAt
```

Recommended part union:

```text
TimelinePart
  TextPart
  ReasoningPart
  StaticToolPart
  ExpandableToolPart
  DiffPart
  FilePart
  PermissionRequestPart
  QuestionRequestPart
  TodoStatusPart
  RuntimeStatusPart
```

Recommended event union:

```text
RuntimeSyncEvent
  SessionCreated
  SessionUpdated
  SessionDeleted
  SessionStatusChanged
  SessionDiffChanged
  TodoUpdated
  MessageUpdated
  MessageRemoved
  MessagePartUpdated
  MessagePartRemoved
  MessagePartDelta
  PermissionAsked
  PermissionReplied
  QuestionAsked
  QuestionReplied
  QuestionRejected
  BranchUpdated
  RuntimeConnectionChanged
```

These names should be made OpenAgents product surface-native during implementation. The important
constraint is not the exact naming; it is that runtime facts are explicit and
typed.

### Foldkit/Effect Translation

In OpenAgents product surface, the OpenChamber-style architecture should become:

- Effect services for runtime/BFF clients.
- Foldkit messages for user actions and runtime events.
- Foldkit commands for network calls, subscriptions, and submissions.
- Pure update functions for model transitions.
- UI registry components for rendering projected state.

Example flow:

1. User submits a composer command.
2. Foldkit update marks submission pending and emits a command.
3. Runtime service calls the OpenAgents product surface BFF or runner adapter.
4. Runtime stream emits typed events.
5. Foldkit update applies each event to the workroom model.
6. The timeline and right panel re-render from the same state.

This keeps UI code from owning transport details and keeps effects out of
rendering.

## Suggested Port Plan

### Phase 0: Document And Type The Workroom Contract

Add or update OpenAgents product surface documentation and model types for:

- Workroom sessions.
- Timeline turns.
- Timeline parts.
- Runtime events.
- Composer state.
- Right-panel state.
- Runtime connection state.

Do this before wiring a live runtime. It will prevent a direct string-message
integration that later has to be unwound.

### Phase 1: Static OpenChamber-Shaped Projection

Use existing OpenAgents product surface workroom registry primitives to create a richer workroom
projection:

- Left rail with grouped sessions and attention badges.
- Center turn timeline with static activity rows and expandable diffs.
- Bottom composer with mode controls and pending-change strip.
- Right context panel with review/files/status tabs.
- Permission/question dock as active blocker UI.

This phase should stay entirely within OpenAgents product surface's dark operational design system.

### Phase 2: Runtime Event Reducer

Introduce a typed event reducer and tests:

- Apply session create/update/delete.
- Apply status/todo updates.
- Apply message and part updates.
- Apply part deltas without replacing whole turns.
- Apply permission/question events.
- Apply changed-file or diff events.
- Preserve selection and panel state during streaming.

This is where OpenChamber's "clone only touched fields" rule matters most.

### Phase 3: BFF/Runtime Bridge

Connect the reducer to an authoritative OpenAgents product surface runtime stream:

- Browser subscribes to OpenAgents product surface BFF or Cloud event stream.
- Server-side adapter talks to OpenCode or an OpenCode-like runner when needed.
- Credentials and runtime URLs stay server-side.
- Permission/question replies post to OpenAgents product surface commands, not raw local OpenCode
  endpoints from the browser.
- The adapter maps OpenCode events into OpenAgents product surface `RuntimeSyncEvent` values.

This preserves product authority while using OpenCode's mature event protocol as
reference.

### Phase 4: Performance And Responsive Layout

After the event shape is stable:

- Add virtualization/windowing for long timelines.
- Add coalescing for high-frequency deltas.
- Add reconnect/backoff diagnostics.
- Add mobile drawer projections.
- Add screenshot/browser verification for desktop and mobile.

OpenChamber already proves these concerns become necessary once the transcript
contains many tool events.

## Concrete UI Registry Deltas For OpenAgents product surface

The current `apps/web/src/ui/index.ts` workroom registry should grow toward
these primitives:

- `workroomTurn`
- `workroomUserTurnHeader`
- `workroomAssistantBlock`
- `workroomActivityGroup`
- `workroomStaticToolRow`
- `workroomExpandableToolPart`
- `workroomDiffPart`
- `workroomPermissionCard`
- `workroomQuestionCard`
- `workroomStatusRow`
- `workroomChangedFilesStrip`
- `workroomContextTabs`
- `workroomRuntimeConnectionBadge`

The existing `WorkroomTimelinePart` union should expand beyond `text`, `tool`,
`diff`, and `file` to include:

- `reasoning`
- `permission`
- `question`
- `todo`
- `status`
- `command`
- `error`
- `review`

Keep component APIs small. Most OpenChamber complexity should live in model
projection and update logic, not in UI helper signatures.

## Concrete Product Lessons For Autopilot

OpenChamber is useful because it treats agent work as inspectable operations.
That maps directly to Autopilot.

For Autopilot review and approval:

- Use the timeline to show what happened.
- Use the right panel to show what changed and what needs approval.
- Use blockers/docks for decisions.
- Use status rows for active progress and todos.
- Use session rail attention markers to show where the operator is needed.

For Autopilot acceptance and receipts:

- Receipt events should be timeline parts and context-panel records.
- Acceptance evidence should be linkable from turns.
- Projection summaries should appear beside the transcript, not buried after it.

For Autopilot runtime authority:

- Browser UI should not own runner authority.
- OpenCode-like protocol should be adapted through OpenAgents product surface services.
- Runtime events should be normalized before they reach UI projection.

For Autopilot operator UX:

- Avoid marketing-page framing.
- Prefer dense, readable, dark operational surfaces.
- Keep chat and review in one workspace.
- Make blockers impossible to miss but visually compact.
- Keep repeated run navigation fast.

## Risks And Guardrails

### Risk: Copying Local Filesystem Assumptions

OpenChamber is comfortable with local filesystem and terminal routes because it
is a local/desktop/web runtime for OpenCode. OpenAgents product surface's Cloud-facing Autopilot
surface has different authority constraints. Do not expose local-style file
routes directly in the browser product.

Guardrail: all filesystem, Git, terminal, and runner operations should go
through OpenAgents product surface services with explicit authorization and Source Authority
semantics.

### Risk: Flattening Runtime Events Into Text

If OpenAgents product surface integrates a runner by appending text chunks, it will lose tool,
permission, question, diff, and review structure.

Guardrail: define `RuntimeSyncEvent` and `TimelinePart` unions before live
integration.

### Risk: Side Panel Drift

If the right panel fetches independent state and the transcript uses a separate
stream, they can drift.

Guardrail: project timeline and side-panel state from the same normalized event
stream wherever possible.

### Risk: Over-Porting Settings

OpenChamber exposes many useful settings because it supports local runtimes and
multiple transports. OpenAgents product surface does not need all controls on day one.

Guardrail: model transport/render/diff preferences internally first, expose only
the settings operators actually need.

### Risk: React/Zustand Architecture Leakage

OpenChamber's implementation is optimized for React and Zustand. OpenAgents product surface is a
Foldkit/Effect codebase.

Guardrail: port the product model and event reducer ideas, not store and
component architecture.

## Open Questions

1. What runtime should OpenAgents product surface support first: an SHC runner stream, a direct
   OpenCode server adapter, or an OpenAgents BFF canonical stream?
2. Should OpenAgents product surface prioritize GitHub PR/review context over raw Git status in the
   right panel?
3. What is the first "permission" equivalent in OpenAgents product surface: edit approval, Source
   Authority approval, deployment approval, account action approval, or all of
   them behind one blocker model?
4. Does OpenAgents product surface need a local desktop/PWA runtime soon, or should the first port
   stay Cloud-first?
5. Should reasoning traces be visible to operators by default, or only under an
   explicit inspection setting?

## Near-Term Recommendation

The first shell-level UI port is complete: OpenAgents product surface now has one logged-in workroom
sidebar instead of a separate app navigation surface and chat/session rail. The
next OpenAgents product surface implementation pass should go deeper into the workroom model:

1. Replace simple sidebar room rows with typed workroom/session data.
2. Expand the workroom model with typed turns, parts, blockers, changed files,
   right-panel tabs, and runtime status.
3. Add focused update tests for turn, part, sidebar, and right-panel projection.
4. Introduce the runtime event reducer on top of the current Worker-owned run
   event API.
5. Then move from polling to a live runtime stream through an OpenAgents product surface-owned
   service.

This lets OpenAgents product surface keep learning the best UX from OpenChamber without inheriting
the wrong runtime authority model.
