# OpenAgents Desktop guarantees

This is the public, agent-readable summary of what OpenAgents Desktop currently
promises. It describes behavior enforced on `main`, not roadmap intent.

The machine source of truth is
[`src/contracts/ux-contracts.ts`](./src/contracts/ux-contracts.ts). A guarantee
is listed as a UX guarantee below only when its contract is `enforced` and its
oracle runs in the normal test sweep.

## Current UX guarantees

### Closed Desktop Runtime Gateway protocol

The signed renderer reaches host runtime state through one versioned,
schema-decoded query/command/event seam.

- Bootstrap reports the gateway lifecycle and only truthful capability state.
- Unsupported conversation commands return `unavailable`, never completed;
  runtime enqueues return only `unknown_pending_reconcile` until confirmed,
  and argument-free native-session commands return bounded phase outcomes.
- Lifecycle events have a monotonic sequence and an owned disposer. Live
  conversation updates carry their own subscription generation/sequence and
  durable cursor through the same decoded event channel.
- Electron main validates the top-level bundled renderer before serving a
  request.
- Tokens, credentials, URLs, raw runtime events, arbitrary IPC, `MessagePort`,
  filesystem/process handles, and command arguments cannot enter the contract.

Protocol v8 carries bounded OpenAgents entry/exit, canonical confirmed-
conversation operations, exact-ref runtime start/interrupt, durable command
outcomes, confirmed bounded agent-timeline and live-agent-graph snapshots, and
typed cursor-aware conversation subscribe/resume/unsubscribe. Provider
execution stays behind the host; only canonical projected facts reach the
renderer.

Contract:
`openagents_desktop.seam.runtime_gateway_closed_protocol.v1`.

### Host-owned Khala Sync persistence

Electron main opens the existing shared Khala Sync SQLite store in an
owner-private directory under Desktop `userData`.

- One installation identity is generated once and reused after restart.
- The shared store schema and semantics remain the only cache/offline-queue
  implementation; Desktop does not create a parallel Sync database.
- Sparse advancing event batches are refused before cursor advance and replay
  from the durable cursor; MustRefetch atomically replaces the exact scope.
- Supported unversioned stores migrate in place. A newer store version refuses
  before additive migration with typed update-or-reset recovery guidance.
- A server-verified native session starts the shared HTTP/WebSocket engine on
  the server-derived owner's personal scope; rotation is re-read host-side.
- The Sync session closes before the store on quit.
- The renderer receives only bounded readiness—never the database path/handle,
  installation refs, rows, pending mutations, or credentials.

Contract: `openagents_desktop.sync.host_owned_sqlite.v1`.

This persistence/authenticated-host contract alone does not claim conversation
projection or live-device acceptance. The separately enforced conversation
contract below owns the bounded confirmed projection.

### Native authoritative conversation continuity

Once personal Sync is live, the host exposes one bounded conversation service
over the canonical `chat_thread` / `chat_message` entities and
`chat.createThread` / `chat.appendMessage` mutators.

- Only server-confirmed rows appear in list results, with stable public-safe
  refs, confirmed entity versions, scope cursor, and actual Sync phase.
- Owner identity, credentials, raw store/session/overlay/transport objects, and
  optimistic bodies do not enter the projection.
- Denial and proven sign-out immediately refuse new mutation, burn queued
  hosted commands, clear subscribed hosted projections, and remove the
  capability. Transient disconnect/close keeps reconstructible queue/cache.
- The normal Desktop sweep proves Desktop start, mobile continuation, matching
  refs/versions/cursor, and restart reconstruction over the real native store
  adapters without duplicate objects.
- Delayed bootstrap/log responses are fenced by the scope generation that
  requested them. Proven unlink/revocation burns the queue and retracts hosted
  personal/thread projections in both native stores; a late transport response
  cannot repopulate or acknowledge that revoked generation.

This is the durable owner-message floor. It does not claim renderer wiring,
provider/runtime event streaming, assistant-role inference, physical-device
acceptance, or a deployed live-account receipt.

Contract: `openagents_desktop.sync.native_conversation_continuity.v1`.

### Closed Runtime Gateway conversation protocol

Protocol v8 carries schema-bounded `conversation.catalog`,
`conversation.thread`, `conversation.timeline`, and exact-intent
`conversation.commandOutcome` queries plus
`conversation.create`, `conversation.append`, `conversation.start`, and
`conversation.interrupt` commands plus `conversation.subscribe` and
`conversation.unsubscribe`.

- Queries return confirmed public-safe refs/bodies/timestamps/entity versions,
  actual scope phase/cursor, and pending count.
- Commands enqueue canonical Sync mutations and return `pending_reconcile` or
  `unknown_pending_reconcile` with the durable mutation id. Enqueue is never
  reported as completed.
