# 103. Replace relay.openagents.com

> **Status: partly implemented.** The importer, the equality check, the
> shadow comparer and the runbook exist and are checked offline. The
> export, the shadow period, the DNS cutover and the rollback rehearsal
> need the owner's sign-off and have not happened (NEEDS_OWNER.md).

relay.openagents.com still runs the Rust relay from before the reset. This
chapter builds the tools for phase R3 of coax/nostr.md section 17, moving
the relay's data and traffic to the Coax relay, and writes down the steps
that use them. It doesn't touch production: every run in this chapter reads
a recorded capture.

Part XII, Nostr in OpenAgents. Result: An importer that checks every id and
signature in an exported event stream again, an event-set equality check
that names the exact difference, a shadow comparer that classifies every
differing answer, and a runbook for the cutover and the rollback.

## What this chapter teaches

- An export format, and an import that checks every event again as if it
  were new, counts every refusal, and never stops on one bad line.
- Event-set equality: a sorted-id digest decides, and Negentropy (chapter
  43) names the difference.
- A shadow period: the same REQs to both relays, every differing answer
  classified with chapter 94's method.
- The gate: the stores agree, no answer is unexplained, and the owner signs
  off before DNS changes.
- A rehearsed rollback to the Rust relay.

## Where the code is

| File | What it does |
| --- | --- |
| [coax/lib/nostr/relay_import.coax](../../coax/lib/nostr/relay_import.coax) | `#authority`: what each export line's result counts as, the tally and the import verdict |
| [coax/lib/nostr/relay_import_run.coax](../../coax/lib/nostr/relay_import_run.coax) | `ri_line` and `ri_import`: each line read and verified as a live EVENT, then offered to the store |
| [coax/lib/nostr/relay_equal.coax](../../coax/lib/nostr/relay_equal.coax) | `#authority`: a store's set at a time, its digest, and the comparison that names the difference with Negentropy |
| [coax/lib/nostr/relay_shadow.coax](../../coax/lib/nostr/relay_shadow.coax) | `#authority`: the shadow replay and its four classes |
| [coax/lib/nostr/fixtures/relay_export.coax](../../coax/lib/nostr/fixtures/relay_export.coax) | The six-line export, written by [relay_export_fixture.py](../../coax/tests/nostr/relay_export_fixture.py) from the 2026-10-28 capture |
| [nostr/examples/relay-cutover.coax](../examples/relay-cutover.coax) | The rehearsal: import, a shadow store that missed a write, equality, the shadow and the gate |

The store is chapter 35's (`store.coax`, `store_core.coax`), the admission
is chapter 34's (`rs_admit_read`, `rs_admit_verified`), and the filters
are read by chapter 23's `filter_read`. Nothing here is a second store.

## The export format

The Rust relay keeps events in one Postgres table, `nostr_event`, with the
id, pubkey, created_at, kind, tags (as `jsonb`), content and sig in
columns, and an `ingest_seq` that numbers rows in the order they arrived
(`migrations/0001_store.sql` at `0b401561a0^`). It had an `import-jsonl`
command, one event per line and at most 131,072 bytes a line, but no
export. This chapter defines the export the other way round:

- UTF-8 text, one NIP-01 event object per line, each line ended by `\n`;
- the seven fields `id`, `pubkey`, `created_at`, `kind`, `tags`, `content`
  and `sig`, in any order and with any JSON spacing;
- at most 131,072 bytes a line (`RI_LINE_MAX`, the same limit);
- every live row of `nostr_event`, in `ingest_seq` order.

Order matters because of replacement. If the table still holds an older
version of a replaceable event, the importer sees it first, and the newer
one replaces it as it did in the Rust relay. Spacing doesn't matter
because `jsonb` doesn't keep the client's bytes: the export can't be the
events exactly as they were sent. That is safe because a NIP-01 id is the
SHA-256 of the values, serialized one canonical way, not of the bytes on
the wire; the importer recomputes it from the values and checks the
signature against it.

The export is taken read-only from a restored copy of a backup unit
(`deploy/backup/nostr-relay-backup` at `0b401561a0^`), never from the
production database. The runbook below gives the query.

## Import with every check again

An export line goes through the same checks as an EVENT from a client. The
importer adds four outcomes that only an export has:

