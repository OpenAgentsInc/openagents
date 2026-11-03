# 39. Delete, expire and vanish

Authors can ask relays to forget their events. This chapter adds deletion
and vanishing to chapter 35's store, next to the expiration it already
enforces.

Part V, A relay. Result: NIP-40 `expiration` tags enforced at admission and
query, NIP-09 kind-5 deletion requests from the same author with tombstones
that survive a restart, and NIP-62 kind-62 requests to vanish.

## What this chapter teaches

- NIP-40: an event with `["expiration", ts]` is refused at or after `ts`,
  and a stored one stops being returned at `ts`.
- NIP-09: a kind 5 request's `e` tags delete only events by the request's
  own pubkey; its `a` tags delete every version of an address up to the
  request's `created_at`. A deletion request never deletes another one.
- Tombstones: each request becomes tombstones that retire the stored events
  they cover and refuse those events if they are sent again. The tombstones
  are rebuilt from the stored requests when the file is replayed, so a
  deleted event can't come back after a restart.
- NIP-62: a kind 62 request that names this relay (or `ALL_RELAYS`) removes
  every event by its pubkey up to its `created_at` and keeps them out.
- The relay (`coax/relay/relay.coax`) admits every event through this code
  (`rsd_admit_deleting` in `store_saved.coax`, called by `relay_store.coax`)
  and rebuilds the tombstones on replay (`rsd_replay_deleting`). A covered
  event gets OK false "blocked: a deletion request covers this event".
- What is not built: the bytes of a deleted event stay in the append-only
  file, and NIP-62's removal of gift wraps addressed to the pubkey.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [store_policy.coax](../../coax/lib/nostr/store_policy.coax) | `ns_expired`, `ns_delete_id`, `ns_delete_address` | NIP-40's expiry test and NIP-09's two rules: same ID and author, never a kind 5; same address and author, no newer than the request (`#authority`) |
| [relay_server.coax](../../coax/lib/nostr/relay_server.coax) | `rs_expiration`, `rs_live`, `rs_admit_policy` | reads the `expiration` tag and refuses an expired event at admission (`invalid: event has expired`) |
| [store_core.coax](../../coax/lib/nostr/store_core.coax) | `ns_store_visible`, `ns_store_compact`, `ns_store_retire` | hides an expired record from queries, drops expired regular events at compaction, and retires a record |
| [store_delete.coax](../../coax/lib/nostr/store_delete.coax) | `nsd_read`, `nsd_covers`, `nsd_apply`, `nsd_plan`, `nsd_apply_plan`, `nsd_admit` | new in this chapter: reads a kind 5 or kind 62 request into tombstones, decides what each covers, retires it, and refuses covered events at admission (`#authority`) |

`store_delete.coax` is new. Its checks were written first, and `coax mutate
--root-only` on it kills 267 of 267 mutants.

## Expiration

NIP-40 adds one tag, `["expiration", "<unix time>"]`. The relay reads it
once, at admission, with `rs_expiration`: the first `expiration` tag's
value as a plain decimal of at most 16 digits, or -1 (never expires) when
there is none or it isn't one. The time is checked in three places:

1. Admission. `rs_admit_policy` refuses an event whose expiration is at or
   before the relay's clock, with `invalid: event has expired` (reason 9).
   This happens before the signature check.
2. Query. `ns_store_visible` serves a record only while
   `ns_expired(expiry, now)` is false, that is while `now < expiry`. At the
   exact second of the expiration the event is gone.
3. Compaction. When the store needs room it drops expired regular events.
   Expired replaceable and addressable heads are kept, so an older version
   at the same address can't come back in their place.

Replay is the exception: an event that expired after it was saved is
admitted again (`ns_file_replay_reason` turns reason 9 into "accept"), and
then hidden by the query rule. Refusing it would make the file look
damaged.

## Deletion requests

A NIP-09 request is a kind 5 event. Each `e` tag names an event ID; each
`a` tag names an address, `<kind>:<pubkey>:<d>`. The relay can't check an
`e` tag's target before it has seen it, so the rule is applied to the
target: an event is deleted only when its pubkey is the request's pubkey.
An `a` tag also carries a pubkey, and it must equal the request's too.

`nsd_read` turns a stored request into tombstones, one per usable tag. An
`e` value must be 64 lowercase hex digits. An `a` value must be 1 to 9
decimal digits, a colon, 64 lowercase hex digits, a colon and a `d` of at
most 256 bytes. Other tags (`k`, `p`) and unusable values are skipped. Each
tombstone records the request's pubkey as the signer and, for `a`, the
request's `created_at` as `until`.

`nsd_covers` decides whether tombstone `k` covers a record. It reuses
`store_policy.coax`'s two NIP-09 rules:

<!-- source: coax/lib/nostr/store_delete.coax covers -->
```jai
nsd_covers :: (tb: [] NsdTombs, k: int, c: RsdRec) -> bool {
  t := tb[0].t[k]; r := c;
  if t.what == NSD_VANISH { return r.kind != 62 && ns_equal32(r.author, t.signer) && r.created <= t.until; }
  m := NsMeta.{id = r.id, author = r.author, kind = r.kind, created = r.created, identifier = cast(string) r.d[0..<r.d_n]};
  if t.what == NSD_E { return r.kind != 62 && ns_delete_id(m, t.signer, t.id); }
  q := NsMeta.{author = t.author, kind = t.kind, created = t.until, identifier = cast(string) t.d[0..<t.d_n]};
  return ns_delete_address(m, q, t.signer);
}
// Whether any tombstone covers c. A tombstone that covers c has c's id (e),
// c's address (a) or c's author (vanish) as its key, so with the lookup
// only those three chains are read.
nsd_blocked :: (tb: [] NsdTombs, c: RsdRec) -> bool {
  if nsd_indexed(tb) {
    return nsd_chain(tb, ns_id_hash(c.id), c) || nsd_chain(tb, ns_address_hash(c.kind, c.author, c.d, c.d_n), c) || nsd_chain(tb, ns_id_hash(c.author), c);
  }
  for k: 0..<tb[0].n { if nsd_covers(tb, k, c) { return true; } }
  return false;
}
// Whether tb has the lookup (longer than t, so a free slot remains).
nsd_indexed :: (tb: [] NsdTombs) -> bool { return tb[0].slots.count > tb[0].t.count; }
// A tombstone's key: what any event it covers shares with it.
nsd_key :: (t: NsdTomb) -> int {
  if t.what == NSD_E { return ns_id_hash(t.id); }
  if t.what == NSD_A { return ns_address_hash(t.kind, t.author, t.d, t.d_n); }
  return ns_id_hash(t.signer);
}
// Whether a tombstone in the chain from h's slot to the next empty one covers c.
nsd_chain :: (tb: [] NsdTombs, h: int, c: RsdRec) -> bool {
  at := h % tb[0].slots.count; tb[0].probes += 1;
  while tb[0].slots[at] != 0 {
    if nsd_covers(tb, tb[0].slots[at] - 1, c) { return true; }
    at = (at + 1) % tb[0].slots.count; tb[0].probes += 1;
  }
  return false;
}
```

