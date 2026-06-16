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
snapshot and completed SSE event for a bounded single-prompt turn. When Pylon
projects a `read_file` tool and the prompt asks for local file inspection, the
bridge calls Pylon's loopback tool callback with the session token and then
keeps the callback URL/token out of logs and public evidence.

The helper logs startup and listener failures only. It does not log prompts,
message bodies, local files, secrets, or provider payloads by default.