<!-- source: coax/lib/nostr/relay_import.coax outcomes -->
```jai
// The Rust relay's import-jsonl line limit (MAX_JSONL_EVENT_BYTES), which is
// also the relay's message limit.
RI_LINE_MAX :: 131072;
// A line's outcome is a relay reason (RS_ACCEPT stored, RS_DUP, RS_BAD_ID,
// RS_BAD_SIG, RS_EXPIRED, ...) or one of these, which only an export has.
RI_EMPTY :: RS_REASONS;          // an empty line
RI_LONG :: RS_REASONS + 1;       // a line over RI_LINE_MAX bytes
RI_CUT :: RS_REASONS + 2;        // the last line has no "\n": the export was cut
RI_EPHEMERAL :: RS_REASONS + 3;  // kinds 20000-29999 are never stored, so never exported
RI_OUTCOMES :: RS_REASONS + 4;
// What one import did: lines read, each outcome's count, and the first
// refused line (1-based, 0 when none).
RiReport :: struct { lines: int; count: [RI_OUTCOMES] int; first_refused: int; }
// The verdict on a whole import.
RI_CLEAN :: 0;       // every line is in the store, or was replaced or had expired
RI_REFUSED :: 1;     // lines were refused (forged, malformed, policy): the owner reviews them
RI_INCOMPLETE :: 2;  // the store doesn't hold the export (cut, too long, full): never serve it
```

A cut export is the case to catch first: a copy that stopped partway ends
in a line with no `\n`. The line checks come before any parsing:

<!-- source: coax/lib/nostr/relay_import.coax frame -->
```jai
// The checks on a line before it is read as an event.
ri_frame :: (line: string, ended: bool) -> int {
  if !ended { return RI_CUT; }
  if line.count == 0 { return RI_EMPTY; }
  if line.count > RI_LINE_MAX { return RI_LONG; }
  return RS_ACCEPT;
}
// The outcome of a read and verified line before the store: RS_ACCEPT means
// "offer it to the store".
ri_check :: (r: RsAdmit) -> int {
  if r.reason != RS_ACCEPT { return r.reason; }
  if r.kclass == RS_EPHEMERAL { return RI_EPHEMERAL; }
  return RS_ACCEPT;
}
```

Then each line is read, its id recomputed and its signature verified, the
cheap checks first, exactly as `coax/relay/relay.coax` admits an EVENT.
The clock is the export's time, so an event that had expired by then is
refused as expired, and one dated more than 15 minutes after it is refused
as too far in the future. No NIP-42 login is asked for:

<!-- source: coax/lib/nostr/relay_import_run.coax line -->
```jai
// Imports one line: every id and signature checked again, then the store.
// now is the export's time: an event expired by then is refused.
ri_line :: (s: [] RsdStore, line: string, ended: bool, now: int, scratch: [] u8) -> int {
  f := ri_frame(line, ended);
  if f != RS_ACCEPT { return f; }
  r := rs_admit_read(line, now, false, false, scratch);
  if r.reason == RS_ACCEPT { r = rs_admit_verified(r, nostr_event_verify_vartime(line, scratch, false)); }
  c := ri_check(r);
  if c != RS_ACCEPT { return c; }
  return rsd_admit(s, r, line);
}
// Imports a whole export into s, counting every line's outcome in rep (and
// writing line k's outcome to outcome[k] while outcome has room).
ri_import :: (s: [] RsdStore, export: string, now: int, scratch: [] u8, rep: [] RiReport, outcome: [] int) {
  rep[0] = RiReport.{};
  at := 0;
  while at < export.count {
    to := ri_line_end(export, at);
    o := ri_line(s, export[at..<to], to < export.count, now, scratch);
    if rep[0].lines < outcome.count { outcome[rep[0].lines] = o; }
    ri_tally(rep, o);
    at = to + 1;
  }
}
```

The verdict separates two kinds of trouble. A store that doesn't hold what
the export holds (a cut file, a line over the limit, a full store) must
never serve. A store that holds every good line but refused some is a
question for the owner: the Rust relay verified signatures too, so a
forged line in its export means its database or the copy was changed.

<!-- source: coax/lib/nostr/relay_import.coax verdict -->
```jai
// The verdict: incomplete when the store can't hold what the export holds,
// refused when any line was refused, clean otherwise. Expired events are
// dropped on purpose (the old relay no longer serves them either).
ri_verdict :: (rep: RiReport) -> int {
  if rep.count[RI_CUT] > 0 || rep.count[RI_LONG] > 0 || rep.count[RS_FULL] > 0 { return RI_INCOMPLETE; }
  kept := rep.count[RS_ACCEPT] + rep.count[RS_DUP] + rep.count[RS_SUPERSEDED] + rep.count[RS_EXPIRED];
  if kept != rep.lines { return RI_REFUSED; }
  return RI_CLEAN;
}
```