- The same stable intent ref resolves after reconnect or restart to pending,
  accepted, settled, expired, failed, or canceled with its confirmed version.
  Expired commands are terminal and never reach provider dispatch.
- Not-live and read failure are typed, body-free unavailable results.
- Live subscriptions carry bounded canonical snapshots with explicit
  provisional/confirmed/interrupted delivery. Exact generation replacement,
  stale unsubscribe, capacity, reset, and disposal are host-owned and typed.
- Owner identity, credentials, native/store/session/overlay/transport objects,
  raw events, provider authority, and generic IPC do not cross preload.

The renderer selects this authority once at boot when the confirmed catalog is
live; otherwise it retains explicit local-only mode. It does not mix catalogs.
The built-live and physical-mobile receipt remains separate acceptance.

Contract: `openagents_desktop.seam.runtime_gateway_conversation.v1`.

### Confirmed agent timeline protocol

Protocol v8 carries `agent.timeline` by bounded exact `runRef` and
`conversation.timeline` by exact confirmed `threadRef`.
Electron main composes the shared confirmed reader only while authenticated
personal Sync is live.

- A live result carries the exact confirmed run and its server-projected
  `routeRef`, lifecycle/version, scope phase/cursor/pending count, and at most
  500 ordered bounded canonical lifecycle items.
- The server-projected `agent_run.routeId` is the only thread/route attachment
  fact. The renderer cannot derive a route from a run ID.
- Non-live, not-found, and read-failure outcomes are typed and body-free.
- Bounded runtime/backend/WorkContext classification may cross; owner,
  objective, repository contents, provider source, raw payload, external
  callbacks, credentials, store/session/transport, and process authority may
  not.

The same projection drives visible Desktop and mobile timeline rendering.
Runtime events are admitted only at the turn's exact durable next sequence and
valid lifecycle state. After interruption/close/terminal settlement, stale
provider output cannot enter the projection. If a hosted worker dies after its
durable start claim, the server projects one interrupted terminal without
replaying inference; Desktop and mobile reconstruct the same partial history
plus terminal after store restart.
Live-account/physical-device proof remains the final #8676 gate.

Contract: `openagents_desktop.seam.runtime_gateway_agent_timeline.v1`.

### Confirmed live-agent graph protocol

Protocol v8 adds `graphRefs` and validated `openagents.live_agent_graph.v1`
post-images to the existing thread subscription snapshot.

- Graphs come only from the authenticated canonical thread scope and only
  while that scope is live; cached rows cannot claim current authority.
- Exact reconnect resumes at the durable cursor. A proven cursor gap returns
  one newest authoritative-refetch snapshot rather than replaying provider
  history or opening another stream.
- One update carries at most eight graphs and at most 2,000 nodes / 4,000
  edges in aggregate. Matching graph refs make correlation explicit.
- Provider callbacks, histories, credentials, store/session/transport handles,
  and process authority remain host-only. Unsupported facts stay explicit
  unknowns in the canonical graph.

Contract:
`openagents_desktop.seam.runtime_gateway_live_agent_graph.v1`.

### Visible authoritative Sync conversation mode

### Local-first identity and optional account link

Desktop creates an immutable device-local identity before OpenAuth. Local
authority uses separate SQLite tables, `LocalRevision`, and a device-local
scope that hosted Sync rejects. Runtime Gateway v8 exposes only the tier, never
the identity/owner ref. A server-verified account link adds personal Sync;
disconnect, denial, failed link, and restart preserve the local identity and
local rows. The workbench, history, local Pylon, and local conversation path do
not require an OpenAgents account.

Contract: `openagents_desktop.seam.identity.local_first_account_link.v1`.

The existing Effect Native shell consumes Runtime Gateway v8 through a typed
renderer adapter whenever confirmed conversation Sync is live at boot.

- Sidebar and transcript map only confirmed thread/message projections.
- New-chat and composer actions generate stable refs, enqueue canonical
  chat plus runtime mutations, and wait for exact refs and terminal state.
- Confirmed text, reasoning, connection, tool/plan, usage, interruption, and
  terminal items stream into the active transcript during the turn.
- A confirmation timeout renders an honest pending-reconciliation error; it is
  never converted to completion.
- Authority mode is selected once per renderer lifetime. Signed-out/not-live
  startup stays in the explicit local-only host and never merges its catalog
  with account-linked Sync later in that renderer lifetime.
- The adapter adds no preload method, IPC channel, owner/auth field, or second
  UI architecture.

Canonical `chat_message` rows remain owner-role input. Assistant/system rows
come only from bounded `agent_run_event` projections. Live GUI/account
acceptance remains separate evidence.

Contract: `openagents_desktop.chat.authoritative_sync_mode.v1`.

### OS-encrypted native-session custody

Electron main owns one versioned native OpenAgents session record encrypted by
Electron `safeStorage` beneath the private Desktop `userData` root.