- An `e` tombstone covers the record with that ID when the record's author
  is the signer, and never a kind 5 (`ns_delete_id`) or a kind 62.
- An `a` tombstone covers a record at the same address (kind and author,
  and `d` for addressable kinds; `ns_same_key`) whose `created_at` is at
  most `until`, and only when the address's author is the signer
  (`ns_delete_address`). Regular kinds have no address, so an `a` tag never
  covers them.

`nsd_apply` adds one tombstone and retires every live record it covers:

<!-- source: coax/lib/nostr/store_delete.coax apply -->
```jai
// Adds t and retires the live records it covers: how many, or -1 (nothing
// changed) when the table is full. With the store's lookups an e or a tag
// reads the one record that can be live with its id or at its address; a
// vanish reads every record.
nsd_apply :: (s: [] RsdStore, tb: [] NsdTombs, t: NsdTomb) -> int {
  if tb[0].n >= tb[0].t.count { return -1; }
  k := tb[0].n; tb[0].t[k] = t; tb[0].n += 1;
  if nsd_indexed(tb) {
    at := nsd_key(t) % tb[0].slots.count;
    while tb[0].slots[at] != 0 { at = (at + 1) % tb[0].slots.count; }
    tb[0].slots[at] = k + 1;
  }
  gone := 0;
  if ns_indexed(s) && t.what != NSD_VANISH {
    q: RsdRec; q.kind = t.kind; q.author = t.author; q.d = t.d; q.d_n = t.d_n;
    i := ifx t.what == NSD_E then ns_find_id(s, t.id) else ns_find_address(s, q);
    if i >= 0 && s[0].recs[i].live && nsd_covers(tb, k, s[0].recs[i]) { ns_store_retire(s, i); gone = 1; }
    return gone;
  }
  for i: 0..<s[0].n {
    if s[0].recs[i].live && nsd_covers(tb, k, s[0].recs[i]) { ns_store_retire(s, i); gone += 1; }
  }
  return gone;
}
```

A retired record is not served. Its bytes stay in the arena until the next
compaction and in the file for good.

### Lookups instead of scans

Read as written, `nsd_blocked` reads every tombstone for every event, and
`nsd_apply` reads every record for every tombstone. That is fine for a
chapter's dozen events and too slow for a relay's hundreds of thousands
(issue #12631: an import of 60,000 events spent 203 of its 246 seconds
there and in `ns_store_plan`'s own scan). Both scans have a lookup beside
them that gives the same answers.

A tombstone that covers an event shares one thing with it: an `e`
tombstone the event's ID, an `a` tombstone its address, a vanish its
author (the signer). `nsd_key` hashes that thing, and `nsd_apply` puts the
tombstone in `slots`, an open-addressed table, at the first free slot from
its key. `nsd_blocked` then reads only three chains: from the slots that
the event's ID, address and author hash to, up to the next empty slot, and
asks `nsd_covers` about each tombstone it meets. The hash only chooses
where to look; `nsd_covers` still decides, so a collision costs a look and
never an answer. Nothing is removed from the table, so a chain is never
broken.

`nsd_apply` uses the store's own lookups (chapter 35's `store_core.coax`
keeps the latest record for each ID and each address beside the records):
only the latest record with an ID, or at an address, can be live, so an
`e` or `a` tombstone reads that one record. A vanish still reads every
record, once per request. The relay sizes the tombstone table from its
input (the export, or the `--store` file) plus room for new requests, so
the earlier fixed 8,192 is gone. Without the tables (empty slices) both
functions scan as before, and the checks run the same random sessions both
ways and compare every answer.

## Requests to vanish

A NIP-62 request is a kind 62 event with at least one `relay` tag. When one
of the tags is this relay's service URL, or `ALL_RELAYS` exactly in
uppercase, `nsd_read` returns one tombstone of type `NSD_VANISH`. It covers
every record by the signer with `created_at` at most the request's,
whatever the kind, including the signer's kind 5 requests, but not kind 62
requests. A request that names only other relays is stored and has no
effect here. Because kind 5 never covers kind 62, a deletion request can't
undo a request to vanish.

NIP-62 also asks relays to delete gift wraps (kind 1059) that `p`-tag the
pubkey. That is not built: those events are signed by random keys, and the
store doesn't index `p` tags.

## Admission and restart

Admission puts the tombstones in front of the store and applies a request
after it is stored:

<!-- source: coax/lib/nostr/store_delete.coax admit -->
```jai
// One event's admission with deletion and vanish: refuse (NSD_DELETED) what
// a tombstone covers, refuse (NSD_NO_ROOM) a request whose tombstones don't
// fit (out is the caller's room to read them into), else store it as
// ns_store_admit does and, when it was stored, apply its tombstones.
nsd_admit :: (s: [] RsdStore, tb: [] NsdTombs, c: RsdRec, text: string, verified: bool, relay: string, out: [] NsdTomb) -> int {
  p := nsd_plan(s, tb, c, text, verified, relay, out);
  return nsd_apply_plan(s, tb, p, c, text, out);
}
// nsd_admit split as ns_store_plan and ns_store_apply are, so a durable file
// can record the event between them. targets is how many of out the plan
// read (the request's tombstones).
NsdPlan :: struct { store: NsPlan; targets: int; }
nsd_plan :: (s: [] RsdStore, tb: [] NsdTombs, c: RsdRec, text: string, verified: bool, relay: string, out: [] NsdTomb) -> NsdPlan {
  if !verified { return NsdPlan.{store = NsPlan.{result = NSS_UNVERIFIED}}; }
  if nsd_blocked(tb, c) { return NsdPlan.{store = NsPlan.{result = NSD_DELETED}}; }
  n := nsd_read(text, c, relay, out);
  if n < 0 || tb[0].n + n > tb[0].t.count { return NsdPlan.{store = NsPlan.{result = NSD_NO_ROOM}}; }
  return NsdPlan.{store = ns_store_plan(s, c, text, true), targets = n};
}
nsd_apply_plan :: (s: [] RsdStore, tb: [] NsdTombs, p: NsdPlan, c: RsdRec, text: string, out: [] NsdTomb) -> int {
  r := ns_store_apply(s, p.store, c, text);
  if r == NSS_ACCEPT { tb[0].last = 0; for k: 0..<p.targets { tb[0].last += nsd_apply(s, tb, out[k]); } }
  return r;
}
```

1. An unverified event is refused as in chapter 35.
2. An event that a tombstone covers is refused with `NSD_DELETED`, whatever
   bytes it arrives as. The relay's reason text for it is not written yet;
   NIP-01 would send it as `OK false` with a `blocked:` prefix.
3. A request whose tombstones don't fit in the table is refused with
   `NSD_NO_ROOM` before anything is stored.
4. Otherwise the store decides as before. Only when it answers "accepted"
   are the request's tombstones applied. A duplicate request adds nothing.

The tombstones have no file of their own. The requests are ordinary
regular events, so the store writes them to its file. On a restart, replay
sends each frame through the same admission in file order. A deleted
event's frame comes before its request's frame, so it is stored, and then
retired again when the request's frame is replayed. A deleted event can't
appear after its request in the file, because admission refused it. The
same holds for a version of an address newer than an `a` tag's `until` and
for events after a vanish's `created_at`: they are not covered.

## Worked example: deletion by hand

Alice has the BIP-340 vector 1 key (`dff1d77f...`), Bob the vector 2 key
(`dd308afe...`). NOW is 1793491200. IDs are shown by their first 4 bytes,
from `coax/tests/nostr/nip59_reference.py`.

The store holds four events:

| Event | Author | Kind | `created_at` | ID |
| --- | --- | --- | --- | --- |
| a1 | Alice | 1 | NOW-300 | `7a83b645` |
| a2 | Alice | 1 | NOW-200 | `d7995cd8` |
| b1 | Bob | 1 | NOW-250 | `6125f8fe` |
| post@-400 | Alice | 30023, `d` = `post` | NOW-400 | `7138c561` |

Alice sends D1 (`361b38a4`), kind 5 at NOW-100, with tags
`["e", a1]`, `["e", b1]`, `["a", "30023:dff1d77f...:post"]`, `["k", "1"]`
and `["k", "30023"]`. Work out each tag:

1. `e` a1: a tombstone (signer Alice, ID `7a83b645...`). a1's author is
   Alice, so a1 is retired.
2. `e` b1: a tombstone (signer Alice, ID `6125f8fe...`). b1's author is
   Bob, not the signer, so b1 stays. The tombstone can never cover b1:
   b1's ID fixes its author.
3. `a`: the address's pubkey is Alice's, the signer's. A tombstone (30023,
   Alice, `post`, until NOW-100). post@-400 is at that address and
   NOW-400 ≤ NOW-100, so it is retired.
4. `k`, `k`: not tombstones.

D1 makes 3 tombstones and retires 2 records; the store keeps a2, b1 and
D1. Then:

| Step | Event | Covered by | Result |
| --- | --- | --- | --- |
| 6 | post@-150, Alice's 30023 `post` at NOW-150 | the `a` tombstone: NOW-150 ≤ NOW-100 | refused, deleted |
| 7 | post@-50 at NOW-50 | nothing: NOW-50 > NOW-100 | accepted |
| 8 | a1 again | the first `e` tombstone | refused, deleted |
| 9 | DB, Bob's kind 5 at NOW-40: `e` a2, `a` Alice's `post` | | accepted; 2 tombstones, 0 retired |
| 10 | D2, Alice's kind 5 at NOW-30: `e` D1 | | accepted; 1 tombstone, 0 retired |

In step 9, Bob's `e` tombstone has signer Bob, and a2's author is Alice: a2
stays. Bob's `a` tag names Alice's address, so `ns_delete_address` fails on
the signer: post@-50, at NOW-50 and so older than DB's NOW-40, stays. In
step 10, D1 is a kind 5, which `ns_delete_id` never deletes.

Then the expiration steps, at NOW:

| Step | Event | Result |
| --- | --- | --- |
| 11 | x60, expiration NOW+60 | accepted; served at NOW and NOW+59, not at NOW+60 |
| 12 | x-10, expiration NOW-10 | refused at admission, reason 9, expired |

After step 12 the store holds 9 records, 7 of them live, and 6 tombstones.
A query with an empty filter at NOW returns the 7 live events newest first:
x60, D2, DB, post@-50, D1, a2, b1.

On a restart the file has 9 frames: a1, a2, b1, post@-400, D1, post@-50,
DB, D2, x60. Replay stores a1 and post@-400 and retires them again at D1's
frame. x60 is kept, though it has expired. So the replayed store has the
same 7 live records and 6 tombstones, and a1 and post@-150 are still
refused.

The vanish steps, on the first store at NOW:

| Step | Event | Result |
| --- | --- | --- |
| 15 | V-elsewhere, Bob's kind 62 at NOW-10, `relay` `wss://elsewhere.example` | accepted, no tombstone |
| 16 | V, Bob's kind 62 at NOW, `relay` `wss://relay.example.com` | accepted; 1 tombstone; retires b1 and DB (Bob's, at or before NOW); V-elsewhere stays (kind 62) |
| 17 | b1 again | refused, deleted |
| 18 | DB again | refused, deleted |
| 19 | DV, Bob's kind 5 at NOW+1: `e` V | accepted; V stays |
| 20 | b2, Bob's kind 1 at NOW+2 | accepted: after V's `created_at` |

