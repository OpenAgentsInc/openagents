# Coder Connect

Coder Connect gives an explicitly paired client read-only access to selected
retained Codex and Claude history on a desktop. Its Rust client and host use
NIP-42 authenticated relay connections and signed, encrypted private `3188`
artifacts. The local operator chooses the roots, recipient, relay, and grant
lifetime before any history can be disclosed.

This is the bounded [SESS history-observer profile](../../nips/openagents/NIP-SESS.md#read-only-observation-of-retained-foreign-history).
It is separate from [CTRL task control](../../docs/coder/runtime/nostr-task-control.md) and never
creates, resumes, steers, cancels, or approves an engine session. It does not
claim managed session history or a gap-free WS projection. The reader's
[supported files and limits](../coder-history/README.md) define which retained
sources it can project.

## Pair a client

The client creates its own secret key in its protected local store and shows
its public key. Keep the secret on the client. On the desktop, pair that public
key with explicitly selected roots:

```sh
cargo run -p coder-connect -- pair \
  --client <client-public-key> \
  --relay wss://relay.example/ \
  --codex-root "$HOME/.codex" \
  --claude-root "$HOME/.claude" \
  --expires-secs 86400 > connection.json
```

The angle-bracket public key and example relay are placeholders. Select a relay
that authenticates private-artifact publishers and enforces author/recipient
visibility. A successful synthetic fixture does not establish that behavior
on an arbitrary public relay.

Pairing creates a host key and a durable grant under
`~/.openagents/coder-connect/`, or the directory passed with `--state`. The
directory is private, the key and state files have mode `0600`, and separate
client and host keys are required. Commands take no secret-key argument and
never print a key.

`connection.json` is the public bootstrap object: the host and client public
keys, relay, grant ID, source-category labels, expiry, and the original encrypted
signed grant. It contains no local source path or transcript. Transfer it to
the client through a trusted pairing channel. The client must pin the host
from this transfer; a signature from an arbitrary replacement host cannot
establish trust by itself.

Each selected root authorizes disclosure of its supported retained chat files,
including their exact raw record bytes. The reader does not scan credentials
or unrelated files, but text previously recorded inside a chat is still chat
content. Omitting a root excludes it. No ambient root discovery widens a grant.

## Serve and revoke

Run the desktop observer while the client reads:

```sh
cargo run -p coder-connect -- serve
```

If the store has grants for several relays, select one with `--relay URL`.
Each host process serves one admitted relay. Its finite subscription reconnects
with bounded backoff; Ctrl+C stops serving without changing source files or
revoking the grant. Starting it again uses the same private state and key.

Revoke a grant using the ID from the connection code:

```sh
cargo run -p coder-connect -- revoke --grant <grant-id>
```

`--source <source-scope-id>` verifies that a selected source belongs to this
grant and revokes the whole grant. Removing only part of its disclosure scope
requires a new pairing. Revocation persists before the command returns and
blocks later reads and cached retries. It cannot erase content already received
by a client or recall an already admitted reply in flight.

Grant lifetime defaults to one day and cannot exceed 30 days. Renewal creates
a new grant rather than reviving an expired or revoked ID. Source-root
replacement also needs new admission; the host pins canonical path, device,
and inode and checks them before and after reading.

`public-key` prints only the existing host public key. `serve --once` is a
finite diagnostic mode. `--loopback-test` permits `ws` only to a numeric
loopback address for synthetic fixtures; normal operation requires `wss`.
URLs with credentials, query parameters, fragments, or control characters are
refused. The host does not follow client-provided artifact or source URLs.

## Reusable client API

Use `coder-connect` with `default-features = false` on a thin client. The host
reader and private desktop store are excluded; the crypto, transport, and
portable reader DTOs remain available.

```rust,ignore
let code = ConnectionCode::parse(&connection_bytes)?;
code.verify(&client_secret, now, RelayPolicy::Production)?;
let client = Client::new(code, client_secret)?;

let result = client.observe(Query::Catalog(CatalogRequest::default())).await?;
// Select an opaque source_id from the returned catalog, then request Page.
```

The client returns `Observation::Catalog(CatalogPage)` or
`Observation::Page(TranscriptPage)`. A transcript request carries its source
ID, prior reader cursor, and `max_bytes`; it never accepts a desktop path.
Retain the next cursor even at EOF to poll for appended bytes. `SourceChanged`
requires a catalog refresh and a new source view, not an append to old text.
`Conflict` on a catalog cursor means its membership changed; restart from the
first catalog page. Raw base64 bytes, offsets, stable record IDs, line boundaries,
and next-record cursors are checked before the client accepts a transcript page.

`prepare` returns the exact signed `Pending` packet; `send` can retry it while
fresh. An exact retry may return its original retained snapshot, not a new
capture. `verify_reply` validates a supplied signed response without a socket.
`Error.code` distinguishes revoked, expired, source-changed, unavailable,
rate-limited, transport, and malformed responses. Offline grant verification
checks identity and declared expiry; it cannot check current revocation.
Polling uses a new request identity for a new observation. A disconnected
client must label its content cached and distinguish receipt time from source
or host capture time.

The same `Client` reuses a serial connection for up to 24 successful exchanges
or 75 seconds, within a fixed 90-second transport lease and frame budget. Each
read has an eight-second network deadline. Cancellation or an unsuccessful
exchange drops the socket; the next read authenticates a new connection.
Callers can apply a shorter deadline without leaving a half-consumed socket
in the connection pool.

## Bounds and verification

Each request is valid for at most 60 seconds and never beyond its grant. A grant
admits at most 240 new reads per 60-second window; an exact retained retry does
not consume another read. Catalog pages contain at most 32 entries. Transcript
pages return at most 32 KiB of raw bytes, with at most 112 KiB of encoded reader
data inside the 128 KiB observer-body ceiling. The shared envelope and NIP-44
limits still apply. Exhaustion is explicit; source bytes are not silently
truncated and presented as complete.

The private store holds at most 64 retained grants and 256 current reply entries
per grant within a 64 MiB store ceiling. Local administration and reads use a
short exclusive store lock. The host saves exact reply evidence before sending
it. An uncertain save returns no reply; reopening uses the last atomically
retained state. Revocations survive process restart and are kept through the
grant's expiry and request window.

Focused checks cover synthetic catalog and transcript reads, original request
correlation, source replacement, late expiry, persistent rate limits,
revocation and exact retries, private store permissions, malformed source
bytes, authenticated encrypted relay exchange, reusable sockets, cancelled
exchange recovery, and subprocess pairing/revocation. The fixtures use temporary
synthetic histories and throwaway keys. The mobile integration fixture reads
more than two raw page bounds, compares every retained byte with its source,
appends a record, reconstructs the cache in a fresh App, and erases it after an
authenticated revocation. It does not exercise a physical device.

The [September 26 receipt and logs](fixtures/2026-09-26-observer/receipt.json)
retain the targeted tests and one explicitly requested synthetic check through
`wss://relay.openagents.com`. That check returned a catalog and a complete
two-record transcript. It first exposed a TLS provider-selection panic, fixed
by selecting a socket-local Rustls provider with WebPKI certificate roots.
The successful check establishes that bounded exchange, not long-term relay
retention, all relay access policies, or performance superiority. No real chat
history, model call, or benchmark was used.

```sh
cargo test -p coder-connect
cargo clippy -p coder-connect --all-targets -- -D warnings
cargo clippy -p coder-connect --no-default-features --lib -- -D warnings
```

The production smoke is ignored by default. Run it only when publication of
generated fixture text to the selected relay is authorized:

```sh
CODER_CONNECT_SYNTHETIC_RELAY=wss://relay.example/ \
  cargo test -p coder-connect production_relay_reads_only_generated_history \
  -- --ignored --nocapture
```