The decisions are in `relay_import.coax`, an `#authority` unit whose checks
don't verify signatures, so its mutation run stays fast. The reading is in
`relay_import_run.coax`, which calls the verifier. That split is the one
chapter 35 makes between `store_core.coax` and `store.coax`.

## Event-set equality

What does "the two relays hold the same events" mean? Not the same rows:
a relay may still hold replaced versions until it compacts, and expired
events it no longer serves. The set that counts is what a relay serves at
a time `now`: its live, unexpired records. Each becomes a NIP-77 item, its
`created_at` and id, sorted by time and then by id:

<!-- source: coax/lib/nostr/relay_equal.coax items -->
```jai
// The store's set at now, sorted: the count (at most out.count).
re_items :: (s: [] RsdStore, now: int, out: [] NegItem) -> int {
  n := 0;
  for i: 0..<s[0].n {
    if !ns_store_visible(s, i, now) || n >= out.count { continue; }
    out[n] = NegItem.{created = s[0].recs[i].created, id = s[0].recs[i].id};
    n += 1;
  }
  neg_sort(out[0..<n]);
  return n;
}
// SHA-256 over the sorted items: 8 bytes of created_at, big-endian, then the id.
re_digest :: (items: [] NegItem, out: [] u8) {
  st: [SHA_STATE_BYTES] u8;
  #assert sha256_init(st[..]) == SHA_OK;
  for item: items {
    b: [40] u8;
    for k: 0..<8 { b[k] = cast(u8) ((item.created >> (56 - 8 * k)) & 255); }
    for k: 0..<32 { b[8 + k] = item.id[k]; }
    #assert sha256_update_checked(st[..], b[..], false) == SHA_OK;
  }
  #assert sha256_final_checked(st[..], out[0..<32], false) == SHA_OK;
}
```

Two sets are equal when their digests are equal. The digest is SHA-256 over
the whole sorted list, so equal digests mean equal lists unless SHA-256
collides.

### Which method, and why

There were three candidates: compare two sorted-id digests, run chapter
43's Negentropy, or merge two sorted lists. This chapter uses the first to
decide and the second to name the difference:

- **The digest decides.** Negentropy's range fingerprint is a sum of ids
  mod 2^256, hashed with the count. A sum is easy to match: ids
  {1, 4} and {2, 3} in the same range share every fingerprint. Ids are
  SHA-256 outputs, so a match by accident is unlikely, but a cutover gate
  shouldn't rest on a sum. A checked case in `relay_equal.coax` builds that
  collision and gets `RE_UNNAMED`, not `RE_EQUAL`.
- **Negentropy names the difference.** At cutover the Rust relay is a
  remote server that speaks NIP-77. The same messages that run here
  between two in-memory stores can run over a `NEG-OPEN` to it, so
  checking a million events doesn't mean downloading a million events.
- **The merge checks Negentropy.** The chapter's checks merge the two
  sorted lists, the simplest correct way, and compare its answer with
  Negentropy's over 24 generated pairs of 5 to 650 items.

<!-- source: coax/lib/nostr/relay_equal.coax compare -->
```jai
// Compares sorted item lists a and b. q and r hold one Negentropy message
// each way; d.only_a and d.only_b are the caller's, and so is d.max_rounds,
// the most round trips it may take.
re_compare :: (a: [] NegItem, b: [] NegItem, d: [] ReDiff, q: [] u8, r: [] u8) -> int {
  d[0].a_n = a.count; d[0].b_n = b.count; d[0].only_a_n = 0; d[0].only_b_n = 0; d[0].rounds = 0; d[0].error = NEG_OK;
  re_digest(a, d[0].a_digest[..]); re_digest(b, d[0].b_digest[..]);
  if cast(string) d[0].a_digest[..] == cast(string) d[0].b_digest[..] { d[0].verdict = RE_EQUAL; return RE_EQUAL; }
  client: [1] Neg; server: [1] Neg;
  if neg_init(client[..], a, true, 0) != NEG_OK { return re_fail(d, client[0].error); }
  if neg_init(server[..], b, false, 0) != NEG_OK { return re_fail(d, server[0].error); }
  client[0].have = d[0].only_a; client[0].need = d[0].only_b;
  n := neg_initiate(client[..], q);
  while true {
    if n < 0 { return re_fail(d, client[0].error); }
    if n == 0 { break; }
    if d[0].rounds >= d[0].max_rounds { return re_fail(d, NEG_SPACE); }
    d[0].rounds += 1;
    m := neg_reconcile(server[..], cast(string) q[0..<n], r);
    if server[0].error != NEG_OK { return re_fail(d, server[0].error); }
    n = neg_reconcile(client[..], cast(string) r[0..<m], q);
  }
  d[0].only_a_n = client[0].have_n; d[0].only_b_n = client[0].need_n;
  if d[0].only_a_n + d[0].only_b_n == 0 { d[0].verdict = RE_UNNAMED; return RE_UNNAMED; }
  d[0].verdict = RE_DIFFER;
  return RE_DIFFER;
}
re_fail :: (d: [] ReDiff, e: int) -> int { d[0].error = e; d[0].verdict = RE_ERROR; return RE_ERROR; }
// Compares two stores as they serve at now. ia and ib hold their items.
re_stores :: (a: [] RsdStore, b: [] RsdStore, now: int, ia: [] NegItem, ib: [] NegItem, d: [] ReDiff, q: [] u8, r: [] u8) -> int {
  na := re_items(a, now, ia); nb := re_items(b, now, ib);
  return re_compare(ia[0..<na], ib[0..<nb], d, q, r);
}
```

