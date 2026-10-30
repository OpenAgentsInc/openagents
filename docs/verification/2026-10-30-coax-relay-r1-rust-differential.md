# Coax relay R1: the differential against the Rust relay in a sandbox

2026-10-30, claude-relay-rust-diff. Plan: [coax/nostr.md section 17](../../coax/nostr.md).
R1 records: [2026-10-29-coax-relay-r1.md](2026-10-29-coax-relay-r1.md).

The owner approved this on 2026-10-29 with three conditions: the only network
access is fetching the relay's pinned crates, no production data is used, and
nothing contacts relay.openagents.com. This run met all three. The Rust relay
ran against an empty, throwaway Postgres in a network namespace with only
loopback, and every event was signed with relay_session.py's test-only keys.

## How the sandbox is built

`coax/tests/nostr/rust_relay_sandbox.sh` (rerunnable; `SCRATCH` defaults to
`${TMPDIR:-/tmp}/coax-rust-relay` and must be outside the repository):

- **extract** (no network): `git archive 0b401561a0^` of `crates/nostr-relay`,
  `crates/nostr`, `crates/agent-contract`, `migrations/`, `nips/` and
  `tests/fixtures/` (files that the crates read at compile time with
  `include_str!`), plus the old `Cargo.lock`, into `SCRATCH/src`. Nothing is
  restored into the tree. The old workspace (about 1,900 packages) is replaced
  by a minimal workspace of the relay and its two path crates. The
  hakari `openagents-workspace-hack` crate is replaced by an empty crate,
  because it only unified features across the old workspace.
- **fetch** (the only networked step): `cargo fetch` in bwrap with the
  network shared, then a check that each of the 144 packages in the
  pruned lock has the same version and checksum as in the old `Cargo.lock`,
  then `cargo fetch --locked`. `--locked` can't be the first step, because
  the minimal workspace prunes the lock. cargo fetch runs no build scripts.
  It took 7.6 s.
- **build** (network off): `cargo build --offline --locked --release -p nostr-relay`
  with the crate's own toolchain (rustup's installed 1.97.1, mounted
  read-only). It finished in 2 min 33 s on a host at load 60.
- **run** (network off): a throwaway Postgres 17.11 (nix) is created with
  `initdb` in `SCRATCH/pg`, listening on a Unix socket in the private /tmp and
  on 127.0.0.1:54329. The relay migrates the empty database itself. Then the
  command runs.

The bwrap sandbox (`--unshare-all --die-with-parent --new-session
--clearenv`) mounts these paths read-only: /nix, the system profile, /bin,
/usr/bin, /lib64, /etc/passwd, /etc/group, /etc/hosts and the toolchain.
SCRATCH is writable and /tmp is private. `HOME` and `CARGO_HOME` are in
SCRATCH. The rest of $HOME (~/work, ~/.ssh, ~/.cargo) is not mounted. For
`run`, only the repository worktree and the `@noble/curves` node_modules
directory are added, read-only. The probe saw no network
(`curl https://example.com` failed) and an empty `~`.

The Rust relay's configuration matches production's NIP-11 document
(coax/nostr.md section 17). `NOSTR_RELAY_URL=ws://127.0.0.1:PORT` gives a
challenge on every connection, `NOSTR_RELAY_MAX_LIMIT=127`, and
`auth_required` is false. The rate limits are R2 and were lifted, so that
they never decide a verdict here: events, REQs and connections per IP, and
events per pubkey.

## The driver

`coax/tests/nostr/relay_rust_differential.py`, run by
`coax/tests/nostr/rust_relay_differential.sh`. The wrapper builds the Coax
relay with ASan+UBSan outside the sandbox, then runs the driver inside it.
The driver signs 169 events once with the test-only key (`relay_sign.ts`)
and plays the same script against both relays, one after the other. It
records 114 observations and compares them one by one. Each one is
classified in this order:

1. **match**: identical frames.
2. **intentional** (`KNOWN`): decided and already written down in the R1
   records.
3. **owner** (`OWNER`): reported, and left for NEEDS_OWNER.md.
4. **text**: the same verdicts and NIP-01 machine prefixes (`invalid`,
   `duplicate`, ...), with only the human-readable text after the prefix
   differing.
5. **unexplained**: the run fails.

An AUTH event is signed per relay (its challenge and URL differ), so its OK
is compared without the id. Rust's EOSE third element is folded into the
plain EOSE and reported once. Every REQ that only reads stored events is
closed after its EOSE, so it can't collect live events later.

The script covers these areas:

- **NIP-11**: status, the six limits, NIPs and extra fields.
- **AUTH**: the handshake and challenge, and a write before AUTH. AUTH with
  a wrong challenge, another relay, an event 601 s old, kind 1, no relay tag,
  two challenge tags, a bad signature, a wrong id, the right answer and the
  same key again. The AUTH event is not delivered.
- **EVENT admission**: valid with live delivery, duplicate, bad id, bad
  signature, created_at 1000 s and 800 s ahead, expired, expiring in an hour,
  kind 22242, 257 and 256 tags, ephemeral 20001 (delivered live, not stored)
  and a malformed EVENT.
- **Replacement**: kind 0 newer then older; kind 3 same-second ties, higher
  id first, then lower, then higher again; kind 10002 ties, lower id first;
  30617 ties, newer and older; another `d` kept.
- **The website's kinds**: 30617, 30621, 30623, NIP-34 1621 with statuses
  1630 and 1632, and NIP-22 1111, shaped like the recording.
- **REQ**: `ids`, `authors`, `kinds`, `since`, `until`, since plus until,
  `limit` 3 and 0, `#a`, `#e`, `#p`, `#d`, `#t`, `#E`, the project page's
  issues REQ, two filters, overlapping filters, no match and since > until.
- **Limits on 130 more kind 1 events**: `limit` 500 (max_limit), no limit
  (default_limit), 127, 128, and two filters each with a limit. The event
  bytes are as published.
- **CLOSE**: its answer, nothing after it, and an unknown sub.
- **Limits**: sub id 64, 65 and empty; 16 and 17 filters; an empty
  `kinds`; `#e` and `#p` not hex; eight open filters (query cost); an
  unknown field; a negative limit; no filter; the 33rd subscription; a
  reused sub id.
- **Other messages**: COUNT, an unknown verb, text that is not JSON, ping, a
  binary frame, a 131074-byte frame, and close 1000.

## Result

`SCRATCH=... coax/tests/nostr/rust_relay_differential.sh` (final files):
**114 observations: 88 match, 15 same verdict with other text,
7 intentional, 4 for the owner, 0 unexplained. No sanitizer report.**
The Rust session took 8.6 s and the Coax session (ASan) 63.0 s.

What matched exactly, among the 88:

- NIP-11 `max_message_length` 131072, `max_subscriptions` 32, `max_limit`
  127, `max_subid_length` 64, `auth_required` false and `restricted_writes`
  true.
- Every AUTH refusal reason for a wrong challenge, another relay, 601 s and
  kind 1, and the accepted answers.
- All of these OKs: valid, duplicate, created_at 1000 s ahead (the same text),
  800 s accepted, expiring later accepted, kind 22242, 257 tags refused and
  256 accepted, and ephemeral accepted, delivered live and never stored.
- Replacement: kind 0 superseded, both tie orders for kinds 3, 10002 and
  30617 (the lower id wins on the same second), newer and older 30617, and a
  second `d` kept.
- Every single-filter REQ: the same ids in the same order.
- max_limit and default_limit both serve 127 events, newest first then the
  lowest id. The event bytes are as published.
- CLOSE: no answer, nothing afterwards, and silence for an unknown sub.
- The 64-character sub id; 16 and 17 filters; an empty list; `#e` and `#p`
  not hex; query cost; the 33rd subscription; a reused id.
- Ping, a binary frame, close 1009 for the big frame, and close 1000.

### Every difference

