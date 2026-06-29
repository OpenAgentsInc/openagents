# Logged-In Sidebar Consolidation

Date: 2026-06-03

Status: Implemented for GitHub issue #8. Follow-up refactor moved the
production logged-in shell to `apps/web`. The chat composer follow-up removed
the local mock assistant stream and now launches real Worker-owned Autopilot
runs backed by SHC dispatch.

## Summary

OpenAgents product surface now treats the logged-in left rail as one shared workroom sidebar instead
of separate app navigation and chat/session rails. The production logged-in UI
is now served from the Foldkit/Vite app in `apps/web`; the Worker owns auth,
API routes, and the session bootstrap endpoint.

The implemented direction follows the OpenChamber sidebar structure while
keeping OpenAgents product surface's existing dark, compact, mono operational design:

- one desktop rail in the 258-280px range;
- product/account header;
- muted active navigation rows;
- section labels;
- recent/project workroom sections;
- compact session rows with status and attention metadata;
- bottom signed-in user/account strip;
- one mobile disclosure menu for the same navigation/session model.

The right review/files/context panel remains separate and is still owned by the
chat/workroom route.

The production Worker no longer renders the authenticated product shell from
`loginHtml(...)`. Product routes serve the app shell, and `apps/web` calls
`/api/auth/session` to load the cookie-backed user, team rooms, recent
missions, admin flag, and token summary data.

## Code Structure

Primary files:

- `apps/web/src/page/loggedIn/model.ts`
  - Adds `SidebarModel`, `SidebarNavItem`, `SidebarSessionSection`,
    `SidebarSessionItem`, and footer row schema fields.
  - Initializes sidebar state from authenticated session/team bootstrap data in
    `initSidebar()`.
  - Builds Team Rooms as clean app routes such as
    `/teams/openagents-core-team/chat` instead of pointing every room at the
    generic `/chat` route.
  - Adds a Missions section below Team Rooms from recent Worker-owned
    Autopilot runs. Mission rows link to `/t/:missionId`, carry
    status/attention metadata, and highlight when the thread route is active.
    New mission IDs are UUIDs. Older stored `agent_run_<32 hex>` rows are
    exposed through UUID route aliases instead of leaking the storage prefix in
    the sidebar.

- `apps/web/src/ui/index.ts`
  - Adds `workroomSidebar`.
  - Keeps `workroomSessionRail` available as an older primitive, but the logged
    in app now uses the unified sidebar.
  - Extends `WorkroomSessionItem` with optional `attention`.
  - Adopts the OpenChamber-style full-height chat structure: one flexing
    timeline viewport, a bottom composer dock outside the scroll region, and a
    right context rail that spans the route height.

- `apps/web/src/page/loggedIn/view.ts`
  - Owns the shared sidebar and shell for all logged-in routes.
  - Renders Chat, Dashboard, Settings, and NotFound inside the same
    `workroomShell`.
  - Keeps the authenticated default route personal: root/session/login defaults
    initialize and redirect to `/chat`; team room routes are only entered from
    explicit `/teams/:teamRef/chat` links.
  - Treats `/t/:threadId` as a real sidebar href so mission rows can become
    active instead of incorrectly highlighting `/chat`.
  - Hydrates `/t/:threadId` on direct page load and in-app navigation by
    fetching `/api/omni/agent-runs/:threadId`.
  - Adds a single mobile disclosure menu sourced from the same sidebar model.
  - Wires the sidebar `New` action as a real Foldkit message that clears the
    current transcript, resets the composer, clears the active run projection,
    and returns focus to the prompt.

- `apps/web/src/page/loggedIn/page/chat.ts`
  - No longer renders its own left rail.
  - Returns the center timeline, composer, and right review/context panel.
  - Uses full-height workroom wrappers so the message area stretches between the
    header and composer instead of content-sizing at the top of the page.
  - No longer seeds the production chat surface with the GitHub writeback smoke
    run fixture.
  - Submits the composer on Enter while preserving Shift+Enter for multiline
    draft editing.
  - Projects run metadata, source, runtime, backend, runner ID, repository,
    event count, and token totals from `/api/omni/agent-runs` responses instead
    of emitting hardcoded assistant prose.
  - Reconstructs the saved user turn from the persisted run goal when a mission
    is opened from the sidebar, then renders the saved SHC/OpenCode event
    timeline.
  - Does not render placeholder controls for unwired file upload, image attach,
    mode switching, search, or right-panel tab switching.