The first store is Negentropy's client, so `have` is "only in the first"
and `need` is "only in the second". Negentropy compares ids inside ranges,
not timestamps: one id at two different times is a difference only the
digest sees. Verified stores can't hold that (the id hashes `created_at`),
which is one more reason the digest decides.

## The shadow period

During the shadow period the proxy in front of relay.openagents.com sends
every write to both relays, and readers ask both. Chapter 94's method
compares the answers: run both, normalize each answer to its ids in order,
compare, and classify every difference.

<!-- source: coax/lib/nostr/relay_shadow.coax classify -->
```jai
sh_same :: (a: NegId, b: NegId) -> bool {
  for k: 0..<32 { if a.b[k] != b.b[k] { return false; } }
  return true;
}
sh_in :: (ids: [] NegId, n: int, id: NegId) -> bool {
  for i: 0..<n { if sh_same(ids[i], id) { return true; } }
  return false;
}
// Whether id is in the known set difference.
sh_known :: (d: ReDiff, id: NegId) -> bool {
  return sh_in(d.only_a, d.only_a_n, id) || sh_in(d.only_b, d.only_b_n, id);
}
// The answer without the known difference's ids.
sh_strip :: (x: ShAnswer, d: ReDiff) -> ShAnswer {
  y: ShAnswer;
  for i: 0..<x.n { if !sh_known(d, x.ids[i]) { y.ids[y.n] = x.ids[i]; y.n += 1; } }
  return y;
}
// Whether the first n ids of a and b are the same, in order.
sh_prefix :: (a: ShAnswer, b: ShAnswer, n: int) -> bool {
  for i: 0..<n { if !sh_same(a.ids[i], b.ids[i]) { return false; } }
  return true;
}
// Whether the shorter of a and b is the start of the other.
sh_starts :: (a: ShAnswer, b: ShAnswer) -> bool {
  for i: 0..<a.n {
    if i == b.n { return true; }
    if !sh_same(a.ids[i], b.ids[i]) { return false; }
  }
  return true;
}
// The class of answers a and b to one filter whose limit is limit.
sh_classify :: (a: ShAnswer, b: ShAnswer, limit: int, d: ReDiff) -> int {
  if a.n == b.n && sh_prefix(a, b, a.n) { return SH_MATCH; }
  if a.n == b.n {
    all := true;
    bi := b.ids;
    for i: 0..<a.n { if !sh_in(bi[..], b.n, a.ids[i]) { all = false; } }
    if all { return SH_ORDER; }
  }
  sa := sh_strip(a, d); sb := sh_strip(b, d);
  // a stripped answer shorter than the other's must come from a full answer
  if sh_starts(sa, sb) && (sa.n >= sb.n || a.n == limit) && (sb.n >= sa.n || b.n == limit) { return SH_EXPLAINED; }
  return SH_UNEXPLAINED;
}
```

The classes, in the order they are tried:

| Class | Meaning | Allows cutover |
| --- | --- | --- |
| match | the same ids in the same order | yes |
| order | the same ids in another order: the Rust relay merges a REQ's filters into one order, the Coax relay answers filter by filter (chapter 94, owner question 2) | yes |
| explained | with the known set difference taken out, the answers agree; or one answer was full at its `limit` and a known missing event pushed an older one out of it | yes |
| unexplained | anything else | no |

