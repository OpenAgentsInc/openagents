# 33. A client that survives restarts

A real client goes offline, is killed and starts again. This chapter builds
the state a client keeps on disk so that a restart loses nothing and repeats
nothing: the events it has not finished publishing, where each subscription
stopped on each relay, the events it has already shown, and the deletions it
must keep honoring. All of it is one append-only journal over a byte buffer,
replayed after a crash.

Part VIII, A client. Result: A durable client journal that stores verified
originals, an offline outbox that sends each event again after a restart
without a second copy anywhere, subscriptions that resume with `since`, and
deletions that are never resurrected on reconnect.

## What this chapter teaches

- Store each event's original bytes with its ID, and never re-serialize
  them.
- The outbox: an event is journaled before it is sent, and each relay's `OK`
  after it arrives. A restart resends the same bytes, and the relay answers
  `duplicate:` and keeps one copy.
- Delivery to the application is an append to the journal, so it happens
  once, and each subscription's cursor moves with it.
- NIP-09 deletions recorded as tombstones before the deletion event itself,
  and honored on every later delivery.
- Replay after a crash: cut a partial last record, refuse damage that has
  committed records after it, and refuse a journal from another version or
  another key.

Chapter 31 ran one publish on one connection, and chapter 32 chose relays
and merged one subscription across them. Both kept their state in memory.
This chapter is the layer that remembers that state across a restart. It
opens no socket and reads no clock: the caller passes each relay's answers
in, and the host writes the journal's new bytes to a file and syncs it.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [client_journal.coax](../../coax/lib/nostr/client_journal.coax) | `cj_open(j, buf, total, owner)` | replays a journal, cuts a crash's partial last record, or refuses the journal (`#authority`) |
| | `cj_relay(j, buf, url)` | numbers a relay once and journals it |
| | `cj_queue(j, buf, id, relays, bytes)`, `cj_ok(j, buf, id, r, outcome)` | the outbox: journals your signed event before it is sent, then each relay's `OK` |
| | `cj_deliver(j, buf, sub, r, created_at, id, author, bytes, deletes)` | journals an event a relay sent and delivers it once, or reports a duplicate or a deleted event; moves the subscription's cursor |
| | `cj_since(j, sub, r)`, `cj_live(j, k)`, `cj_find(j, id)` | what a restart asks with `since`; whether a stored event is shown; the store index of an ID |
| | `cj_apply`, `cj_same` | applies one record (the writers and replay share it); compares two states |
| [outbox.coax](../../coax/lib/nostr/outbox.coax) | `ob_relay`, `ob_is` | chapter 32's relay pool, which numbers the journal's relays |
| [sha256.coax](../../coax/lib/crypto/sha256.coax) | `sha256_hash_checked` | chapter 4's SHA-256, for each record's check |

The new file carries `#authority;`. It decides which events reach the
application, which publishes are sent again, and which deletions hide an
event.

## What a client must not forget

A client that keeps everything in memory loses four things when it is
killed:

1. **Publishes in flight.** You pressed Send. The event went to r1, which
   answered `OK`, and to r2, which stored it but whose `OK` never arrived.
   r3 was offline. After a restart, the client must send the event to r2
   and r3 again and not to r1.
2. **Where each subscription stopped.** `relay.coax` (chapters 29 to 31) resumes a
   subscription on a reconnect with `since` set to the newest `created_at`
   it delivered. That number lives in memory. Without it, a restart asks
   every relay for everything again.
3. **What the application already showed.** Without the IDs, a restart
   shows the same events a second time.
4. **What was deleted.** A NIP-09 deletion request (kind 5) asks clients to
   hide the events it names. A relay that ignored the request still has
   them and sends them on the next `REQ`. Without the tombstone, the deleted
   event comes back.

The journal holds all four, plus the relay list that numbers the relays.

## The journal format

The journal starts with a 36-byte header:

| Bytes | Value |
| --- | --- |
| 0–2 | `NCJ` |
| 3 | the version, 1 |
| 4–35 | the owner's 32-byte public key |

Then come records. Each record is:

| Bytes | Value |
| --- | --- |
| 2 | n, the length of the type byte and the body, big-endian, 1 to 4096 |
| 2 | n xor 0xffff |
| 1 | the record type |
| n − 1 | the body |
| 4 | the first 4 bytes of SHA-256 over the type and the body |

A record takes 8 + n bytes. The six types:

| Type | Body | n |
| --- | --- | --- |
| 1 `RELAY` | the relay's URL | 1 + URL length |
| 2 `QUEUE` | the event ID (32), the relay set (8, bit r for relay r), the signed event's exact bytes | 41 + event length |
| 3 `OK` | the event ID (32), the relay number (1), the outcome (1: accepted 0, duplicate 1, refused 2) | 35 |
| 4 `EVENT` | the subscription (1), the relay (1), `created_at` (8), the ID (32), the author (32), the event's exact bytes | 75 + event length |
| 5 `CURSOR` | the subscription (1), the relay (1), `created_at` (8) | 11 |
| 6 `DELETE` | the deleted ID (32), the key that asked (32) | 65 |

For example, the first record of the chapter's journal numbers
`wss://r1.example` (16 bytes), so n = 17:

```jai
00 11 ff ee 01 77 73 73 3a 2f 2f 72 31 2e 65 78 61 6d 70 6c 65 58 08 e9 13
```

That is the length 0x0011, its complement 0xffee, type 1, the URL, and
`58 08 e9 13`, the first 4 bytes of SHA-256 over `01` and the URL.

The writers build the state by appending one record and then applying it
with `cj_apply`, the same procedure replay uses. So the state after a replay
is the state the writers left; `cj_same` compares the two, and every check
of the library replays its journal and compares.

A `QUEUE` record also puts your own event in the store, so a relay that
sends it back to you is a duplicate. A `CURSOR` record is written only when
a duplicate or a deleted event moves a cursor; a new event's `EVENT` record
moves it by itself.

Limits: 64 relays (the pool of chapter 32), 8 subscriptions, 16 publishes,
64 stored events and 32 tombstones. A full table or buffer is reported as
`CJ_FULL` and the record is not written. A kind 5 whose tombstones do not
all fit keeps the ones written, and a retry skips them. Compaction, which
would let the journal start over from a snapshot, is not built.

## Replay: cut or refuse