## The example

The example is
[nostr/examples/delete-expire-vanish.coax](../examples/delete-expire-vanish.coax).
It builds on chapter 35's example: the store's file is a byte buffer, and
`coax run` doesn't link the host's file calls. It:

1. signs 15 events with the two test keys and prints D1;
2. sends steps 1 to 12 through chapter 34's read, policy and signature
   steps, then the tombstones and the store, at NOW, and prints the result,
   the tombstones and retired records, and the store after the requests;
3. queries with an empty filter at NOW, NOW+59 and NOW+60;
4. replays the file into an empty store at NOW+60 and sends a1 and
   post@-150 again;
5. sends steps 15 to 20 to the first store.

Each EVENT goes through `admit_at`: decide, append the frame when the plan
writes, then apply. Replay goes through the same procedure:

<!-- source: nostr/examples/delete-expire-vanish.coax admit -->
```jai
// One EVENT as the relay would take it: read and policy at the clock now,
// the signature, then the tombstones and the store. The plan is decided,
// the frame appended (only when the plan writes), then the plan applied.
save :: (st: [] Store, text: string) -> bool {
  head: [4] u8;
  if !ns_file_header(text.count, head[..]) || st[0].total + 4 + text.count > st[0].file.count { return false; }
  for i: 0..<4 { st[0].file[st[0].total + i] = head[i]; }
  for i: 0..<text.count { st[0].file[st[0].total + 4 + i] = text[i]; }
  st[0].total += 4 + text.count;
  return true;
}
admit_at :: (st: [] Store, text: string, now: int, replaying: bool) -> int {
  r := rs_admit_read(text, now, false, false, st[0].scratch[..]);
  if replaying { r.reason = ns_file_replay_reason(r.reason); }
  if r.reason == RS_ACCEPT { r = rs_admit_verified(r, nostr_event_verify_vartime(text, st[0].scratch[..], false)); }
  if r.reason != RS_ACCEPT { return r.reason; }
  c := rsd_rec(r);
  p := nsd_plan(st[0].s[..], st[0].tb[..], c, text, rsd_verified(r, text), RELAY, st[0].work[..]);
  if !replaying && p.store.write != NS_W_NONE && !save(st, text) { return RS_FULL; }
  return nsd_apply_plan(st[0].s[..], st[0].tb[..], p, c, text, st[0].work[..]);
}
```

<!-- source: nostr/examples/delete-expire-vanish.coax replay -->
```jai
// Restart: chapter 35's replay, each frame through the same admission
// (an event expired since it was saved is kept), so the stored kind 5 and
// kind 62 requests rebuild the tombstones in file order.
replay :: (st: [] Store, now: int) -> bool {
  at := 0;
  while at < st[0].total {
    f := ns_file_next(st[0].file[at..<at + 4], at, st[0].total);
    if f.status != NSF_OK { return false; }
    for i: f.from..<f.to { st[0].text[i - f.from] = st[0].file[i]; }
    r := admit_at(st, cast(string) st[0].text[0..<f.to - f.from], now, true);
    if nsr_replayed(r) != NSR_NEXT { return false; }
    at = f.to;
  }
  return true;
}
```

The whole program:

<!-- source: nostr/examples/delete-expire-vanish.coax example -->
```jai
#load "../../coax/lib/nostr/store.coax";
#load "../../coax/lib/nostr/store_file.coax";
#load "../../coax/lib/nostr/store_recover.coax";
#load "../../coax/lib/nostr/store_delete.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The relay's clock: 2026-11-01T00:00:00Z, and this relay's service URL.
NOW :: 1793491200;
RELAY :: "wss://relay.example.com";
// BIP-340 vectors 1 and 2: published test keys. Never sign with a real key
// in an example. Alice is vector 1, Bob vector 2.
ALICE_KEY :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
BOB_KEY :: "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9";
ALICE :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
BOB :: "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8";
AUX_HEX :: "0000000000000000000000000000000000000000000000000000000000000001";

// A store with room for 16 records and 16 tombstones, and its file (a byte
// buffer here, as in chapter 35's example).
Store :: struct {
  arena: [16384] u8; recs: [16] RsdRec; s: [1] RsdStore;
  tombs: [16] NsdTomb; tb: [1] NsdTombs; work: [16] NsdTomb;
  file: [16384] u8; total: int;
  scratch: [8192] u8; text: [2048] u8;
  nm: [] Names;   // the example's names for event ids
}
store_init :: (st: [] Store) {
  st[0].s[0] = RsdStore.{}; st[0].s[0].arena = st[0].arena[..]; st[0].s[0].recs = st[0].recs[..];
  st[0].tb[0] = NsdTombs.{}; st[0].tb[0].t = st[0].tombs[..];
}
reason_name :: (r: int) -> string {
  names := [12] string.["accepted", "duplicate", "superseded", "bad event", "bad id", "bad signature",
    "auth kind", "tags", "time", "expired", "auth required", "full"];
  if r == NSD_DELETED { return "deleted"; }
  if r == NSD_NO_ROOM { return "no room for its tombstones"; }
  return names[r];
}

// book:begin admit
// One EVENT as the relay would take it: read and policy at the clock now,
// the signature, then the tombstones and the store. The plan is decided,
// the frame appended (only when the plan writes), then the plan applied.
save :: (st: [] Store, text: string) -> bool {
  head: [4] u8;
  if !ns_file_header(text.count, head[..]) || st[0].total + 4 + text.count > st[0].file.count { return false; }
  for i: 0..<4 { st[0].file[st[0].total + i] = head[i]; }
  for i: 0..<text.count { st[0].file[st[0].total + 4 + i] = text[i]; }
  st[0].total += 4 + text.count;
  return true;
}
admit_at :: (st: [] Store, text: string, now: int, replaying: bool) -> int {
  r := rs_admit_read(text, now, false, false, st[0].scratch[..]);
  if replaying { r.reason = ns_file_replay_reason(r.reason); }
  if r.reason == RS_ACCEPT { r = rs_admit_verified(r, nostr_event_verify_vartime(text, st[0].scratch[..], false)); }
  if r.reason != RS_ACCEPT { return r.reason; }
  c := rsd_rec(r);
  p := nsd_plan(st[0].s[..], st[0].tb[..], c, text, rsd_verified(r, text), RELAY, st[0].work[..]);
  if !replaying && p.store.write != NS_W_NONE && !save(st, text) { return RS_FULL; }
  return nsd_apply_plan(st[0].s[..], st[0].tb[..], p, c, text, st[0].work[..]);
}
// book:end admit

// book:begin replay
// Restart: chapter 35's replay, each frame through the same admission
// (an event expired since it was saved is kept), so the stored kind 5 and
// kind 62 requests rebuild the tombstones in file order.
replay :: (st: [] Store, now: int) -> bool {
  at := 0;
  while at < st[0].total {
    f := ns_file_next(st[0].file[at..<at + 4], at, st[0].total);
    if f.status != NSF_OK { return false; }
    for i: f.from..<f.to { st[0].text[i - f.from] = st[0].file[i]; }
    r := admit_at(st, cast(string) st[0].text[0..<f.to - f.from], now, true);
    if nsr_replayed(r) != NSR_NEXT { return false; }
    at = f.to;
  }
  return true;
}
// book:end replay

// Sign one event with a test key into out; the event's text.
sign :: (key_hex: string, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(key_hex, key[..], false).error == NC_OK;
  #assert hex_decode_checked(AUX_HEX, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [8192] u8;
  n := EVENT_MORE;
  while n == EVENT_MORE { n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false); }
  #assert n > 0;
  return cast(string) out[0..<n];
}
// The event's id, as the 64 hex digits in its text.
id_hex :: (event: string) -> string {
  key := "\"id\":\"";
  for i: 0..<event.count - key.count { if event[i..<i + key.count] == key { return event[i + key.count..<i + key.count + 64]; } }
  return "";
}
// Writes a tags array into out: [[name, value], ...].
tags2 :: (out: [] u8, names: [] string, values: [] string) -> string {
  n := json_raw(out, 0, "[");
  for i: 0..<names.count {
    if i > 0 { n = json_raw(out, n, ","); }
    n = json_raw(out, n, "["); n = json_write_string(out, n, names[i]); n = json_raw(out, n, ",");
    n = json_write_string(out, n, values[i]); n = json_raw(out, n, "]");
  }
  n = json_raw(out, n, "]");
  return cast(string) out[0..<n];
}

// Names for the events, by id.
Names :: struct { n: int; ids: [32] string; names: [32] string; }
label :: (nm: [] Names, event: string, name: string) -> string {
  nm[0].ids[nm[0].n] = id_hex(event); nm[0].names[nm[0].n] = name; nm[0].n += 1;
  return event;
}
name_of :: (nm: [] Names, id: [32] u8) -> string {
  digits := "0123456789abcdef"; h: [64] u8;
  for i: 0..<32 { h[2 * i] = digits[id[i] >> 4]; h[2 * i + 1] = digits[id[i] & 15]; }
  for k: 0..<nm[0].n { if nm[0].ids[k] == cast(string) h[..] { return nm[0].names[k]; } }
  return "?";
}
show_time :: (t: int) {
  if t == NOW { print("NOW"); } else if t < NOW { print("NOW-%", NOW - t); } else { print("NOW+%", t - NOW); }
}
show_store :: (st: [] Store) {
  s := st[0].s[..];
  print("    store: % records, % live; % tombstones\n", s[0].n, s[0].live, st[0].tb[0].n);
  print("      live:");
  for i: 0..<s[0].n { if s[0].recs[i].live { print(" %", name_of(st[0].nm, s[0].recs[i].id)); } }
  print("\n      retired:");
  for i: 0..<s[0].n { if !s[0].recs[i].live { print(" %", name_of(st[0].nm, s[0].recs[i].id)); } }
  print("\n");
}
send :: (st: [] Store, step: string, text: string, now: int) {
  before := st[0].total; n := st[0].tb[0].n;
  r := admit_at(st, text, now, false);
  print("% -> % (%)", step, r, reason_name(r));
  if st[0].tb[0].n > n { print(", % tombstones, % records retired", st[0].tb[0].n - n, st[0].tb[0].last); }
  if st[0].total == before { print(", file unchanged"); }
  print("\n");
}
query :: (st: [] Store, now: int) {
  f: NostrFilter; idx: [16] int;
  q := rsd_query(st[0].s[..], f, now, idx[..]);
  print("REQ {} at "); show_time(now); print(": % events, newest first:", q);
  for i: 0..<q { print(" %", name_of(st[0].nm, st[0].s[0].recs[idx[i]].id)); }
  print("\n");
}

main :: () {
  nm: [1] Names; b: [16][1024] u8; t: [8][512] u8;
  a1 := label(nm[..], sign(ALICE_KEY, NOW - 300, 1, "[]", "a1", b[0][..]), "a1");
  a2 := label(nm[..], sign(ALICE_KEY, NOW - 200, 1, "[]", "a2", b[1][..]), "a2");
  b1 := label(nm[..], sign(BOB_KEY, NOW - 250, 1, "[]", "b1", b[2][..]), "b1");
  v1 := label(nm[..], sign(ALICE_KEY, NOW - 400, 30023, "[[\"d\",\"post\"]]", "post v1", b[3][..]), "post@-400");
  addr := "30023:dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659:post";
  d1_names := [5] string.["e", "e", "a", "k", "k"];
  d1_values := [5] string.[id_hex(a1), id_hex(b1), addr, "1", "30023"];
  d1 := label(nm[..], sign(ALICE_KEY, NOW - 100, 5, tags2(t[0][..], d1_names[..], d1_values[..]), "published by accident", b[4][..]), "D1");
  v2 := label(nm[..], sign(ALICE_KEY, NOW - 150, 30023, "[[\"d\",\"post\"]]", "post v2", b[5][..]), "post@-150");
  v3 := label(nm[..], sign(ALICE_KEY, NOW - 50, 30023, "[[\"d\",\"post\"]]", "post v3", b[6][..]), "post@-50");
  db_names := [2] string.["e", "a"]; db_values := [2] string.[id_hex(a2), addr];
  db := label(nm[..], sign(BOB_KEY, NOW - 40, 5, tags2(t[1][..], db_names[..], db_values[..]), "", b[7][..]), "DB");
  dd_names := [1] string.["e"]; dd_values := [1] string.[id_hex(d1)];
  dd := label(nm[..], sign(ALICE_KEY, NOW - 30, 5, tags2(t[2][..], dd_names[..], dd_values[..]), "undo", b[8][..]), "D2");
  ex := label(nm[..], sign(ALICE_KEY, NOW - 20, 1, "[[\"expiration\",\"1793491260\"]]", "gone at NOW+60", b[9][..]), "x60");
  old := label(nm[..], sign(ALICE_KEY, NOW - 20, 1, "[[\"expiration\",\"1793491190\"]]", "gone at NOW-10", b[10][..]), "x-10");
  ve := label(nm[..], sign(BOB_KEY, NOW - 10, 62, "[[\"relay\",\"wss://elsewhere.example\"]]", "", b[11][..]), "V-elsewhere");
  vn_names := [1] string.["relay"]; vn_values := [1] string.[RELAY];
  vn := label(nm[..], sign(BOB_KEY, NOW, 62, tags2(t[3][..], vn_names[..], vn_values[..]), "forget me", b[12][..]), "V");
  du_names := [1] string.["e"]; du_values := [1] string.[id_hex(vn)];
  du := label(nm[..], sign(BOB_KEY, NOW + 1, 5, tags2(t[4][..], du_names[..], du_values[..]), "", b[13][..]), "DV");
  b2 := label(nm[..], sign(BOB_KEY, NOW + 2, 1, "[]", "b2", b[14][..]), "b2");

  print("Alice is BIP-340 vector 1's key %..., Bob vector 2's %...\n", ALICE[0..<8], BOB[0..<8]);
  print("D1, Alice's kind 5 at NOW-100: %\n", d1);

  st: [1] Store; store_init(st[..]); st[0].nm = nm[..];
  print("\nNIP-09: deletion requests\n");
  send(st[..], "1. a1 (Alice, kind 1, NOW-300)", a1, NOW);
  send(st[..], "2. a2 (Alice, kind 1, NOW-200)", a2, NOW);
  send(st[..], "3. b1 (Bob, kind 1, NOW-250)", b1, NOW);
  send(st[..], "4. post@-400 (Alice, 30023 d=post)", v1, NOW);
  show_store(st[..]);
  send(st[..], "5. D1 (Alice, kind 5 at NOW-100: e a1, e b1, a 30023:Alice:post)", d1, NOW);
  show_store(st[..]);
  send(st[..], "6. post@-150 (older than D1)", v2, NOW);
  send(st[..], "7. post@-50 (newer than D1)", v3, NOW);
  send(st[..], "8. a1 again", a1, NOW);
  send(st[..], "9. DB (Bob, kind 5 at NOW-40: e a2, a 30023:Alice:post)", db, NOW);
  send(st[..], "10. D2 (Alice, kind 5 at NOW-30: e D1)", dd, NOW);
  show_store(st[..]);

  print("\nNIP-40: expiration, with the clock fixed\n");
  send(st[..], "11. x60 (expiration NOW+60)", ex, NOW);
  send(st[..], "12. x-10 (expiration NOW-10)", old, NOW);
  query(st[..], NOW);
  query(st[..], NOW + 59);
  query(st[..], NOW + 60);

  print("\nrestart: replay the % byte file into an empty store at NOW+60\n", st[0].total);
  r: [1] Store; store_init(r[..]); r[0].nm = nm[..];
  for i: 0..<st[0].total { r[0].file[i] = st[0].file[i]; }
  r[0].total = st[0].total;
  print("replay ok: %\n", replay(r[..], NOW + 60));
  show_store(r[..]);
  send(r[..], "13. a1 again, after the restart", a1, NOW + 60);
  send(r[..], "14. post@-150 again, after the restart", v2, NOW + 60);
  query(r[..], NOW + 60);

  print("\nNIP-62: requests to vanish\n");
  send(st[..], "15. V-elsewhere (Bob, kind 62, relay wss://elsewhere.example)", ve, NOW);
  send(st[..], "16. V (Bob, kind 62 at NOW, relay wss://relay.example.com)", vn, NOW);
  show_store(st[..]);
  send(st[..], "17. b1 again", b1, NOW);
  send(st[..], "18. DB again", db, NOW);
  send(st[..], "19. DV (Bob, kind 5 at NOW+1: e V)", du, NOW);
  send(st[..], "20. b2 (Bob, kind 1, NOW+2)", b2, NOW);
  show_store(st[..]);
  query(st[..], NOW);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/delete-expire-vanish.coax --run-steps 250000000
```

