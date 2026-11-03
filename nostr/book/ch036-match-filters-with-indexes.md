# 36. Match filters with indexes

A relay answers each filter from its store. A scan is correct but reads
every record. This chapter adds indexes and proves they return exactly what
a scan returns.

Part V, A relay. Result: Answering REQ from the store newest first with query
cost limits, then indexes by ID, author, kind and tag that never change which
events match.

## What this chapter teaches

- Query order: newest `created_at` first, then lowest ID, cut at the
  filter's limit.
- The relay's REQ limits: 32 subscriptions, 16 filters a REQ, `max_limit`
  127, and a query cost of at most 100,000 a REQ.
- Four indexes over the store: by ID, by `(pubkey, kind)`, by single-letter
  tag value, and by time.
- How to choose one index for a filter by counting its candidates, and why
  an index can never change the result: it only decides which records are
  read, and every record read still passes chapter 23's matcher.

## Where the code is

| File | What it holds |
| --- | --- |
| `coax/lib/nostr/store_index.coax` | New in this chapter: `NsiIndex`, `nsi_build`, `nsi_query`, its checks, and the generated check that compares the index with the scan |
| `coax/lib/nostr/store.coax` | `rsd_query`, the scan: every record, `nf_match`, then `ns_store_insert` |
| `coax/lib/nostr/store_core.coax` | `ns_store_newer` (the order), `ns_store_insert` (the cut at the limit), `ns_store_visible` |
| `coax/lib/nostr/relay_server.coax` | `rs_limit`, `rs_cost`, `rs_req` and the limits `RS_MAX_SUBS`, `RS_MAX_FILTERS`, `RS_MAX_LIMIT`, `RS_MAX_COST` |
| `coax/lib/nostr/filter.coax` | `nf_match`, chapter 23's matcher |
| `nostr/examples/match-indexes.coax` | The example |
| `nostr/examples/checks/match-indexes.coax` | The hand-worked counts as checks |

`store_index.coax` is not `#authority`, for the same reason as `store.coax`:
loading `store.coax` brings the relay's whole graph into every mutation run.