- Access, refresh, and server-derived owner fields exist only inside the
  encrypted payload.
- The enclosing directory/file are owner-private and replacement is atomic.
- Custody is unavailable when OS encryption is unavailable; Linux
  `basic_text` is explicitly refused.
- Malformed, undecryptable, incomplete, and retired-epoch records purge
  fail-closed.
- Runtime Gateway receives only signed-out, credential-present-unverified, or
  unavailable capability copy; preload and renderer receive no credential.

This custody guarantee does not claim Desktop PKCE sign-in. A recovered record
remains unverified until the validation boundary below accepts it.

Contract: `openagents_desktop.session.os_encrypted_custody.v1`.

### Recovered native-session validation and rotation

On startup, Electron main validates a recovered encrypted credential through
the existing native-session GET using the bearer and bounded refresh headers.

- A valid server-derived owner produces bounded `session_ready` capability
  state only.
- OpenAuth replacement credentials are rewritten to encrypted custody before
  readiness is projected.
- 401/403 and server-owner mismatch purge the record.
- Network, server, and response-schema failures retain custody but project
  unavailable so no private shared work can render.
- Owner and token values never enter Runtime Gateway, preload, or renderer.

This is session verification, not live Khala Sync or interactive Desktop
sign-in.

Contract:
`openagents_desktop.session.recovered_validation_rotation.v1`.

### Loopback PKCE entry and fail-closed sign-out

Electron main owns the complete Desktop-native OpenAuth host flow.

- It binds literal `127.0.0.1` on an OS-assigned port and accepts only the
  exact callback method/path/state/code.
- It generates cryptographic state and S256 verifier/challenge, launches the
  exact `openagents-desktop` GitHub authorization request, and closes the
  listener after one terminal callback or timeout.
- The callback page is no-store and never reflects code, state, tokens, or
  errors.
- Code exchange is followed by server-owner verification and immediate
  rotation persistence before encrypted custody becomes verified.
- Sign-out clears locally only after the server proves access and refresh
  revocation; failure retains custody.
- Runtime Gateway session commands have no arguments and return only bounded
  completed/cancelled/unavailable phase.

This host guarantee does not claim live Sync, package identity, or physical
acceptance. Visible controls are covered separately below.

Contract: `openagents_desktop.session.loopback_pkce_entry_exit.v1`.

### Typed Effect Native session controls

Desktop Settings renders the explicit Runtime Gateway session phase and one
appropriate visible action.

- Signed-out, denied, and unavailable states offer `Sign in with GitHub`.
- Session-ready offers `Sign out`.
- Loading, unverified, and authenticating states disable the action and show
  honest progress copy.
- Both controls dispatch typed Effect Native intents to argument-free Runtime
  Gateway commands.
- Runtime responses are schema-decoded to bounded phase only; no browser URL,
  callback, owner, credential, or storage data reaches renderer state.
- Session-ready copy does not claim live network Sync.

Contract: `openagents_desktop.session.effect_native_controls.v1`.

### Loss-accounted local Codex history and subagents

Desktop indexes active and archived Codex rollouts without an age ceiling.

- The left sidebar remains a top-level conversation list; every child and
  grandchild stays attached in the semantic agent tree.
- Selecting an agent loads a source-order page of typed messages, reasoning
  summaries, plans, collaboration, tools, approvals, usage, lifecycle, errors,
  context and explicit gaps. The visible timeline is a product projection:
  standalone usage/session/context/lifecycle/developer rows are suppressed and
  matching tool calls/results are composed into one tool row; blank/trivial
  messages and unpersisted-reasoning placeholders are omitted. The raw bounded
  page and accounting still retain every record; unknown/corrupt records are
  never silently dropped.
- The resizable inspector exposes agent topology or structured selected-item
  fields; pages are bounded to 500 and large lists use Effect Native windowing.
- Credential-shaped fields are visibly redacted and encrypted/raw reasoning is
  not projected. Completeness remains a tested host/data invariant and Electron
  acceptance check instead of permanent conversation chrome.
- Missing or malformed local history produces an honest, usable empty state.
- History access is read-only. This projection does not grant authority to
  resume a Codex session, send a message, browse arbitrary files, sync history,
  or dispatch work.

Contract:
`openagents_desktop.seam.codex_loss_accounted_history.v2`.

### Real-Electron Codex trace acceptance

The normal Desktop verification sweep opens the built Electron app against a
checked-in, privacy-safe Codex catalog and exercises the recorded-demo path.
The deterministic gate never reads ambient `~/.codex` history. A separately
identified real-history acceptance may opt in with
`OPENAGENTS_DESKTOP_CODEX_SESSIONS`; that evidence is not the CI baseline.

- Top-level rows are named, newest-first, and contain no child sessions.
- A nested trace exposes its bounded agent topology, keyboard bindings,
  composed tool row, and structured inspector; acceptance verifies the
  completeness equation directly from the typed page response.
