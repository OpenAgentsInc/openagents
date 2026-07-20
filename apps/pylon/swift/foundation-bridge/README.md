# Apple Foundation Models Bridge

This helper exposes Apple's local Foundation Models runtime over the loopback
HTTP contract that Pylon and Autopilot use for the local Apple FM mode.

## Requirements

- Apple Silicon Mac.
- macOS 26 or newer.
- Xcode / Command Line Tools with Swift 6.2 or newer.
- Apple Intelligence and Foundation Models available for the logged-in user.

Unsupported hardware, disabled Apple Intelligence, and unavailable local model
assets are returned as typed non-ready health responses instead of crashes.

## Build

From the repository root:

```sh
bash apps/pylon/swift/foundation-bridge/build.sh
```

The script builds the SwiftPM target in release mode and installs a wrapper at
`apps/pylon/bin/foundation-bridge`.

## Run

```sh
apps/pylon/bin/foundation-bridge
```

The default port is `11435`. To run on a different port:

```sh
apps/pylon/bin/foundation-bridge 11436
```

## Contract

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`
- `POST /v1/sessions`
- `POST /v1/sessions/{id}/responses/stream`

The session endpoints are intentionally minimal for the first local Autopilot
MVP: session creation returns a local session id, and response streaming emits a
snapshot and completed SSE event for a bounded single-prompt turn. When the
controller projects the bounded read-only tools (`read_file`, `list_files`,
`code_search`) and the prompt asks to inspect the local workspace, the bridge
selects one tool plus a parsed argument and calls the controller's loopback tool
callback with the session token, then keeps the callback URL/token out of logs
and public evidence. The controller confines every tool to the workspace and
refuses escapes; native model-driven tool-argument generation is a future
enhancement.

## Frozen wire schema

The exact JSON this bridge accepts and emits on every endpoint (field names and
casing included) is frozen as a versioned Effect Schema contract at
`packages/runtime/src/backends/apple-fm/wire.ts`
(`APPLE_FM_BRIDGE_WIRE_VERSION = "openagents.apple_fm.bridge.wire.v0.2"`). It is
the single source of truth both this Swift helper and every TypeScript consumer
are proven against. Conformance is enforced by
`packages/runtime/src/backends/apple-fm/wire-conformance.test.ts`: captured wire
fixtures decode through the schema, a deliberate shape drift is rejected, and an
opt-in admitted-Mac sweep
(`OPENAGENTS_APPLE_FM_REAL_BRIDGE=1`) validates the live bridge. Bump both
`bridgeVersion` in `main.swift` and `APPLE_FM_BRIDGE_WIRE_VERSION` together when
the wire changes.

The helper logs startup and listener failures only. It does not log prompts,
message bodies, local files, secrets, or provider payloads by default.