The host appends each record and syncs the file before it acts on the
result: it sends nothing and shows nothing before then. So a crash can
damage only the record being appended, and only in the ways an interrupted
append does: the file ends inside the record, or the file grew and the new
bytes are still zeros, or the last record's bytes are incomplete and its
check fails. Replay cuts that record off.

Anything else is damage to committed bytes, and skipping a committed record
can do harm: a skipped `DELETE` brings a deleted event back, and a skipped
`EVENT` delivers an event twice. Replay refuses such a journal and changes
nothing.

| What replay finds at a record | Result |
| --- | --- |
| The end of the file | `CJO_DONE` |
| Fewer than 9 bytes left (the smallest record) | cut |
| A length whose complement does not match, or out of 1 to 4096, and only zeros from there to the end | cut |
| The same, with any non-zero byte after it | `CJO_CORRUPT` |
| A record that runs past the end of the file | cut |
| A whole last record whose check fails | cut |
| A whole record whose check fails, with bytes after it | `CJO_CORRUPT` |
| A record that checks but that the writers never write (an unknown type, a second copy of an ID, an `OK` for nothing pending, a cursor that does not move, a stored event its author deleted) | `CJO_RECORD` |

A header that does not start `NCJ` is `CJO_MAGIC`; version 2 is
`CJO_VERSION`; another key is `CJO_OWNER`. A file shorter than 36 bytes
whose bytes match the start of the header is a torn header: replay writes
the header again (`CJO_CUT` with `at` 0). An empty file is `CJO_NEW`.

The result's `at` is how much of the file to keep. The host truncates the
file to `at` and writes `buf[at ..< len]`.

<!-- source: coax/lib/nostr/client_journal.coax open -->
```jai
// Replays the journal buf[0 ..< total] for owner: the state into j, and
// what to keep of the file. total 0 starts a new journal.
cj_open :: (j: [] Cj, buf: [] u8, total: int, owner: [32] u8) -> CjOpen {
  fresh: Cj; j[0] = fresh; j[0].owner = owner;
  if buf.count < CJ_HEADER || total < 0 || total > buf.count { return CjOpen.{status = CJO_SMALL}; }
  h := cj_header(buf, total, owner);
  if h != CJO_DONE { return CjOpen.{status = h}; }
  if total < CJ_HEADER {
    // New, or torn while the header was written: write it (again).
    for i: 0..<CJ_HEADER { buf[i] = cj_header_byte(owner, i); }
    o := cj_opened(j, ifx total == 0 then CJO_NEW else CJO_CUT, 0, 0);
    j[0].len = CJ_HEADER;
    return o;
  }
  p := CJ_HEADER; records := 0;
  while p < total {
    // Fewer bytes than the smallest record (a length, a type, a check):
    // a torn append, with nothing committed after it.
    if total - p < 9 { return cj_opened(j, CJO_CUT, p, records); }
    n := cast(int) cj_get(buf, p, 2); c := cast(int) cj_get(buf, p + 2, 2);
    if n + c != 0xffff || n < 1 || n > CJ_RECORD_MOST {
      // Not a length: a crash's zeros, or damage.
      if cj_zeros(buf, p, total) { return cj_opened(j, CJO_CUT, p, records); }
      return CjOpen.{status = CJO_CORRUPT, at = p, records = records};
    }
    end := p + 8 + n;
    if end > total { return cj_opened(j, CJO_CUT, p, records); }
    sum := cj_check(buf, p, n);
    if cj_get(sum[..], 0, 4) != cj_get(buf, p + 4 + n, 4) {
      // A last record that fails its check is a partial append; any other is damage.
      if end == total { return cj_opened(j, CJO_CUT, p, records); }
      return CjOpen.{status = CJO_CORRUPT, at = p, records = records};
    }
    if !cj_apply(j, buf, p + 4, n) { return CjOpen.{status = CJO_RECORD, at = p, records = records}; }
    records += 1; p = end;
  }
  return cj_opened(j, CJO_DONE, p, records);
}
```

The check finds damage. It is not a signature: anyone who can write the
journal file can write a journal that replays. The journal is local state,
like a key in the Keychain.

## Publishing across a crash

`cj_queue` journals your signed event before anything is sent. `cj_ok`
journals each relay's answer as it arrives. The order of the steps of one
publish, and what a crash after each leaves:

| Step | Durable after it | After a restart |
| --- | --- | --- |
| 1. `cj_queue` appends `QUEUE` | — (the append is not finished) | The application's call never returned, so it asks again; the relays have nothing |
| 2. the append is synced | the event, pending on r1 r2 r3 | sent to r1 r2 r3 |
| 3. EVENT sent to r1, r1 stores it | the same | sent to r1 again: `OK true "duplicate:"` |
| 4. r1's `OK` journaled | r1 acknowledged | sent to r2 r3 |

A relay that receives the same event twice keeps one copy and answers
`["OK", <id>, true, "duplicate: already have this event"]` (NIP-01). Both
answers mean the relay has the event, so both clear its pending bit. A
final refusal (`OK false` with a reason other than `auth-required:` or
`rate-limited:`) clears it too and is recorded as refused. A transient
refusal is not journaled: the publish stays pending.

`cj_queue` with an ID already journaled returns the same slot and writes
nothing. So the application can ask for every publish it did not see finish
without a second copy in the outbox.

<!-- source: coax/lib/nostr/client_journal.coax publish -->
```jai
// Journals your signed event (bytes, with this id) for the relays in the
// set, before anything is sent: its publish slot. The same id again
// returns the same slot and writes nothing, so an application that retries
// after a restart cannot publish twice.
cj_queue :: (j: [] Cj, buf: [] u8, id: [32] u8, relays: u64, bytes: string) -> int {
  if !j[0].open { return -CJ_SHUT; }
  k := cj_pub_of(j, id);
  if k >= 0 { return k; }
  if relays == 0 || (relays & cj_not(cj_known(j))) != 0 || bytes.count < 1 || bytes.count > CJ_RECORD_MOST - CJ_QUEUE_FIXED { return -CJ_ARG; }
  n := CJ_QUEUE_FIXED + bytes.count;
  if !cj_room(j, buf, n) { return -CJ_FULL; }
  b := cj_begin(j, buf, CJ_R_QUEUE, n);
  cj_put_id(buf, b, id); cj_put(buf, b + 32, relays, 8);
  for i: 0..<bytes.count { buf[b + 40 + i] = bytes[i]; }
  return cj_status(cj_commit(j, buf, n), j[0].pubs - 1);
}
// Journals relay r's OK for your event id: CJK_ACKED when the publish was
// pending there, CJK_UNKNOWN (nothing written) otherwise.
cj_ok :: (j: [] Cj, buf: [] u8, id: [32] u8, r: int, outcome: int) -> int {
  if !j[0].open { return -CJ_SHUT; }
  k := cj_pub_of(j, id);
  if k < 0 || r < 0 || r >= OB_RELAYS || (j[0].pub[k].pending & (cast(u64) 1 << r)) == 0 { return CJK_UNKNOWN; }
  if outcome < CJ_ACCEPTED || outcome > CJ_REFUSED { return -CJ_ARG; }
  if !cj_room(j, buf, 35) { return -CJ_FULL; }
  b := cj_begin(j, buf, CJ_R_OK, 35);
  cj_put_id(buf, b, id); buf[b + 32] = cast(u8) r; buf[b + 33] = cast(u8) outcome;
  return cj_status(cj_commit(j, buf, 35), CJK_ACKED);
}
```

