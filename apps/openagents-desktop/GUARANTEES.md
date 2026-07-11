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
- Lifecycle events have a monotonic sequence and an owned disposer.
- Electron main validates the top-level bundled renderer before serving a
  request.
- Tokens, credentials, URLs, raw runtime events, arbitrary IPC, `MessagePort`,
  filesystem/process handles, and command arguments cannot enter the contract.

Protocol v6 carries bounded OpenAgents entry/exit, canonical confirmed-
conversation operations, exact-ref runtime start/interrupt, and confirmed
bounded agent-timeline snapshots. Provider execution stays behind the host;
only canonical projected lifecycle items reach the renderer.

Contract:
`openagents_desktop.seam.runtime_gateway_closed_protocol.v1`.

### Host-owned Khala Sync persistence

Electron main opens the existing shared Khala Sync SQLite store in an
owner-private directory under Desktop `userData`.

- One installation identity is generated once and reused after restart.
- The shared store schema and semantics remain the only cache/offline-queue
  implementation; Desktop does not create a parallel Sync database.
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

This is the durable owner-message floor. It does not claim renderer wiring,
provider/runtime event streaming, assistant-role inference, physical-device
acceptance, or a deployed live-account receipt.

Contract: `openagents_desktop.sync.native_conversation_continuity.v1`.

### Closed Runtime Gateway conversation protocol

Protocol v6 carries schema-bounded `conversation.catalog`,
`conversation.thread`, and `conversation.timeline` queries plus
`conversation.create`, `conversation.append`, `conversation.start`, and
`conversation.interrupt` commands.

- Queries return confirmed public-safe refs/bodies/timestamps/entity versions,
  actual scope phase/cursor, and pending count.
- Commands enqueue canonical Sync mutations and return `pending_reconcile` or
  `unknown_pending_reconcile` with the durable mutation id. Enqueue is never
  reported as completed.
- Not-live and read failure are typed, body-free unavailable results.
- Owner identity, credentials, native/store/session/overlay/transport objects,
  raw events, provider authority, and generic IPC do not cross preload.

The renderer selects this authority once at boot when the confirmed catalog is
live; otherwise it retains explicit local-only mode. It does not mix catalogs.
The built-live and physical-mobile receipt remains separate acceptance.

Contract: `openagents_desktop.seam.runtime_gateway_conversation.v1`.

### Confirmed agent timeline protocol

Protocol v6 carries `agent.timeline` by bounded exact `runRef` and
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
Live-account/physical-device proof remains the final #8676 gate.

Contract: `openagents_desktop.seam.runtime_gateway_agent_timeline.v1`.

### Visible authoritative Sync conversation mode

### Local-first identity and optional account link

Desktop creates an immutable device-local identity before OpenAuth. Local
authority uses separate SQLite tables, `LocalRevision`, and a device-local
scope that hosted Sync rejects. Runtime Gateway v6 exposes only the tier, never
the identity/owner ref. A server-verified account link adds personal Sync;
disconnect, denial, failed link, and restart preserve the local identity and
local rows. The workbench, history, local Pylon, and local conversation path do
not require an OpenAgents account.

Contract: `openagents_desktop.seam.identity.local_first_account_link.v1`.

The existing Effect Native shell consumes Runtime Gateway v6 through a typed
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
but it must not move the center timeline. Effect Native preserves the keyed
Timeline's horizontal and vertical offsets while rebuilding its children, and
the built-Electron journey checks both modifier transitions after scrolling.

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

## Desktop safety boundary

The normal desktop test sweep also mechanically enforces these host boundaries:

- Electron renderer sandboxing is enabled with context isolation on, Node
  integration off, webviews off, and web security on.
- Permission requests, navigation, new windows, and webview attachment are
  denied by default.
- The renderer receives fixed, validated capabilities through the preload
  bridge—never raw IPC, Node APIs, arbitrary commands, tokens, or a
  `MessagePort`.
- The renderer Content Security Policy permits no remote script or connection
  surface.
- Local Codex history scanning runs in a persistent worker rather than on the
  Electron main thread.

The mechanical oracle is
[`tests/electron-boundary.test.ts`](./tests/electron-boundary.test.ts).

## Verify the guarantees

From the repository root:

```sh
bun install
bun test apps/openagents-desktop
bun run --cwd apps/openagents-desktop typecheck
OPENAGENTS_DESKTOP_SMOKE=1 bun run --cwd apps/openagents-desktop smoke
```

The focused Codex-history oracle is:

```sh
bun test apps/openagents-desktop/tests/codex-history.e2e.test.ts
```

The Runtime Gateway seam oracle is:

```sh
bun test apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts
```

The host persistence oracle is:

```sh
bun test apps/openagents-desktop/tests/desktop-sync-host.test.ts
```

The native-session custody oracle is:

```sh
bun test apps/openagents-desktop/tests/desktop-session-vault.test.ts
```

The recovered-session oracle is:

```sh
bun test apps/openagents-desktop/tests/desktop-session-recovery.test.ts
```

The loopback PKCE entry/exit oracle is:

```sh
bun test apps/openagents-desktop/tests/desktop-session-pkce.test.ts
```

## Not guaranteed yet

This document does not promise release packaging, signing/notarization,
automatic updates, server-authoritative FleetRun creation, full-history eager
rendering, or remote/cloud Codex-history sync. Those remain outside the current
enforced Desktop contract.

When behavior changes, update the typed contract, its oracle, and this document
in the same change. Do not expand this page from aspiration alone.
