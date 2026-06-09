# Team Room Shared History and Inline Autopilot Runs Audit

Date: 2026-06-03

Status: audit complete; OpenAgents product surface reference implementation now includes durable
team history, team sync, compact inline Autopilot run cards, bounded child-run
context, and idempotent answer-back messages.

Primary implementation target: `vortex/`

Reference implementation/source material: `openagents/`

## 2026-06-03 OpenAgents product surface Implementation Update

Issue #16 completed the OpenAgents product surface-side reference flow for team Autopilot
invocations:

- exact `@autopilot <prompt>` team messages pre-generate the parent
  `team_chat_messages.id`, store the durable `autopilot_intent` row, and launch
  the child run through the team chat API;
- the child run goal now includes a bounded team context bundle containing the
  parent team id, parent team chat message id, normalized prompt, selected team
  file ids, and recent team conversation with author provenance;
- PDF references are resolved after the explicit Autopilot route is selected,
  using authorized team file metadata and choosing the most recent team PDF when
  the prompt references a PDF;
- team rooms render the invoking message plus one compact linked
  `Autopilot run` card, using the current child run status/latest action when
  that run is loaded locally, while full event lists and controls remain on
  `/t/:threadId`;
- terminal completed runner callbacks append one deterministic system answer
  message back into the parent team room, using the latest assistant/final child
  event text when available;
- answer-back provenance is stored in `metadata_json`, including child run id,
  source event id, parent team chat message id, and selected team file ids;
- the answer-back message id is deterministic per child run, making duplicate
  terminal callbacks and retries idempotent.

Regression coverage added in this implementation:

- Worker tests for PDF file selection, team context goal construction, explicit
  file id selection, and final assistant answer extraction;
- Foldkit scene coverage that team Autopilot intent messages render a compact
  linked card and do not render the full run timeline in the team room;
- existing `bun run test:web` and `bun run test:api` suites cover the durable
  team chat, sync projection, and run detail paths.

## 2026-06-03 File Context and Dispatch Hygiene Update

Team-room Autopilot launches now keep the visible room/thread goal separate
from the hidden SHC dispatch prompt. Public D1 run records, sync payloads, and
the chat UI store the user's prompt, while the SHC assignment receives the
bounded team context bundle.

The file selector now handles general file references such as "summarize the
file I just added", not only explicit PDF mentions. It still honors explicit
selected file ids first, then infers the most recent authorized team file for
singular file prompts, or up to eight recent files for plural file prompts.
Text-like selected files get a bounded R2 excerpt in the hidden dispatch prompt
so the child run can answer from the upload instead of merely seeing a file id.

Failed runner events also surface nested provider errors in the chat timeline.
For example, an upstream `model_not_supported` response is shown as the actual
model error instead of the generic "reported a failure event" summary.

## Request

Restore the team-room behavior where a team room opens onto shared message
history instead of a blank "start a chat with Autopilot" surface. A team member
should be able to invoke Autopilot from the room by tagging it, for example
`@autopilot ok`. That invocation should create an Autopilot workroom/run as a
thread-like child of the team room, render an inline run-status component in
the room, and keep that inline component compact: current status plus the latest
thing the run did, not the full run transcript. Clicking the component should
open the full Autopilot thread/workroom where the user can inspect, interact
with, continue, steer, or cancel the run. Live updates should use the new sync
path where available.

Clarification after the initial audit: the desired room workflow includes
team-scoped file context and a room-visible Autopilot answer. A user should be
able to drop a PDF on the team-room input, the team should be able to discuss
that PDF in shared history, and a later message such as
`@autopilot summarize that PDF` should launch a child Codex/opencode workroom
that can read the relevant room files and recent room conversation. While the
child run is active, the team room shows only a compact inline status card. When
the child run finishes, Autopilot should post a normal visible answer back into
the team room, preferably using the final assistant message from the child
Codex/opencode thread as the first implementation source.

The user explicitly asked to start with a full audit, document it in the OpenAgents product surface
docs folder, and push that audit.

## Sources Reviewed

- Root workspace invariants and routing:
  - `AGENTS.md`
  - `INVARIANTS.md`
- Omni docs:
  - `docs/omni/README.md`
  - `docs/omni/vortex-business-workrooms-synthesis.md`
  - `docs/omni/vortex-mobile-voice-companion-synthesis.md`
  - `docs/omni/vortex-to-omni-product-gap-analysis-roadmap.md`
  - `docs/omni/coding-on-autopilot-wedge-spec.md`
  - `docs/omni/autopilot-coding-mvp-ship-scope.md`
  - adjacent Omni synthesis docs where they mention threads, workrooms,
    projections, mobile sync, approvals, artifacts, and receipts