The output (the full transcript is
[nostr/fixtures/delete-expire-vanish.out](../fixtures/delete-expire-vanish.out)),
without the line that prints D1:

```text
Alice is BIP-340 vector 1's key dff1d77f..., Bob vector 2's dd308afe...

NIP-09: deletion requests
1. a1 (Alice, kind 1, NOW-300) -> 0 (accepted)
2. a2 (Alice, kind 1, NOW-200) -> 0 (accepted)
3. b1 (Bob, kind 1, NOW-250) -> 0 (accepted)
4. post@-400 (Alice, 30023 d=post) -> 0 (accepted)
    store: 4 records, 4 live; 0 tombstones
      live: a1 a2 b1 post@-400
      retired:
5. D1 (Alice, kind 5 at NOW-100: e a1, e b1, a 30023:Alice:post) -> 0 (accepted), 3 tombstones, 2 records retired
    store: 5 records, 3 live; 3 tombstones
      live: a2 b1 D1
      retired: a1 post@-400
6. post@-150 (older than D1) -> 30 (deleted), file unchanged
7. post@-50 (newer than D1) -> 0 (accepted)
8. a1 again -> 30 (deleted), file unchanged
9. DB (Bob, kind 5 at NOW-40: e a2, a 30023:Alice:post) -> 0 (accepted), 2 tombstones, 0 records retired
10. D2 (Alice, kind 5 at NOW-30: e D1) -> 0 (accepted), 1 tombstones, 0 records retired
    store: 8 records, 6 live; 6 tombstones
      live: a2 b1 D1 post@-50 DB D2
      retired: a1 post@-400

NIP-40: expiration, with the clock fixed
11. x60 (expiration NOW+60) -> 0 (accepted)
12. x-10 (expiration NOW-10) -> 9 (expired), file unchanged
REQ {} at NOW: 7 events, newest first: x60 D2 DB post@-50 D1 a2 b1
REQ {} at NOW+59: 7 events, newest first: x60 D2 DB post@-50 D1 a2 b1
REQ {} at NOW+60: 6 events, newest first: D2 DB post@-50 D1 a2 b1

restart: replay the 3713 byte file into an empty store at NOW+60
replay ok: true
    store: 9 records, 7 live; 6 tombstones
      live: a2 b1 D1 post@-50 DB D2 x60
      retired: a1 post@-400
13. a1 again, after the restart -> 30 (deleted), file unchanged
14. post@-150 again, after the restart -> 30 (deleted), file unchanged
REQ {} at NOW+60: 6 events, newest first: D2 DB post@-50 D1 a2 b1

NIP-62: requests to vanish
15. V-elsewhere (Bob, kind 62, relay wss://elsewhere.example) -> 0 (accepted)
16. V (Bob, kind 62 at NOW, relay wss://relay.example.com) -> 0 (accepted), 1 tombstones, 2 records retired
    store: 11 records, 7 live; 7 tombstones
      live: a2 D1 post@-50 D2 x60 V-elsewhere V
      retired: a1 b1 post@-400 DB
17. b1 again -> 30 (deleted), file unchanged
18. DB again -> 30 (deleted), file unchanged
19. DV (Bob, kind 5 at NOW+1: e V) -> 0 (accepted), 1 tombstones, 0 records retired
20. b2 (Bob, kind 1, NOW+2) -> 0 (accepted)
    store: 13 records, 9 live; 8 tombstones
      live: a2 D1 post@-50 D2 x60 V-elsewhere V DV b2
      retired: a1 b1 post@-400 DB
REQ {} at NOW: 9 events, newest first: b2 DV V V-elsewhere x60 D2 post@-50 D1 a2
```