## Delivering once and resuming with since

`cj_deliver` takes an event that `relay.coax` has already verified, with
the subscription and the relay it came from. It decides:

- **deleted** (`CJD_DELETED`), when a tombstone names this ID and this
  author
- **duplicate** (`CJD_DUP`), when the ID is in the store
- **new** (`CJD_NEW`) otherwise: the event is appended as an `EVENT` record,
  and the application reads it from the journal

The store's IDs are the dedup set, so nothing reaches the application
twice, across any number of restarts. For a duplicate or a deleted event,
the cursor still moves when the event is newer than it, with a `CURSOR`
record; otherwise a relay that sends the same old events after every
restart is asked for them again each time.

`cj_since(j, sub, r)` is the `since` for subscription `sub`'s next `REQ` to
relay `r`: the newest `created_at` delivered or seen there, or −1 for none.
As in `relay.coax`, `since` is inclusive, so the boundary event comes again
and is a duplicate.

<!-- source: coax/lib/nostr/client_journal.coax deliver -->
```jai
// An event relay r sent for subscription sub, already verified: its
// created_at, id, author and exact bytes, and for a kind 5 the ids its e
// tags name. CJD_NEW: journaled and delivered (the tombstones first).
// CJD_DUP: stored already. CJD_DELETED: its author asked to delete it.
// Either of the last two moves the cursor when the event is newer.
cj_deliver :: (j: [] Cj, buf: [] u8, sub: int, r: int, created_at: int, id: [32] u8, author: [32] u8, bytes: string, deletes: [] [32] u8) -> int {
  if !j[0].open { return -CJ_SHUT; }
  if !cj_sub_ok(sub) || !cj_relay_ok(j, r) || created_at < 0 || bytes.count < 1 || bytes.count > CJ_RECORD_MOST - CJ_EVENT_FIXED { return -CJ_ARG; }
  seen := ifx cj_tombed(j, id, author) then CJD_DELETED else ifx cj_find(j, id) >= 0 then CJD_DUP else CJD_NEW;
  if seen != CJD_NEW {
    if cj_since(j, sub, r) >= created_at { return seen; }
    if !cj_room(j, buf, 11) { return -CJ_FULL; }
    b := cj_begin(j, buf, CJ_R_CURSOR, 11);
    buf[b] = cast(u8) sub; buf[b + 1] = cast(u8) r; cj_put(buf, b + 2, cast(u64) created_at, 8);
    return cj_status(cj_commit(j, buf, 11), seen);
  }
  for target: deletes {
    if cj_tombed(j, target, author) { continue; }
    if !cj_room(j, buf, 65) { return -CJ_FULL; }
    b := cj_begin(j, buf, CJ_R_DELETE, 65);
    cj_put_id(buf, b, target); cj_put_id(buf, b + 32, author);
    if !cj_commit(j, buf, 65) { return -CJ_FULL; }
  }
  n := CJ_EVENT_FIXED + bytes.count;
  if !cj_room(j, buf, n) { return -CJ_FULL; }
  b := cj_begin(j, buf, CJ_R_EVENT, n);
  buf[b] = cast(u8) sub; buf[b + 1] = cast(u8) r; cj_put(buf, b + 2, cast(u64) created_at, 8);
  cj_put_id(buf, b + 10, id); cj_put_id(buf, b + 42, author);
  for i: 0..<bytes.count { buf[b + 74 + i] = bytes[i]; }
  return cj_status(cj_commit(j, buf, n), CJD_NEW);
}
```

## Deletions that stay deleted