- Vortex repository guidance:
  - `vortex/AGENTS.md`
  - `vortex/INVARIANTS.md`
  - `vortex/DESIGN.md`
  - `vortex/convex/_generated/ai/guidelines.md`
- Vortex implementation:
  - `vortex/convex/schema.ts`
  - `vortex/convex/threads.ts`
  - `vortex/convex/lib/threadAccess.ts`
  - `vortex/convex/projectContext.ts`
  - `vortex/convex/autopilotLegal.ts`
  - `vortex/lib/autopilot/workspace-thread-scope.ts`
  - `vortex/components/ai-elements/prompt-input.tsx`
  - `vortex/components/autopilot/prototype.tsx`
  - `vortex/lib/json-render/workroom-events.ts`
  - `vortex/lib/json-render/codex-run-workroom.ts`
  - `vortex/app/(public)/chat/page.tsx`
  - `vortex/app/(oatmeal)/t/[threadId]/page.tsx`
  - `vortex/app/api/chat/route.ts`
  - `vortex/server/programs/chat/submitThreadTurn.ts`
  - `vortex/server/collaborativeThreadRegressions.test.ts`
  - `vortex/server/workspaceThreadScope.test.ts`
- OpenAgents product surface docs:
  - `openagents/docs/2026-06-02-cloudflare-only-openagents-sync-audit.md`
  - `openagents/docs/2026-06-03-openchamber-ui-ux-port-research.md`
  - `openagents/docs/2026-06-03-logged-in-sidebar-consolidation.md`
  - `openagents/docs/2026-06-02-shc-agent-deployment-runbook.md`
  - `openagents/docs/more.md`
- OpenAgents product surface implementation:
  - `openagents/workers/api/migrations/0004_core_team_memberships.sql`
  - `openagents/workers/api/migrations/0012_team_chat_messages.sql`
  - `openagents/workers/api/migrations/0014_autopilot_thread_ids.sql`
  - `openagents/workers/api/migrations/0020_thread_files.sql`
  - `openagents/workers/api/src/index.ts`
  - `openagents/workers/api/src/omni-runs.ts`
  - `openagents/packages/sync-schema/src/index.ts`
  - `openagents/packages/sync-worker/src/index.ts`
  - `openagents/apps/web/src/route.ts`
  - `openagents/apps/web/src/page/loggedIn/model.ts`
  - `openagents/apps/web/src/page/loggedIn/message.ts`
  - `openagents/apps/web/src/page/loggedIn/update.ts`
  - `openagents/apps/web/src/page/loggedIn/page/chat.ts`
  - `openagents/apps/web/src/page/loggedIn/page/files.ts`
  - related logged-in tests referenced by search

## High-Level Finding

The active product implementation is Vortex, not OpenAgents product surface. The workspace contract
routes new Cloud-facing Autopilot UX, workroom, review, approval, acceptance,
projection, and BFF work to `vortex/`. OpenAgents product surface is still useful because it contains
the prior team-room shape: teams, memberships, explicit team-room routes,
D1-backed team chat messages, `autopilot_intent` chat rows, Autopilot thread
ids, and a Cloudflare sync layer with workspace/thread/agent-run/team scopes.

Vortex already has the more complete durable workroom substrate: scoped
Convex threads and messages, organization/project membership checks, author
provenance, workroom events, generated UI specs, program runs, approvals,
receipts, Codex run projections, and tests for collaborative access. What it
does not have is a canonical team-room entry surface that always resolves to
shared room history, nor a mention-triggered path that launches an Autopilot
workroom from inside that shared room and renders only a compact run projection
inline. It also does not yet have a complete team-room file context contract or
an answer-back contract where the child run posts its final useful response into
the parent room.

The correct port is therefore product-contract level, not a direct database
port. Bring OpenAgents product surface's team-room behavior into Vortex using Vortex's existing
Convex thread/workroom model.

## Product Contract To Implement

1. A team room is a shared room, not a blank personal Autopilot prompt.
2. Opening a team room loads shared message history immediately.
3. Opening a team room also loads team-room files attached to that room.
4. Dropping a PDF or other allowed file on the team-room input creates a
   durable team-scoped file record, visible to team members and available to
   later room-scoped Autopilot runs.