- Explicit conversation selection expands every branch; subsequent agent,
  item, paging, and inspector actions preserve the ref-only restoration record.
- A real renderer reload restores the selected trace/item and expanded set,
  including a selected child whose canonical root remains in the visible
  catalog window; missing, orphaned, cyclic, and off-window refs fail closed.
- Smoke output contains only aggregate counts and readiness timings; it never
  prints titles, transcript text, paths, raw records, credentials, or refs.

Contract:
`openagents_desktop.seam.codex_trace_electron_acceptance.v1`.

The accepted trace shell also owns the full Electron content rectangle: there
is no decorative outer inset, split panes cannot widen the document, and the
sidebar/timeline/inspector are bounded independent scroll owners. Large
catalogs use Effect Native virtualization and the selected conversation has a
non-color-only `aria-selected` state plus visible active treatment immediately
on press.

The history worker reuses its immutable metadata graph for catalog and page
queries and projects only the requested bounded page while computing the full
loss equation with a lightweight pass. On the owner-local 131-agent,
29,262-item trace this reduced cold selected-page projection from about
1,406 ms to 182 ms; the built-Electron warm journey measured 96–97 ms.

Variable-height trace rows are not forced through fixed-height virtualization.
Each page is capped at 50 rows, timeline labels are compact 360-character
previews, and the inspector retains the complete bounded item. The shared DOM
SplitPane lowering establishes a real flex viewport for every pane; the
Electron gate requires the center timeline to have at least a 100px client
viewport, overflow content, and a successful `scrollTop` change. On the
owner-local 131-agent trace the measured viewport is 546px over 3,061px of
content, and committed selected-page readiness is 89–91ms.

The sidebar initially constructs and exposes only the 40 newest top-level
conversations. A final in-list `Load 40 more` action is the only way to reveal
each older batch; restarting resets the disclosure window and does not restore
an ancient selection outside it. On the owner catalog (1,231 roots), the exact
state-to-view benchmark improved from 127.358ms median / 136.622ms p95 to
4.975ms median / 10.766ms p95—a 25.6× median reduction.

Keyboard conversation traversal uses `Command+ArrowUp/ArrowDown` on macOS and
`Control+ArrowUp/ArrowDown` elsewhere. It dispatches the same typed selection
intent as a row press, loads the target page, and reveals the next 40-row batch
when traversal crosses the current disclosure boundary. `Command+1…9` (or
`Control+1…9`) jumps directly to one of the first nine conversations; while the
modifier is held, those rows replace timestamps with their numeric hints.
Editable controls keep their native keys. The built-Electron journey proves
numeric jump, one down/up round trip, and a 45-key held traversal. Held repeats
update typed pending selection immediately, preserve the NavRail scroll offset,
and debounce page projection to the final target after 110ms; stale responses
cannot commit and the selected row is centered inside the viewport.

Pressing or releasing the platform modifier by itself may update sidebar hints,
but it must not move the center timeline. In the bottom-anchored flow the reader
scrolls a keyed Stack that mixes prose rows and Timeline segments, so Effect
Native preserves the horizontal and vertical offsets of every keyed scroll
container — not only the Timeline `<ol>` — across a root commit, restoring them
after the whole tree (including rebuilt SplitPane pane wrappers, which detach and
reattach the retained scroll node) is committed. The built-Electron journey opens
the transcript at its end, parks it mid-transcript, and checks both modifier
transitions after scrolling.

The trace workspace has no duplicate application/header bar, selected title,
state label, or accounting banner. Selection lives in the sidebar. It is one
Effect Native `NavRail`: its uniform icon-action group and compact conversation
group carry controlled selection, accessible labels, trailing timestamps, and
item-local typed intents. Commands and Settings retain the same intent and
keyboard command paths without app-local row components.

The center pane is an Effect Native `Timeline` with stable event keys,
controlled selection, typed message/reasoning/tool/error variants, generated
catalog icons, compact detail/status metadata, and typed selection. The
right pane uses tree-mode `NavRail` for agent depth, expansion, roving keyboard
bindings, and trailing lifecycle state. Lifecycle is rendered as a generated,
color-coded catalog icon rather than a repeated status word; the full state
remains in each row's accessibility label. Selected-item fields use the typed
`Table` contract. Neither pane owns app-local list-row primitives.

A parent timeline projects each confirmed child-start edge as an inline
subagent card. The card carries the exact child thread ref, current lifecycle
state, and one bounded, redacted latest-activity preview read from the child's
own history tail. Pressing it dispatches `HistoryAgentSelected`, the same typed
navigation used by the full right-hand tree. It never merges the child
transcript into the parent, infers an edge from prose, treats a preview as a
completion receipt, or duplicates later interaction notifications as new
launch cards. Missing child history remains explicit in the source item.

