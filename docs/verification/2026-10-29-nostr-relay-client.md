# Native Nostr relay client: WebSocket, socket effect, relay state machine

Board task #7de9 `nostr-relay-state-machine` (claimed by claude-relay), part
of the native Pylon prerequisite for Psionic chapter 19. Recorded
2026-10-29. This is the first partial push. The end-to-end relay test is not
done yet (see "Missing").

## What landed

| File | What | Checks | Mutation |
| --- | --- | --- | --- |
| `coax/lib/crypto/sha1.coax` | Bounded one-shot SHA-1 (at most 4096 bytes) for Sec-WebSocket-Accept only. Never use it for integrity or signatures | RFC 3174 §7.3 vectors 1, 2 and 4, the empty string, padding boundaries 55/56/63/64/65/119 against `python3 hashlib`, and the RFC 6455 sample digest | 199/199 killed |
| `coax/lib/net/ws.coax` (`#authority`) | RFC 6455 client: key and accept values, the opening request (refuses header injection), response admission (status 101, Upgrade, Connection token, exactly one matching accept, refuses extensions and subprotocols we didn't ask for, 8192-byte head limit), masked client frames with minimal lengths, server frame reading (RSV, opcodes, masked-server refusal, control frames that are fragmented or over 125 bytes, non-minimal or MSB-set lengths, a per-frame size limit), fragmented message assembly with interleaved control frames, strict UTF-8 for text, close codes and payloads, and refusal-to-close-code mapping | 23 checks (including sha1's) | 1024/1024 killed |
| `coax/platform/socket-native.{h,c}` | The host effect: getaddrinfo, a nonblocking connect bounded by a timeout, buffered write and flush, and a read with a timeout, a byte reader and close, plus a monotonic clock. #foreign scalars in the process-native style. SIGPIPE is suppressed | Compiles clean with `gcc -std=c11 -Wall -Wextra`. Not yet run (see "Missing") | n/a (C) |
| `coax/lib/nostr/relay.coax` (`#authority`) | The relay client machine. See below | 13 scripted-transcript checks | 555/555 killed |

The mutation runs used `coax mutate FILE --jobs 3 --shard I/3`, merged with
`--merge`, on the final file contents, with no `// mutant: equivalent` lines.
Where a boundary was observably equivalent (`min` at equality, or a loop
bound past the last write), the code was restated so that every mutant can be
observed. The restated forms are the `cap_ms - 1` comparison, the `newest + 1`
comparison and the merged SHA-1 schedule loop. Their behavior is unchanged.

### relay.coax: decisions

- **Connection generations.** `rl_start` opens a new generation. Input,
  `rl_opened` and `rl_dropped` for any other generation are stale and
  ignored.
- **Subscriptions.** There are 8 bounded slots. Each wire id is
  `s<slot>.<serial>`, and the serial grows every time a slot is reused, so a
  frame for an old or closed subscription is unknown. The states are WANT,
  LIVE, CAUGHT (EOSE seen), CLOSING, REFUSED and AUTH. EOSE is recorded as
  "caught up". It never claims the set is complete.
- **Duplicate suppression.** A 64-id ring is shared across subscriptions.
  It is a bounded window, not a promise.
- **Forged events.** The host verifies the signature (message.coax's
  `nostr_message_event_verify_vartime`) and passes `verified`. An unverified
  event is refused and counted. It is not recorded as seen, so the genuine
  copy is still admitted.
- **Publishes.** There are 8 slots, keyed by event id. A repeated pending id
  returns the same slot. A publish moves QUEUED → SENT, with an ack deadline
  of `ack_ms` (default 10 s), then ACCEPTED or REFUSED on OK, or TIMEOUT
  (outcome unknown). A late OK still resolves a timed-out publish. Publishes
  that were sent but never acknowledged are resent after a reconnect.
- **Reconnect.** The delay is `base_ms << attempt`, capped at `cap_ms`
  (defaults 500 ms and 30 s), plus caller jitter mod (half the delay + 1).
  Attempts stop counting at 16. On reopen, every LIVE, CAUGHT or AUTH-waiting
  subscription is requested again, with `since` set to the newest
  `created_at` it delivered. NIP-01 `since` is inclusive, so the boundary
  events come again and are dropped as duplicates. A REFUSED subscription
  stays refused.
- **Cancellation.** Unsubscribing sends CLOSE only when the relay holds the
  REQ. A drop frees any pending CLOSE. `rl_shutdown` stops reconnecting.
- **NOTICE** is counted. **CLOSED** is final, except `auth-required`, which
  waits for AUTH.
- **AUTH hook (not NIP-42).** An AUTH challenge sets CHALLENGED, and
  `rl_next` returns `RL_OUT_AUTH` until the host reports a signed AUTH event
  through `rl_auth_sent(id)`. An accepted OK for that id moves every
  auth-required subscription back to WANT, once per generation. A refused
  OK falls back to no AUTH. A reconnect re-arms everything. Signing the AUTH
  event is #f426, which waits on d7's signing work.
- **Limits.** Queue overflow returns -1 from `rl_subscribe` and
  `rl_publish`. Every limit is a constant or a struct field. The machine
  does no host I/O.

Decoded input (`RlIn`) uses message.coax's type numbering (NM_EVENT=1 …
NM_NOTICE=5) and adds AUTH=6. Wiring message.coax in will be one adapter
call when it lands on main. It is uncommitted in d7's worktree and still in
its mutation gate, and these files do not load it.

## Missing (not claimed)

1. **The end-to-end test (d).** It needs a stdlib Python relay fixture under
   `coax/tests/net/`, plus a Coax driver that runs socket-native → ws.coax →
   message.coax → relay.coax. The driver must prove connect, REQ→EVENT→EOSE,
   publish→OK, and server drop → reconnect → resubscribe with `since`. It is
   not written, so `socket-native.c` has not been exercised against a live
   peer. The driver needs message.coax to decode frames, and the plan is to
   serve the signed events in `coax/lib/nostr/fixtures/coax_relay.coax` so
   that no signing is needed.
2. **Mask keys and handshake nonces from the entropy contract.** `ws.coax`
   takes them as caller bytes. The driver still has to draw them from
   `coax_entropy_native`.
3. **wss:// (TLS).** It is deliberately absent. Adding a TLS library is a
   separate owner decision (`coax/nostr.md` §1.2 says TLS stays in OS and
   browser transports). Until then, the native client reaches only ws://
   relays, for example a local relay or one behind a local TLS terminator.
4. **#7de9 stays open.** Its acceptance also names AUTH/reconnect with a real
   NIP-42 proof (#f426) and wiring to message.coax (#6015).