The numbers are the results: 0 is accepted, 9 is expired, and 30 is
`NSD_DELETED`. The refusals are the ones worked out above: Bob's deletion
of a2 (step 9) and Alice's deletion of D1 (step 10) retire nothing; a1,
post@-150, b1 and DB are refused with 30 and leave the file unchanged; and
x-10 is refused with 9 before its signature is checked. After the restart,
a1 and post@-150 are still refused.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/delete-expire-vanish.coax](../examples/checks/delete-expire-vanish.coax),
which loads the example, so running the example doesn't run them:

- b1, D1 and V signed by Coax are byte for byte the events that
  `coax/tests/nostr/nip59_reference.py` writes and signs. That script is
  pure Python (BIP-340 from the BIP, `hashlib`, `json`) and shares no code
  with Coax;
- Python's events for steps 1 to 12 get the results worked out by hand
  above, D1 retires 2 records and DB and D2 none, and the store holds the
  hand list of 7 originals;
- x60 is served at NOW+59 and not at NOW+60;
- the file replays at NOW+60 to the same 7 originals and 6 tombstones, a1
  and post@-150 are refused after the restart, and the file is unchanged;
- steps 15 to 20 get the hand results, V retires 2 records, and the store
  ends with the hand list of 9 originals and 8 tombstones.