Codex's structured AGENTS instruction injection is classified as `metadata`,
not as a user chat turn. It renders as one faint `Agent metadata · Click to
expand` disclosure; the bounded content appears only while selected, a second
press collapses it, and the right-hand agent tree remains in place throughout.
Ordinary user messages never enter this presentation path.

Inter-agent communication is projected from the typed `agent_message`, not the
adjacent trigger marker. The raw `inter_agent_communication_metadata` record is
retained in the page ledger but hidden from chat; the following message becomes
one agent card with a humanized action/task, sender-to-recipient route, bounded
payload, and parsed inspector fields. The structured recommended-plugins
injection is likewise retained as hidden system metadata and never presented as
a user turn.

### Large-thread first-content performance

After a thread is selected, the local bounded first-content projection must
finish in **less than 50 milliseconds**, regardless of total rollout size.

- Large rollouts are read from a bounded tail window; the selection path must
  not parse the complete rollout.
- Filesystem and parsing work stays off Electron's main process.
- The enforced oracle creates a sparse **256 MiB** rollout and fails at 50 ms.

This is specifically a local projection budget. It does not claim that every
machine will paint a complete window within 50 ms, and it does not permit
unbounded history hydration.

Contract:
`openagents_desktop.chat.thread_first_content_under_50ms.v1`.

### Startup: window first, branded boot frame, no blank screen

The application never shows a blank (or off-palette) frame at startup
(2026-07-13 incident; owner statement recorded verbatim in
`src/contracts/ux-contracts.ts`).

- Electron main creates the window BEFORE any local database open,
  OS-keychain custody, or session network verification on the production
  `whenReady` path. The network session settle after the window is
  fire-and-forget — never awaited.
- The renderer paints a static branded boot frame (khala background, psi-bar
  shimmer; every color an exact `@effect-native/tokens` khalaTheme value)
  with the first HTML parse, before the bundle even evaluates.
- The shell mounts interactable — composer focusable, sidebar present —
  BEFORE the local coding-history scan. Hydration streams in afterwards
  behind an explicit `Scanning coding history…` row; the
  `No local Codex history found.` claim renders only after the scan settles.
- Fixture-mode bench budgets (medians): `windowReadyToShow` < 1500 ms and
  `shellMounted` < 2500 ms, enforced by `scripts/startup-bench.ts` receipts.
  Real-wiring numbers come from `OPENAGENTS_DESKTOP_STARTUP_TRACE`
  (measured on a real profile: shell mount went from 5.4–7.0 s to ~0.7 s;
  see `docs/fable/2026-07-13-desktop-startup-incident.md`).

This governs boot ordering and honest loading presentation. It does not
change the post-selection 50 ms projection budget above, and it does not
bound full history hydration time on arbitrary `~/.codex` sizes (bounding the
scan itself is follow-up, now off the critical path).

Contract:
`openagents_desktop.startup.window_first_no_blank_frame.v1`.

### Transient-visibility affordances are pointer/focus-driven and commit-idempotent

A hover/focus-reveal affordance (the per-message **details** toggle, and any
opacity-0-at-rest reveal) is visible strictly as a function of pointer/focus —
never of composer input, global state, or re-render timing.

- The resting `opacity: 0` is keyed on the affordance itself and does **not**
  depend on any ancestor, so a momentary DOM detach during reconciliation
  cannot expose it. Only the reveal to `opacity: 1` depends on the row's
  `:hover` / `:focus-within`.
- The shared React-backed Effect Native DOM renderer commit is **idempotent**:
  re-committing an unchanged keyed subtree performs zero DOM moves of
  persisted content. React owns reconciliation, while Effect Native keys and
  catalog attributes preserve transcript message wrappers in place. Detaching
  and re-attaching a node restarts its CSS
  transition/animation, so an unrelated re-render (a keystroke elsewhere) can no
  longer replay a hover-reveal transition, a running-tool title shimmer, or a
  disabled-reason popover animation.
- The Electron smoke types into the composer while not hovering a message row
  and fails if the details affordance's computed opacity ever rises above 0, or
  if its DOM node is replaced or re-parented. A provider-agnostic render-dom
  unit guard proves the persisted affordance is never moved when only a sibling
  changes.

Contract:
`openagents_desktop.chat.details_affordance_visibility_is_pointer_only.v1`.

### Hidden-ref turn checkpoints for the coding workbench

The host captures workspace state at coding-turn boundaries (turn start and
turn completion, on both local lanes) as hidden Git refs under
`refs/openagents/checkpoints/<thread>/<turn>`.

- Every snapshot is built through an isolated temporary `GIT_INDEX_FILE`:
  capture never writes user branches, HEAD, the user index, stashes, or the
  worktree, and the refs live outside `refs/heads`/`refs/tags`, so no branch
  UI, `git status`, or default push ever sees them.