The explained class is what ties the shadow to the equality check. A shadow
difference is never explained by a rule written for the occasion: either
the equality check already named the events, or the answer is
unexplained.

## The example

The example rehearses the whole path offline. Its export is six lines
built by `coax/tests/nostr/relay_export_fixture.py` from the recorded
capture `coax/lib/nostr/fixtures/coax_relay.jsonl` (what
relay.openagents.com sent for the Coax project page on 2026-10-28,
chapters 24 and 36). Five lines are copied byte for byte. Line 5 is a
forgery: status `a6adbc17...` with its `e` tag pointed at issue
`791f0799...`, its id recomputed and its signature copied from the
original. Anyone can compute an id; only the key's owner can sign one.

### Worked trace by hand

The six lines, in export order:

| Line | Kind | created_at | id | Expected |
| --- | --- | --- | --- | --- |
| 1 | 30617 repository | 1793214821 | `813d9fb0...` | stored |
| 2 | 30621 project | 1793214822 | `375b0d29...` | stored |
| 3 | 1621 issue | 1793217786 | `791f0799...` | stored |
| 4 | 1621 issue | 1793217787 | `0c13c6ed...` | stored |
| 5 | 1631 status (forged) | 1793218578 | `34ffdff2...` | the id matches its values; BIP-340 fails: `invalid: bad signature` |
| 6 | 1631 status | 1793218893 | `2a4b0130...` | stored |

Store A imports all six at `NOW` = 1793220000, after the newest event, so
nothing is too new or expired. Five are stored and one is refused at line
5. The verdict is "refused": every good line is in the store, and the
owner reviews line 5.

Store B is the shadow relay, and it missed line 4. It imports the other
five: four stored, the forgery refused at its line 4.

Sorted by `(created_at, id)`, A's items are 813d, 375b, 791f, 0c13, 2a4b,
and B's are the same without 0c13. Their digests differ, so Negentropy
runs. A has 5 items, under 32, so A's first message is one IdList of all
five. B answers with its own IdList of four. A finds 0c13 in its own list
and not in B's: one id only in A, none only in B, after one round trip.

The shadow asks both stores the project page's five filters:

| REQ | A sends | B sends | Class | Why |
| --- | --- | --- | --- | --- |
| project | 375b | 375b | match | |
| issues | 0c13, 791f | 791f | explained | strip the known 0c13 and both are 791f |
| comments | nothing | nothing | match | no comments in this export |
| status | 2a4b | 2a4b | match | the forgery is in neither store |
| repo | 813d | 813d | match | |

The issues answer is newest first: 0c13 (…787) before 791f (…786). The
gate stays shut: the import refused a line and the stores differ. Then
B is sent the whole export again. Lines 1, 2, 3 and 6 are duplicates, line
4 is stored, line 5 is refused again, the digests become equal and every
answer matches.

The example's main:

<!-- source: nostr/examples/relay-cutover.coax main -->
```jai
main :: () {
  a: [1] Store; b: [1] Store; w: [1] Work;
  store_init(a[..]); store_init(b[..]);
  va := import(a[..], w[..], RX_EXPORT, "store A, the export");
  vb := import(b[..], w[..], without(w[..], RX_EXPORT, 4), "store B, the shadow relay (it missed line 4)");
  e := compare(a[..], b[..], w[..]);
  s := shadow(a[..], b[..], w[..]);
  print("cutover: %\n", ifx va == RI_CLEAN && e == RE_EQUAL && s then "yes" else "no: a forged line and an event B lacks");
  // The missing write is sent to B again; the forgery stays out.
  vb = import(b[..], w[..], RX_EXPORT, "store B, the whole export again");
  e = compare(a[..], b[..], w[..]);
  s = shadow(a[..], b[..], w[..]);
  print("cutover: %\n", ifx e == RE_EQUAL && s then "the stores agree; the forged line still needs the owner's review" else "no");
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/relay-cutover.coax
```