The relay answers REQ through the index: `coax/relay/relay.coax` keeps it
beside the store in `coax/lib/nostr/relay_store.coax`. After each admission
`nsi_add` takes the one appended record in place; an admission that may have
compacted the store (`relay_route.coax`'s `rr_may_compact`) marks it stale,
and `nsi_build` rebuilds it before the next query.

What is not built: an index on disk (a restart rebuilds it from the replayed
store), and an incremental update after a compaction.

## Query order and limits

Chapter 35's store answers a filter with `rsd_query`. It reads every record,
skips the ones that are retired or expired, and keeps those that match. Each
match goes through `ns_store_insert`, which keeps `out` sorted by
`ns_store_newer`: the larger `created_at` first, and on the same second the
lower ID first. When `out` holds `rs_limit(f)` records, a match that sorts
after all of them is dropped. The answer is the first `limit` matches in
that order.

`rs_limit(f)` is the filter's `limit` when it is below 128, and 127
otherwise. A filter with no `limit` gets 127, the relay's `default_limit`.

Before a REQ runs, `rs_req` charges each filter a cost and refuses the REQ
when the sum passes 100,000. The cost is `(limit + selectors) * factor`,
where `selectors` counts every listed ID, author, kind and tag value, and
the factor says how narrow the filter is:

| The filter has | Factor |
| --- | --- |
| `ids` | 1 |
| `authors` and `kinds` | 4 |
| a tag list | 10 |
| `authors` or `kinds` | 20 |
| only `since` or `until` | 50 |
| nothing | 100 |

An open filter `{}` costs `(127 + 0) * 100 = 12,700`. Seven of them cost
88,900 and pass; eight cost 101,600 and the REQ is closed with
`restricted: query cost exceeds the configured limit`. The frame reader
refuses a 17th filter before any cost is counted (`restricted: too many
filters`), and a 33rd subscription on one connection is refused with
`restricted: too many active subscriptions`.

## Four indexes

Each index is an array of record numbers in a fixed order, so a binary
search finds the range of records that share a key.

- **By ID.** Sorted by the 32 ID bytes. One `ids` value finds 0 or 1
  record.
- **By author and kind.** Sorted by public key, then kind. One
  `(author, kind)` pair is the range from `(author, kind)` to
  `(author, kind + 1)`. An author alone is the range of records with that
  public key, whatever their kind.
- **By tag.** One entry per tag whose name decodes to one ASCII letter and
  whose second element is a string. The key is a 31-bit hash of the name
  and the decoded value, so `["t","a"]` has the same key as
  `["t","a"]`. A hash collision only adds candidates, and `nf_match` drops
  them.
- **By time.** Sorted in query order. `since` and `until` become one range
  by binary search, and because the range is already in answer order, the
  walk stops after `limit` matches.

`nsi_build` sorts the record numbers with a heap sort whose comparison
breaks every tie by record number (`nsi_before`; tag entries by key, then
record), so records that tie in an order (one author and kind, or one tag
key) stay in record order, and each order has one right answer that a
check can state. It first sorted by insertion, which took about a minute
on the first query after a 50,000-event replay (chapter 103).

The rule that keeps the answer exact: an index must return every record
that could match (it may return more), and every record it returns goes
through `ns_store_visible`, `nf_match` and `ns_store_insert`, exactly as in
the scan. The index changes how many records are read, never which ones
match or their order.

## Choosing an index

`nsi_query` counts the candidates each index would read for the filter and
reads the smallest:

1. A filter with `ids` uses the ID index.
2. Otherwise, the time range is the default, with as many candidates as
   records between `until` and `since`.
3. A filter with `authors` counts its author (or author and kind) ranges.
   It wins when its count is no larger than the time range's.
4. Each tag list counts its values' ranges. A tag wins when its count is
   smaller than the best so far, or equal to the time range's.

The count comes from the same code that reads the index (`nsi_range` with
`read` false), so the winning count is exactly `scanned`, the number of
records the answer reads. Only the time walk can read fewer, when it stops
at the limit.

A filter whose fields `nf_valid` refuses matches nothing in the scan, and
neither does one whose `since` is after its `until`, so `nsi_query` returns
0 without reading. A refused index, one that was never
built or whose store has changed since, returns `n = -1`.

<!-- source: coax/lib/nostr/store_index.coax query -->
```jai
// Reads record i as a candidate: inserted into out in query order when it is
// visible, matches f and is not already there.
nsi_take :: (s: [] RsdStore, f: NostrFilter, now: int, i: int, n: int, limit: int, out: [] int) -> int {
  if !ns_store_visible(s, i, now) || !rsd_match(f, rsd_event(s, i)) { return n; }
  for k: 0..<n { if out[k] == i { return n; } }
  return ns_store_insert(s, i, n, limit, out);
}
// One index's candidates at[from..<to]: counted, and taken into out (n[0]
// long) when read is set. Choosing an index counts; answering reads.
nsi_range :: (s: [] RsdStore, f: NostrFilter, now: int, limit: int, out: [] int, n: [] int, read: bool, at: [] int, from: int, to: int) -> int {
  if read { for i: from..<to { n[0] = nsi_take(s, f, now, at[i], n[0], limit, out); } }
  return to - from;
}
// Each ids value's record, if it is stored.
nsi_ids :: (s: [] RsdStore, x: NsiIndex, f: NostrFilter, now: int, limit: int, out: [] int, n: [] int, read: bool) -> int {
  c := 0;
  for v: f.ids {
    key: [32] u8; nsi_hex32(v, key[..]);
    c += nsi_range(s, f, now, limit, out, n, read, x.by_id, nsi_bound(s, x.by_id, x.n, key, false, false), nsi_bound(s, x.by_id, x.n, key, false, true));
  }
  return c;
}
// Each author's records, or with kinds each (author, kind) pair's.
nsi_authors :: (s: [] RsdStore, x: NsiIndex, f: NostrFilter, now: int, limit: int, out: [] int, n: [] int, read: bool) -> int {
  c := 0;
  for a: f.authors {
    key: [32] u8; nsi_hex32(a, key[..]);
    if f.has_kinds { for k: f.kinds { c += nsi_range(s, f, now, limit, out, n, read, x.by_ak, nsi_ak_lower(s, x, key, k), nsi_ak_lower(s, x, key, k + 1)); } }
    else { c += nsi_range(s, f, now, limit, out, n, read, x.by_ak, nsi_bound(s, x.by_ak, x.n, key, true, false), nsi_bound(s, x.by_ak, x.n, key, true, true)); }
  }
  return c;
}
// Each value's entries in one tag list.
nsi_tags :: (s: [] RsdStore, x: NsiIndex, f: NostrFilter, t: NostrTagFilter, now: int, limit: int, out: [] int, n: [] int, read: bool) -> int {
  c := 0;
  for v: t.values { key := nsi_key(t.name, v); c += nsi_range(s, f, now, limit, out, n, read, x.tag_rec, nsi_tag_lower(x, key), nsi_tag_lower(x, key + 1)); }
  return c;
}
// Up to rs_limit(f) (and out.count) visible records matching f, newest
// first, then lowest id, exactly as rsd_query, read through one index.
nsi_query :: (s: [] RsdStore, ix: [] NsiIndex, f: NostrFilter, now: int, out: [] int) -> NsiResult {
  return nsi_query_limit(s, ix, f, now, rs_limit(f), out);
}
// The same with the caller's limit in place of the filter's: a NIP-45 count
// (chapter 40) passes out.count to read every match.
nsi_query_limit :: (s: [] RsdStore, ix: [] NsiIndex, f: NostrFilter, now: int, limit: int, out: [] int) -> NsiResult {
  x := ix[0];
  if !x.ok || x.n != s[0].n || x.used != s[0].used { return NsiResult.{n = -1, plan = NSI_REFUSED}; }
  later := ns_store_seen(s, now);
  if !nf_valid(f) || limit == 0 || out.count == 0 || (f.has_since && f.has_until && f.since > f.until) { return NsiResult.{}; }
  n: [1] int; tag: int;
  res := NsiResult.{plan = NSI_TIME};
  span := nsi_time_range(s, x, f);
  best := span.to - span.from;
  if f.has_ids { res.plan = NSI_ID; }
  else {
    if f.has_authors { c := nsi_authors(s, x, f, now, limit, out, n[..], false); if c <= best { best = c; res.plan = ifx f.has_kinds then NSI_AUTHOR_KIND else NSI_AUTHOR; } }
    for k: 0..<f.tags.count { c := nsi_tags(s, x, f, f.tags[k], now, limit, out, n[..], false); if c < best || (c == best && res.plan == NSI_TIME) { best = c; tag = k; res.plan = NSI_TAG; } }
  }
  if res.plan == NSI_ID { res.scanned = nsi_ids(s, x, f, now, limit, out, n[..], true); }
  else if res.plan == NSI_TAG { res.scanned = nsi_tags(s, x, f, f.tags[tag], now, limit, out, n[..], true); }
  else if res.plan == NSI_TIME {
    // time order is the answer's order: stop at the limit
    at := span.from;
    while at < span.to && n[0] < limit && n[0] < out.count { res.scanned += 1; n[0] = nsi_take(s, f, now, x.by_time[at], n[0], limit, out); at += 1; }
  } else { res.scanned = nsi_authors(s, x, f, now, limit, out, n[..], true); }
  res.n = n[0];
  return res;
}
```

A candidate passes `rsd_match` (`store.coax`): filter.coax's `nf_match`,
and, when the relay sets the filter's `gate`, whether the filter's readers
may read it. The relay sets it for every REQ, COUNT and sync; chapter 103
describes the rules (OpenAgents' private kinds, from the Rust relay). The
gate is checked before the limit, so a hidden event never takes a place in
the answer.

## Worked example: eight events and eleven filters

The example stores eight events signed with two published test keys: A is
BIP-340 vector 1's key and B is vector 2's. `NOW` is 1,793,491,200. The IDs
come from SHA-256 over each event's serialization; Python's `hashlib`
computes the same eight prefixes.

| Event | Author | Kind | Time | Tags | ID |
| --- | --- | --- | --- | --- | --- |
| e1 | A | 1 | NOW-300 | `t nostr` | `2f95523a` |
| e2 | A | 1 | NOW-200 | `t coax` | `37211f98` |
| e3 | B | 1 | NOW-200 | `t nostr`, `t coax` | `09762c70` |
| e4 | B | 7 | NOW-100 | `e` e1, `p` A | `2b76c678` |
| e5 | A | 30023 | NOW-50 | `d intro`, `t coax` | `9c6986c5` |
| e6 | B | 1 | NOW | none | `fac797e9` |
| e7 | A | 1 | NOW | `t nostr` | `9efdbc3f` |
| e8 | B | 0 | NOW-400 | none | `75e5c061` |

Work out the indexes first.

- By ID: e3 (`0976`), e4 (`2b76`), e1 (`2f95`), e2 (`3721`), e8 (`75e5`),
  e5 (`9c69`), e7 (`9efd`), e6 (`fac7`).
- By author and kind: B's public key `dd30...` sorts before A's `dff1...`,
  so B's events come first: e8 (B, 0), then e3 and e6 (B, 1), e4 (B, 7),
  then e1, e2, e7 (A, 1) and e5 (A, 30023).
- By time: e7 and e6 at NOW, with `9efd` before `fac7`; e5; e4; e3 and e2
  at NOW-200, with `0976` before `3721`; e1; e8.
- Tags: nine entries, one for each tag above. `d intro` is indexed too.

Now each filter. The scan always reads all eight records.

1. `{"ids":[e3]}`: the ID index reads 1 record. Answer: e3.
2. `{"authors":[A],"kinds":[1]}`: the `(A, 1)` range is e1, e2, e7, 3
   candidates against 8 for time. In query order: e7, e2, e1.
3. `{"#t":["coax"]}`: the `t coax` entries are e2, e3, e5, 3 candidates.
   e5 is newest; e2 and e3 tie at NOW-200, and `0976 < 3721` puts e3
   first. Answer: e5, e3, e2.
4. `{"since":NOW-200}`: the time range runs from the start to the first
   record older than NOW-200: e7, e6, e5, e4, e3, e2, 6 candidates.
   All 6 match.
5. The same with `"limit":2`: the walk stops after 2 matches, so it reads
   2 records. Answer: e7, e6.
6. `{"kinds":[1],"#t":["nostr"],"limit":2}`: `t nostr` is e1, e3, e7, 3
   candidates. All are kind 1. In order e7, e3, e1, and the limit keeps
   e7, e3.
7. `{"authors":[B]}`: B's range is e8, e3, e6, e4, 4 candidates. Answer:
   e6, e4, e3, e8.
8. `{"#e":[e1],"#p":[A]}`: each tag has 1 candidate (e4); the first tag
   wins the tie. Answer: e4.
9. `{"authors":[A],"#t":["nostr"]}`: A's range has 4, `t nostr` has 3. The
   tag wins with 3 candidates; e3 is B's, so the answer is e7, e1.
10. `{"kinds":[1],"limit":500}`: no index covers kinds alone, so the time
    range reads all 8. The limit is 127. Answer: e7, e6, e3, e2, e1.
11. `{"#t":["rust"]}`: no entry has that key; 0 records are read and the
    answer is empty.

Costs follow from the table: filter 2 is `(127 + 2) * 4 = 516`, filter 3
is `(127 + 1) * 10 = 1,280`, and filter 5 is `(2 + 0) * 50 = 100`.

The broken cases: after the index is built, e9 (B, kind 1, NOW, `t coax`)
is stored. The store now has 9 records, so `nsi_query` refuses the old
index with `n = -1`. After a rebuild, `#t coax` reads 4 candidates and
answers e9, e5, e3, e2. A build with room for only 8 records in the time
index refuses, and so does every query on that index.

## The example

The example is [nostr/examples/match-indexes.coax](../examples/match-indexes.coax).
It:

1. prints the relay's limits;
2. signs the eight events, admits each one through chapter 34's read,
   policy and signature steps, and prints the store;
3. builds the indexes and prints each order;
4. runs the eleven filters through `nsi_query` and through `rsd_query`, and
   prints the cost, the limit, the plan, the records each read and whether
   the answers are equal;
5. shows the refusals: a stale index, a short array, a REQ over the cost
   limit and a REQ with 17 filters;
6. admits the 237 events that relay.openagents.com sent for the Coax
   project page's five REQs, each verified with BIP-340, and compares the
   index's answer to each REQ with the recorded IDs and order.

Each filter goes through `ask`:

<!-- source: nostr/examples/match-indexes.coax query -->
```jai
// One filter, answered through the index and by a full scan.
ask :: (r: [] Relay, label: string, text: string) {
  strs: [64] string; ints: [64] int; tags: [8] NostrTagFilter; bytes: [1024] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter;
  #assert filter_read(text, f[..], sp[..]) == FR_OK;
  a: [16] int; b: [16] int;
  q := nsi_query(r[0].s[..], r[0].ix[..], f[0], NOW, a[..]);
  n := rsd_query(r[0].s[..], f[0], NOW, b[..]);
  same := q.n == n;
  for i: 0..<n { if same && a[i] != b[i] { same = false; } }
  print("% %\n", label, text);
  print("    cost %, limit %; plan %, read % of % records -> % events:", rs_cost(f[0]), rs_limit(f[0]), PLAN[q.plan], q.scanned, r[0].s[0].n, q.n);
  for i: 0..<q.n { print(" %", rec_name(a[i])); }
  print("\n    full scan read %: the same events in the same order: %\n", r[0].s[0].n, same);
}
```

The whole program:

<!-- source: nostr/examples/match-indexes.coax example -->
```jai
#load "../../coax/lib/nostr/store_index.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The relay's clock for the whole run: 2026-11-01T00:00:00Z.
NOW :: 1793491200;
// BIP-340 vectors 1 and 2's published test keys and aux. Never sign with a
// real key in an example.
KEY_A :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_A :: "0000000000000000000000000000000000000000000000000000000000000001";
KEY_B :: "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9";
AUX_B :: "c87aa53824b4d7ae2eb035a2b5bbbccc080e76cdc6d1692c4b0b62d798e6d906";
// Their x-only public keys (BIP-340 test-vectors.csv).
PUB_A :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
PUB_B :: "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8";

PLAN :: [6] string.["none", "by id", "by author and kind", "by author", "by tag", "by time"];

// The store, its index and the arrays both own.
Relay :: struct {
  arena: [8192] u8; recs: [16] RsdRec; s: [1] RsdStore;
  by_id: [16] int; by_ak: [16] int; by_time: [16] int; tag_key: [64] int; tag_rec: [64] int;
  ix: [1] NsiIndex;
  scratch: [4096] u8;
}
relay_init :: (r: [] Relay) {
  r[0].s[0] = RsdStore.{}; r[0].s[0].arena = r[0].arena[..]; r[0].s[0].recs = r[0].recs[..];
  r[0].ix[0] = NsiIndex.{by_id = r[0].by_id[..], by_ak = r[0].by_ak[..], by_time = r[0].by_time[..], tag_key = r[0].tag_key[..], tag_rec = r[0].tag_rec[..]};
}
// One EVENT through the relay's admission: read, signature, store.
admit :: (r: [] Relay, text: string) -> int {
  a := rs_admit_read(text, NOW, false, false, r[0].scratch[..]);
  if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(text, r[0].scratch[..], false)); }
  if a.reason != RS_ACCEPT { return a.reason; }
  return rsd_admit(r[0].s[..], a, text);
}
sign :: (key_hex: string, aux_hex: string, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(key_hex, key[..], false).error == NC_OK;
  #assert hex_decode_checked(aux_hex, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [8192] u8;
  n := EVENT_MORE;
  while n == EVENT_MORE { n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false); }
  #assert n > 0;
  return cast(string) out[0..<n];
}
to_hex :: (b: [] u8, out: [] u8) -> string {
  digits := "0123456789abcdef";
  for i: 0..<b.count { out[2 * i] = digits[b[i] >> 4]; out[2 * i + 1] = digits[b[i] & 15]; }
  return cast(string) out[0..<2 * b.count];
}
// Record i's name: records are stored in insertion order, e1 first.
rec_name :: (i: int) -> string { n := [9] string.["e1", "e2", "e3", "e4", "e5", "e6", "e7", "e8", "e9"]; return n[i]; }
show_time :: (t: int) { if t == NOW { print("NOW"); } else { print("NOW-%", NOW - t); } }

// book:begin query
// One filter, answered through the index and by a full scan.
ask :: (r: [] Relay, label: string, text: string) {
  strs: [64] string; ints: [64] int; tags: [8] NostrTagFilter; bytes: [1024] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter;
  #assert filter_read(text, f[..], sp[..]) == FR_OK;
  a: [16] int; b: [16] int;
  q := nsi_query(r[0].s[..], r[0].ix[..], f[0], NOW, a[..]);
  n := rsd_query(r[0].s[..], f[0], NOW, b[..]);
  same := q.n == n;
  for i: 0..<n { if same && a[i] != b[i] { same = false; } }
  print("% %\n", label, text);
  print("    cost %, limit %; plan %, read % of % records -> % events:", rs_cost(f[0]), rs_limit(f[0]), PLAN[q.plan], q.scanned, r[0].s[0].n, q.n);
  for i: 0..<q.n { print(" %", rec_name(a[i])); }
  print("\n    full scan read %: the same events in the same order: %\n", r[0].s[0].n, same);
}
// book:end query

// book:begin recording
// What wss://relay.openagents.com (the production Rust relay) sent for the
// Coax project page's five REQs, recorded on 2026-10-28 by
// capture.ts, now `openagents project capture` (coax/lib/nostr/fixtures/coax_relay.coax):
// 237 events, each sub's in the relay's order. The same REQs as
// coax/tests/nostr/relay_differential.py.
SUBS :: [5] string.["project", "issues", "comments", "status", "repo"];
REQS :: [5] string.[
  "{\"kinds\":[30621,30623],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"],\"#d\":[\"coax\"]}",
  "{\"kinds\":[1621],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"],\"#a\":[\"30621:9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575:coax\"]}",
  "{\"kinds\":[1111],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"]}",
  "{\"kinds\":[1630,1631,1632,1633],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"]}",
  "{\"kinds\":[30617],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"]}"];
// The capture's clock: after the newest recorded event (1793219505).
CAPTURE :: 1793220000;
Big :: struct {
  arena: [400000] u8; recs: [256] RsdRec; s: [1] RsdStore;
  by_id: [256] int; by_ak: [256] int; by_time: [256] int; tag_key: [4096] int; tag_rec: [4096] int;
  ix: [1] NsiIndex; scratch: [65536] u8;
}
frame_end :: (text: string, from: int) -> int { at := from; while at < text.count && text[at] != '\n' { at += 1; } return at; }
is_event_frame :: (l: string) -> bool { return l.count > 10 && l[0..<10] == "[\"EVENT\",\""; }
// An EVENT line's sub, and its event: the bytes after the second comma.
sub_of :: (l: string) -> string { at := 10; while l[at] != '"' { at += 1; } return l[10..<at]; }
event_of :: (l: string) -> string { c := 0; at := 0; while c < 2 { if l[at] == ',' { c += 1; } at += 1; } return l[at..<l.count - 1]; }

recording :: (b: [] Big, text: string) {
  b[0].s[0] = RsdStore.{}; b[0].s[0].arena = b[0].arena[..]; b[0].s[0].recs = b[0].recs[..];
  b[0].ix[0] = NsiIndex.{by_id = b[0].by_id[..], by_ak = b[0].by_ak[..], by_time = b[0].by_time[..], tag_key = b[0].tag_key[..], tag_rec = b[0].tag_rec[..]};
  // Admit every recorded event, each signature checked with BIP-340.
  at := 0; events := 0; ok := 0;
  while at < text.count {
    to := frame_end(text, at); l := text[at..<to]; at = to + 1;
    if !is_event_frame(l) { continue; }
    t := event_of(l);
    a := rs_admit_read(t, CAPTURE, false, false, b[0].scratch[..]);
    if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(t, b[0].scratch[..], false)); }
    if a.reason == RS_ACCEPT && rsd_admit(b[0].s[..], a, t) == RS_ACCEPT { ok += 1; }
    events += 1;
  }
  built := nsi_build(b[0].s[..], b[0].ix[..]);
  print("% recorded events, % verified and stored; index built: %, % tag entries\n", events, ok, built, b[0].ix[0].tags);
  for k: 0..<5 {
    strs: [64] string; ints: [64] int; tags: [8] NostrTagFilter; bytes: [1024] u8;
    sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
    f: [1] NostrFilter; #assert filter_read(REQS[k], f[..], sp[..]) == FR_OK;
    x: [128] int; y: [128] int;
    q := nsi_query(b[0].s[..], b[0].ix[..], f[0], CAPTURE, x[..]);
    n := rsd_query(b[0].s[..], f[0], CAPTURE, y[..]);
    // the recorded ids of this sub, in order, against the index's answer
    i := 0; same := true; at = 0;
    while at < text.count {
      to := frame_end(text, at); l := text[at..<to]; at = to + 1;
      if !is_event_frame(l) || sub_of(l) != SUBS[k] { continue; }
      h: [64] u8;
      if i >= q.n { same = false; } else { id := b[0].recs[x[i]].id; if event_of(l)[7..<71] != to_hex(id[..], h[..]) { same = false; } }
      i += 1;
    }
    scan := q.n == n; for j: 0..<n { if scan && x[j] != y[j] { scan = false; } }
    print("  % (% recorded): plan %, read % of % records, % events; recorded ids in recorded order: %; scan agrees: %\n",
      SUBS[k], i, PLAN[q.plan], q.scanned, b[0].s[0].n, q.n, same && i == q.n, scan);
  }
}
// book:end recording

main :: () {
  r: [1] Relay; relay_init(r[..]);
  print("limits: % subscriptions, % filters a REQ, max_limit %, query cost %\n\n", RS_MAX_SUBS, RS_MAX_FILTERS, RS_MAX_LIMIT, RS_MAX_COST);

  b1: [1024] u8; e1 := sign(KEY_A, AUX_A, NOW - 300, 1, "[[\"t\",\"nostr\"]]", "hello nostr", b1[..]);
  scratch: [4096] u8; h: [64] u8;
  id1 := nostr_event_read(e1, scratch[..], false).id;
  tags4: [256] u8;
  t4 := json_raw(tags4[..], 0, "[[\"e\",\""); t4 = json_raw(tags4[..], t4, to_hex(id1[..], h[..]));
  t4 = json_raw(tags4[..], t4, "\"],[\"p\",\""); t4 = json_raw(tags4[..], t4, PUB_A); t4 = json_raw(tags4[..], t4, "\"]]");
  b2: [1024] u8; e2 := sign(KEY_A, AUX_A, NOW - 200, 1, "[[\"t\",\"coax\"]]", "coax notes", b2[..]);
  b3: [1024] u8; e3 := sign(KEY_B, AUX_B, NOW - 200, 1, "[[\"t\",\"nostr\"],[\"t\",\"coax\"]]", "both tags", b3[..]);
  b4: [1024] u8; e4 := sign(KEY_B, AUX_B, NOW - 100, 7, cast(string) tags4[0..<t4], "+", b4[..]);
  b5: [1024] u8; e5 := sign(KEY_A, AUX_A, NOW - 50, 30023, "[[\"d\",\"intro\"],[\"t\",\"coax\"]]", "an article", b5[..]);
  b6: [1024] u8; e6 := sign(KEY_B, AUX_B, NOW, 1, "[]", "plain note", b6[..]);
  b7: [1024] u8; e7 := sign(KEY_A, AUX_A, NOW, 1, "[[\"t\",\"nostr\"]]", "latest", b7[..]);
  b8: [1024] u8; e8 := sign(KEY_B, AUX_B, NOW - 400, 0, "[]", "{\"name\":\"b\"}", b8[..]);
  b9: [1024] u8; e9 := sign(KEY_B, AUX_B, NOW, 1, "[[\"t\",\"coax\"]]", "late", b9[..]);

  print("the store: eight events from two test keys, A (vector 1) and B (vector 2)\n");
  texts := [8] string.[e1, e2, e3, e4, e5, e6, e7, e8];
  for i: 0..<8 {
    result := admit(r[..], texts[i]);
    rec := r[0].recs[i]; id := rec.id;
    print("  % % kind % ", rec_name(i), ifx rec.author[0] == 0xdf then "A" else "B", rec.kind); show_time(rec.created);
    print(" id % tags % -> %\n", to_hex(id[0..<4], h[..]), rsd_text(r[0].s[..], i)[rec.tags_from - rec.from..<rec.tags_to - rec.from], ifx result == RS_ACCEPT then "accepted" else "refused");
  }

  #assert nsi_build(r[0].s[..], r[0].ix[..]);
  ix := r[0].ix[0];
  print("\nthe indexes, as record names\n  by id:    ");
  for i: 0..<ix.n { id := r[0].recs[ix.by_id[i]].id; print(" %(%)", rec_name(ix.by_id[i]), to_hex(id[0..<2], h[..])); }
  print("\n  by author, kind:");
  for i: 0..<ix.n { rec := r[0].recs[ix.by_ak[i]]; print(" %(%,%)", rec_name(ix.by_ak[i]), ifx rec.author[0] == 0xdf then "A" else "B", rec.kind); }
  print("\n  by time:  ");
  for i: 0..<ix.n { print(" %", rec_name(ix.by_time[i])); }
  print("\n  % tag entries:", ix.tags);
  for i: 0..<ix.tags { print(" %", rec_name(ix.tag_rec[i])); }
  print(" (sorted by key)\n\n");

  q: [512] u8; n := 0;
  id3 := r[0].recs[2].id;
  n = json_raw(q[..], 0, "{\"ids\":[\""); n = json_raw(q[..], n, to_hex(id3[..], h[..])); n = json_raw(q[..], n, "\"]}");
  ask(r[..], "1.", cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "{\"authors\":[\""); n = json_raw(q[..], n, PUB_A); n = json_raw(q[..], n, "\"],\"kinds\":[1]}");
  ask(r[..], "2.", cast(string) q[0..<n]);
  ask(r[..], "3.", "{\"#t\":[\"coax\"]}");
  ask(r[..], "4.", "{\"since\":1793491000}");
  ask(r[..], "5.", "{\"since\":1793491000,\"limit\":2}");
  ask(r[..], "6.", "{\"kinds\":[1],\"#t\":[\"nostr\"],\"limit\":2}");
  n = json_raw(q[..], 0, "{\"authors\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}");
  ask(r[..], "7.", cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "{\"#e\":[\""); n = json_raw(q[..], n, to_hex(id1[..], h[..])); n = json_raw(q[..], n, "\"],\"#p\":[\""); n = json_raw(q[..], n, PUB_A); n = json_raw(q[..], n, "\"]}");
  ask(r[..], "8.", cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "{\"authors\":[\""); n = json_raw(q[..], n, PUB_A); n = json_raw(q[..], n, "\"],\"#t\":[\"nostr\"]}");
  ask(r[..], "9.", cast(string) q[0..<n]);
  ask(r[..], "10.", "{\"kinds\":[1],\"limit\":500}");
  ask(r[..], "11.", "{\"#t\":[\"rust\"]}");

  print("\nrefusals\n");
  f: NostrFilter; out: [16] int;
  print("e9 (B, kind 1, NOW, #t coax) -> %\n", ifx admit(r[..], e9) == RS_ACCEPT then "accepted" else "refused");
  s := nsi_query(r[0].s[..], r[0].ix[..], f, NOW, out[..]);
  print("query {} on the index built before e9 -> n %, plan % (refused: the store changed)\n", s.n, s.plan);
  print("rebuild: %\n", nsi_build(r[0].s[..], r[0].ix[..]));
  ask(r[..], "12.", "{\"#t\":[\"coax\"]}");
  r[0].ix[0].by_time = r[0].by_time[0..<8];
  print("build with room for 8 records in the time index, 9 stored -> %\n", nsi_build(r[0].s[..], r[0].ix[..]));
  s = nsi_query(r[0].s[..], r[0].ix[..], f, NOW, out[..]);
  print("query {} on that index -> n %, plan %\n", s.n, s.plan);
  open := "[\"REQ\",\"s\",{},{},{},{},{},{},{},{}]";
  why := rs_req(ms_read(open), open);
  print("REQ with 8 open filters (8 x 12700 = 101600) -> CLOSED \"%: %\"\n", rs_prefix(why), rs_text(why));
  seven := "[\"REQ\",\"s\",{},{},{},{},{},{},{}]";
  print("REQ with 7 open filters (88900) -> %\n", ifx rs_req(ms_read(seven), seven) == RS_ACCEPT then "accepted" else "refused");
  many: [256] u8; m := json_raw(many[..], 0, "[\"REQ\",\"s\"");
  for i: 0..<17 { m = json_raw(many[..], m, ",{\"limit\":1}"); }
  m = json_raw(many[..], m, "]");
  msg := ms_read(cast(string) many[0..<m]);
  print("REQ with 17 filters -> frame reader error % (MS_TOO_MANY is %): CLOSED \"%: %\"\n", msg.error, MS_TOO_MANY, rs_prefix(RS_FILTERS), rs_text(RS_FILTERS));

  print("\nthe Coax project page's five REQs against relay.openagents.com's recording\n");
  big: [1] Big;
  recording(big[..], COAX_RELAY);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/match-indexes.coax --run-steps 250000000
```

The full transcript is
[nostr/fixtures/match-indexes.out](../fixtures/match-indexes.out). The
indexes, filters 3, 5 and 9, the refusals and the recording:

```text
the indexes, as record names
  by id:     e3(0976) e4(2b76) e1(2f95) e2(3721) e8(75e5) e5(9c69) e7(9efd) e6(fac7)
  by author, kind: e8(B,0) e3(B,1) e6(B,1) e4(B,7) e1(A,1) e2(A,1) e7(A,1) e5(A,30023)
  by time:   e7 e6 e5 e4 e3 e2 e1 e8
  9 tag entries: e4 e5 e4 e2 e3 e5 e1 e3 e7 (sorted by key)
...
3. {"#t":["coax"]}
    cost 1280, limit 127; plan by tag, read 3 of 8 records -> 3 events: e5 e3 e2
    full scan read 8: the same events in the same order: true
5. {"since":1793491000,"limit":2}
    cost 100, limit 2; plan by time, read 2 of 8 records -> 2 events: e7 e6
    full scan read 8: the same events in the same order: true
9. {"authors":["dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"],"#t":["nostr"]}
    cost 1290, limit 127; plan by tag, read 3 of 8 records -> 2 events: e7 e1
    full scan read 8: the same events in the same order: true
...
refusals
e9 (B, kind 1, NOW, #t coax) -> accepted
query {} on the index built before e9 -> n -1, plan -1 (refused: the store changed)
rebuild: true
12. {"#t":["coax"]}
    cost 1280, limit 127; plan by tag, read 4 of 9 records -> 4 events: e9 e5 e3 e2
    full scan read 9: the same events in the same order: true
build with room for 8 records in the time index, 9 stored -> false
query {} on that index -> n -1, plan -1
REQ with 8 open filters (8 x 12700 = 101600) -> CLOSED "restricted: query cost exceeds the configured limit"
REQ with 7 open filters (88900) -> accepted
REQ with 17 filters -> frame reader error 5 (MS_TOO_MANY is 5): CLOSED "restricted: too many filters"

the Coax project page's five REQs against relay.openagents.com's recording
237 recorded events, 237 verified and stored; index built: true, 1372 tag entries
  project (2 recorded): plan by author and kind, read 2 of 237 records, 2 events; recorded ids in recorded order: true; scan agrees: true
  issues (105 recorded): plan by author and kind, read 105 of 237 records, 105 events; recorded ids in recorded order: true; scan agrees: true
  comments (121 recorded): plan by author and kind, read 121 of 237 records, 121 events; recorded ids in recorded order: true; scan agrees: true
  status (8 recorded): plan by author and kind, read 8 of 237 records, 8 events; recorded ids in recorded order: true; scan agrees: true
  repo (1 recorded): plan by author and kind, read 1 of 237 records, 1 events; recorded ids in recorded order: true; scan agrees: true
```

## How the code is checked

### The recording of relay.openagents.com

The last part of the example is the independent check.
`coax/lib/nostr/fixtures/coax_relay.coax` holds what the production Rust
relay at relay.openagents.com sent for the Coax project page's five REQs on
2026-10-28: 237 events. The example stores them, sends the same five
filters through the index, and compares each answer with the recorded IDs,
one by one, in the recorded order. All five agree, and the scan agrees with
the index. `coax/tests/nostr/relay_differential.py` makes the same
comparison through the running relay over a WebSocket.

<!-- source: nostr/examples/match-indexes.coax recording -->
```jai
// What wss://relay.openagents.com (the production Rust relay) sent for the
// Coax project page's five REQs, recorded on 2026-10-28 by
// capture.ts, now `openagents project capture` (coax/lib/nostr/fixtures/coax_relay.coax):
// 237 events, each sub's in the relay's order. The same REQs as
// coax/tests/nostr/relay_differential.py.
SUBS :: [5] string.["project", "issues", "comments", "status", "repo"];
REQS :: [5] string.[
  "{\"kinds\":[30621,30623],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"],\"#d\":[\"coax\"]}",
  "{\"kinds\":[1621],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"],\"#a\":[\"30621:9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575:coax\"]}",
  "{\"kinds\":[1111],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"]}",
  "{\"kinds\":[1630,1631,1632,1633],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"]}",
  "{\"kinds\":[30617],\"authors\":[\"9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575\"]}"];
// The capture's clock: after the newest recorded event (1793219505).
CAPTURE :: 1793220000;
Big :: struct {
  arena: [400000] u8; recs: [256] RsdRec; s: [1] RsdStore;
  by_id: [256] int; by_ak: [256] int; by_time: [256] int; tag_key: [4096] int; tag_rec: [4096] int;
  ix: [1] NsiIndex; scratch: [65536] u8;
}
frame_end :: (text: string, from: int) -> int { at := from; while at < text.count && text[at] != '\n' { at += 1; } return at; }
is_event_frame :: (l: string) -> bool { return l.count > 10 && l[0..<10] == "[\"EVENT\",\""; }
// An EVENT line's sub, and its event: the bytes after the second comma.
sub_of :: (l: string) -> string { at := 10; while l[at] != '"' { at += 1; } return l[10..<at]; }
event_of :: (l: string) -> string { c := 0; at := 0; while c < 2 { if l[at] == ',' { c += 1; } at += 1; } return l[at..<l.count - 1]; }

recording :: (b: [] Big, text: string) {
  b[0].s[0] = RsdStore.{}; b[0].s[0].arena = b[0].arena[..]; b[0].s[0].recs = b[0].recs[..];
  b[0].ix[0] = NsiIndex.{by_id = b[0].by_id[..], by_ak = b[0].by_ak[..], by_time = b[0].by_time[..], tag_key = b[0].tag_key[..], tag_rec = b[0].tag_rec[..]};
  // Admit every recorded event, each signature checked with BIP-340.
  at := 0; events := 0; ok := 0;
  while at < text.count {
    to := frame_end(text, at); l := text[at..<to]; at = to + 1;
    if !is_event_frame(l) { continue; }
    t := event_of(l);
    a := rs_admit_read(t, CAPTURE, false, false, b[0].scratch[..]);
    if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(t, b[0].scratch[..], false)); }
    if a.reason == RS_ACCEPT && rsd_admit(b[0].s[..], a, t) == RS_ACCEPT { ok += 1; }
    events += 1;
  }
  built := nsi_build(b[0].s[..], b[0].ix[..]);
  print("% recorded events, % verified and stored; index built: %, % tag entries\n", events, ok, built, b[0].ix[0].tags);
  for k: 0..<5 {
    strs: [64] string; ints: [64] int; tags: [8] NostrTagFilter; bytes: [1024] u8;
    sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
    f: [1] NostrFilter; #assert filter_read(REQS[k], f[..], sp[..]) == FR_OK;
    x: [128] int; y: [128] int;
    q := nsi_query(b[0].s[..], b[0].ix[..], f[0], CAPTURE, x[..]);
    n := rsd_query(b[0].s[..], f[0], CAPTURE, y[..]);
    // the recorded ids of this sub, in order, against the index's answer
    i := 0; same := true; at = 0;
    while at < text.count {
      to := frame_end(text, at); l := text[at..<to]; at = to + 1;
      if !is_event_frame(l) || sub_of(l) != SUBS[k] { continue; }
      h: [64] u8;
      if i >= q.n { same = false; } else { id := b[0].recs[x[i]].id; if event_of(l)[7..<71] != to_hex(id[..], h[..]) { same = false; } }
      i += 1;
    }
    scan := q.n == n; for j: 0..<n { if scan && x[j] != y[j] { scan = false; } }
    print("  % (% recorded): plan %, read % of % records, % events; recorded ids in recorded order: %; scan agrees: %\n",
      SUBS[k], i, PLAN[q.plan], q.scanned, b[0].s[0].n, q.n, same && i == q.n, scan);
  }
}
```

### The hand-worked counts

`nostr/examples/checks/match-indexes.coax` builds the example's store from
unsigned records with the same authors, kinds, times and tags, and asserts
the plans and candidate counts worked out above.

<!-- source: nostr/examples/checks/match-indexes.coax check -->
```jai
// Records with the example's authors, kinds, times and tags, without signatures.
add :: (r: [] Relay, i: int, author: u8, kind: int, created: int, tags: string) {
  c: RsdRec; c.id[0] = cast(u8) (i + 1); c.author[0] = author; c.kind = kind; c.created = created; c.expiry = -1;
  if kind == 30023 { c.d[0] = 'i'; c.d_n = 1; }
  c.tags_from = 0; c.tags_to = tags.count;
  #assert ns_store_admit(r[0].s[..], c, tags, true) == NSS_ACCEPT;
}
#test "the example's counts, worked by hand from its event table" {
  // e1..e8 in the example: by id the index reads 1; A's kind 1 is e1 e2 e7;
  // #t coax is e2 e3 e5; since NOW-200 is e2..e7; B is e3 e4 e6 e8.
  r: [1] Relay; relay_init(r[..]);
  add(r[..], 0, 0xdf, 1, NOW - 300, "[[\"t\",\"nostr\"]]"); add(r[..], 1, 0xdf, 1, NOW - 200, "[[\"t\",\"coax\"]]");
  add(r[..], 2, 0xdd, 1, NOW - 200, "[[\"t\",\"nostr\"],[\"t\",\"coax\"]]"); add(r[..], 3, 0xdd, 7, NOW - 100, "[[\"p\",\"x\"]]");
  add(r[..], 4, 0xdf, 30023, NOW - 50, "[[\"d\",\"intro\"],[\"t\",\"coax\"]]"); add(r[..], 5, 0xdd, 1, NOW, "[]");
  add(r[..], 6, 0xdf, 1, NOW, "[[\"t\",\"nostr\"]]"); add(r[..], 7, 0xdd, 0, NOW - 400, "[]");
  #assert nsi_build(r[0].s[..], r[0].ix[..]) && r[0].ix[0].tags == 8;
  f: NostrFilter; out: [16] int; h: [64] u8;
  a: [32] u8; a[0] = 0xdf; authors := [1] string.[to_hex(a[..], h[..])]; kinds := [1] int.[1];
  f.has_authors = true; f.authors = authors[..]; f.has_kinds = true; f.kinds = kinds[..];
  q := nsi_query(r[0].s[..], r[0].ix[..], f, NOW, out[..]);
  #assert q.plan == NSI_AUTHOR_KIND && q.scanned == 3 && q.n == 3 && out[0] == 6 && out[1] == 1 && out[2] == 0;
  f = NostrFilter.{}; values := [1] string.["coax"]; tags := [1] NostrTagFilter.[NostrTagFilter.{name = 't', values = values[..]}]; f.tags = tags[..];
  q = nsi_query(r[0].s[..], r[0].ix[..], f, NOW, out[..]);
  #assert q.plan == NSI_TAG && q.scanned == 3 && q.n == 3 && out[0] == 4;
  f = NostrFilter.{}; f.has_since = true; f.since = NOW - 200;
  q = nsi_query(r[0].s[..], r[0].ix[..], f, NOW, out[..]); #assert q.plan == NSI_TIME && q.scanned == 6 && q.n == 6;
  f.has_limit = true; f.limit = 2;
  q = nsi_query(r[0].s[..], r[0].ix[..], f, NOW, out[..]); #assert q.scanned == 2 && q.n == 2;
  f = NostrFilter.{}; a[0] = 0xdd; authors[0] = to_hex(a[..], h[..]); f.has_authors = true; f.authors = authors[..];
  q = nsi_query(r[0].s[..], r[0].ix[..], f, NOW, out[..]); #assert q.plan == NSI_AUTHOR && q.scanned == 4 && q.n == 4 && out[3] == 7;
}
```

```shell
coax/bin/coax test nostr/examples/checks/match-indexes.coax --run-steps 250000000
```

### The library's checks

`coax/bin/coax test coax/lib/nostr/store_index.coax` runs eight `#test`s.
The first is this chapter's worked example again, built from unsigned
records with the same authors, kinds, times, tags and first two ID bytes
(`NOW` is 1000). It asserts every entry of the three orders and the nine
tag entries, the keys of `t nostr`, `t coax` and `d intro`, and for each of
the eleven filters the plan, the count read and the answer. Ten more
filters pin each rule of the choice at its edge:

| Filter | Candidates | Plan, read |
| --- | --- | --- |
| `authors [A]`, `kinds [1]`, `#t nostr` | `(A, 1)` 3, tag 3 | author and kind, 3: the author index keeps a tie |
| `authors [A]`, `kinds [30023]`, `#t coax` | `(A, 30023)` 1, tag 3 | author and kind, 1 |
| `authors [B]`, `kinds [0]` | `(B, 0)` 1 | author and kind, 1: not B's kind 1 |
| `since NOW`, `until NOW-100` | | none, 0: nothing can match |
| `since NOW-200`, `until NOW-200` | time 2 | time, 2: e3, e2 |
| `kinds [1]`, room for 2 in `out` | time 8 | time, 2: the walk stops when `out` is full |
| `authors [B]`, `until NOW-300` | time 2 (e1, e8), B 4 | time, 2: e8 |
| `#d intro` and `#t coax`, in either order | 1 and 3 | tag, 1 |
| `#t nostr`, `since NOW` | time 2, tag 3 | time, 2: e7 |
| `#t nostr`, `since NOW-50` | time 3, tag 3 | tag, 3: the tag takes a tie with time |

Then it stores e9, checks that the old index is refused, rebuilds, and
asserts the new orders and `#t coax` reading 4.

The other tests:

- arrays exactly as long as the store build, a tag array one entry short
  refuses, and an ID or author past every entry reads nothing;
- each plan reads only its candidates, including escaped tag names and
  values, and `#t a` reads 3 although `t b`'s key is `t a`'s plus 1;
- the cheapest index wins, duplicate list values and duplicate tags are
  read once, and an invalid filter answers nothing;
- tag names `a`, `z`, `A` and `Z`, an empty tag value, and kinds 0 and
  65535 are indexed, and the `until` bound reads only what it allows;
- a stale or unbuilt index is refused, and each short array refuses the
  build;
- retired and expired records are read but not served.

The last test generates 500 stores of 4 to 14 records (regular,
replaceable, addressable, ephemeral and expiring kinds, tied times, escaped
tag names) and 8 filters for each from the same seed, with random `ids`,
`authors`, `kinds`, one or two tag lists, `since`, `until`, `limit`, query
time and output room. For all 4,000 filters, `nsi_query` returns the same
records in the same order as `rsd_query`. It then asserts its totals: how
many filters each plan answered (1,085 none, 1,469 by ID, 285 by author
and kind, 250 by author, 432 by tag, 479 by time), 4,718 records read,
1,554 events served, and a sum over every answer's order. Equal answers
alone can't see a change that only picks another index or reads more
records; the totals can, and they also pin the generated stores and
filters themselves.

Run on 2026-11-01: all 310 checks of `store_index.coax` (with the store
and relay checks it loads) pass, and the checks file passes. `coax mutate
coax/lib/nostr/store_index.coax --root-only` made 588 mutants and
killed all of them: zero survivors. The file is not `#authority`, and no
mutant is marked equivalent.

## Summary

- A query answers the first `limit` matches, newest first, then lowest ID;
  the limit is at most 127, and a REQ's filters may cost at most 100,000.
- An index is a sorted array of record numbers. A filter's candidates are
  one binary-searched range a key.
- `nsi_query` reads the index with the fewest candidates. The time index
  can stop at the limit, because its order is the answer's order.
- Every candidate still passes the visibility check and `nf_match`, so an
  index can't change which events match; a generated check over 4,000
  filters and the recording of relay.openagents.com check that the answers
  are equal, and hand-worked filters pin each plan and count read.

## Dependencies

- [23. Filters](ch023-filters.md)
- [35. Store events](ch035-store-events.md)

## Sources

- [NIP-01](../nips/official/01.md): filters, `limit`, and "newest first,
  then lowest id".
- [NIP-11](../nips/official/11.md): `max_subscriptions`, `max_limit` and
  `default_limit`.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
