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

## Part 2: entropy and the end-to-end test (claude-relay-e2e)

Recorded 2026-10-29, at 22:30 CDT.

| File | What |
| --- | --- |
| `coax/platform/entropy-foreign.{c,h}` | A scalar `#foreign` bridge over `coax_entropy_native`. `cxrand_fill(n)` draws at most 64 bytes, and `cxrand_byte(i)` reads them. Admission and completion stay in `entropy.coax`: the bridge passes its buffer size as capacity, so the Coax policy refuses n < 0 or n > 64. A failed draw holds no bytes. The bridge is for mask keys, nonces and jitter, not for keys |
| `coax/tests/net/relay_e2e_frames.coax` | **Test only.** The REQ, CLOSE and EVENT texts the driver sends, and `rxf_decode`, which matches a received text byte for byte against the frames the fixture is scripted to send (EVENT for E1 to E4, EOSE, OK true, OK false) and refuses everything else. It is not a NIP-01 parser. The seven events are the captured `status` events on lines 241 to 235 of `coax/lib/nostr/fixtures/coax_relay.coax`. They are taken as verified, and each one's id and created_at is checked against its text. 3 checks |
| `coax/tests/net/relay_e2e.coax` | The native driver (prefix `rx_`): socket-native.c, then ws.coax, then the adapter, then relay.coax. Every mask key, the 16-byte handshake nonce and the backoff jitter come from `rx_draw`, which calls `cxrand_fill`. There is no fixed or time-based value |
| `coax/tests/net/relay_fixture.py` | A local relay written with the Python standard library only, bound to 127.0.0.1. It speaks the server side of RFC 6455, follows a fixed script per connection, and records what the client sent. It answers connections that are not WebSocket upgrades with 400 and doesn't count them: during the runs something else on the host sent `HEAD /` to the port |
| `coax/tests/net/relay_e2e.py` | The harness. It builds the driver with `-fsanitize=address,undefined -fno-sanitize-recover=all`, runs it against the fixture, and checks both what the driver reports and what the relay saw |

Run: `COAX=<coax binary> python3 coax/tests/net/relay_e2e.py`. On this
host (load average about 70) the build takes 13 to 25 s and the run 2.2 s.
The runs used the main checkout's `coax/bin/coax`, because the worktree's
own `coax/build.sh` hadn't finished.

What the run proved, checked from both ends:

- **Connect and handshake.** Three TCP connects. The relay saw `GET /`,
  `Upgrade: websocket`, `Connection: Upgrade`, version 13, and a 16-byte
  base64 key that differed on every connection. ws.coax admitted the 101
  response and its accept value.
- **REQ, EVENT, EOSE.** The first REQ had no `since`. E1, E2 and E3 were
  each delivered once, and EOSE moved the subscription to caught up.
- **Publishing.** A got OK true (accepted). B got OK false with
  `blocked: fixture policy` (refused). The relay checked that each EVENT
  text was exactly `["EVENT",<fixture event>]`.
- **Ping and pong.** The driver answered the relay's ping with the same
  payload. The driver pinged the relay, and the pong carried the payload
  back.
- **Drop, backoff and reconnect.** The relay dropped the TCP connection on
  C without sending OK, and C stayed unacknowledged. It also refused the
  first retry's handshake. The measured backoff delays were 583 ms and then
  1444 ms, inside 500 to 750 and 1000 to 1500 (base doubled, plus jitter
  below half the delay). On the third connection the REQ had the same
  subscription id and `since` = 1793217794, E3's created_at. E3 came again
  and was dropped as a duplicate. E4, which "arrived" during the outage,
  was delivered. Only C was resent, and it got OK true. The relay saw
  A, B and C on connection 1 and C on connection 3.
- **Clean close.** CLOSE was sent for the subscription, then a close frame
  with code 1000. The relay echoed 1000 and then closed the TCP connection.
  At the end the machine was RL_SHUT, with no forged events, no ack
  timeouts and no unscripted frames.
- **Entropy.** The relay saw 10 masked frames with 10 distinct, nonzero
  mask keys. The driver also checks that the bridge refuses a 65-byte
  request.
- **Sanitizers.** ASan and UBSan stayed silent, with halt on error, over
  the whole run. This was the first time `socket-native.c` ran against a
  live peer.

`coax test coax/tests/net/relay_e2e_frames.coax` passed: 16 checks, its own
3 and relay.coax's 13. These are test files with no `#authority`, so they
had no mutation run. ws.coax and relay.coax are unchanged since part 1, so
their mutation results (1024/1024 and 555/555 at d7373bcc82) still stand
and were not rerun.

### message.coax: not wired

`git log origin/main -- coax/lib/nostr/message.coax` was empty at the start
of this work and again before the push. The e2e therefore runs through the
test-only `rxf_decode`. The wiring left to do is one call: in
`relay_e2e.coax`'s `rx_take`, replace `rxf_decode(text, sub, scratch)` with
`nostr_message_read(text)`, mapped onto `RlIn` (type → kind, sub, id,
accepted, `auth_required` from the CLOSED prefix, created_at, and verified
from `nostr_message_event_verify_vartime`). Then replace `rxf_req`,
`rxf_close` and `rxf_publish` with `nostr_message_write_req`, `_close` and
`_event`. After that the fixture can serve the frames unchanged, and
`relay_e2e_frames.coax` keeps only the event table.

## Missing (not claimed)

1. **message.coax wiring (#6015).** See the one-call TODO above.
2. **wss:// (TLS).** The owner has since allowed a TLS library in the C host
   (AGENTS, 02fe6a8a27). It is not started here.
3. **NIP-42 (#f426).** The e2e has no AUTH challenge, because signing an AUTH
   event waits on d7's signing work.
4. **#7de9 stays open.** Its acceptance names AUTH and reconnect with a real
   NIP-42 proof (#f426).
5. **A reusable client loop.** The connect, send and receive loop lives in
   the test driver (`rx_step`). Moving it into `coax/lib/` is the next step
   for Pylon, together with the message.coax wiring.
