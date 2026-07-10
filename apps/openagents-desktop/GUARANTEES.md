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
  completed.
- Lifecycle events have a monotonic sequence and an owned disposer.
- Electron main validates the top-level bundled renderer before serving a
  request.
- Tokens, credentials, URLs, raw runtime events, arbitrary IPC, `MessagePort`,
  filesystem/process handles, and command arguments cannot enter the contract.

This first slice does not claim OpenAgents sign-in, Khala Sync, or provider
streaming; those capabilities remain explicitly unavailable.

Contract:
`openagents_desktop.seam.runtime_gateway_closed_protocol.v1`.

### Host-owned Khala Sync persistence

Electron main opens the existing shared Khala Sync SQLite store in an
owner-private directory under Desktop `userData`.

- One installation identity is generated once and reused after restart.
- The shared store schema and semantics remain the only cache/offline-queue
  implementation; Desktop does not create a parallel Sync database.
- The store closes deterministically on quit.
- The renderer receives only bounded readiness—never the database path/handle,
  installation refs, rows, pending mutations, or credentials.
- Network Sync remains unavailable until OpenAgents sign-in is implemented.

Contract: `openagents_desktop.sync.host_owned_sqlite.v1`.

### Recent local Codex chats

When local Codex history is available, opening Desktop projects top-level Codex
chats updated during the last 24 hours into the sidebar, newest first.

- Known child, sub-agent, and side sessions are excluded.
- Sidebar loading is metadata-only; selecting a chat projects basic metadata
  and a bounded set of recent user and assistant messages.
- Missing or malformed local history produces an honest, usable empty state.
- History access is read-only. This projection does not grant authority to
  resume a Codex session, send a message, browse arbitrary files, sync history,
  or dispatch work.

Contract:
`openagents_desktop.seam.codex_recent_history_projection.v1`.

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

## Not guaranteed yet

This document does not promise release packaging, signing/notarization,
automatic updates, server-authoritative FleetRun creation, full-history eager
rendering, or remote/cloud Codex-history sync. Those remain outside the current
enforced Desktop contract.

When behavior changes, update the typed contract, its oracle, and this document
in the same change. Do not expand this page from aspiration alone.
