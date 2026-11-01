# 27. A WebSocket server

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A relay is a WebSocket server. This chapter builds the server half with the
limits a public relay needs.

Part VII, Transport. Result: The server side of RFC 6455: accept the upgrade,
refuse unmasked client frames, write unmasked frames, enforce frame and
message limits and close cleanly.

## What this chapter teaches

- Validating the upgrade request: method, `Upgrade`, `Connection`, version 13,
  key length.
- Refusing unmasked client frames with 1002.
- Reassembling fragmented messages under a byte limit.
- The closing handshake from the server side.

## What exists today

- `coax/lib/net/ws_server.coax`, used by the Coax relay in `coax/relay/`.

## Dependencies

- [26. WebSocket frames](ch026-websocket-frames.md)

## Done means

- Every malformed upgrade refuses with its HTTP status.
- A session from the Coax client passes under ASan and UBSan.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