- Capture is bounded to tracked plus non-ignored untracked files, with a
  per-file size exclusion and a total-file refusal bound. Each capture emits a
  typed local completion signal.
- A typed diff query reports turn-over-turn file changes and a bounded patch
  between any two of a thread's checkpoints.
- Revert is an explicit staged command: stage (validated, non-mutating),
  inspect (the plan, the patch, and a mandatory irreversible-effects
  statement), then commit or clear. Staging refuses dirty conflicting state —
  uncheckpointed worktree edits in any path the revert would rewrite — and
  commit re-refuses on post-stage drift. A committed revert restores the
  checkpoint's exact bytes (text and binary), deletes later-turn artifacts,
  and still never touches user branches, HEAD, or the user index. A pre-revert
  baseline snapshot is retained as redo material.
- Checkpoint snapshots can contain secrets: refs stay in the local repository
  only and never enter Sync projections, renderer state, or push surfaces.
  Deleting a thread's checkpoints removes every hidden ref under that thread's
  namespace and only that thread's.
- This substrate authorizes no visible renderer affordance under the MVP
  surface allowlist; it is host-side capability behind the owner-local
  executor invariant.

Contract:
`openagents_desktop.workbench.turn_checkpoints.v1`.

### One-click harness maintenance with re-probe, pinning, and provenance receipts

Settings shows per-harness version/channel truth for the local coding
harnesses (Codex CLI, Claude Code, OpenCode) and a one-click update that
drives a typed Runtime Gateway command into the shared
`@openagentsinc/pylon-core` maintenance engine (`pylon accounts maintenance`
is the CLI parity surface over the same engine).

- Detection classifies the installed binary's channel (npm/bun/pnpm global,
  Homebrew, or the harness's native installer) and updates only through that
  channel's own path; an unclassifiable install refuses one-click updates
  instead of guessing.
- Before an update runs, a pin records the expected version, binary sha256,
  and channel. An explicit different channel is refused as a channel jump —
  never silently switched.
- Success is only reported after the swapped binary RE-PROBES: it must answer
  a version probe on the same channel with a moved version. A failed re-probe,
  a channel change, or an unchanged version is a typed maintenance failure
  with the previous state recorded intact.
- Every run persists an append-only provenance receipt (source, command,
  bounded output excerpt, before/after states, re-probe result) under the
  shared Pylon home; the renderer projection carries versions, channel, and
  advisory only — never paths, tokens, or raw command output.
- Maintenance updates binaries, never auth state: probe and update spawns
  scrub `CODEX_HOME`/`CLAUDE_CONFIG_DIR`/`GROK_HOME`, login/auth-flow
  arguments are refused by a typed guard, and the default `~/.codex` login
  home is never read or written.
- One update per harness runs at a time, and Electron main wires the actions
  post-window — nothing is added to the pre-window startup path.

Contract:
`openagents_desktop.settings.harness_maintenance_one_click.v1`.

### Composer focused on open

On window open — fresh launch and macOS re-activate with an existing window —
keyboard focus lands in the message composer at shell-interactable, so the
first keystroke types into it with zero clicks.

- Background history hydration never steals open-time focus.
- Automatic settle passes claim only unowned focus (body/root); focus the
  user placed elsewhere is never moved.

Contract:
`openagents_desktop.composer.focused_on_open.v1`.

### Sidebar session search actually filters

Typing in the sidebar session search filters over the FULL loss-accounted
catalog store — every root, including beyond the current sidebar page — with
case-insensitive substring matching on titles and workspace labels.

- Instant title matches come from the hydrated catalog; the byte-bounded host
  content index merges in when it settles.
- While the host index is in flight the empty state says "Searching…";
  "No sessions match." renders only once settled; clearing restores the list.
- The index remains a rebuildable cache, never catalog/page authority.

Contract:
`openagents_desktop.history.session_search_filters.v1`.

### Truthful sidebar history header

The coding-history header states the projection's real scope: "scanning…"
before hydration settles, a counted "N of M" disclosure with explicit "Load K
more" paging while the catalog is paged, and "all N" only when every
catalogued session is shown. A label never claims more than the projection
delivers.

- Catalog title scans, page reads, and search-index content reads are
  byte-bounded/streaming: an oversized (multi-GB) rollout degrades to a
  fallback title instead of silently collapsing the catalog to the recent
  list.

Contract:
`openagents_desktop.history.sidebar_header_truthful_scope.v1`.

### The MVP visible surface is mechanically enforced against the rendered shell

Owner statement (rc.10 review, 2026-07-14, verbatim): "This menu, when I click
the settings button, looks horrible. This folder thing looks horrible. I
thought we made a pass removing all screens that are not specifically called
for in the MVP. You need to clean all this up and make a pass to remove
everything from the sidebar and all UI that's not specifically called for in
our MVP spec."

