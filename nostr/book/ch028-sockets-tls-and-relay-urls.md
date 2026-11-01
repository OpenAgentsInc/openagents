# 28. Sockets, TLS and relay URLs

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

AGENTS.md lets the C host link one TLS library so native clients reach
`wss://` relays. This chapter draws the line: the host does the handshake,
certificate checks and record encryption; Coax does everything else.

Part VII, Transport. Result: The host boundary: the C host opens sockets and
does TLS only; Coax parses `ws://` and `wss://` URLs, decides every frame and
owns every protocol decision above the socket.

## What this chapter teaches

- `uri.coax`: bounded absolute DNS URIs, refusing credentials, fragments and
  IP literals.
- `relay_uri.coax`: choosing `ws://` or `wss://` and the port.
- `relay_conn.coax`: open, send, poll, ping, close and drop, with `RC_TEXT`,
  `RC_NONE`, `RC_CLOSED` and `RC_FAIL`.
- `relay_conn_rules.coax` (`#authority`) decides what each server frame asks
  for.
- Apple TLS through Network.framework, Linux through OpenSSL 3; no plaintext
  fallback for `wss://`.

## What exists today

- The files listed above, plus `coax/platform/socket-native.c`.
- Records:
  [2026-10-30-relay-conn.md](../../docs/verification/2026-10-30-relay-conn.md),
  [2026-10-30-apple-tls.md](../../docs/verification/2026-10-30-apple-tls.md),
  [2026-10-29-nostr-relay-tls.md](../../docs/verification/2026-10-29-nostr-relay-tls.md).

## Dependencies

- [26. WebSocket frames](ch026-websocket-frames.md)
- [Coax chapter 20, Living beside C, C++ and
  Swift](../../coax/book/ch20-interop.md)

## Done means

- `coax/tests/net/tls.sh` passes against local TLS endpoints under ASan and
  UBSan.
- A certificate for the wrong name refuses the connection.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