```text
store A, the export: 6 lines
  line 1: stored
  line 2: stored
  line 3: stored
  line 4: stored
  line 5: refused: "invalid: bad signature"
  line 6: stored
  5 stored, 1 refused (first at line 5); verdict: refused lines: the owner reviews them
store B, the shadow relay (it missed line 4): 5 lines
  line 1: stored
  line 2: stored
  line 3: stored
  line 4: refused: "invalid: bad signature"
  line 5: stored
  4 stored, 1 refused (first at line 4); verdict: refused lines: the owner reviews them
equality: A has 5, B has 4; digests 172667b25fd3... and 80d7a0ae7a10...: different after 1 Negentropy round trip(s)
  only in A: 0c13c6ed2696572356537f5a6b8340b3431a779cae2f96ae1bbb5df6f6f3f414
shadow: 5 REQs replayed to both stores
  project: A sends 1, B sends 1: match
  issues: A sends 2, B sends 1: explained
  comments: A sends 0, B sends 0: match
  status: A sends 1, B sends 1: match
  repo: A sends 1, B sends 1: match
cutover: no: a forged line and an event B lacks
store B, the whole export again: 6 lines
  line 1: kept: "duplicate: already have this event"
  line 2: kept: "duplicate: already have this event"
  line 3: kept: "duplicate: already have this event"
  line 4: stored
  line 5: refused: "invalid: bad signature"
  line 6: kept: "duplicate: already have this event"
  1 stored, 1 refused (first at line 5); verdict: refused lines: the owner reviews them
equality: A has 5, B has 5; digests 172667b25fd3... and 172667b25fd3...: equal
shadow: 5 REQs replayed to both stores
  project: A sends 1, B sends 1: match
  issues: A sends 2, B sends 2: match
  comments: A sends 0, B sends 0: match
  status: A sends 1, B sends 1: match
  repo: A sends 1, B sends 1: match
cutover: the stores agree; the forged line still needs the owner's review
```

The refusal of line 5 is the deliberately broken input: its id is right,
so it passes the id check and fails only at BIP-340, with the relay's own
`invalid: bad signature`.

## The runbook

The steps below are the R3 plan. Steps marked **owner** change production
or need the owner's sign-off, and are in the workspace's `NEEDS_OWNER.md`.
Nothing in this chapter has run any of them.

### Before: what must be true first

1. The relay modules in the next section that the Rust relay's NIP-11
   document advertises are wired into `coax/relay/relay.coax`, or the
   owner decides which advertised NIPs are dropped (R2).
2. The Coax store holds the whole export. Today it holds at most 2,048
   records and 4 MiB (`RSD_MAX`, `RSD_ARENA`); an export larger than that
   imports as `RI_INCOMPLETE` (`relay_import.coax` checks this), and the
   cutover can't start.
3. An import command writes the relay's `--store` file: `ri_line` with
   `rsd_admit_saved` in place of `rsd_admit`, so the relay starts from the
   imported events and verifies them all again on its first replay.

### Export (owner)

4. Take the newest backup unit (`nostr-relay-<stamp>.dump` and its
   manifest) and check its digest against the manifest.
5. Restore it into a throwaway Postgres on a machine that is not the
   production host, as `coax/tests/nostr/rust_relay_sandbox.sh` does for
   the differential. Production's database is never read directly.
6. Export, in a read-only transaction:

   ```text
   BEGIN TRANSACTION READ ONLY ISOLATION LEVEL REPEATABLE READ;
   COPY (SELECT json_build_object('id', id, 'pubkey', pubkey,
     'created_at', created_at, 'kind', kind, 'tags', tags,
     'content', content, 'sig', sig)
     FROM nostr_event ORDER BY ingest_seq) TO STDOUT;
   COMMIT;
   ```

   Record the line count, the file's SHA-256 and the newest `created_at`;
   that time is the import's `now`.

### Import and compare

7. Import with re-verification. A verdict of incomplete stops here. A
   verdict of refused lists every refused line for the owner, who decides
   before the cutover continues.
8. Start the Coax relay from the imported store on a tailnet address, with
   `--store`.
9. Compare the event sets: the Coax store against the restored Rust
   database, through the sandboxed Rust relay's NIP-77 endpoint. The
   result must be equal, or every difference must be one the owner
   accepted in step 7.

### Shadow period (owner)

10. Put both relays behind the proxy: every EVENT goes to both, and the
    Rust relay's answer is the one clients get.
11. Replay readers' REQs to both and classify the answers. Run for at
    least a week of normal traffic (the owner sets the length). Any
    unexplained answer stops the shadow period and becomes a bug with its
    minimized input (chapter 94).
12. At the end, compare the event sets again, with the same `now` on both
    sides.

### Cutover (owner)

13. Gate, all of them true: the last equality check is equal; the shadow
    period has no unexplained answer; the owner has signed off in
    `NEEDS_OWNER.md`.
14. Make the Rust relay read-only (its writes refused), wait for the last
    writes to reach both, and compare once more.
