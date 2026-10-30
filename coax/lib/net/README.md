# Bounded absolute DNS URIs

`uri.coax` is a shared syntax policy for trusted configuration and bounded
protocol locators. It returns ranges into the original string; it never
normalizes or decodes the input. Load it and call `uri_parse(text)`.

A successful `UriParts` has `ok=true`, `scheme` (`URI_HTTPS=1`, `URI_HTTP=2`,
`URI_WSS=3`), a host range excluding the port, the effective numeric port,
a path range, and explicit query presence plus the query start. Every field
is zero on refusal. A missing path is empty. Consumers select their permitted
schemes, ports, query rules and locator semantics separately.

The supported subset is exact lowercase absolute schemes, ASCII DNS hosts
up to 253 bytes with labels of 1–63 bytes, and an alphabetic first byte in
the final label. Single-label DNS names are allowed. Optional explicit ports
are decimal 1–65535 without a leading zero. Total input is at most 2048 bytes.
Credentials, fragments, trailing host dots, IP literals, alternate numeric IP
spellings, controls, raw non-ASCII, backslashes and malformed escapes refuse.
Percent escapes are preserved; encoded C0 controls, DEL and backslash refuse.
An opaque escaped high byte such as `%FF` is permitted without a UTF-8 claim.
The first question mark separates the query; subsequent ones are query data.

This deliberately supports less than general URI syntax. It does not resolve
DNS, validate TLS, prevent SSRF or authorize identity. A DNS name can resolve
to a private address. A network consumer must check every resolved address,
pin the selected peer and enforce its transport contract. A public locator
allowlist remains a separate policy.

## Evidence

Coax 0.16.0: 8 checks pass and 227/227 mutants are killed, with no exemptions.
`python3 coax/tests/net/uri.py` passes three focused tests: Python's independent
`urlsplit` component oracle, checked/release native results and unchanged input,
the actual parser under ASan/UBSan, and checked/release Wasm with denied host
effects. The finite cases cover exact limits, invalid bytes and zero refusal
metadata; they are not a general URI-parser proof. The retained source and
artifact hashes are in `coax/tests/net/uri-evidence-2026-10-28.json`.

On this Mac the optional Wasm test uses Homebrew LLVM, wasi-libc and
wasi-runtimes; add `/opt/homebrew/opt/lld/bin` to PATH. Python and Bun are
verification tools only. The shipped policy and ordinary builds need neither.

# Relay connections

`relay_conn.coax` is the reusable client connection to a Nostr relay over
`ws://` or `wss://`: `relay_conn_open(c, uri)`, `relay_conn_send(c, text)`,
`relay_conn_poll(c, timeout_ms)` (`RC_TEXT` with the message in
`c[0].text[0..<c[0].text_n]`, or `RC_NONE`, `RC_CLOSED`, `RC_FAIL`),
`relay_conn_ping`, `relay_conn_close(c, code)` and `relay_conn_drop(c)`. It
does the I/O only, through `socket-native.c`, `tls-native.c` and the entropy
bridge. `relay_uri.coax` picks the transport, `ws.coax` frames and checks the
handshake, and `relay_conn_rules.coax` (`#authority`) decides what each server
frame asks for: deliver text, answer a ping, echo a close, or refuse (binary
data with 1003, violations with ws.coax's close codes, frames or messages over
128 KiB with 1009). Reconnecting, backoff and resubscription belong to the
caller, through `coax/lib/nostr/relay.coax`. `coax/tests/net/relay_e2e.py`
runs the whole client against the scripted fixture over both transports.
Evidence: [docs/verification/2026-10-30-relay-conn.md](../../../docs/verification/2026-10-30-relay-conn.md).

Apple TLS now uses Network.framework and Security through the same
`relay_conn` interface. Source `coax/platform/tls-env.sh` for native flags:
Apple links platform frameworks with `-fblocks`; Linux links OpenSSL 3.
`cxtls_connect` owns TCP and TLS under one deadline, verifies the chain and
DNS name, and returns an opaque TLS handle. The old descriptor upgrade
`cxtls_start` remains Linux-only. No plaintext fallback occurs for `wss://`.

Apple's receive completion does not reveal whether TLS close_notify arrived.
The native effect reports `CXTLS_END_UNVERIFIED` (-12), which ordinary polling
treats as failure. A complete authenticated WebSocket close frame is required
before the closing handshake can accept that stream end. `last_read` keeps
the native result for reconnect diagnostics. A timed-out read retains its
pending receive, and cancelled callbacks cannot affect a reused slot.
The [Apple TLS record](../../../docs/verification/2026-10-30-apple-tls.md)
retains macOS sanitizer, Linux regression, certificate-refusal and read-only
public-relay evidence. iOS compilation is checked; iOS runtime is unqualified.