NIP-09 says a deletion request is a kind 5 event whose `e` tags name the
events to delete, and that only events with the same `pubkey` as the
request are deleted. Clients should hide them. The caller passes the IDs a
kind 5's `e` tags name in `deletes`. `cj_deliver` writes one `DELETE` record
for each (ID, the request's author) before the kind 5's own `EVENT` record.

The order matters. If the process dies between a `DELETE` and the `EVENT`,
the kind 5 is not in the store, so the relay's next answer delivers it as
new; the tombstones are already there and are not written again. If the
order were reversed, a crash between them would store the kind 5 without
its tombstones, and the kind 5 would never be new again.

A tombstone applies to its signer's events only. `cj_live(j, k)` is false
for a stored event whose author asked to delete it, and `cj_deliver` refuses
the same ID from the same author from any relay at any later time. A kind 5
from another key that names alice's event creates a tombstone for that key,
which hides nothing of alice's.

## Worked example: a session killed mid-publish, by hand

The example signs four events with test keys. Alice (key 2) writes two
notes, a1 and a2, at t0+10 and t0+20, and at t0+25 a kind 5, k5, whose `e`
tag names a1. You (key 1) write N at t0+30. The relays start with:

| Relay | Holds |
| --- | --- |
| r1 | a1, a2, k5 |
| r2 | a2, k5 |
| r3 | a1, a2, k5 |

### The events' lengths

The signer writes NIP-01's object as
`{"id":"…","pubkey":"…","created_at":…,"kind":…,"tags":…,"content":"…","sig":"…"}`.
With a 64-digit ID and key, a 10-digit time and a 128-digit signature, the
fixed part is 7 + 64 + 12 + 64 + 15 + 10 + 8 + 8 + 12 + 9 + 128 + 2 = 339
bytes, plus the kind's digits, the tags and the content:

| Event | Kind | Tags | Content | Length |
| --- | --- | --- | --- | --- |
| a1 | 1 | `[]` (2) | `first note` (10) | 339 + 1 + 2 + 10 = 352 |
| a2 | 1 | `[]` (2) | `second note` (11) | 353 |
| k5 | 5 | `[["e","<64 hex>"]]` (7 + 64 + 3 = 74) | `posted by mistake` (17) | 431 |
| N | 1 | `[]` (2) | `hello from a client that restarts` (33) | 375 |

### The first session's journal

The session opens a new journal (36 bytes), numbers r1, r2 and r3, asks r1
for alice's events (r3 is down), then r2, then publishes N to all three. r1
answers `OK true`; r2 stores N and answers `OK true`, and the process dies
while that `OK` is being journaled. Each record is 8 + n bytes:

| Record | Type | What | n | Starts at |
| --- | --- | --- | --- | --- |
| 1–3 | `RELAY` | r1, r2, r3 | 17 | 36, 61, 86 |
| 4 | `EVENT` | a1 from r1: new | 75 + 352 | 111 |
| 5 | `EVENT` | a2 from r1: new | 75 + 353 | 546 |
| 6 | `DELETE` | a1, alice (k5's tag) | 65 | 982 |
| 7 | `EVENT` | k5 from r1: new | 75 + 431 | 1055 |
| 8 | `CURSOR` | a2 from r2 is a duplicate: r2's cursor to t0+20 | 11 | 1569 |
| 9 | `CURSOR` | k5 from r2 is a duplicate: r2's cursor to t0+25 | 11 | 1588 |
| 10 | `QUEUE` | N for r1 r2 r3 | 41 + 375 | 1607 |
| 11 | `OK` | r1 accepted | 35 | 2031 |
| 12 | `OK` | r2 accepted | 35 | 2074, ending at 2117 |

### The crash

The process dies when the file holds 2095 bytes: 21 of record 12's 43.
Replay reads records 1 to 11 and stops at 2074. There are 21 bytes left, so
the length is whole: 0x0023 (35) with complement 0xffdc. The record needs
2074 + 8 + 35 = 2117 bytes, and the file has 2095. It runs past the end, so
replay cuts the file to 2074 bytes, with 11 records.

The state after replay, from records 1 to 11:

- Store: a1, a2, k5, N. a1 is hidden: record 6 is alice's tombstone for it.
- Cursors for subscription 0: r1 at t0+25 (records 4, 5 and 7), r2 at
  t0+25 (record 9), r3 none.
- N: pending on r2 and r3 (record 10 set r1 r2 r3; record 11 cleared r1).

r2's `OK` is lost, but r2 has N.

### The restart

1. The application asks to publish N again. `cj_queue` finds N's ID in slot
   0 and writes nothing.
2. Subscription 0 resumes on each relay:
   - r1 since t0+25: r1 sends k5 (t0+25, the inclusive boundary). It is a
     duplicate and the cursor does not move: nothing written.
   - r2 since t0+25: the same.
   - r3 with no `since`: a1 is deleted (alice's tombstone), and its t0+10
     moves r3's cursor: a `CURSOR` record, 19 bytes, to 2093. a2 is a
     duplicate: cursor to t0+20, 2112. k5 is a duplicate: cursor to t0+25,
     2131.
3. N is sent where it is pending. r2 already has it and answers
   `OK true "duplicate: already have this event"`: an `OK` record with
   outcome 1, to 2174. r3 stores it and answers `OK true`: 2217.

At the end each relay holds one copy of N. r2 was sent N twice; r1 and r3
once. The application was shown a2, k5 and N once each, and a1 was hidden
before the crash and refused after it.

### A crash at every byte

The same reasoning gives the result for every length L the file can have
when the process dies, 0 to 2117:

- L = 0: a new journal.
- L from 1 to 35: a torn header, written again.
- L at a record boundary (36, 61, …, 2117): replayed whole.
- Anything else: cut back to the boundary before L.

That is 1 new, 13 whole and 2118 − 14 = 2104 cut.

How many times N is sent in all depends only on whether a relay had stored
N when the process died with its `OK` unjournaled. While record 11 is being
appended (L from 2031 to 2073, 43 lengths), r1 has N; while record 12 is
(L from 2074 to 2116, 43 lengths), r2 has it. In those 86 cases that relay
is sent N again, 4 sends in all. In the other 2032 cases N is sent 3 times:
before record 10 is whole, N was never sent, and the restart sends it to all
three; at L = 2117, r3 is the only one left.

## The example

[nostr/examples/restarts.coax](../examples/restarts.coax) signs the four
events, runs the first session against three scripted relays, kills it at
2095 bytes, replays, restarts, and then does the same for every L from 0 to
2117.

The scripted relays answer as a NIP-01 relay does:

<!-- source: nostr/examples/restarts.coax relays -->
```jai
// A scripted relay: which of the four events it holds, how many times it
// was sent your event, and whether it is reachable. It answers EVENT as a
// NIP-01 relay does: OK true, or OK true "duplicate:" when it already has
// the id, and never stores a second copy.
Scripted :: struct { holds: [4] bool; received: int; up: bool; }
relays_at_start :: () -> [3] Scripted {
  s: [3] Scripted;
  s[0].holds = [4] bool.[true, true, true, false]; s[1].holds = [4] bool.[false, true, true, false]; s[2].holds = [4] bool.[true, true, true, false];
  for r: 0..<3 { s[r].up = true; }
  return s;
}
// The relay's answer to your EVENT: CJ_ACCEPTED or CJ_DUPLICATE.
relay_event :: (s: [] Scripted, r: int) -> int {
  s[r].received += 1;
  if s[r].holds[MINE] { return CJ_DUPLICATE; }
  s[r].holds[MINE] = true;
  return CJ_ACCEPTED;
}
```

Subscriptions resume from the journal's cursors, and pending publishes are
sent and their `OK`s journaled:

<!-- source: nostr/examples/restarts.coax deliver -->
```jai
// Relay r answers subscription 0's REQ, with since= from the journal: every
// event it holds that alice wrote at or after since, oldest first. Each
// goes through cj_deliver; a kind 5 names the id its e tag carries.
resubscribe :: (s: [] Session, sr: [] Scripted, ev: [] Ev, r: int) {
  since := cj_since(s[0].j[..], 0, r);
  if s[0].loud {
    if since < 0 { print("  REQ to r% (no since)\n", r + 1); } else { print("  REQ to r% since t0+%\n", r + 1, since - T0); }
  }
  for i: 0..<3 {
    if !sr[r].holds[i] || ev[i].at < since { continue; }
    targets: [1] [32] u8; targets[0] = ev[A1].id; deletes := targets[0..<0];
    if i == K5 { deletes = targets[..]; }
    d := cj_deliver(s[0].j[..], s[0].buf[..], 0, r, ev[i].at, ev[i].id, ev[i].author, ev_text(ev, i), deletes);
    if died(s) { return; }
    if s[0].loud { print("    EVENT % (t0+%): %, journal % bytes\n", NAMES[i], ev[i].at - T0, DELIVERED[d], s[0].j[0].len); }
  }
}
```

<!-- source: nostr/examples/restarts.coax publish -->
```jai
// Sends your event to each relay where it is pending and journals each OK.
// The relay stores the event before its OK reaches the journal.
resend :: (s: [] Session, sr: [] Scripted, ev: [] Ev, slot: int) {
  for r: 0..<3 {
    if (s[0].j[0].pub[slot].pending & (cast(u64) 1 << r)) == 0 { continue; }
    if !sr[r].up { if s[0].loud { print("  r% is down: nothing sent\n", r + 1); } continue; }
    answer := relay_event(sr, r);
    k := cj_ok(s[0].j[..], s[0].buf[..], ev[MINE].id, r, answer);
    if died(s) { if s[0].loud { print("  EVENT N to r%: OK true%, then the process dies while journaling it\n", r + 1, ifx answer == CJ_DUPLICATE then " \"duplicate:\"" else ""); } return; }
    if s[0].loud { print("  EVENT N to r%: OK true%, journal % bytes\n", r + 1, ifx answer == CJ_DUPLICATE then " \"duplicate: already have this event\"" else "", s[0].j[0].len); }
  }
}
// Journals your event for r1 r2 r3 before anything is sent.
publish :: (s: [] Session, sr: [] Scripted, ev: [] Ev) {
  slot := cj_queue(s[0].j[..], s[0].buf[..], ev[MINE].id, 7, ev_text(ev, MINE));
  if died(s) { return; }
  if s[0].loud { print("  publish N to r1 r2 r3: slot %, journal % bytes\n", slot, s[0].j[0].len); }
  resend(s, sr, ev, slot);
}
```

The two sessions. The second asks for N again, resumes the subscription on
every relay and sends what is pending:

<!-- source: nostr/examples/restarts.coax sessions -->
```jai
// The first session, from a new journal: the relays, alice's events from
// r1 then r2 (r3 is down), then your note. It dies when the journal grows
// past s.crash.
first_session :: (s: [] Session, sr: [] Scripted, ev: [] Ev) {
  o := cj_open(s[0].j[..], s[0].buf[..], 0, you_key(ev));
  if died(s) { return; }
  for r: 0..<3 { n := cj_relay(s[0].j[..], s[0].buf[..], RELAYS[r]); if died(s) { return; } }
  if s[0].loud { print("  open: %; relays r1 r2 r3 numbered 0 1 2, journal % bytes\n", OPENED[o.status], s[0].j[0].len); }
  sr[2].up = false;
  resubscribe(s, sr, ev, 0); if s[0].dead { return; }
  resubscribe(s, sr, ev, 1); if s[0].dead { return; }
  publish(s, sr, ev);
}
// The second session, after a restart from the file's `total` bytes: the
// application asks again for every publish whose call never returned (the
// same id: cj_queue writes nothing when it is journaled), the subscription
// resumes on every relay from its cursor, and pending publishes are sent.
second_session :: (s: [] Session, sr: [] Scripted, ev: [] Ev, total: int) -> CjOpen {
  s[0].crash = 1 << 40; s[0].dead = false;
  for r: 0..<3 { sr[r].up = true; }
  o := cj_open(s[0].j[..], s[0].buf[..], total, you_key(ev));
  if o.status > CJO_NEW { return o; }
  if s[0].loud { print("  open: %, file kept to % bytes, % records\n", OPENED[o.status], o.at, o.records); }
  for r: 0..<3 { n := cj_relay(s[0].j[..], s[0].buf[..], RELAYS[r]); }
  slot := cj_queue(s[0].j[..], s[0].buf[..], ev[MINE].id, 7, ev_text(ev, MINE));
  if s[0].loud { print("  the application publishes N again: slot %, journal % bytes\n", slot, s[0].j[0].len); }
  for r: 0..<3 { resubscribe(s, sr, ev, r); }
  resend(s, sr, ev, slot);
  return o;
}
```

<!-- source: nostr/examples/restarts.coax example -->
```jai
main :: () {
  ev := sign_events();
  print("Four signed events\n");
  for i: 0..<4 {
    sb: [8] u8;
    print("  %: id %..., % bytes, t0+%, by %\n", NAMES[i], short_id(ev[i].id, sb[..]), ev[i].n, ev[i].at - T0, ifx i == MINE then "you" else "alice");
  }

  print("\nThe first session, killed while journaling r2's OK\n");
  s: [1] Session; whole_first(s[..], ev[..]);
  b := record_bounds(s[0].buf[..], s[0].j[0].len);
  for k: 0..<b.n { print("  record %: % at %, % bytes\n", k + 1, TYPES[b.types[k]], b.starts[k], b.starts[k + 1] - b.starts[k]); }
  last := b.starts[b.n - 1]; end := s[0].j[0].len;
  mid := (last + end) / 2;
  print("  the process dies when the journal reaches % bytes (record % ends at %)\n", mid, b.n, end);
  t: [1] Session; t[0].crash = mid; t[0].loud = true; tr := relays_at_start(); first_session(t[..], tr[..], ev[..]);
  o := cj_open(t[0].j[..], t[0].buf[..], mid, you_key(ev[..]));
  print("\nReplay of the % bytes: %, file kept to % bytes, % records\n", mid, OPENED[o.status], o.at, o.records);
  print_state(t[..], ev[..]);

  print("\nA crash at every byte of the last record\n");
  same := 0;
  for cut: last..<end + 1 {
    k: [1] Cj; c := cj_open(k[..], s[0].buf[..], cut, you_key(ev[..]));
    if cj_same(k[..], t[0].j[..]) && c.at == last && c.status == ifx cut == last then CJO_DONE else CJO_CUT { same += 1; }
  }
  k: [1] Cj; c := cj_open(k[..], s[0].buf[..], end, you_key(ev[..]));
  print("  cuts at % to %: % of % replay to the state above (cut at %; at % nothing to cut)\n", last + 1, end - 1, same - 1, end - last - 1, last, last);
  print("  the whole % bytes: %, publish N pending on", end, OPENED[c.status]);
  for r: 0..<3 { if (k[0].pub[0].pending & (cast(u64) 1 << r)) != 0 { print(" r%", r + 1); } }
  print("\n");

  print("\nThe restart from those % bytes\n", mid);
  out := crash_and_restart(ev[..], mid, true);
  print("  copies of N: r1 % r2 % r3 %; times sent: r1 % r2 % r3 %; reopened the same: %\n",
        out.copies[0], out.copies[1], out.copies[2], out.received[0], out.received[1], out.received[2], out.reopened);

  print("\nA crash at every byte of the first session's journal, 0 to %\n", end);
  runs := 0; good := 0; by: [3] int; sent: [8] int;
  for cut: 0..<end + 1 {
    r := crash_and_restart(ev[..], cut, false); runs += 1; by[r.opened] += 1;
    ok := r.reopened && r.pending == 0 && !r.live[A1] && r.live[A2] && r.live[K5] && r.live[MINE];
    for i: 0..<4 { ok = ok && r.stored[i] <= 1; }
    for x: 1..<4 { ok = ok && r.stored[x] == 1; }
    for q: 0..<3 { ok = ok && r.copies[q] == 1; }
    if ok { good += 1; }
    sent[r.received[0] + r.received[1] + r.received[2]] += 1;
  }
  print("  % restarts: % new, % cut, % replayed whole\n", runs, by[CJO_NEW], by[CJO_CUT], by[CJO_DONE]);
  print("  % of % end with N once on r1 r2 r3, nothing pending, a1 hidden, a2 k5 N shown, no event stored twice\n", good, runs);
  print("  N sent 3 times in % restarts, 4 times in % (none sent it more often: %)\n", sent[3], sent[4], runs - sent[3] - sent[4]);

  print("\nRefusals\n");
  refuse(s[..], ev[..]);
}
```

The refusals: a journal opened with alice's key, version 2, a bit flipped in
a committed record and in the last one, a damaged length, a file that is not
a journal, and two bad arguments:

<!-- source: nostr/examples/restarts.coax refuse -->
```jai
refuse :: (s: [] Session, ev: [] Ev) {
  end := s[0].j[0].len; k: [1] Cj;
  alice := ev[A1].author;
  o := cj_open(k[..], s[0].buf[..], end, alice);
  print("  your journal opened as alice's: %\n", OPENED[o.status]);
  c := s[0].buf; c[3] = 2; o = cj_open(k[..], c[..], end, you_key(ev));
  print("  version byte 2: %\n", OPENED[o.status]);
  b := record_bounds(s[0].buf[..], end);
  c = s[0].buf; c[b.starts[4] + 100] ^= 1; o = cj_open(k[..], c[..], end, you_key(ev));
  print("  one bit flipped inside record 5 (EVENT a2): % at %, % records before it\n", OPENED[o.status], o.at, o.records);
  last := b.starts[b.n - 1];
  c = s[0].buf; c[last + 20] ^= 1; o = cj_open(k[..], c[..], end, you_key(ev));
  print("  one bit flipped inside the last record: %, kept to % bytes\n", OPENED[o.status], o.at);
  c = s[0].buf; c[b.starts[1] + 1] = 0; o = cj_open(k[..], c[..], end, you_key(ev));
  print("  record 2's length changed to 0: % at %\n", OPENED[o.status], o.at);
  c = s[0].buf; c[0] = 0; o = cj_open(k[..], c[..], end, you_key(ev));
  print("  a file that does not start \"NCJ\": %\n", OPENED[o.status]);
  o = cj_open(k[..], s[0].buf[..], end, you_key(ev));
  none: [1] [32] u8;
  d := cj_deliver(k[..], s[0].buf[..], 0, 3, ev[A2].at, ev[A2].id, ev[A2].author, ev_text(ev, A2), none[0..<0]);
  print("  an event from relay 3 (r4, never added): %\n", ifx d == -CJ_ARG then "argument" else "?");
  d = cj_queue(k[..], s[0].buf[..], ev[A2].id, 8, ev_text(ev, A2));
  print("  a publish to relay set {3}: %\n", ifx d == -CJ_ARG then "argument" else "?");
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/restarts.coax --run-steps 250000000
```

Output:

```text
Four signed events
  a1: id 33f19dfb..., 352 bytes, t0+10, by alice
  a2: id a2c7833a..., 353 bytes, t0+20, by alice
  k5: id e3878191..., 431 bytes, t0+25, by alice
  N: id 7f77f19b..., 375 bytes, t0+30, by you

The first session, killed while journaling r2's OK
  record 1: RELAY at 36, 25 bytes
  record 2: RELAY at 61, 25 bytes
  record 3: RELAY at 86, 25 bytes
  record 4: EVENT at 111, 435 bytes
  record 5: EVENT at 546, 436 bytes
  record 6: DELETE at 982, 73 bytes
  record 7: EVENT at 1055, 514 bytes
  record 8: CURSOR at 1569, 19 bytes
  record 9: CURSOR at 1588, 19 bytes
  record 10: QUEUE at 1607, 424 bytes
  record 11: OK at 2031, 43 bytes
  record 12: OK at 2074, 43 bytes
  the process dies when the journal reaches 2095 bytes (record 12 ends at 2117)
  open: new; relays r1 r2 r3 numbered 0 1 2, journal 111 bytes
  REQ to r1 (no since)
    EVENT a1 (t0+10): new, journal 546 bytes
    EVENT a2 (t0+20): new, journal 982 bytes
    EVENT k5 (t0+25): new, journal 1569 bytes
  REQ to r2 (no since)
    EVENT a2 (t0+20): duplicate, journal 1588 bytes
    EVENT k5 (t0+25): duplicate, journal 1607 bytes
  publish N to r1 r2 r3: slot 0, journal 2031 bytes
  EVENT N to r1: OK true, journal 2074 bytes
  EVENT N to r2: OK true, then the process dies while journaling it

Replay of the 2095 bytes: cut, file kept to 2074 bytes, 11 records
  store: a1 (deleted) a2 k5 N
  cursors for subscription 0: r1 t0+25 r2 t0+25 r3 none
  publish N: pending r2 r3, acknowledged r1

A crash at every byte of the last record
  cuts at 2075 to 2116: 42 of 42 replay to the state above (cut at 2074; at 2074 nothing to cut)
  the whole 2117 bytes: replayed, publish N pending on r3

The restart from those 2095 bytes
  open: cut, file kept to 2074 bytes, 11 records
  the application publishes N again: slot 0, journal 2074 bytes
  REQ to r1 since t0+25
    EVENT k5 (t0+25): duplicate, journal 2074 bytes
  REQ to r2 since t0+25
    EVENT k5 (t0+25): duplicate, journal 2074 bytes
  REQ to r3 (no since)
    EVENT a1 (t0+10): deleted, journal 2093 bytes
    EVENT a2 (t0+20): duplicate, journal 2112 bytes
    EVENT k5 (t0+25): duplicate, journal 2131 bytes
  EVENT N to r2: OK true "duplicate: already have this event", journal 2174 bytes
  EVENT N to r3: OK true, journal 2217 bytes
  store: a1 (deleted) a2 k5 N
  cursors for subscription 0: r1 t0+25 r2 t0+25 r3 t0+25
  publish N: pending none, acknowledged r1 r2 r3
  copies of N: r1 1 r2 1 r3 1; times sent: r1 1 r2 2 r3 1; reopened the same: true

A crash at every byte of the first session's journal, 0 to 2117
  2118 restarts: 1 new, 2104 cut, 13 replayed whole
  2118 of 2118 end with N once on r1 r2 r3, nothing pending, a1 hidden, a2 k5 N shown, no event stored twice
  N sent 3 times in 2032 restarts, 4 times in 86 (none sent it more often: 0)

Refusals
  your journal opened as alice's: owner
  version byte 2: version
  one bit flipped inside record 5 (EVENT a2): corrupt at 546, 4 records before it
  one bit flipped inside the last record: cut, kept to 2074 bytes
  record 2's length changed to 0: corrupt at 61
  a file that does not start "NCJ": magic
  an event from relay 3 (r4, never added): argument
  a publish to relay set {3}: argument
```

Every value matches the hand tables: the event lengths 352, 353, 431 and
375, the twelve record offsets, the cut at 2074 with 11 records, the cursors
and pending relays after replay, the restart's journal lengths 2093, 2112,
2131, 2174 and 2217, the 1, 2104 and 13 opens, and the 86 restarts that send
N a fourth time. A bit flipped in record 5 refuses the journal at 546 with 4
records before it; the same flip in the last record is cut.

## What is not built

- Partitions. Events from every relay are merged by ID; the journal does
  not record which relays disagree about an event or a deletion.
- `a`-tag deletions (NIP-09's addressable form) and NIP-62's requests to
  vanish. The relay's store (chapter 35) has both; this client honors `e`
  tags only.
- Compaction. The journal only grows, and the tables have fixed sizes; a
  client would write a snapshot and start a new journal.
- Re-verifying signatures on replay. Records are checked for damage, not for
  forgery; the events were verified when they arrived.
- The file itself. The host appends, syncs and truncates; this chapter's
  journal is a byte buffer, as in chapter 35's example.
- Addressable and replaceable versions. Every event is stored by ID; chapter
  22's newest-version rule is not applied to the store.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/restarts.coax](../examples/checks/restarts.coax),
which loads the example. They compare the code with values from outside it:

- the signers against the published x coordinates of 1G and 2G
- the event lengths against the NIP-01 count by hand, and the record
  offsets against the hand table
- the open result for a cut at every record boundary and the bytes beside
  it, and at every byte of the last two records, against the hand rule
- the state after the crash at 2095 and the second session's five records
  against the hand trace
- the number of times N is sent against the rule "4 exactly when L is from
  2031 to 2116", at 14 lengths

<!-- source: nostr/examples/checks/restarts.coax check -->
```jai
// The x coordinates of 1G and 2G on secp256k1, as published (SEC 2 gives
// G; 2G is its well-known double). Key 1 is you and key 2 is alice.
G1 :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
G2 :: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
#test "the signers are 1G and 2G" #steps 2000000000 {
  ev := sign_events(); g1: [32] u8; g2: [32] u8;
  a := hex_decode_checked(G1, g1[..], false); b := hex_decode_checked(G2, g2[..], false);
  #assert cj_eq(ev[MINE].author, g1) && cj_eq(ev[A1].author, g2) && cj_eq(ev[K5].author, g2);
}

// NIP-01's serialization of a signed event, counted by hand: 339 fixed
// bytes ({"id":"…","pubkey":"…","created_at":…,"kind":,"tags":,
// "content":"…","sig":"…"} with a 64-digit id and key, a 10-digit time and
// a 128-digit signature), plus the kind's digits, the tags and the content.
HAND_LENGTHS :: [4] int.[352, 353, 431, 375];
// Where each record of the first session starts, from the hand table:
// RELAY 8 + 1 + 16, EVENT 8 + 75 + n, DELETE 8 + 65, CURSOR 8 + 11,
// QUEUE 8 + 41 + n, OK 8 + 35.
HAND_STARTS :: [13] int.[36, 61, 86, 111, 546, 982, 1055, 1569, 1588, 1607, 2031, 2074, 2117];
#test "event lengths and record offsets match the hand table" #steps 2000000000 {
  ev := sign_events();
  contents := [4] int.[10, 11, 17, 33]; tags := [4] int.[2, 2, 74, 2];
  for i: 0..<4 { #assert ev[i].n == HAND_LENGTHS[i] && ev[i].n == 339 + 1 + tags[i] + contents[i]; }
  sizes := [12] int.[25, 25, 25, 83 + 352, 83 + 353, 73, 83 + 431, 19, 19, 49 + 375, 43, 43];
  at := 36; for k: 0..<12 { #assert HAND_STARTS[k] == at; at += sizes[k]; } #assert at == HAND_STARTS[12];
  s: [1] Session; whole_first(s[..], ev[..]);
  b := record_bounds(s[0].buf[..], s[0].j[0].len);
  #assert b.n == 12 && s[0].j[0].len == 2117;
  for k: 0..<13 { #assert b.starts[k] == HAND_STARTS[k]; }
}

// The hand rule for a file cut to L bytes: 0 is a new journal, 1 to 35 a
// torn header, a record boundary replays whole, and anything else is cut
// back to the last boundary at or before L.
hand_open :: (cut: int) -> CjOpen {
  if cut == 0 { return CjOpen.{status = CJO_NEW}; }
  if cut < 36 { return CjOpen.{status = CJO_CUT}; }
  k := 0; while k < 12 && HAND_STARTS[k + 1] <= cut { k += 1; }
  return CjOpen.{status = ifx HAND_STARTS[k] == cut then CJO_DONE else CJO_CUT, at = HAND_STARTS[k], records = k};
}
#test "cuts open as the hand rule says" #steps 2000000000 {
  ev := sign_events(); s: [1] Session; whole_first(s[..], ev[..]);
  for cut: 0..<2118 {
    // Every boundary and the bytes beside it, and every byte of the last two records.
    near := false; for k: 0..<13 { if cut >= HAND_STARTS[k] - 1 && cut <= HAND_STARTS[k] + 1 { near = true; } }
    if cut < 38 { near = true; }
    if !near && cut < HAND_STARTS[10] { continue; }
    j: [1] Cj; o := cj_open(j[..], s[0].buf[..], cut, you_key(ev[..])); h := hand_open(cut);
    #assert o.status == h.status && o.at == h.at && o.records == h.records;
  }
}

// After the chapter's crash at 2095 bytes: the hand trace's state, and the
// second session's five records (three cursors on r3, r2's duplicate OK,
// r3's OK) ending at 2217.
#test "the restart after the crash at 2095 matches the hand trace" #steps 2000000000 {
  ev := sign_events();
  t: [1] Session; t[0].crash = 2095; tr := relays_at_start(); first_session(t[..], tr[..], ev[..]);
  o := cj_open(t[0].j[..], t[0].buf[..], 2095, you_key(ev[..]));
  #assert o.status == CJO_CUT && o.at == 2074 && o.records == 11;
  j := t[0].j[..];
  #assert cj_since(j, 0, 0) == T0 + 25 && cj_since(j, 0, 1) == T0 + 25 && cj_since(j, 0, 2) == -1;
  #assert j[0].pub[0].pending == 6 && j[0].pub[0].accepted == 1 && j[0].events == 4 && j[0].tombs == 1;
  #assert !cj_live(j, 0) && cj_live(j, 1) && cj_live(j, 2) && cj_live(j, 3);
  r := crash_and_restart(ev[..], 2095, false);
  #assert r.opened == CJO_CUT && r.at == 2074 && r.reopened && r.pending == 0;
  #assert r.received[0] == 1 && r.received[1] == 2 && r.received[2] == 1;
  for q: 0..<3 { #assert r.copies[q] == 1; }
  s: [1] Session; s[0].crash = 2095; sr := relays_at_start(); first_session(s[..], sr[..], ev[..]);
  x := second_session(s[..], sr[..], ev[..], 2095);
  b := record_bounds(s[0].buf[..], s[0].j[0].len);
  types := [5] int.[CJ_R_CURSOR, CJ_R_CURSOR, CJ_R_CURSOR, CJ_R_OK, CJ_R_OK];
  #assert b.n == 16 && s[0].j[0].len == 2217;
  for k: 0..<5 { #assert b.types[11 + k] == types[k]; }
}

// N reaches a relay twice only when the crash cuts the OK of a relay that
// already stored it: L from 2031 (r1's OK) to 2116 (inside r2's OK).
#test "N is sent a fourth time only after a cut OK" #steps 2000000000 {
  ev := sign_events();
  cuts := [14] int.[0, 35, 36, 1606, 1607, 1608, 2030, 2031, 2052, 2073, 2074, 2095, 2116, 2117];
  for cut: cuts {
    r := crash_and_restart(ev[..], cut, false);
    want := ifx cut >= 2031 && cut < 2117 then 4 else 3;
    #assert r.received[0] + r.received[1] + r.received[2] == want && r.pending == 0 && r.reopened;
    #assert !r.live[A1] && r.live[A2] && r.live[K5] && r.live[MINE] && r.stored[A2] == 1 && r.stored[K5] == 1 && r.stored[MINE] == 1;
  }
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/restarts.coax --run-steps 250000000
```

On 2026-11-01 this reported 406 of 406 checks passed, including the
checks of every file the example loads. Root-only mutation of the checks
file: 22 mutants, 22 killed, after the boundary loop was widened to cover
cuts at 0 and 1.

### The library's own checks

[client_journal.coax](../../coax/lib/nostr/client_journal.coax) checks a new
journal and its header byte for byte, each header refusal, a torn header at
every length, the relay pool to 64 relays, the outbox's pending, accepted
and refused bits, the bounds on bytes and on every table to the byte,
delivery, cursors and tombstones (including another key's kind 5), a cut at
every byte of the last record, a flipped bit at every byte of every record,
zero-filled tails, every kind of record the writers never write, the check
against SHA-256 computed apart, and that a replayed journal is the same
state as the one the writers left.

The file was written test first: 252 of 252 checks passed on 2026-11-01,
with the checks of the files it loads. Root-only mutation on 2026-11-01:
769 mutants, 769 killed, none marked equivalent. Earlier runs found
survivors that changed the code: the cursor record must move a cursor (a
cursor that only keeps the larger value has an unobservable comparison), a
tail shorter than the smallest record (9 bytes) is cut before its length is
read, buffer room is a separate test from writing a record's header, and
`cj_same` is checked field by field. Mutation of the loaded files
(`outbox.coax`, `nip65.coax`, `sha256.coax`) was not rerun: none of them
changed.

## Summary

- One append-only journal holds the relays, the outbox, the store, the
  cursors and the tombstones; every writer appends and then applies the
  same procedure replay uses.
- Journal before acting: a publish before it is sent, an event before it is
  shown, a kind 5's tombstones before the kind 5. A restart then resends the
  same bytes, relays answer `duplicate:`, and nothing is delivered twice.
- Subscriptions resume with `since` from the journal's cursors; a duplicate
  or deleted event still moves the cursor.
- Replay cuts a crash's partial last record and refuses committed damage, a
  journal of another version and a journal of another key.

## Dependencies

- [32. Many relays and the outbox
  model](ch032-many-relays-and-the-outbox-model.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Sources

- [NIP-01](../nips/official/01.md)
- [NIP-09](../nips/official/09.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