15. Point relay.openagents.com at the Coax relay (the proxy upstream, or
    DNS with a short TTL set a day before).
16. Keep the Rust relay running read-only, with its database untouched, as
    the fallback.

### Rollback, rehearsed before step 15 (owner)

17. Rehearse on a staging name first: point it at the Coax relay, then
    back at the Rust relay, and check that a client reads and writes
    through it each time.
18. To roll back for real: point relay.openagents.com back at the Rust
    relay and make it writable again.
19. Events written to the Coax relay after the cutover aren't in the Rust
    database. Export them from the Coax store (its `--store` file, every
    event verified again on replay) and import them into the Rust relay
    with its own `import-jsonl`, then compare the event sets.
20. The rollback rehearsal passes when service is restored from the Rust
    relay and the final equality check is equal.

## What the relay doesn't use yet

`coax/relay/relay.coax` loads `relay_store.coax` (the store, `store_saved.coax`,
with chapter 36's index beside it), `nip42-event.coax`, `relay_route.coax`
and `message_agree.coax`. The table says which modules from Part IX and
Part X are wired into it, and what the rest still need.

| Module | Chapter | Status | What wiring it needs, or how it is wired |
| --- | --- | --- | --- |
| `store_index.coax` | 36 | Wired | `relay_store.coax` keeps an `NsiIndex` beside the store: `nsi_add` after each admission that appends, a rebuild (`nsi_build`) before the next query after one that may have compacted (`rr_may_compact`, `rr_index_step`), and `ry_req` answers with `nsi_query` (`rd_query`) |
| `store_delete.coax` | 39 | Wired | Every admission goes through `nsd_plan` and `nsd_apply_plan` (`rsd_admit_deleting`), saving to the `--store` file between them; a covered event is refused "blocked:"; replay (`rsd_replay_deleting`) rebuilds the tombstones from the saved requests. A kind 62 must name the relay's URL (the NIP-42 one) or `ALL_RELAYS` |
| `relay_rate.coax` | 44 | Wired | An `RtConn` in each `RyConn` and one `RtKeys`: `rt_event` once an EVENT's signature holds, `rt_req` before a REQ's filters, refused "rate-limited:"; the served NIP-11 document is `rt_nip11(RR_NIP11, ...)`. Limits from `--rate EVENTS,REQS,KEY_EVENTS`, else 120, 120 and 60 |
| `nip45.coax` | 40 | Wired | `rr_message` routes `COUNT` to `ry_count`, which charges it as a REQ and answers with `n45_answer` over a current index (`rd_count`) |
| `nip50.coax` via `filter_read_search` | 41 | Not wired | Read filters with `filter_read_search` in `rs_filter`, build and update the word index (`n50_build`), answer search filters with `n50_query`, and advertise `50` |
| `nip70.coax` | 42 | Wired | `ry_event` calls `nip70_admit` (accepts true) with the connection's `RrAuth` after the signature and the rate limit, so a `["-"]` event is accepted only from its author |
| `nip77.coax` | 43 | Not wired | Per-connection `NEG-OPEN`, `NEG-MSG` and `NEG-CLOSE` sessions over the store's items (`re_items` builds them), with `NEG-ERR` on limits and time outs; also step 9's remote check |
| `nip13.coax` | 56 | Wired | `--pow BITS` (default 0, off; `rr_pow`): `nip13_admit` before the signature, refused "pow: ..."; NIP-11 then carries `min_pow_difficulty` and lists 13 (`rr_nip11_pow`) |
| `nip86.coax` | 67 | Not wired | An HTTP POST branch in `ry_http` for `application/nostr+json+rpc` that calls `n86_serve` after NIP-98 auth, and `n86_admit` in admission for banned keys and allowed kinds |
| `nip43.coax` | 67 | Not wired | A relay key (`n43_self` in NIP-11), membership state from kinds 13534, 8000 and 8001, and the join and leave requests routed to `n43_request` |
| `nip29.coax` | 59 | Not wired | Group state per `h` tag, `g29_admit` for events with an `h` tag, and the relay-signed metadata, admins and members events (`g29_write_*`) |

The served NIP-11 document lists what is wired: `supported_nips` is
`[1,9,11,40,42,45,62,70]` in `RR_NIP11` (and its copy `RS_NIP11`), and
`rr_nip11_pow` adds 13 when `--pow` is above 0 (chapter 37).

## How the code is checked

### Checks in the library

- `relay_import.coax` (`#authority`): every outcome's frame, the check on a
  read line, the tally, the first refused line, and every verdict branch
  with distinct counts. `coax mutate --root-only`: 78 mutants, 78 killed.
- `relay_import_run.coax`: a cut line, an empty line, an event 901 seconds
  too new, a changed byte (`invalid: event id does not match its
  content`), and two verified lines, one stored and the forgery refused.
  Root-only mutation: 28 mutants, 27 killed. The survivor turns the
  `authed` argument of `rs_admit_read` to true, which `relay_server.coax`
  reads only when a login is required, and the import never requires one.
- `relay_equal.coax` (`#authority`): the digest of one item and of none
  against Python's `hashlib`, the highest `created_at` byte, equal sets
  without a round trip, one missing each way, a difference across buckets,
  600 items that take two round trips (and stop when allowed one), the
  fingerprint collision that gives `RE_UNNAMED`, one id at two times,
  unsorted items, no room, a client message and a server message that
  don't fit, and live, expired and replaced records in real stores.
  Root-only mutation: 159 mutants, 159 killed.
- `relay_shadow.coax` (`#authority`): match, order and unexplained; a
  known missing event on either side; the limit case both ways and under
  the limit; stripping; a replay over two stores with and without the
  known difference; a filter that doesn't read; `sh_passes`. Root-only
  mutation: 114 mutants, 114 killed.

### Checks in the example

The checks are in
[nostr/examples/checks/relay-cutover.coax](../examples/checks/relay-cutover.coax),
which loads the example:

- the trace above: five stored, the forgery refused at the signature (not
  the id), B's refusal at its line 4, the items in the hand order, the
  one difference after one round trip, the shadow's classes, "issues"
  unexplained when the known difference is withheld, and equality after
  the resend;
- both digests against values computed separately in Python by
  `relay_export_fixture.py` (`172667b2...` and `80d7a0ae...`);
- Negentropy's difference against a merge of the sorted lists over 24
  generated pairs of 5, 31, 200 and 650 items.

```shell
coax/bin/coax test nostr/examples/checks/relay-cutover.coax
```

### What was run and what was not

Run on 2026-11-01: the checks above, the mutation runs above, and the
example in debug and release with its transcript. Not run: anything
against relay.openagents.com or its database, the export, the shadow
period, the cutover and the rollback rehearsal. They wait for the owner.
The R1 differential and flood records still describe the relay this would
cut over to:
[2026-10-30-coax-relay-r1-rust-differential.md](../../docs/verification/2026-10-30-coax-relay-r1-rust-differential.md)
and
[2026-10-30-coax-relay-r1-tailnet-flood.md](../../docs/verification/2026-10-30-coax-relay-r1-tailnet-flood.md).

## Summary

- The export is JSON lines in ingest order. The import checks every id and
  signature again, counts every refusal, and gives one of three verdicts.
- A sorted-id digest decides equality; Negentropy names the difference, so
  the same check can run against the Rust relay over NIP-77.
- The shadow comparer explains a difference only with events the equality
  check already named, or the limit they push against.
- In the example, a forged status is refused at BIP-340, and the one event
  the shadow store missed is named, explains the one answer it changes,
  and is put back.
- The cutover and the rollback rehearsal are written down and wait for the
  owner; so do the store's capacity and the R2 wiring they depend on.

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [35. Store events](ch035-store-events.md)
- [37. NIP-11 relay information](ch037-nip-11-relay-information.md)
- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [39. Delete, expire and vanish](ch039-delete-expire-and-vanish.md)
- [40. Count events](ch040-count-events.md)
- [41. Search](ch041-search.md)
- [42. Protected events](ch042-protected-events.md)
- [43. Negentropy sync](ch043-negentropy-sync.md)
- [44. Limits, policy and floods](ch044-limits-policy-and-floods.md)
- [94. Differential tests](ch094-differential-tests.md)

## Sources

- [NIP-01](../nips/official/01.md): ids, signatures, replaceable events.
- [NIP-11](../nips/official/11.md): the document both relays publish.
- [NIP-77](../nips/official/77.md): Negentropy syncing.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md), section 17,
  phase R3.
- The Rust relay at `0b401561a0^`: `migrations/0001_store.sql`,
  `crates/nostr-relay/src/bulk_import.rs` and
  `deploy/backup/nostr-relay-backup`.
- The recorded capture:
  [coax/lib/nostr/fixtures/coax_relay.jsonl](../../coax/lib/nostr/fixtures/coax_relay.jsonl).