5. Plain messages in a team room append to shared history with durable author
   provenance and do not automatically start a model chat or Autopilot run.
6. A bounded exact mention, such as `@autopilot summarize that PDF`, creates a child
   Autopilot workroom/run from the team room.
7. The child workroom receives a bounded context bundle containing the invoking
   message, relevant team-room file ids, selected recent team conversation, and
   the parent room/link metadata.
8. The invoking message remains visible in the team room.
9. The team room renders an inline run-status component attached to that
   invocation.
10. The inline component shows only:

- the run status
- the latest meaningful run event or latest action summary
- a link/action to open the full workroom

11. The inline component does not show the full Autopilot transcript, event
    list, tool timeline, artifacts list, or steering surface.
12. Clicking the component opens the full Autopilot thread/workroom where the
    complete event stream, artifacts, receipts, approvals, and controls are
    available.
13. When the child workroom completes, Autopilot posts a normal answer message
    into the parent team room. The first implementation should use the final
    assistant/final message from the Codex/opencode child thread when available,
    with provenance back to the child run.
14. Live updates should flow through the sync layer where that layer exists,
    with polling or Convex live queries only as fallback/bridge behavior.

## Omni Direction

The Omni docs consistently describe workrooms as the durable control surface:
chat opens workrooms, workrooms produce business state, and business state stays
scoped to the correct user, team, or project. The business-workroom synthesis is
especially relevant: team/project scoping, role-aware shared workrooms, and
author provenance are part of the product contract, not incidental UI details.

The mobile/voice synthesis adds an important projection rule. Cross-device
state should sync as records, not as raw transcript replay. Threads/messages,
workroom events, artifacts, approvals, receipts, generated UI, and voice state
are distinct durable records. That supports the requested inline run card: the
team room should subscribe to a compact projection of a run, while the full
thread/workroom route subscribes to the detailed run records.

The coding-on-Autopilot docs describe a thread becoming a workroom and the
workroom producing events, artifacts, receipts, reviews, proof, and admin
surfaces. That maps directly to a child workroom launched from a team-room
message. The team-room parent should not become the whole workroom transcript;
it should contain a linkable projection and the eventual answer that matters to
the team conversation.

## Vortex Current State

### Durable Thread Model

`vortex/convex/schema.ts` defines:

- `organizations`
- `organizationMembers`
- `projects`
- `projectMembers`
- `threads`
- `messages`
- workroom, generated UI, program run, approval, receipt, provenance, and Codex
  run tables

The `threads` table already supports `organizationId`, `projectId`,
`threadKind`, `visibility`, generation status, route ids, stream ids, and
indexes by user, organization, project, status, and route. The `messages` table
already carries `threadId`, `userId`, optional `organizationId`, optional
`projectId`, role, content, status, model/provider metadata, and timestamps.

This is the right canonical store for Vortex team-room messages. A separate
OpenAgents product surface-style `team_chat_messages` table is not necessary unless the product
needs a non-thread room aggregate that deliberately sits outside the existing
thread model.

### Access Control and Provenance

`vortex/convex/lib/threadAccess.ts` implements read/write/admin checks for
private, project, and organization-scoped threads. Organization members can
read organization-visible threads. Project access resolves through project
roles. Owners/admins retain elevated privileges. This matches the shared-room
privacy boundary.

`vortex/convex/threads.ts` already projects author data for threads, messages,
workroom events, generated UI, program runs, approvals, and receipts. The
collaborative regression tests assert that shared workspace views retain user
provenance. That invariant must remain intact for team-room history and
Autopilot invocation cards.

### Current Team Workspace UX

`vortex/components/autopilot/prototype.tsx` already has a workspace sidebar
model with personal, organization, and project contexts. The sidebar labels the
organization action as "New team chat", and recent threads can be scoped to an
organization or project. Private recents are intentionally excluded from
organization workspaces by `includePrivateThreadRecentsForWorkspace`.

The blank-room behavior comes from the current selection/new-chat flow:

- selecting a team workspace calls the same new-chat path
- that path clears `activeClientThreadId`
- the active durable thread becomes `null`
- the message list and workroom events are cleared
- the first submitted prompt creates a new scoped organization thread through
  `api.threads.appendTurn`

That behavior is coherent for "start a new Autopilot chat", but it is not a
team room. A team room needs a deterministic shared thread or room record that
is loaded on entry.

### Current Chat Submit Path

The Vortex chat submit path always treats a submitted prompt as a chat turn:

- append local user message
- call `api.threads.appendTurn`
- insert an assistant placeholder
- stream `/api/chat`
- complete or fail the assistant message in Convex

There is no current `@autopilot` mention split. There is also no current
plain-team-message path that appends to shared room history without starting an
assistant/model turn.

### Current File and Attachment State

Vortex has attachment and file-adjacent pieces, but not a complete shared
team-room file contract:

- `components/ai-elements/prompt-input.tsx` has client-side attachment state
  and file picker/drop affordances.
- `components/autopilot/prototype.tsx` has file-drop/upload prototype screens
  and a legal-workflow file start path.
- `convex/projectContext.ts` stores project files with summaries and source
  refs.
- `convex/autopilotLegal.ts` stores legal document workflow metadata but
  explicitly withholds document contents in the MVP.

Those pieces prove the UI and project/document concepts exist, but they do not
yet provide the requested team-room behavior: drop a PDF into a shared room,
show the file to all authorized team members, let the team discuss it, and later
pass that file plus bounded room context into a child Codex/opencode workroom.

### Current Autopilot Workroom Launch Path

Vortex can launch Codex VM workrooms, but the currently inspected direct UI path
starts from an outcome/workroom contract rather than a team-room mention. The
launch flow issues a provider grant, posts to `/api/workrooms/start`, moves the
active thread to the workroom thread, and dispatches workroom update events.

That path should be reused where possible, but the entry point must change:
`@autopilot` in a team room should create a linked child workroom while leaving
the parent team room visible with a compact run card.

### Current Workroom Rendering

`vortex/lib/json-render/workroom-events.ts` builds a workroom status section
from workroom events. It includes a `WorkroomStatusCard`, but it also includes
recent event lists, artifacts, and receipts. That is too much for the requested
inline team-room component.

`vortex/lib/json-render/codex-run-workroom.ts` builds a richer Codex run
workroom spec with status, timeline, tools, artifacts, logs, and controls. That
is appropriate for the full drilldown route. It is not the inline room
projection.

Vortex needs a compact projection component/spec for the parent room and should
continue using the richer workroom specs in the child workroom route.

### Current Routes

Vortex currently has `/chat` and `/t/:threadId` surfaces for the prototype. The
inspected tree does not expose a first-class `/teams/:teamRef/chat` route. Team
workspace behavior exists inside the prototype sidebar, not as a durable route
that resolves a team room by slug/ref.

A first-class route is recommended because OpenAgents product surface's route model and docs already
use explicit team room deep links such as `/teams/openagents-core-team/chat`.

### Vortex Tests Already Cover Related Invariants

`vortex/server/collaborativeThreadRegressions.test.ts` already covers:

- private thread owner-only behavior
- organization/project read/write boundaries
- shared workspace author provenance
- Codex launch access for project writers
- rejection without runner state creation for unauthorized launch attempts

`vortex/server/workspaceThreadScope.test.ts` covers:

- organization workspaces exclude private thread recents
- actor labels render as "You" for the viewer and stable display names for
  other authors

These tests are a strong base. The team-room work should extend them, not
replace them.

## OpenAgents product surface Prior Behavior and Reference Points

### Teams and Memberships

`0004_core_team_memberships.sql` defines `teams` and `team_memberships`, and
bootstraps an `OpenAgents Core Team` with slug `openagents-core-team`.

The worker has membership helpers such as `readTeamsForUser` and
`readActiveTeamMembershipRole`, and the session API includes teams for the
logged-in user. The logged-in sidebar/docs use explicit team room links.

This maps to Vortex organizations/memberships, not to a new Vortex team table.

### Team Chat Messages

`0012_team_chat_messages.sql` defines `team_chat_messages` with:

- `team_id`
- `author_user_id`
- `kind`
- `body`
- `autopilot_thread_id`
- `agent_run_id`
- `metadata_json`
- timestamps and soft delete

`kind` includes:

- `message`
- `autopilot_intent`
- `system`

`0014_autopilot_thread_ids.sql` backfills and indexes
`autopilot_thread_id` for `autopilot_intent` rows.

This is the previous concrete data shape for shared team history plus linked
Autopilot runs.

### Team Chat API

`handleTeamChatMessagesApi` serves:

- `GET /api/teams/:teamId/chat/messages`
- `POST /api/teams/:teamId/chat/messages`

