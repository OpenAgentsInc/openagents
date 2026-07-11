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
- Unsupported conversation commands return `unavailable`, never accepted or
  completed; argument-free native-session commands return only bounded phase
  outcomes after the host action finishes.
- Lifecycle events have a monotonic sequence and an owned disposer.
- Electron main validates the top-level bundled renderer before serving a
  request.
- Tokens, credentials, URLs, raw runtime events, arbitrary IPC, `MessagePort`,
  filesystem/process handles, and command arguments cannot enter the contract.

The Gateway now carries bounded OpenAgents entry/exit, canonical confirmed-
conversation operations, and confirmed redacted agent-timeline snapshots.
Provider process streaming remains explicitly unavailable.

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
- Denial, sign-out, reconnect-before-live, and close remove the capability.
- The normal Desktop sweep proves Desktop start, mobile continuation, matching
  refs/versions/cursor, and restart reconstruction over the real native store
  adapters without duplicate objects.

This is the durable owner-message floor. It does not claim renderer wiring,
provider/runtime event streaming, assistant-role inference, physical-device
acceptance, or a deployed live-account receipt.

Contract: `openagents_desktop.sync.native_conversation_continuity.v1`.

### Closed Runtime Gateway conversation protocol

Protocol v3 carries schema-bounded `conversation.catalog` and
`conversation.thread` queries plus `conversation.create` and
`conversation.append` commands.

- Queries return confirmed public-safe refs/bodies/timestamps/entity versions,
  actual scope phase/cursor, and pending count.
- Commands enqueue canonical Sync mutations and return `pending_reconcile`
  with the durable mutation id. Enqueue is never reported as completed.
- Not-live and read failure are typed, body-free unavailable results.
- Owner identity, credentials, native/store/session/overlay/transport objects,
  raw events, provider authority, and generic IPC do not cross preload.

The renderer selects this authority once at boot when the confirmed catalog is
live; otherwise it retains explicit local-only mode. It does not mix catalogs.
This contract does not claim a provider-neutral stream or live GUI acceptance.

Contract: `openagents_desktop.seam.runtime_gateway_conversation.v1`.

### Confirmed agent timeline protocol

Protocol v3 also adds one `agent.timeline` query by bounded exact `runRef`.
Electron main composes the shared confirmed reader only while authenticated
personal Sync is live.

- A live result carries the exact confirmed run and its server-projected
  `routeRef`, lifecycle/version, scope phase/cursor/pending count, and at most
  500 ordered confirmed event facts.
- The server-projected `agent_run.routeId` is the only thread/route attachment
  fact. The renderer cannot derive a route from a run ID.
- Non-live, not-found, and read-failure outcomes are typed and body-free.
- Owner/objective/repository/runtime/backend, provider source, raw payload,
  external callbacks, credentials, store/session/transport, and process
  authority cannot cross the schema.

This is query substrate only. Runtime launch, visible timeline rendering,
canonical chat creation for the route, interrupt/resume, and live-account proof
remain later D1 work.

Contract: `openagents_desktop.seam.runtime_gateway_agent_timeline.v1`.

### Visible authoritative Sync conversation mode

### Local-first identity and optional account link

Desktop creates an immutable device-local identity before OpenAuth. Local
authority uses separate SQLite tables, `LocalRevision`, and a device-local
scope that hosted Sync rejects. Runtime Gateway v5 exposes only the tier, never
the identity/owner ref. A server-verified account link adds personal Sync;
disconnect, denial, failed link, and restart preserve the local identity and
local rows. The workbench, history, local Pylon, and local conversation path do
not require an OpenAgents account.

Contract: `openagents_desktop.seam.identity.local_first_account_link.v1`.

The existing Effect Native shell consumes Runtime Gateway v5 through a typed
renderer adapter whenever confirmed conversation Sync is live at boot.

- Sidebar and transcript map only confirmed thread/message projections.
- New-chat and composer actions generate stable refs, enqueue canonical
  mutations, and wait for those exact refs to be confirmed.
- A confirmation timeout renders an honest pending-reconciliation error; it is
  never converted to completion.
- Authority mode is selected once per renderer lifetime. Signed-out/not-live
  startup stays in the explicit local-only host and never merges its catalog
  with account-linked Sync later in that renderer lifetime.
- The adapter adds no preload method, IPC channel, owner/auth field, or second
  UI architecture.

Owner messages are currently rendered as user-role rows because canonical
`chat_message` has no assistant role. Provider/runtime assistant streaming and
live GUI/account acceptance remain later D1 evidence.

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
  context and explicit gap rows. Unknown/corrupt records are never dropped.
- The resizable inspector exposes agent topology or structured selected-item
  fields; pages are bounded to 500 and large lists use Effect Native windowing.
- Credential-shaped fields are visibly redacted, encrypted/raw reasoning is
  not projected, and the completeness equation is always visible.
- Missing or malformed local history produces an honest, usable empty state.
- History access is read-only. This projection does not grant authority to
  resume a Codex session, send a message, browse arbitrary files, sync history,
  or dispatch work.

Contract:
`openagents_desktop.seam.codex_loss_accounted_history.v2`.

### Real-Electron Codex trace acceptance

The normal Desktop verification sweep now opens the built Electron app against
the machine's real local Codex catalog and exercises the recorded-demo path.

- Top-level rows are named, newest-first, and contain no child sessions.
- A nested trace exposes its bounded agent topology, completeness equation,
  keyboard bindings, tool row, and structured inspector.
- Explicit conversation selection expands every branch; subsequent agent,
  item, paging, and inspector actions preserve the ref-only restoration record.
- A real renderer reload restores the selected trace/item and expanded set.
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
when traversal crosses the current disclosure boundary. Editable controls keep
their native arrow behavior. The built-Electron journey proves one down/up
round trip plus a 45-key held traversal. Repeat events coalesce into the latest
target behind one in-flight page load, stale responses cannot commit, and the
sidebar centers/retries the selected row until it is inside the viewport.

The trace workspace has no duplicate application/header bar. The selected
trace title and state live only in the workspace heading. The sidebar is one
Effect Native `NavRail`: its uniform icon-action group and compact conversation
group carry controlled selection, accessible labels, trailing timestamps, and
item-local typed intents. Commands and Settings retain the same intent and
keyboard command paths without app-local row components.

The center pane is an Effect Native `Timeline` with stable event keys,
controlled selection, compact detail/status metadata, and typed selection. The
right pane uses tree-mode `NavRail` for agent depth, expansion, roving keyboard
bindings, and trailing lifecycle state; selected-item fields use the typed
`Table` contract. Neither pane owns app-local list-row primitives.

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