- The sidebar dock renders EXACTLY the cited allowlist, in order: New chat,
  Chat, ProductSpec, AssuranceSpec, Project home, Settings. Each entry carries
  its ProductSpec/owner-directive citation in
  `src/renderer/mvp-visible-surfaces.ts`.
- Files and the command palette have no dock icon. They stay reachable only
  through their closed CW-AC-12 command identities (⌘K palette, native
  Commands menu, deep link) because the spec places file/Git review "beside
  the conversation" and calls for no dock affordances.
- The review workspace is the CW-AC-14 READ-ONLY boundary: branch/status
  truth, per-file status, exact diff review, and composer attachment. No
  commit, push, stage/unstage, discard, branch switch/create, or issue/PR
  authoring control renders. The Files browser renders no file
  create/rename/delete/reveal control. The typed substrate intents remain
  host-side capability and authorize no visible affordance.
- Enforcement is against the ACTUAL rendered view tree, not a static list:
  `desktopMvpSurfaceViolations` walks `desktopShellView` output for every
  reachable workspace state, fails on any non-allowlisted dock item, any
  silent allowlist loss, and any forbidden surface key — and its falsifier
  tests prove a planted non-MVP surface is rejected. The built-Electron smoke
  asserts the same exact dock composition and mutation-affordance absence.

Contracts:
`openagents_desktop.mvp.visible_surface_allowlist.v1`,
`openagents_desktop.mvp.visible_surface_sweep.v1`
(oracles: `src/renderer/mvp-visible-surfaces.test.ts`, shell/settings/command
suites, built-host smoke).

## Desktop safety boundary

The normal desktop test sweep also mechanically enforces these host boundaries:

- Electron renderer sandboxing is enabled with context isolation on, Node
  integration off, webviews off, and web security on.
- Effect Native remains the only application/component/state/intent grammar.
  React 19, Vite, Tailwind, and any later Base UI adapters are renderer implementation details;
  portable shell/projection modules cannot import them or carry JSX,
  `ReactNode`, callbacks, or Tailwind class strings. Zustand, TanStack Router,
  Effect Atom React, and a second schema/theme/icon authority remain absent.
- Permission requests, navigation, new windows, and webview attachment are
  denied by default.
- The renderer receives fixed, validated capabilities through the preload
  bridge—never raw IPC, Node APIs, arbitrary commands, tokens, or a
  `MessagePort`.
- The renderer Content Security Policy permits no remote script or connection
  surface.
- Local Codex history scanning runs in a persistent worker rather than on the
  Electron main thread.
- The service-topology oracle reads the checked-in implementation modules and
  binds every declared service to real construction symbols plus the module
  that composes them. A renamed/removed/uncomposed factory, undeclared source
  authority, wider installation scope, or missing module fails the normal
  package sweep instead of trusting manifest prose.
- Source-derived renderer filesystem, process, network, or secret authority is
  forbidden. Ambient cwd/`AsyncLocalStorage` selection and unnamed
  `Effect.runPromise`/`ManagedRuntime` exits are also rejected.
- Cache, freshness, installation scope, and disposal declarations live on the
  same typed entry as the checked construction evidence. The selected
  workspace is an explicit WorkContext service, and the process-owned Codex
  history worker has one host with an app-shutdown disposal path.
- The production main process uses one replaceable lifecycle owner for runtime,
  WorkContext, authenticated Sync, account-connect, history, and per-window
  gateway subscriptions. Replacement closes the previous narrower slot before
  publishing the next; app shutdown closes windows/gateway before dependencies
  and is idempotent. Account children/timers, history workers, gateway
  listeners, and an in-flight native sign-in have terminal teardown paths.
- Runtime operations may carry only bounded `operationRef`, Desktop lifecycle
  `sessionRef`, `correlationRef`, and optional `runRef`. Main rejects a context
  that does not match the decoded command/run, the gateway echoes the same
  context, and real runtime commands append those refs to private Sync
  causality. Paths, URLs, bodies, owner data, credentials, provider payloads,
  native handles, and raw errors are not fields in this context or its journal.
- The built smoke drives that correlation through the real renderer, preload,
  IPC, Runtime Gateway, and a clearly identified substitute Sync command, then
  explicitly disposes the host and requires an aggregate `active: 0` receipt
  before exit.

The mechanical oracle is
[`tests/electron-boundary.test.ts`](./tests/electron-boundary.test.ts).
The source-coupled service oracle is
[`tests/service-topology.test.ts`](./tests/service-topology.test.ts).
The replacement/disposal and correlation oracles are
[`src/desktop-host-lifecycle.test.ts`](./src/desktop-host-lifecycle.test.ts) and
[`src/desktop-operation-context.test.ts`](./src/desktop-operation-context.test.ts).

## Verify the guarantees