Both require a browser session and active team membership. `GET` returns
bounded chronological history. `POST` validates a bounded body and accepts a
kind. When `kind` is `autopilot_intent`, the API launches an Autopilot mission,
stores `agentRunId`, stores an `autopilotThreadId`, and returns a `threadUrl`
such as `/t/<threadId>`.

That is the closest existing match to the requested behavior.

### Thread Files and Team Files

OpenAgents product surface now has a concrete file reference model for the clarified workflow.
`0020_thread_files.sql` adds `thread_files` with:

- `scope` as `personal` or `team`
- `thread_id`
- `team_id`
- `owner_user_id`
- `filename`
- `content_type`
- `size_bytes`
- R2 object key and checksum
- upload and scan status

The worker exposes `/api/thread-files` for personal/team-thread file listing
and upload, `/api/thread-files/:fileId/download` for authorized downloads, and
`/api/teams/:teamId/files` for team file listing. Team file access is gated by
active team membership. Team uploads fall back to a deterministic
`team:<teamId>:chat` thread id when a specific thread id is not supplied.

`0021_thread_file_pages.sql` extends this with shareable first-party file
detail pages:

- `thread_files.download_enabled` controls whether the raw artifact download
  endpoint is available. The default is enabled.
- `thread_file_message_refs` indexes messages that attach, select, or receive
  an Autopilot answer for a file.
- `GET /api/thread-files/:fileId` returns authorized file metadata,
  `canManage`, the current download state, and message references with anchors
  back to the chat message.
- `PATCH /api/thread-files/:fileId` lets the owner, team owner, or team admin
  toggle `downloadEnabled`.
- Team file pages use `/teams/:teamRef/files/:fileId`; personal file pages use
  `/files/:fileId`.

The logged-in room panel loads files by thread scope and shows uploaded files
beside run/artifact state. This is not yet the final drag-and-drop UX described
by the user, but it is a useful reference for the required shared-file records.
Team file list rows now open the file detail page instead of immediately
downloading the raw object, so users can inspect references first.

### OpenAgents product surface UI State

The logged-in Foldkit app has a `TeamChat` route and a `teamChatThreadId`
helper returning `team:<teamId>:chat`. The room view can render team context,
file scope, chat messages, run status, run events, artifacts, and tokens.

At audit time, the reducer's `SubmittedChatComposer` path still launched an
Autopilot run for every submitted prompt. The client did not have a wired
`LoadTeamMessages` or `PostTeamMessage` command, so the most reliable OpenAgents product surface
source for "previous version had team message history" was the D1/API layer and
route model.

As of issue #14, OpenAgents product surface's logged-in web client now models durable team chat
messages, loads `/api/teams/:teamId/chat/messages` on `TeamChatRoute`, renders
author-labeled team history in the workroom timeline, posts plain team messages
with `kind: "message"`, and only creates a team Autopilot intent when the
submitted text exactly starts with `@autopilot `. Personal `/chat` still uses
the direct `/api/omni/agent-runs` launch path.

### OpenAgents product surface Sync

OpenAgents product surface has the new sync substrate:

- `workspace:<userId>`
- `team:<teamId>`
- `thread:<threadId>`
- `agent-run:<runId>`
- D1 `sync_changes` outbox snapshots
- Durable Object WebSocket streams
- schema-validated patches and cursor gaps

The worker authorizes team sync scopes by active team membership and authorizes
thread/agent-run scopes by the user's run access.

Current run launches notify workspace, agent-run, and thread scopes. As of issue
#15, team chat message writes and team-scoped thread-file uploads also append
`team:<teamId>` sync changes and notify the matching SyncRoom. The logged-in web
client subscribes `TeamChatRoute` and `TeamFilesRoute` to that team scope and
projects `team_chat_messages` / `thread_files` patches into the existing team
chat and file state.

`TeamFileRoute` also subscribes to the team scope so a direct file detail page
can stay consistent with room-scoped file updates. File references are written
when a team message attaches a file, when an `@autopilot` message selects a
team file as context, and when an Autopilot answer back references the selected
file ids. Each reference stores the parent message id and renders as a
`#message-<id>` link into the team chat timeline.

## Required Vortex Changes

### 1. First-Class Team Room Resolution

Add a Vortex team-room route, preferably:

```text
/teams/:teamRef/chat
```

On entry:

- resolve `teamRef` to a Vortex organization slug/ref
- require active organization membership
- get or create the canonical room thread
- load the room's messages immediately
- load the room's files immediately
- subscribe to future room updates

Recommended canonical room thread:

```text
clientThreadId = team-room:<organizationId>:main
routeId = /teams/<organizationSlug>/chat
organizationId = <organizationId>
projectId = undefined
visibility = organization
threadKind = team_room
```

If adding a new `threadKind` is considered an invariant/policy expansion,
update `vortex/INVARIANTS.md` with the new scoped-team-room contract and add
matching tests.

### 2. Split Team Room Composer Semantics

The team-room composer should not reuse the current "always stream a chat turn"
behavior.

For team rooms:

- plain text appends a shared team-room message
- dropped/selected files create durable team-room file records
- exact `@autopilot` mention launches a linked Autopilot workroom
- empty messages are ignored
- unauthorized users cannot append or launch

The mention parser should be deliberately bounded. The workspace contract
forbids ad hoc string/keyword matching for user-facing intent routing. This
case is not semantic intent routing if it is implemented only after the user is
already in the team-room composer and it accepts only a literal mention token,
for example:

```text
@autopilot <prompt>
```

Do not expand this into keyword detection such as "run autopilot", "ask
autopilot", or fuzzy aliases unless a typed semantic selector or modeled parser
is introduced.

The phrase after the mention may contain natural references such as "that PDF".
Resolve those references from explicit room state after the mention route has
already been selected. The first implementation can use a bounded deterministic
rule, for example "the most recent PDF attached to this team room that the
invoking user can read", plus explicit file chips when the UI exposes them. Do
not use loose keyword routing to decide whether Autopilot should run.

### 3. Store the Parent Message and Child Workroom Link

When a team member submits `@autopilot summarize that PDF`, Vortex should
persist:

- the parent team-room user message
- the child workroom thread
- a link between parent message and child workroom/run
- the room file ids selected or resolved for the child run
- the run's compact projection state
- the eventual answer-back message id once Autopilot posts into the parent room

There are two reasonable Vortex-native approaches:

1. Add a dedicated link table, for example `teamRoomRunLinks`, with
   organization id, parent room thread id, parent message id, child workroom
   thread id/client id, run ids, selected file ids, current status, latest
   summary, final answer message id, and timestamps.
2. Reuse generated UI specs attached to the parent message, provided the link
   and status projection are queryable without scanning or replaying the child
   workroom transcript.

The dedicated link table is cleaner for sync and compact room projection. It
also avoids mixing full workroom timeline state into the parent room message.

### 4. Create the Child Workroom Context Bundle

The child Codex/opencode workroom should not receive an unbounded room dump.
Create an explicit context bundle at launch time:

- parent organization/team id
- parent room thread id/client id
- invoking parent message id
- normalized prompt after the `@autopilot` mention
- selected/resolved file ids and download/read handles
- bounded recent room messages with author provenance
- authorization snapshot for the invoking user

For `@autopilot summarize that PDF`, the bundle should include the referenced
PDF file record and a stable way for the runner to read it. The child thread can
contain the full tool work, extraction, parsing, and reasoning transcript. The
parent room should contain only the invoking message, compact status projection,
and final answer.

### 5. Add a Compact Inline Run Projection

Do not reuse the full `workroomEventsToJsonRenderSpec` or full
`codexRunToJsonRenderSpec` inline in the team room. They include detailed
timelines, artifacts, logs, receipts, or controls.

Add a compact spec/component with a narrow contract:

```ts
type InlineRunStatus = {
  href: string
  latestSummary?: string
  parentMessageId: string
  runId?: string
  status:
    | 'queued'
    | 'starting'
    | 'running'
    | 'waiting_for_input'
    | 'blocked'
    | 'completed'
    | 'failed'
    | 'cancelled'
  title: string
  updatedAt: number
  workroomClientThreadId: string
}
```

The component should render:

- run title or normalized prompt
- status label
- latest event summary
- compact timestamp if useful
- link/open action to the full workroom
- optional attachment count/name hint, for example `1 PDF`

It should not render:

- the full event list
- full transcript
- tool call list
- artifact gallery
- receipts
- approval controls
- steering input

Those belong in `/t/:threadId` or the future workroom route.

### 6. Full Workroom Drilldown

Clicking the inline card should open the child workroom route. Today the
closest Vortex route is `/t/:threadId`, and the prototype can load a durable
thread by client thread id. That route should render the full Codex/workroom
projection and controls:

- event timeline
- assistant/workroom messages
- artifacts
- receipts/provenance
- approvals
- continue/steer/cancel controls where authorized

The parent team room should remain a room timeline with compact workroom cards,
not a duplicated workroom transcript.

