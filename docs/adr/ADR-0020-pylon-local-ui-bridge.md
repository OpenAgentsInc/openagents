# ADR-0020: Pylon Local UI Bridge (Pusher-Compatible)

## Status

Accepted

## Date

2026-01-15

## Context

We need a zero-config way for browser-based UIs (local or hosted) to discover a
user's local Pylon runtime and read capability status (e.g., Codex auth, rate
limits, identity). The UI must not require inbound connections to Pylon and
should work for production sites over HTTPS.

## Decision

We will ship a local, Pusher-compatible WebSocket bridge inside the `pylon`
daemon and have the browser connect directly to it over `wss://localhost`.

> Pylon will expose a local WS bridge on `127.0.0.1:8081` using the Pusher
> protocol (`/app/{app_key}`), and will emit `pylon.capabilities` on the
> `pylon.system` channel in response to `client-pylon.discover`.

We will also default `pylon` (no subcommand) to run `pylon start -f`, so users
can run a single command (`cargo pylon`) to bring the bridge online.

The bridge also forwards Codex app-server traffic on `pylon.codex` using
`client-codex.connect` + `client-codex.request` + `client-codex.respond`, and
emits `pylon.codex.event`/`pylon.codex.response`/`pylon.codex.status`.

### Schema / Spec Authority

- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) — Local UI bridge surface

## Scope

This ADR covers:
- The local Pylon WS bridge protocol and default port/key.
- The discovery request/response event contract.
- The Codex app-server request/response bridge (`pylon.codex`).
- The default `pylon` CLI behavior (no-arg start).

This ADR does NOT cover:
- Remote relay protocols (NIP-89/90).
- Multi-device syncing or server-side persistence of bridge events.

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Endpoint | Stable: `wss://127.0.0.1:8081/app/{app_key}` |
| App key | Stable default: `local-key` |
| Channels | Stable: `pylon.system`, `pylon.codex` |
| System events | Stable: `pylon.capabilities`, `pylon.system.pong` |
| Codex events | Stable: `pylon.codex.event`, `pylon.codex.response`, `pylon.codex.status` |
| Client requests | Stable: `client-pylon.discover`, `client-pylon.ping`, `client-codex.connect`, `client-codex.request`, `client-codex.respond`, `client-codex.disconnect` |

Backward compatibility expectations:
- The bridge payload is additive only; fields may be added but not removed.

## Consequences

**Positive:**
- One-command local discovery for hosted UIs.
- No inbound traffic from Pylon to the web app.
- Uses existing Pusher/Echo client tooling in the browser.

**Negative:**
- Requires TLS trust for `wss://localhost` in HTTPS contexts.
- Running `pylon` without args now starts the daemon (behavior change).

**Neutral:**
- The bridge is local-only and does not affect Nostr relay protocols.

## Alternatives Considered

1. **Local HTTP JSON endpoint** — Rejected due to HTTPS mixed-content issues.
2. **Browser extension or native app** — Rejected for higher install friction.
3. **Reverse tunnel to cloud** — Rejected due to privacy and operational cost.

## References

- `crates/pylon/` — local Pylon bridge/runtime implementation
- `apps/autopilot-desktop/src/main.rs` — desktop integration surface
- `docs/protocol/PROTOCOL_SURFACE.md` — protocol-level contract context