| Observation | Rust | Coax | Class |
| --- | --- | --- | --- |
| NIP-11 `supported_nips` | 1, 9, 11, 40, 45, 50, 65, 67, 77, 94, 17, 42, 59, 70 | 1, 11, 40, 42 | intentional (R2 adds the rest) |
| NIP-11 extra limitation fields | also `due_delivery_mode`, `max_not_before_delta` | `created_at_upper_limit`, `default_limit`, `max_event_tags` | intentional (R2) |
| A write before AUTH | OK true | OK false "auth-required: authenticate before publishing" | intentional (section 17: NIP-42 for writes; R1 record, AUTH policy) |
| AUTH with two challenge tags | OK true | OK false "invalid: authentication event must have exactly one relay tag and one challenge tag" | intentional (nip42.coax is stricter; R1 record) |
| A malformed EVENT (`["EVENT","not an object"]`) | NOTICE "invalid: EVENT contains an invalid event object" | NOTICE "invalid: malformed message" | intentional (Known differences) |
| COUNT | `["COUNT","cnt",{"count":135}]` | CLOSED "error: COUNT is not supported by this relay" | intentional (NIP-45 is R2) |
| EOSE | `["EOSE",sub,["finish"]]`, and `["more"]` when a limit cut the result (NIP-67) | `["EOSE",sub]` | intentional (R1 record, differential) |
| NIP-34 issue without a `30617` `a` tag | OK false "invalid: invalid event: the event names its repository in a 30617 a tag" | OK true | **owner** (NEEDS_OWNER question 1) |
| NIP-22 comment without `P` | OK false "invalid: invalid event: a nostr root requires a P tag" | OK true | **owner** (question 1) |
| REQ with two filters (1621, 1111) | one list, newest first across filters | filter by filter | **owner** (question 2) |
| REQ with two limited filters | the merged order, and 127 shared (64 each) | each filter's own order and limit | **owner** (question 2) |
| AUTH with no relay tag | "invalid: authentication relay URL does not match" | "invalid: authentication event must have exactly one relay tag and one challenge tag" | text |
| AUTH bad signature, EVENT bad signature | "invalid: invalid Schnorr signature" | "invalid: bad signature" | text |
| AUTH wrong id, EVENT bad id | "invalid: event id mismatch: expected X, received Y" | "invalid: event id does not match its content" | text |
| Expired (NIP-40) | "invalid: event expired at T; current time is N" | "invalid: event has expired" | text |
| A retired replaceable or addressable id sent again (kind 3, 30617) | OK true "duplicate: newer replaceable event already stored" | OK true "duplicate: already have this event" | text |
| Sub id 65 characters or empty | NOTICE "invalid: REQ subscription id ..." | NOTICE "invalid: subscription id ..." | text |
| Unknown filter field, negative limit | CLOSED "invalid: REQ contains an invalid or unsupported filter" | CLOSED "invalid: malformed filter" | text |
| REQ with no filter | CLOSED "invalid: REQ must contain a subscription id and at least one filter" | CLOSED "invalid: malformed filter" (after the fix below) | text |
| Unknown verb | NOTICE "invalid: unsupported message verb \"HELLO\"" | NOTICE "invalid: unsupported message verb" | text |
| Text that is not JSON | NOTICE "invalid: malformed JSON" | NOTICE "invalid: malformed message" | text |

The text differences are left as they are. NIP-01 gives the text after the
prefix to humans, every verdict and prefix agrees, and changing a reason
text in relay_server.coax would rerun its 6,087-mutant graph (24 shards of
about 13 minutes) for no behavioural change. If one should match the Rust
text exactly, it's a one-line change to `rs_text` plus that mutation run.

### The bug found and fixed

**`["REQ", sub]` with no filter opened a subscription.** The Coax relay
answered EOSE and kept a slot open that matched nothing. The Rust relay
answers CLOSED invalid. message_server.coax reads zero filters as valid (it
caps the count at 16), and nothing refused it later. The fix is in the
routing unit, so neither big mutation graph is touched:

- The check came first: `#test "a REQ needs at least one filter"` in
  `coax/lib/nostr/relay_route.coax` (0 and -1 are refused with
  `RR_RS_FILTER`; 1 and 16 are accepted).
- Then `rr_req_filters`, plus the mirrored `RR_RS_FILTER :: 19`. It is tied
  to `RS_FILTER` by a top-level `#assert` in `coax/relay/relay.coax`, and
  `ry_req` calls it before `rs_req`. The refusal reuses `RS_FILTER`
  ("invalid: malformed filter"): it has the same prefix as Rust's text and
  needs no new reason in relay_server.coax.
- `coax test coax/lib/nostr/relay_route.coax --library`: 11 of 11.
  `coax mutate coax/lib/nostr/relay_route.coax --library --jobs 4`:
  **255 mutants, 255 killed**, with no `// mutant: equivalent` lines.
  relay_server.coax and message_server.coax are unchanged, so their results
  in the R1 record stand.
- `relay_session.py` (72/72) and `relay_differential.py` (the recording,
  13/13) pass with the change.

## Reruns

- `coax/tests/nostr/rust_relay_differential.sh`: 0 unexplained (above).
- `python3 coax/tests/nostr/relay_session.py` (ASan/UBSan): 72 of 72, no sanitizer report.
- `python3 coax/tests/nostr/relay_differential.py` (ASan/UBSan, the recording): 13 of 13.

## What is still missing

- The owner's two answers (NEEDS_OWNER.md, Coax relay). If either is yes,
  the work goes into relay_server.coax or the store query, with that unit's
  mutation run.
- The driver plays the script against one relay after the other, not
  concurrently. Live delivery is covered on one connection pair, not under
  load. The flood measurement belongs to the parallel R1 work.
- Not compared, because they are R2: NIP-09 deletion, NIP-45 COUNT beyond
  the refusal, NIP-50, NIP-77, NIP-29, gift wraps, NIP-70, rate limits and
  connection limits (lifted on the Rust side here).