### 7. Post the Final Autopilot Answer Back Into the Team Room

When the child Codex/opencode run reaches a terminal successful state, Vortex
should append a normal team-room message from Autopilot. For the PDF summary
example, the visible parent room flow should become:

```text
chris: @autopilot summarize that PDF
[compact inline run status card]
Autopilot: Here's a summary: ...
```

The first implementation source should be the final assistant/final message in
the child Codex/opencode thread. If the runner emits a structured final answer,
use that. If the run completes without a final answer, fall back to a bounded
completion summary from the run record. If the run fails, post a concise failure
message only when the failure is useful to the team room; otherwise leave the
inline card in failed state and keep diagnostics in the child thread.

The answer-back message should include provenance:

- child workroom thread id/client id
- run id
- source final child message id or event id when available
- selected file ids used by the run
- author identity as Autopilot/system, not the invoking human

This final answer is part of the team-room history. Future members opening the
room should see the original request, the compact run card/history state, and
the Autopilot answer without opening the child workroom.

### 8. Live Updating Through Sync

The desired sync scopes are:

```text
team-room:<organizationId> or team:<organizationId>
thread:<workroomClientThreadId>
agent-run:<runId> or codex-run:<runId>
```

For the parent room, sync should include:

- new team-room messages
- new team-room file records
- new parent-message-to-workroom links
- compact run projection patches: status, latest summary, updatedAt
- final Autopilot answer-back messages

For the child workroom route, sync should include the full event/workroom/run
records and the file/context bundle required by that workroom.

In OpenAgents product surface, the sync substrate already supports team/thread/agent-run scopes, but
team chat messages need explicit outbox writes and `team:<teamId>` notifications
after POST. In Vortex, Convex live queries already provide a subscription model,
but the inspected app still uses custom browser events and polling for some
workroom refreshes. If the "new sync thing" is being ported from OpenAgents product surface into
Vortex, the team-room work should be one of its first concrete consumers.

If Vortex remains Convex-native for this pass, use Convex queries as the live
source and keep the data model shaped so it can be mirrored into OpenAgents
Sync scopes later without changing product behavior.

## Suggested Implementation Sequence

1. Add Vortex tests for team-room access and deterministic room resolution.
2. Add the canonical team-room thread resolver.
3. Add a first-class `/teams/:teamRef/chat` route that loads that room.
4. Add durable team-room file records and upload/drop handling.
5. Split team-room composer behavior from generic Autopilot chat behavior.
6. Add a bounded exact `@autopilot` mention parser with unit tests.
7. Add context-bundle construction for selected files and recent room messages.
8. Add the parent-message-to-child-workroom link record or queryable generated
   UI attachment.
9. Reuse the existing workroom/Codex launch path from the new team-room action.
10. Add the compact inline run-status spec/component.
11. Append the final Autopilot answer back into the parent room on completion.
12. Wire live updates for room messages, room files, final answers, and compact
    run projections.
13. Add full route drilldown tests and browser verification.

## Test Plan

### Convex and Server Tests

- A team member opening `/teams/<slug>/chat` resolves to the same canonical
  organization room thread every time.
- A non-member cannot read or write the team room.
- A member's plain message appends to the room with author provenance.
- A member can upload a PDF into the team room, and another active member can
  see the file metadata and download/open it through the authorized path.
- A non-member cannot read the team-room PDF metadata or bytes.
- Organization room recents do not include private personal threads.
- `@autopilot summarize that PDF` in a team room creates:
  - a parent user message
  - a child workroom thread
  - a parent-message-to-child-workroom link
  - selected/resolved file refs for the PDF
  - a bounded context bundle with recent room messages
  - an initial compact run projection
- A failed or unauthorized launch does not leave runner state behind.
- The mention parser accepts exact `@autopilot <prompt>` after the team-room
  route has already been selected.
- The mention parser rejects fuzzy keyword routing.
- A room phrase such as "that PDF" resolves only against authorized room file
  records after the exact mention route is selected.
- The inline projection query returns only status/latest summary/link metadata,
  not full event transcripts.
- On successful child completion, Vortex appends a parent-room Autopilot answer
  sourced from the final child assistant/final message when one exists.
- The answer-back message stores provenance to the child workroom/run, source
  child message or event, and selected file ids.
- Full workroom queries still return the detailed thread/workroom records.

### UI Tests