From the repository root:

```sh
pnpm install
pnpm test apps/openagents-desktop
pnpm --dir apps/openagents-desktop run typecheck
OPENAGENTS_DESKTOP_SMOKE=1 pnpm --dir apps/openagents-desktop run smoke
```

The focused Codex-history oracle is:

```sh
pnpm test apps/openagents-desktop/tests/codex-history.e2e.test.ts
```

The Runtime Gateway seam oracle is:

```sh
pnpm test apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts
```

The host persistence oracle is:

```sh
pnpm test apps/openagents-desktop/tests/desktop-sync-host.test.ts
```

The native-session custody oracle is:

```sh
pnpm test apps/openagents-desktop/tests/desktop-session-vault.test.ts
```

The recovered-session oracle is:

```sh
pnpm test apps/openagents-desktop/tests/desktop-session-recovery.test.ts
```

The loopback PKCE entry/exit oracle is:

```sh
pnpm test apps/openagents-desktop/tests/desktop-session-pkce.test.ts
```

The turn-checkpoint oracle is:

```sh
pnpm test apps/openagents-desktop/tests/turn-checkpoints.test.ts
```

## Release artifact and update-safety guarantees (#8786)

Two live 2026-07-13 incidents define the failure classes these guarantees
refuse, and both are cited as the standing evidence base:

- **T3 Code shipped a fully-notarized app inside an UNSIGNED, un-notarized
  DMG.** macOS Gatekeeper assesses the outermost quarantined artifact, so the
  download died on arrival with the "damaged" dialog and the correct app
  inside was unreachable (`docs/teardowns/2026-07-13-t3-code-teardown.md`,
  night addendum: installed-artifact verification).
- **ChatGPT's updater swapped a working app for one the machine refused to
  exec and never noticed**, leaving a dead icon
  (`docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md`,
  lessons 1–3).

### The outermost artifact is what ships: notarize + staple the DMG itself

`make:mac` notarizes the DMG itself (Apple's ticket covers the nested app)
and staples the ticket to BOTH the `.app` and the `.dmg`, then fails the make
closed unless every Gatekeeper oracle is green
(`forge.config.ts` `postMake`, `scripts/macos-gatekeeper.ts`):

- `codesign --verify --deep --strict` on the app,
- `spctl -a -t open --context context:primary-signature` on the image,
- `spctl -a -t exec` on the app,
- `xcrun stapler validate` on both.

`spctl` acceptance must be as **Notarized Developer ID** — a
signed-but-unnotarized artifact is a red row, not a warning.

### No unsigned release fallback (fail closed)

When the Developer ID identity or notary credentials are absent, the make
and preflight lanes REFUSE — a build without a signing ceremony can no
longer silently produce a release-shaped artifact. The only escape valve is
explicit (`OA_ALLOW_UNSIGNED_DEV=1` for make, `--allow-unsigned-dev` for
preflight) and renames the output `-UNSIGNED-DEV`; release preflight and
`publish-release.ts` refuse that marker unconditionally, so a dev artifact
can never be mistaken for — or published as — a release.

Every oracle is a pure interpreter over recorded command output, so the full
set (including the refusal paths) is unit-tested against fixtures WITHOUT
owner signing credentials; owner-key ceremonies stay owner-gated. Oracles:
[`tests/macos-gatekeeper.test.ts`](./tests/macos-gatekeeper.test.ts).

### Applying an update is not success — the first demonstrated launch is

After an update applies, the machine holds in `awaiting_launch_receipt` with
the previous release still staged. The freshly launched build writes a typed
first-launch receipt (`openagents.desktop.launch_receipt.v1`); only a
schema-valid receipt for EXACTLY the applied version confirms the update. No
receipt within the bounded window (10 minutes) → automatic rollback to the
retained previous version with a typed diagnostic
(`launch_receipt_missing`). A late receipt never resurrects a rolled-back
update, and the wait survives a crash/relaunch (the window is re-evaluated
against the wall clock by the clock-free `evaluateLaunchReceipt`).

Contracts: `src/update-contract.ts` (receipt schema + evaluation),
`src/update-rollback.ts` (state machine). Oracles:
[`tests/launch-receipt.test.ts`](./tests/launch-receipt.test.ts) and
[`tests/update-rollback.test.ts`](./tests/update-rollback.test.ts).

## Not guaranteed yet

This document does not promise automatic update delivery (the live feed
host wiring), server-authoritative FleetRun creation, full-history eager
rendering, or remote/cloud Codex-history sync. Those remain outside the
current enforced Desktop contract. Release packaging and signing/notarization
are now covered by the fail-closed release-lane guarantees above; the
clean-machine install/update/rollback acceptance ceremony itself stays
owner-gated.

When behavior changes, update the typed contract, its oracle, and this document
in the same change. Do not expand this page from aspiration alone.
