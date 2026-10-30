# Coax relay R1: the first part (relay-server-r1, #cae0)

2026-10-29, claude-relay-server. Plan: [coax/nostr.md section 17](../../coax/nostr.md).
The work is partial. This record says what is built, what the checks
proved and what R1 still needs. It never touched relay.openagents.com or its
database. The only production data used is the read-only capture already in
the repository (`coax/lib/nostr/fixtures/coax_relay.jsonl`).

## What landed

| File | What | Checks | Mutation |
| --- | --- | --- | --- |
| `coax/lib/net/ws_server.coax` (`#authority`) | RFC 6455 server half. It admits the opening request: `GET /target HTTP/1.1`, Upgrade, a Connection token, version 13, exactly one 16-byte base64 key, and an 8192-byte head limit. It writes the 101 with Sec-WebSocket-Accept. It reports a plain GET that accepts `application/nostr+json` as a NIP-11 request. It writes HTTP responses with an exact length and CORS headers. It reads masked client frames (unmasked ones are refused), unmasks them and writes unmasked server frames and closes. Message assembly, UTF-8, close codes and the refusal-to-close-code mapping are `ws.coax`'s, loaded and reused | 33 (with ws.coax's and sha1's) | 1414/1414 killed (`b01a65cfe3`) |
| `coax/lib/nostr/message_server.coax` (`#authority`) | The relay side of NIP-01 frames, beside `message.coax` (#6015, the client side). Reads EVENT [event], REQ [sub, filter...] (at most 16) and CLOSE [sub]. AUTH and COUNT are reported by verb only, because reading AUTH belongs to nip42.coax. Writes EVENT (with the original bytes), OK, EOSE, CLOSED and NOTICE. A reason is "prefix: text" with one of the eight NIP-01 prefixes or auth-required. Sub ids are 1..64 characters and frames at most 270336 bytes, as in message.coax | 24 | 1179/1179 killed (`f59c903601`) |
| `coax/lib/nostr/filter_read.coax` (`#authority`) | `filter_read`: one filter object into a `NostrFilter` (filter.coax had no parser), with views into caller storage. An absent list differs from an empty one. Hex ids, authors, `#e` and `#p` are checked. Tag keys are `#` and one letter, case-sensitive, so `#E`, `#q`, `#a` and `#d` work. Unknown or repeated keys and wrong types are refused. The result passes `nf_valid`. It round-trips with `nf_write` | 3 (205 in the graph) | 4131/4131 killed |
| `coax/lib/nostr/relay_server.coax` (`#authority`) | The relay's policy. EVENT admission runs in this order: `rs_admit_read` reads with the id check and rejects malformed or oversized d tags, refuses kind 22242, allows at most 256 tags, created_at at most 900 s ahead and no expired NIP-40 event, then runs the NIP-42 hook. `rs_admit_verified` admits only on a verified result for the same id. The program passes it `nostr_event_verify_vartime`'s result, so the unit's checks never run a full BIP-340 verify. It defines the OK, CLOSED and NOTICE reasons, using the Rust relay's text where it had one. REQ rules: empty lists refused, `#e`/`#p` hex, the Rust relay's query cost (100000), and `max_limit`/`default_limit` 127. It sets the kind classes and the replacement rule: newer wins, and on the same second the lowest id wins. It reads tag values and expiration, keeps a per-connection subscription table (32 slots, at most 8192 bytes of filters each, reparsed on match) and serves the NIP-11 document | 11 (253 in the graph) | 6087/6087 killed |
| `coax/tests/nostr/relay_store_double.coax` | **Test-only** in-memory store behind the narrow interface the relay needs. It admits events (id dedup, replacement through `rs_replace`, ephemeral events not stored) and answers queries by filter (newest first, then lowest id, `rs_limit`, expired records skipped). It is not durable | 1 | not `#authority` (test double) |
| `coax/relay/relay.coax`, `relay-host.c`, `build.sh` | The relay program. `relay-host.c` reads `--listen ADDRESS` (an IPv4 or IPv6 literal: 127.0.0.1 or a tailnet address) and `--port N`, then runs the Coax main on a 512 MiB stack thread. The Coax main loop handles accept, HTTP/NIP-11, WebSocket, NIP-01, fan-out to live subscriptions, ping/pong and closes. ws:// only | via the sessions | n/a |
| `coax/platform/socket-server-native.{c,h}` | Listener effects: bind a literal address, listen, poll, accept (32 handles), nonblocking read, buffered write and flush, close, and the wall clock. Nothing else | via the sessions | n/a (C) |
| `coax/tests/nostr/relay_session.py`, `relay_sign.ts` | Scripted sessions (standard-library client) against an ASan+UBSan build. `relay_sign.ts` signs with a test-only key (the sha256 of a fixed label) through `@noble/curves` | 50 | n/a |
| `coax/tests/nostr/relay_differential.py` | The same five REQs the project page sends, against the recorded behaviour of relay.openagents.com | see below | n/a |
| `coax/tests/nostr/relay_server_fixtures.{py,coax}` | Events whose ids are correct (from Python hashlib) and whose all-f signature is refused before a full verify. The pubkey is the generator's x | n/a | n/a |

Host choice: the program is `coax/relay/` (relay.coax, relay-host.c,
build.sh). The listener effect is a new file in `coax/platform/`. The
client `socket-native.c` is unchanged.

## What the checks proved

`python3 coax/tests/nostr/relay_session.py` (ASan+UBSan, 127.0.0.1, a free
port; build 15 to 75 s and run 20 to 40 s, depending on host load) passed 50 of 50:

- NIP-11: 200 with CORS. The document states `max_message_length` 131072,
  `max_subscriptions` 32, `max_limit` 127, `max_subid_length` 64 and NIPs
  [1, 11, 40].
- Handshake accept values. A REQ on an empty relay gets EOSE only.
- A publish gets OK true "", and an open subscription receives the event
  live with its bytes unchanged. Publishing it again gets "duplicate:
  already have this event". A bad signature gets OK false "invalid: bad
  signature". Changed content gets "invalid: event id does not match its
  content", with the event's id. Refused events are not delivered.
- Kind 0: an older version after a newer one gets OK true "duplicate: newer
  replaceable event already stored", and only the newest is served.
  Addressable 30617 at the same second: the lower id replaces the higher,
  the higher is then a duplicate, and one version is served.
- Kinds 30621, 30623, 1621, 1630, 1633 and 1111 are accepted. REQs by `#a`,
  `#d` with kinds, `#e`, `#E`, `#q`, `#p`, authors+kinds+limit, ids,
  `limit` 1 (newest), `limit` 0 (EOSE only), `since` and `until` return
  exactly the expected sets.
- NIP-40: an expired event is refused. One that expires 15 s later is
  accepted and served, and after its time it is no longer served.
  created_at 1000 s ahead is refused with the Rust relay's text.
- After CLOSE nothing more arrives.
- Limits: a 65-character sub id gets a NOTICE, 17 filters get "restricted:
  too many filters", an empty list gets "invalid: filter arrays must not be
  empty", eight open filters exceed the query cost, `#e` that is not hex
  gets CLOSED, and a 33rd subscription gets "restricted: too many active
  subscriptions". Reusing an id replaces the subscription.
- AUTH gets OK false "restricted: NIP-42 is not configured on this relay"
  (the Rust relay's text). Publishing kind 22242 is refused. An unknown verb
  and a binary frame get NOTICEs, and ping gets a pong with the same
  payload.
- A 131074-byte frame gets close 1009. Close 1000 is answered.
- No sanitizer report.
- Restart: 0 events after restart. This is expected, because the test
  double is not durable (see "Missing").

The sessions found two problems, both now fixed. An OK for an id mismatch
carried a zero id; a check now covers it. The expiry test raced a fixed
window: on a loaded host the session took 15 to 22 s to reach it. It now
signs the expiring event at publish time, with 8 s to live.

`python3 coax/tests/nostr/relay_differential.py` passed 12 of 12, twice (at 8b7fca771f and on the final files). It
compares against relay.openagents.com's recorded behaviour, read-only. The
owner allows a sandboxed build of the Rust relay; it did not fit this time
box (see "Missing"). The test publishes the 237 events the production relay
sent for the project page's five REQs (capture.ts, 2026-10-28) to a fresh
Coax relay, and every one gets OK true after a full id and signature check.
It then sends the same five REQs. For each of project (2), issues (105),
comments (121), status (8) and repo (1), the result is the same event ids
in the same order, and every EVENT frame carries the same event bytes as
the recording. There is one difference: the Rust relay's EOSE carries a
third element, `["EOSE","sub",["finish"]]`, and ours is the plain NIP-01
`["EOSE","sub"]`. No sanitizer report.

Throughput note: under ASan at -O1, publishing the 237 events took 111.8 s
(207.3 s on the second run, while 48 mutation workers shared the host), about
0.47 s each at best, almost all of it BIP-340 verification in
Coax-emitted C. A release build without sanitizers has not been measured.
The relay serves one event at a time, so a flood of valid events is bounded
by verification speed.

## Mutation

Every run used `coax mutate FILE --library --jobs N --shard I/N` and
`--merge` on the final file contents, with no `// mutant: equivalent`
lines.

| Unit | Mutants | Result |
| --- | --- | --- |
| `ws_server.coax` (with ws.coax and sha1.coax) | 1414 | 1414 killed, 3 shards |
| `message_server.coax` (with json.coax) | 1179 | 1179 killed, 3 shards |
| `relay_server.coax` (the whole graph: ws_server, message_server, filter_read, filter, event, json, schnorr) | 6087 | **6087/6087 killed** on the files at `2c69195ee8`, 24 shards. Earlier contents: 6054 of 6083, then 6082 of 6083 |
| `filter_read.coax` (with filter, event, json, schnorr) | 4131 | **4131/4131 killed** on the files at `2c69195ee8`, 24 shards |

The two big graphs load event.coax, and their aggregate types hold views.
Every mutant therefore runs all of the graph's checks (`full checks`), so
the runs needed 24 shards of 2 jobs, about 13 minutes each, on a host at load
50 to 100.

The first relay_server run (6083 mutants, 29 survivors, all in
relay_server.coax and filter_read.coax) led to these fixes:

- Restated `rs_id_less`, `rs_newer` and `rs_limit` so that each boundary can
  be observed. `rs_limit` reads "below 128": at 127 both branches agree.
- Added `rs_d`, which refuses a d tag over 256 bytes.
- Split admission into `rs_admit_read` and `rs_admit_verified`. The
  signature branch can now be tested without a 76M-step verify, and
  `rs_admit_verified` also requires the verified id to equal the id that
  was read.
- Added checks for cost exactly at and one over 100000, filters of exactly
  8193 bytes, a stored filter that no longer reads, a closed slot, an empty
  tag value, and a JSON error in a filter value.

A second run left one survivor, the folded `RS_MAX_LIMIT - 1`, which led to
the "below 128" form.

## Missing for R1

- **Durable storage.** nostr-event-store (#8eb5) is held by
  codex-nostr-profiles and stalled; a note on #8eb5 says R1 needs it. The
  relay uses the narrow interface (`rsd_admit`, `rsd_query`, `rsd_text`)
  over a test-only in-memory double, so a restart forgets everything. The
  append-only fsync'd log, replay with re-verification and the indexes are
  that task's job, not a second store here.
- **NIP-42.** Done in the second part (below).
- **message.coax (#6015).** Landed (`c7f0d7a222`). message_server.coax
  keeps its own values and is unchanged (its 1179/1179 above stands): loading
  message.coax into it would put message.coax's 4,162 mutants into its
  mutation graph. Instead `coax/lib/nostr/message_agree.coax` (not
  `#authority`) loads both and `coax/relay/relay.coax` loads it, so a drift
  fails the relay build. Top-level `#assert`s, inside the compiler's default
  100,000-step bound: `MS_MAX_FRAME`, `MS_MAX_FILTERS` and `MS_SUB_MAX` equal
  `NM_MAX_FRAME`, `NM_MAX_FILTERS` and `NM_MAX_SUB`; `ms_prefix_ok` admits
  exactly `NM_PREFIXES` (each read by `nm_prefix` as its own index) plus
  auth-required (read as another word). Its check (`coax test
  coax/lib/nostr/message_agree.coax`, 219/219 with its graph) adds what
  is too costly at compile time: `ms_sub_ok`/`ms_chars` equal
  `nm_sub_ok`/`nm_chars` for 0..66 one- and two-byte characters, every OK,
  CLOSED and NOTICE the relay writes (each prefix, accepted and refused),
  EOSE and EVENT read back by `nostr_message_read` with the same id, verdict,
  prefix, sub and event bytes, and a client's REQ (16 filters), CLOSE and
  EVENT from `nostr_message_write_*` read back by `ms_read`. With it loaded,
  `relay_session.py` passes 72 checks and `relay_differential.py` 13, under
  ASan/UBSan.
- **Rust differential beyond the recording.** The owner allows a sandboxed
  build of the Rust relay from history. It did not fit this time box, so it
  was not built. The differential uses the read-only recording only.
- **Known differences from the Rust relay.** A malformed EVENT (no
  readable id) gets a NOTICE, where Rust echoes an id hint in OK. Admission
  checks policy before the signature, so an event that is invalid in two
  ways can get a different reason. Content larger than 64 KiB decoded is
  refused (event.coax's buffer). Rust's per-IP and per-pubkey rate limits,
  connection limits, NIP-09/45/50/77/29/59/17/70/65/67/94/98 and the
  extensions are R2. COUNT gets CLOSED "error: COUNT is not supported by
  this relay".
- **The rest of R1's exit evidence.** The client relay machine (#7de9) and
  the Pylon have not yet been pointed at it on the tailnet. Bounded memory
  under a flood is fixed by construction: 16 connections, 32 subscriptions,
  8 KiB of filters each, 8 authenticated keys each, 2048 events and 4 MiB
  in the double, and a refusal past every limit. It has not yet been
  measured under a flood. The session glue's decisions are now
  `relay_route.coax` (`#authority`, second part below).
- **TLS.** None. ws:// only, on 127.0.0.1 or a tailnet literal. Use
  tls-native's server mode later or a TLS proxy.

## Second part: NIP-42 and the routing unit (claude-relay-r1b)

2026-10-29, claude-relay-r1b, on top of the first part. The store was not
touched: claude-store is building the durable store behind the same
interface as `relay_store_double.coax` (`rsd_admit`, `rsd_query`,
`rsd_text`), and the swap is one `#load` line in `relay.coax`.
`relay_server.coax`, `nip42.coax` and `nip42-event.coax` are unchanged, so
their mutation results above and in the NIP-42 record still stand.

| File | What | Checks | Mutation |
| --- | --- | --- | --- |
| `coax/lib/nostr/relay_route.coax` (`#authority`, new) | The routing decisions `relay.coax` used to make inline: which handler or reply each NIP-01 message gets (`rr_message`), what the HTTP head (`rr_http`), a frame read (`rr_frame_read`) and a frame event (`rr_frame_event`) lead to, the free connection slot and the connection generation, fan-out (only a newly stored event, only to an open WebSocket subscription that matches), the REQ's sent-once list across filters (`rr_take`), the challenge's hex (`rr_challenge`), the AUTH answers and their texts (`rr_auth_answer`, `rr_auth_prefix`, `rr_auth_text`), the per-connection authenticated keys (`rr_auth`, `rr_authed`, `rr_authed_as`, at most 8, each once) and the write gate. It loads nothing: it takes the other units' verdicts as integers, so mutating it never re-mutates event.coax, Schnorr or JSON. Its mirrored codes are tied to their sources by top-level `#assert`s in `relay.coax`, so a change there fails the program's build. It also holds the NIP-11 document the relay now serves (NIPs 1, 11, 40, 42; `auth_required` false, `restricted_writes` true) | 10 | **250/250 killed**, one run, `--jobs 3` |
| `coax/relay/relay.coax` | Host names renamed `rl_`/`RL_`/`RlConn`/`RlWork` to `ry_`/`RY_`/`RyConn`/`RyWork` (nip42.coax loads the client `relay.coax`, whose names are `rl_`). Loads nip42-event.coax and relay_route.coax. Each connection has a generation, a `Nip42` record and an `RrAuth` record. After the 101 it sends `["AUTH", <64 hex>]` (`nip42_issue`, 32 bytes of host entropy). AUTH: `nostr_event_read` (for the id; a wrong id gets OK false "invalid: event id does not match its content", unreadable gets a NOTICE), then `nip42_admit` (signature, ±600 s, the relay URL canonicalized, the connection's current challenge and generation), then `rr_auth`. EVENT passes `rr_writes_need_auth(true)` and `rr_authed` to `rs_admit_read`. AUTH never reaches the store or fan-out | via the sessions | n/a |
| `coax/relay/relay-host.c` | `--url URL` (the relay URL AUTH events must name; default `ws://ADDRESS:PORT` with the bound port) and `ry_host_random` (one byte of `getentropy`). The relay refuses to start when the URL is not one nip42.coax accepts (an IPv6 literal needs `--url`) | via the sessions | n/a (C) |
| `coax/tests/nostr/relay_session.py`, `relay_differential.py` | Every connection reads its challenge first. A signed-AUTH session (`nip42()`), signed with the test-only key and a second test-only key through `@noble/curves` (`relay_sign.ts`, `RELAY_TEST_KEY_LABEL`). The differential authenticates once with the test key before importing the recording | 70 sessions; differential 13 | n/a |

### The AUTH policy, and why

Read from the Rust relay at `0b401561a0^` (`crates/nostr-relay/src/gateway/auth.rs`
and `server.rs`) and coax/nostr.md section 17 ("NIP-42 required for writes"):

- **A challenge on every connection, first.** The Rust relay queues
  `["AUTH", challenge]` as a connection's first message whenever a relay
  URL is configured. Ours always is: 32 bytes of host entropy as lowercase
  hex, bound to the connection's generation. A reconnect is a new
  generation and a new challenge, and an answer to any other challenge is
  refused.
- **Writes need an authenticated key; any key on the connection.** The Rust
  relay's `auth_required` refuses EVENT unless the connection has at least
  one authenticated key (`is_authenticated`), not necessarily the event's
  author; author-bound checks are only for its closed-membership and
  private kinds, which are R2. We do the same, so a client can import
  events signed by others (the differential does). The refusal is the Rust
  text, "auth-required: authenticate before publishing".
- **Reads stay open.** The Rust relay's `auth_required` also gates REQ, but
  relay.openagents.com runs with it false, and openagents.com's project page
  reads anonymously. Section 17 asks for NIP-42 on writes. So REQ and CLOSE
  need no AUTH, and NIP-11 says `auth_required` false and
  `restricted_writes` true.
- **AUTH answers.** OK true "" for an admitted key; the same key again is
  OK true. Refusals use the Rust texts: "invalid: authentication event must
  have kind 22242", "invalid: the authentication event's timestamp is more
  than 10 minutes from the relay's clock", "invalid: authentication
  challenge does not match" (also for a stale generation), "invalid:
  authentication relay URL does not match", and "invalid: bad signature".
  nip42.coax is stricter than Rust about tags (exactly one relay and one
  challenge tag), which gets "invalid: authentication event must have
  exactly one relay tag and one challenge tag". A ninth key on one
  connection gets "restricted: too many authenticated keys on this
  connection" (Rust keeps an unbounded map; the bound is ours).
- **Kind 22242 is never stored or broadcast.** As an EVENT it is refused
  before anything else ("invalid: authentication events cannot be
  published"); as AUTH it never reaches the store or the fan-out.
- **Not done:** the Rust relay's AUTH rate limits and its agent-attestation
  ("virtual") keys are R2.

### Checks

- `coax test coax/lib/nostr/relay_route.coax --library`: 10 of 10.
- `coax mutate coax/lib/nostr/relay_route.coax --library --jobs 3`:
  250 mutants, 250 killed, no `// mutant: equivalent` lines.
- `python3 coax/tests/nostr/relay_session.py` (ASan+UBSan): 70 of 70. The
  first part's 50, with the old "AUTH: NIP-42 not configured" check now
  "AUTH without a relay tag: refused", plus 20 NIP-42 checks: each
  connection's own 64-hex challenge; a write before AUTH gets
  auth-required; reading needs no AUTH; AUTH refused for a wrong
  challenge, another relay, created 601 s ago, kind 1, two challenge tags,
  a bad signature and a wrong id; refused answers authenticate nothing; the
  right answer (relay URL as `WS://127.0.0.1:PORT/`) gets OK true; the AUTH
  event is not delivered to a kind 22242 subscription and is never stored;
  the same key again, and a second key; a new connection gets a new
  challenge, an answer to another connection's challenge is refused, and
  it is anonymous until it answers. NIP-11 now states NIPs 1, 11, 40, 42,
  reads open, writes restricted. No sanitizer report.
- `python3 coax/tests/nostr/relay_differential.py` (ASan+UBSan): 13 of 13. One AUTH with the test key, then the 237 recorded events (signed by the project key, not the authenticated one) all get OK true in 36.0 s, and the five REQs return the same ids in the same order with the same bytes. No sanitizer report.

### Still missing for R1

Durable storage (claude-store's swap), the client relay machine (#7de9) and
the Pylon on the tailnet, a flood measurement, the sandboxed Rust
differential, and TLS, as listed above.

## Update: the verified store (claude-store, 2026-10-29)

The double is no longer used. The relay loads `coax/lib/nostr/store_saved.coax`.
`--store FILE` keeps events in an append-only file that is replayed with
re-verification on start. relay_session.py passes 72/72, including a restart
with `--store` that brings back the same events. relay_differential.py passes
13/13. Both run under ASan/UBSan. Record:
[2026-10-29-nostr-store-r1.md](2026-10-29-nostr-store-r1.md). Trusted
recovery (#966b) is still missing.