<!-- source: nostr/examples/checks/delete-expire-vanish.coax check -->
```jai
// Steps 1-12 as Python's events, and the verdict worked out by hand for
// each from NIP-09 and NIP-40, at NOW.
SESSION :: [12] string.[PY_A1, PY_A2, PY_B1, PY_V1, PY_D1, PY_V2, PY_V3, PY_A1, PY_DB, PY_D2, PY_X60, PY_XOLD];
HAND :: [12] int.[RS_ACCEPT, RS_ACCEPT, RS_ACCEPT, RS_ACCEPT, RS_ACCEPT, NSD_DELETED, RS_ACCEPT, NSD_DELETED,
  RS_ACCEPT, RS_ACCEPT, RS_ACCEPT, RS_EXPIRED];
// The records each request retired: D1 (step 5) two, DB and D2 none.
RETIRED :: [12] int.[0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0];
// What the store holds live after step 12, in the order it stored them.
HELD :: [7] string.[PY_A2, PY_B1, PY_D1, PY_V3, PY_DB, PY_D2, PY_X60];
// Steps 15-20 at NOW, on the same store: NIP-62.
VANISH :: [6] string.[PY_VE, PY_VN, PY_B1, PY_DB, PY_DV, PY_B2];
VANISH_HAND :: [6] int.[RS_ACCEPT, RS_ACCEPT, NSD_DELETED, NSD_DELETED, RS_ACCEPT, RS_ACCEPT];
FINAL :: [9] string.[PY_A2, PY_D1, PY_V3, PY_D2, PY_X60, PY_VE, PY_VN, PY_DV, PY_B2];

// Whether st holds exactly want live, in index order.
holds_all :: (st: [] Store, want: [] string) -> bool {
  k := 0;
  for i: 0..<st[0].s[0].n {
    if !st[0].s[0].recs[i].live { continue; }
    if k >= want.count || rsd_text(st[0].s[..], i) != want[k] { return false; }
    k += 1;
  }
  return k == want.count && st[0].s[0].live == want.count;
}
// How many events a REQ {} returns at now; negative when text is one of them.
serves :: (st: [] Store, now: int, text: string) -> int {
  f: NostrFilter; idx: [16] int;
  q := rsd_query(st[0].s[..], f, now, idx[..]);
  for i: 0..<q { if rsd_text(st[0].s[..], idx[i]) == text { return -q; } }
  return q;
}

#test "holds_all refuses a missing, an extra, a different and a retired record" {
  held := HELD; a: [1] Store; store_init(a[..]); c: RsdRec;
  for i: 0..<3 { ns_store_append(a[0].s[..], c, held[i]); }
  #assert holds_all(a[..], held[0..<3]) && !holds_all(a[..], held[0..<2]) && !holds_all(a[..], held[0..<4]);
  #assert !holds_all(a[..], held[1..<4]);
  ns_store_retire(a[0].s[..], 1); #assert !holds_all(a[..], held[0..<3]);
  b: [1] Store; store_init(b[..]);
  for i: 0..<3 { ns_store_append(b[0].s[..], c, held[i]); }
  b[0].s[0].live = 4; #assert !holds_all(b[..], held[0..<3]);
}
#test "the example's events are Python's bytes" #steps 2000000000 {
  b: [1024] u8; t: [512] u8;
  #assert sign(BOB_KEY, NOW - 250, 1, "[]", "b1", b[..]) == PY_B1;
  n := [5] string.["e", "e", "a", "k", "k"];
  v := [5] string.[id_hex(PY_A1), id_hex(PY_B1), "30023:dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659:post", "1", "30023"];
  #assert sign(ALICE_KEY, NOW - 100, 5, tags2(t[..], n[..], v[..]), "published by accident", b[..]) == PY_D1;
  rn := [1] string.["relay"]; rv := [1] string.[RELAY];
  #assert sign(BOB_KEY, NOW, 62, tags2(t[..], rn[..], rv[..]), "forget me", b[..]) == PY_VN;
}
#test "Python's events get the hand verdicts, and the store survives a restart" #steps 4000000000 {
  nm: [1] Names; held := HELD;
  a: [1] Store; store_init(a[..]); a[0].nm = nm[..];
  for i: 0..<12 {
    #assert admit_at(a[..], SESSION[i], NOW, false) == HAND[i];
    if i == 4 || i == 8 || i == 9 { #assert a[0].tb[0].last == RETIRED[i]; }
  }
  #assert holds_all(a[..], held[..]) && a[0].tb[0].n == 6;
  #assert serves(a[..], NOW + 59, PY_X60) == -7 && serves(a[..], NOW + 60, PY_X60) == 6;
  r: [1] Store; store_init(r[..]); r[0].nm = nm[..];
  for i: 0..<a[0].total { r[0].file[i] = a[0].file[i]; }
  r[0].total = a[0].total;
  #assert replay(r[..], NOW + 60) && holds_all(r[..], held[..]) && r[0].tb[0].n == 6;
  #assert admit_at(r[..], PY_A1, NOW + 60, false) == NSD_DELETED && admit_at(r[..], PY_V2, NOW + 60, false) == NSD_DELETED;
  #assert serves(r[..], NOW + 60, PY_X60) == 6 && r[0].total == a[0].total;
}
#test "a request to vanish removes Bob's events and keeps them out" #steps 4000000000 {
  nm: [1] Names; final := FINAL;
  a: [1] Store; store_init(a[..]); a[0].nm = nm[..];
  for i: 0..<11 { #assert admit_at(a[..], SESSION[i], NOW, false) == HAND[i]; }
  for i: 0..<6 {
    #assert admit_at(a[..], VANISH[i], NOW, false) == VANISH_HAND[i];
    if i == 1 { #assert a[0].tb[0].last == 2; }
  }
  #assert holds_all(a[..], final[..]) && a[0].tb[0].n == 8;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/delete-expire-vanish.coax --run-steps 250000000
```

On 2026-11-01 `coax test` ran 479 checks, including the loaded units' own
checks, and all passed in 108 s. The session with its replay takes 1.7
billion interpreter steps and about 30 s, the vanish session 1.3 billion.
`coax mutate nostr/examples/checks/delete-expire-vanish.coax --root-only`
killed 24 of 24 mutants in the checks' own procedures.

### The library's own checks

`store_delete.coax` has 12 checks: what each kind of tombstone covers
(another author, another ID, kind 5, kind 62, `until`, another `d`, another
kind, an address naming another author, replaceable and regular kinds), the
retiring count and a full table, lowercase hex only, the `a` value grammar
at each boundary (9 and 10 kind digits, 63 and 64 hex digits, a `d` of 256
and 257 bytes), the tags a request yields (escaped names, values too long
for the buffer, tags with one or more than two strings, a missing tags
array, other kinds), kind 62 with this relay, another relay, `ALL_RELAYS`
and lowercase `all_relays`, and admission: unverified, no room, a duplicate
request, a deleted event as other bytes, and a request the store refuses.
On 2026-11-01 they passed, and `coax mutate
coax/lib/nostr/store_delete.coax --root-only` killed 267 of 267 mutants. The
first run left 11 survivors; the fixes were checks for an `a` value with a
bad hex digit and a tags value that is not an array, a check that a request
with two tombstones counts both, rewriting the `a` reader so that `:` can't
pass as a digit, and reading a tag's strings without a lookahead that hid
tags after a one-string tag.

`store_policy.coax`, `store_core.coax` and `relay_server.coax` did not
change. Their records were not rerun for this chapter:
[2026-10-29-nostr-store-r1.md](../../docs/verification/2026-10-29-nostr-store-r1.md)
(`store_core.coax` 590 of 590 mutants killed).

### What is not solved

- The file is append-only, so a deleted event's bytes stay in it and are
  read again on every restart. NIP-62 says relays should truly delete; that
  needs the file to be rewritten without those frames, which is not built.
- Gift wraps addressed to a vanished pubkey are not deleted.
- The tombstone table is fixed in size. When it is full, requests are
  refused with `NSD_NO_ROOM`; nothing is evicted.

## Summary

- An expired event is refused at admission and not served from the second
  it expires; replay keeps it so the file stays valid.
- A kind 5 request becomes tombstones: `e` deletes only the requester's own
  events, `a` deletes the requester's versions of an address up to the
  request's `created_at`, and no request deletes another kind 5 or a kind
  62.
- A kind 62 request naming this relay or `ALL_RELAYS` removes everything
  by its pubkey up to its `created_at`, kind 5 requests included.
- Covered events are refused when sent again. The tombstones come from the
  stored requests, so replaying the file rebuilds them after a restart.

## Dependencies

- [35. Store events](ch035-store-events.md)

## Sources

- [NIP-09](../nips/official/09.md), Event Deletion Request.
- [NIP-40](../nips/official/40.md), Expiration Timestamp.
- [NIP-62](../nips/official/62.md), Request to Vanish.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