- `apps/web/src/subscriptions.ts`
  - Opens the authenticated `workspace:{userId}` OpenAgents Sync WebSocket from
    the last known cursor and emits Foldkit messages for connection state,
    patches, and cursor gaps.
  - Opens additional `thread:{threadId}` streams on mission routes and
    `agent-run:{runId}` streams while an active run is visible.
  - Demotes active run polling to a fallback that only activates when the
    relevant `agent-run:{runId}` stream is failed or closed.
  - Keeps non-chat routes inactive for active-run polling so background pages
    do not poll run details.

- `apps/web/src/page/loggedIn/view.scene.test.ts`
  - Verifies the unified sidebar appears on Chat, Dashboard, and Settings.
  - Verifies recent Missions render below Team Rooms, link to
    `/t/:missionId`, and highlight on the active thread route.

- `apps/web/src/page/loggedIn/update.test.ts` and
  `apps/web/src/subscriptions.test.ts`
  - Verify the `New` chat action clears active chat state.
  - Verify composer submission launches the real SHC run command, records
    source/token metadata from runner events, and polls active run details.
  - Verify the sync snapshot and patch reducers project Missions into the
    sidebar without a browser refresh and reload snapshots on cursor gaps.
  - Verify mission route entry loads the transcript from
    `thread:{threadId}` sync snapshots first, falls back to the compatibility
    detail API only when the snapshot is empty, and updates active transcripts
    from `agent-run:{runId}` event patches.
  - Verify a queued launch response exits the local `Launching` state so the UI
    can poll the persisted run instead of waiting on SHC dispatch.
  - Verify mission route navigation fetches and renders saved run details
    instead of only changing the active sidebar row.

- `workers/api/src/index.ts`
  - Serves `apps/web/dist` for `/`, `/login`, `/chat`,
    `/teams/:teamRef/chat`, `/dashboard`, `/settings`, and `/t/:threadId`.
  - Exposes `/api/auth/session` as the authenticated app bootstrap payload.
  - Includes a compact recent Missions projection from `agent_runs`, using the
    run goal, repository, status, update time, and `/t/:missionId` href.
  - Resolves `/api/omni/agent-runs/:missionId` for both real UUID run IDs and
    legacy UUID aliases backed by older `agent_run_<32 hex>` storage IDs.
  - Returns `202 queued` run detail immediately on user mission launch, then
    dispatches the SHC control-plane request through `ctx.waitUntil()` so a slow
    runner cannot strand the browser in a local launch state.
  - Logs real SHC dispatch start, success, and failure rows with the mission
    UUID, runner ID, backend, repository, dispatch mode, and sanitized control
    URL. The 2026-06-03 timeout incident was the SHC `oa-codex-control` process
    wedging while still listening on port 8787; the Worker now records an
    actionable control-health timeout message instead of the generic Fetch abort
    text.
  - Keeps the Worker focused on auth, APIs, cookie refresh, and stale-cookie
    cleanup for product routes.
  - Rejects fake runner backend selection on current launch requests.

- `workers/api/src/omni-runs.ts`
  - Removes fake SHC dispatch fallback behavior. If live SHC dispatch is not
    configured, the run records a real dispatch failure instead of pretending a
    queued runner exists.
  - Creates new agent run IDs as UUIDs by default and keeps the legacy
    `agent_run_` prefix limited to old persisted rows.

- `workers/api/src/admin-access.test.ts`
  - Verifies product routes return the Vite app shell instead of Worker-rendered
    product HTML.

## Follow-Up Work

The current sidebar room list is still initialized from authenticated session
bootstrap data rather than a richer live workroom/session projection. The next
model pass should replace those simple room rows with the typed workroom/session
event model described in `docs/2026-06-03-openchamber-ui-ux-port-research.md`.