- Navigate to `/teams/openagents-core-team/chat`.
- Existing room messages are visible on first paint after data load.
- Existing room files are visible on first paint after data load.
- Author labels are visible for shared team messages.
- Dropping a PDF on the team-room input uploads it and shows it in the room's
  file context without starting Autopilot.
- Submitting a plain message adds only a shared room message.
- Team members can discuss the uploaded PDF through normal shared messages.
- Submitting `@autopilot summarize that PDF` adds the user message and an
  inline run card.
- The inline run card shows status plus latest activity only.
- The inline run card does not show a long event list, artifact list, or
  steering input.
- Clicking the card opens `/t/<childWorkroomThreadId>` or the future workroom
  route.
- The full workroom route shows the complete thread/run controls.
- When the child run completes, the parent team room shows an Autopilot answer
  message with the summary.
- A second team member sees the file, parent message, compact run-status
  updates, and final Autopilot answer.

### Sync Tests

- Posting a team-room message emits a room-scope patch.
- Uploading a team-room file emits a room-scope file patch.
- Launching an Autopilot child workroom emits a room-scope link/projection patch.
- Run status changes emit compact projection patches to the parent room scope.
- Child completion emits the final parent-room Autopilot answer patch.
- Full run events emit to the child thread/run scope.
- Reconnect with a stale cursor receives either missed patches or a cursor gap
  that forces snapshot reload.
- Polling/custom event refresh remains a fallback only where sync is not yet
  available.

### Regression Tests

- Existing private thread access remains owner-only.
- Existing project and organization thread write roles still pass.
- Shared workspace author provenance remains intact.
- Existing Codex/project launch authorization still rejects unauthorized
  viewers without creating runner state.

## Risks and Open Questions

- Vortex does not currently expose a first-class team chat route. Adding one is
  straightforward, but it is a product-surface change and should be covered by
  tests.
- Vortex's current generic chat submit path always starts an assistant/model
  turn. Team-room composer semantics need to be split deliberately.
- Vortex does not currently have a single complete team-room file upload,
  authorization, extraction, and runner-read contract. OpenAgents product surface's `thread_files`
  path is a useful reference, but Vortex needs its own Convex/storage/sync
  shape.
- PDF contents need a bounded extraction path before the runner can summarize
  them. The child workroom should receive authorized file handles or extracted
  text/artifacts, not browser-only object URLs.
- The current Vortex Codex VM launch path expects provider-account readiness
  and grant issuance. The team-room `@autopilot` flow needs a clear fallback
  when the invoking user cannot launch a run.
- Answer-back must be idempotent. Runner retries, duplicate terminal events, or
  sync replay must not post multiple final Autopilot summaries into the room.
- OpenAgents product surface's sync layer now publishes and broadcasts team chat, team files, and
  final answer-back messages through `team:<teamId>`. The compact inline
  parent-room run card is implemented in the Foldkit room view; the full run
  event stream remains scoped to `/t/:threadId`.
- Vortex appears to rely on Convex live data plus custom browser events and
  polling in parts of the prototype. If OpenAgents Sync is being ported into
  Vortex, the implementation should avoid building another parallel live-update
  mechanism.
- `vortex/convex/threads.ts` has an adjacent suspicious patch call in
  `appendWorkroomEventForClientThread`: it appears to call `ctx.db.patch` with
  the document id but without the table name. Confirm before implementing the
  workroom-update path.
- If a new `threadKind` or invariant-bearing route is introduced, update
  `vortex/INVARIANTS.md` and add the corresponding regression coverage in the
  same change.

## Recommended Shape

Use Vortex's existing thread model as the source of truth:

- one canonical organization room thread for the team room
- normal message rows for shared room history
- team-room file records for uploads, including PDFs
- child workroom threads for Autopilot runs
- explicit context bundles from parent message, selected files, and bounded
  room conversation into child workrooms
- a compact parent-room run projection linked to the invoking message
- a final Autopilot answer message posted back into the parent room
- full workroom detail only in the child route

Port OpenAgents product surface's behavior and route semantics, not its exact D1 schema. The OpenAgents product surface
`team_chat_messages` table is the historical proof that team rooms had durable
message history and Autopilot-intent rows. Vortex already has a broader durable
contract, so the implementation should make the team room a first-class scoped
thread and attach child workroom projections to that room.

The first build should be considered complete only when the route, shared
history, file upload, plain-message behavior, `@autopilot` child launch,
compact inline live status, full drilldown, final answer-back, and cross-member
update behavior are all covered by automated tests plus a browser verification
pass.
