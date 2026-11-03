# 40. Count events

Some clients need a number, not the events. This chapter adds NIP-45 counts
to chapter 36's indexed store: an exact count for any filters, and a
HyperLogLog sketch that clients can merge across relays.

Part V, A relay. Result: NIP-45 `COUNT` requests and responses, `approximate`
counts, and HyperLogLog with 256 registers whose offset comes from the filter.

## What this chapter teaches

- `["COUNT", id, filter...]` is answered by `["COUNT", id, {"count": n}]`.
  The filters are OR'd together, and an event that matches two of them
  counts once.
- `approximate: true` marks an estimated count. This relay always counts
  exactly, so it never writes the key; the client reader accepts it.
- HyperLogLog (HLL): 256 one-byte registers. The filter's first tag value
  picks an `offset`; each counted event's pubkey byte at `offset` picks a
  register, and the zero bits after it give the value. Because every relay
  derives the same offset from the same filter, a client merges sketches
  from several relays by taking the larger value in each register.
- The estimate: how many distinct pubkeys made a sketch.
- `CLOSED` when the relay refuses to count, and `NOTICE` when the
  subscription ID is too bad to close.
- The relay (`coax/relay/relay.coax`) answers `COUNT` with this code:
  `relay_route.coax` routes it (`RR_COUNT`), the request is charged as a REQ
  against the rate limit, and `relay_store.coax`'s `rd_count` calls
  `n45_answer` over a current index.
- What is not built: cached or precomputed sketches and relay-side
  approximate counts.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [message_server.coax](../../coax/lib/nostr/message_server.coax) | `ms_read` | reads a `COUNT` frame like a `REQ`: verb, subscription ID, filter spans (`#authority`) |
| [relay_server.coax](../../coax/lib/nostr/relay_server.coax) | `rs_req`, `rs_filter` | a REQ's filter rules: empty arrays, `#e`/`#p` hex, query cost (`#authority`) |
| [store_index.coax](../../coax/lib/nostr/store_index.coax) | `nsi_query_limit` | chapter 36's indexed query with the caller's limit in place of the filter's |
| [nip45.coax](../../coax/lib/nostr/nip45.coax) | `n45_count`, `n45_answer` | new: the exact count through the index, and the relay's reply to a `COUNT` frame |
| [nip45.coax](../../coax/lib/nostr/nip45.coax) | `n45_offset`, `n45_hll_add`, `n45_hll_merge`, `n45_hll_estimate`, `n45_hll_write`, `n45_hll_read` | new: the sketch |
| [nip45.coax](../../coax/lib/nostr/nip45.coax) | `n45_write_request`, `n45_write_response`, `n45_read_response` | new: the client's request and the relay's answer as frames |

`nip45.coax` is new. Its checks were written first, and `coax mutate
--root-only` on it kills 429 of 429 mutants. It is not `#authority`, for the
same reason as `store_index.coax`: loading the store brings the relay's
whole graph under every mutation run.

One library change was needed. `nsi_query` caps its answer at the filter's
`limit` (at most 127), so it can't count. It now calls `nsi_query_limit`
with `rs_limit(f)`; `n45_count` calls it with one slot per record. Chapter
36's results don't change, and `store_index.coax`'s 310 checks pass.

## The COUNT frames

A client sends the same filters it would put in a `REQ`:

```jai
["COUNT", "q1", {"kinds": [1, 7], "authors": ["dd30...4eb8"]}]
```

The relay answers with one object:

```jai
["COUNT", "q1", {"count": 2}]
["COUNT", "q1", {"count": 93412452, "approximate": true}]
["COUNT", "f", {"count": 16578, "hll": "0607...0708"}]
```

`ms_read` already reads `COUNT` client frames: chapter 24 gave it the verb,
and it reads the subscription ID and filter spans as for `REQ`. The relay
applies the same filter rules as a `REQ` (`rs_req`), so a count can't cost
more than the query it replaces.

Two choices NIP-45 leaves open:

1. **A filter's `limit` does not cap a count.** NIP-01 defines `limit` for a
   REQ's stored events; a count of the first 127 would not be a count.
   `{"kinds": [1], "limit": 1}` counts every kind 1 event.
2. **A `COUNT` needs at least one filter.** `["COUNT", "q"]` is refused with
   `CLOSED` "invalid: COUNT needs at least one filter". A `REQ` with no
   filters is allowed, but it has no answer to give either.

`n45_read_response` is the client side. It requires `count`, a
non-negative integer; `approximate` must be `true` or `false`; `hll` must be
512 lowercase hex digits. A key given twice is refused. Unknown keys are
skipped, so a later NIP can add one.

<!-- source: coax/lib/nostr/nip45.coax read -->
```jai
// A relay's COUNT answer, as a client reads it. Unknown keys are skipped.
n45_read_response :: (text: string) -> N45Response {
  r: N45Response;
  if !json_valid(text) { return n45_fail(r, N45_JSON); }
  j: [1] Json; json_start(j[..], text);
  if json_next(j[..]) != JSON_ARRAY || json_next(j[..]) != JSON_STRING { return r; }
  if !json_is(json_text(j[..]), "COUNT") { return n45_fail(r, N45_VERB); }
  if json_next(j[..]) != JSON_STRING { return r; }
  r.sub_n = json_unescape(json_text(j[..]), r.sub[..]);
  if r.sub_n < 1 || !ms_sub_ok(cast(string) r.sub[0..<r.sub_n]) { return n45_fail(r, N45_SUB); }
  if json_next(j[..]) != JSON_OBJECT { return r; }
  has_count := false; has_approximate := false;
  while true {
    t := json_next(j[..]);
    if t == JSON_OBJECT_END { break; }
    key := json_text(j[..]);
    v := json_next(j[..]);
    if json_is(key, "count") {
      if has_count { return n45_fail(r, N45_FIELD); }
      n := json_int(json_text(j[..]));
      if v != JSON_NUMBER || !n.ok || n.value < 0 { return n45_fail(r, N45_COUNT); }
      has_count = true; r.count = n.value;
    } else if json_is(key, "approximate") {
      if has_approximate || (v != JSON_TRUE && v != JSON_FALSE) { return n45_fail(r, N45_FIELD); }
      has_approximate = true; r.approximate = v == JSON_TRUE;
    } else if json_is(key, "hll") {
      if r.has_hll { return n45_fail(r, N45_FIELD); }
      buf: [N45_HLL_HEX] u8;
      if v != JSON_STRING || json_unescape(json_text(j[..]), buf[..]) != N45_HLL_HEX || !n45_hll_read(cast(string) buf[..], r.hll[..]) { return n45_fail(r, N45_HLL); }
      r.has_hll = true;
    } else { skipped := json_skip(j[..]); }
  }
  if !has_count { return n45_fail(r, N45_COUNT); }
  if json_next(j[..]) != JSON_ARRAY_END { return r; }
  return n45_fail(r, N45_OK);
}
```

## Counting through the index

`n45_count` answers each filter with `nsi_query_limit`, so each one reads
only the index range chapter 36 would pick. A `seen` mark per record makes
the OR exact: the first filter that finds a record counts it, and later
filters find it marked.

<!-- source: coax/lib/nostr/nip45.coax count -->
```jai
// The distinct visible records matching any filter, read through the index.
// out and seen need one slot per record. With 256 registers and one filter
// that has an offset, regs is that filter's sketch.
n45_count :: (s: [] RsdStore, ix: [] NsiIndex, filters: [] NostrFilter, now: int, out: [] int, seen: [] u8, regs: [] u8) -> N45Count {
  n := s[0].n;
  if out.count < n || seen.count < n { return N45Count.{n = -1}; }
  for i: 0..<n { seen[i] = 0; }
  c: N45Count;
  for f: filters {
    q := nsi_query_limit(s, ix, f, now, n, out[0..<n]);
    if q.n < 0 { return N45Count.{n = -1}; }
    c.scanned += q.scanned;
    for k: 0..<q.n { if seen[out[k]] == 0 { seen[out[k]] = 1; c.n += 1; } }
  }
  offset := 0;
  if filters.count == 1 { offset = n45_offset(filters[0]); }
  if offset == 0 || regs.count != N45_REGISTERS { return c; }
  for i: 0..<N45_REGISTERS { regs[i] = 0; }
  for i: 0..<n { if seen[i] == 1 { a := s[0].recs[i].author; n45_hll_add(regs, a[..], offset); } }
  c.hll = true;
  return c;
}
```

`out` and `seen` need one slot per stored record. When either is shorter,
or the index is out of date, `n45_count` returns `n = -1` and the relay
refuses with `CLOSED` "error: the store index is out of date".

## HyperLogLog

### The offset

A sketch only merges with another one built with the same offset. NIP-45
derives it from the filter:

1. Take the first tag filter (`#e`, `#p`, ...) and its first value.
2. Get 64 hex characters from it: the value itself if it is an event ID or
   pubkey; the pubkey of an address `<kind>:<pubkey>:<d>`; otherwise
   `sha256(value)` as hex.
3. Read the character at position 32 as a hex digit and add 8.

The offset is 8 to 23. A filter without a tag filter has none, and
`n45_offset` returns 0. The relay then writes no `hll`, and neither does it
for a `COUNT` with more than one filter: the offset belongs to one filter,
not to an OR of several.

<!-- source: coax/lib/nostr/nip45.coax offset -->
```jai
// NIP-45's offset for f: from the first tag filter's first value, 8 to 23;
// 0 when f has no tag filter (NIP-45 defines none then).
n45_offset :: (f: NostrFilter) -> int {
  if f.tags.count == 0 || f.tags[0].values.count == 0 { return 0; }
  v := f.tags[0].values[0];
  if nf_hex(v) { return json_hex(v[32]) + 8; }
  pubkey := n45_address(v);
  if pubkey != 0 { return json_hex(v[pubkey + 32]) + 8; }
  // filter_read keeps tag values to 256 bytes (filter.coax's nf_utf8)
  b: [256] u8; h: [32] u8;
  if v.count > b.count { return 0; }
  for i: 0..<v.count { b[i] = v[i]; }
  #assert sha256_hash_checked(b[0..<v.count], h[..], false) == SHA_OK;
  return cast(int) (h[16] >> 4) + 8;   // hex character 32 is byte 16's high half
}
```

Only lowercase hex counts as an ID: an uppercase pubkey is hashed. Chapter
34's filters already refuse uppercase `#e` and `#p` values.

### The registers

For each counted event:

1. The pubkey's byte at `offset` is the register index `ri`.
2. Count the zero bits from bit `(offset + 1) * 8` and add 1.
3. Store that in register `ri` if it is larger.

A pubkey whose bits after the index byte start `1` gives 1, `01` gives 2,
and so on. A run of `k` zeros has probability `2^-(k+1)`, so a register
whose largest value is `v` has seen about `2^v` pubkeys.

<!-- source: coax/lib/nostr/nip45.coax hll -->
```jai
// One pubkey into the 256 registers: its byte at offset picks the register;
// the value is 1 + the zero bits that follow, up to the end of the pubkey.
n45_hll_add :: (regs: [] u8, pubkey: [] u8, offset: int) {
  ri := cast(int) pubkey[offset];
  value := 1;
  at := offset + 1;
  while at < 32 && pubkey[at] == 0 { value += 8; at += 1; }
  if at < 32 { b := pubkey[at]; while (b & 0x80) == 0 { value += 1; b = b << 1; } }
  while cast(int) regs[ri] < value { regs[ri] += 1; }
}
// Another relay's sketch into into: the larger value in each register.
n45_hll_merge :: (into: [] u8, from: [] u8) {
  for i: 0..<N45_REGISTERS { while into[i] < from[i] { into[i] += 1; } }
}
// go-nostr's estimate of how many distinct pubkeys made the registers:
// linear counting, 256 ln(256 / empty registers), while the raw estimate
// is at most 768 and a register is empty; otherwise the raw estimate. (Its
// first test, linear counting at most 220, always agrees: then 108 or more
// registers are empty and the raw estimate is below 47072 / 108 = 436.)
// A register counts at most 44, so the estimate stays below 2^53.
n45_hll_estimate :: (regs: [] u8) -> int {
  m := 256.0; empty := 0; sum := 0.0;
  for i: 0..<N45_REGISTERS {
    if regs[i] == 0 { empty += 1; }
    p := 1.0; for k: 0..<cast(int) regs[i] { if k < 44 { p = p * 0.5; } }
    sum += p;
  }
  raw := cast(int) (0.7182725932495458 * m * m / sum);
  if raw <= 768 && empty > 0 { return cast(int) (m * log(m / cast(f64) empty)); }
  return raw;
}
```

`n45_hll_add` counts to the end of the pubkey, as NIP-45's text says, so
the value can reach 185 (184 zero bits after offset 8). go-nostr's
`nip45/hyperloglog` reads only the 56 bits after the index byte and stores
at most 57. The two differ only when 56 zero bits follow the index byte,
which a random pubkey does with probability `2^-56`; the checks show such
a pubkey.

`n45_hll_add` and `n45_hll_merge` raise a register one step at a time
instead of assigning the maximum. The result is the same, and every changed
comparison gives a different register, so the mutation run can test the
step.

### The estimate

NIP-45 leaves the estimate to implementations. `n45_hll_estimate` gives
go-nostr's:

- `raw = 0.7182725932495458 · 256² / Σ 2^-register`, the HyperLogLog
  estimate with the bias constant for 256 registers.
- If `raw` is at most 768 and some registers are empty, the result is
  linear counting, `256 · ln(256 / empty)`, which is more accurate for small
  counts.
- go-nostr first returns linear counting when it is at most 220. That test
  always agrees with the one above: linear counting at most 220 means at
  least 108 empty registers, and then `raw` is below `47072 / 108 = 436`.
  `n45_hll_estimate` leaves it out.

A register counts at most 44 in the sum, so the estimate stays below 2^53
(at most 3,234,812,183,309,094). No real sketch reaches 44: it would take
about 2^44 pubkeys in one register. With 256 registers the standard error
is about `1.04 / √256 = 6.5%`.

### Worked example: registers by hand

The follower filter for B is `{"kinds": [3], "#p": ["dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"]}`.

1. B's character 32 is `0` (the 33rd character, after
   `dd308afec5777e13121fa72b9cc1b7cc`). The offset is `0 + 8 = 8`.
2. A's pubkey is `dff1d77f2a671c5f 36 18 ...`. Byte 8 is `0x36 = 54`, the
   register. Byte 9 is `0x18 = 00011000`: three zeros, then a one. Register
   54 becomes 4.
3. C's pubkey is `f9308a019258c310 49 34 ...`. Register `0x49 = 73`;
   `0x34 = 00110100` has two leading zeros. Register 73 becomes 3.
4. D's pubkey is `25d1dff95105f525 3c 40 ...`. Register `0x3c = 60`;
   `0x40 = 01000000` has one. Register 60 becomes 2.
5. Three registers are set and 253 are empty. `raw` is far below 768, so
   the estimate is `256 · ln(256 / 253) = 3.02`, which is 3.

The reaction filter's tag value is e1's ID. In the example, e1's ID is
`e0a22dd632697fba4e68de652382d84f634b135da42311e8887476b8238c2d58`; its
character 32 is `6`, so the offset is 14. Byte 14 of B is `0xb7 = 183` and
byte 15 is `0xcc = 11001100`: register 183 becomes 1. C gives register
`0x52 = 82` from `0x29 = 00101001`, value 3; D gives `0x96 = 150` from
`0xad`, value 1.

### Worked example: a merge by hand

Take sketch `x` from the three followers above, and a sketch `y` from three
other pubkeys:

| Register | 9 | 54 | 60 | 73 |
| --- | --- | --- | --- | --- |
| `x` | 0 | 4 | 2 | 3 |
| `y` | 2 | 1 | 0 | 5 |
| `max` | 2 | 4 | 2 | 5 |

The merged sketch has four registers set and 252 empty:
`256 · ln(256 / 252) = 4.03`, so 4. The merge counts a pubkey that both
relays saw once, because the same pubkey gives the same register and value
on both. A sum of the two counts would count it twice.

## The relay's answer

`n45_answer` puts the pieces together for one frame:

<!-- source: coax/lib/nostr/nip45.coax answer -->
```jai
// The relay's reply to one COUNT frame into out: COUNT with the exact count
// (and the sketch when the filter has an offset), CLOSED when it refuses,
// NOTICE when there is no subscription id to close. The length, or -1.
n45_answer :: (text: string, s: [] RsdStore, ix: [] NsiIndex, now: int, w: [] N45Work, out: [] u8) -> int {
  m := ms_read(text);
  if m.type != MS_COUNT && m.error == MS_OK { return -1; }
  why := RS_FRAME;
  if !m.has_sub {
    if m.error == MS_SUB { why = RS_SUBID; }
    return ms_write_notice(rs_prefix(why), rs_text(why), out);
  }
  sub := cast(string) m.sub[0..<m.sub_n];
  if m.error == MS_TOO_MANY { why = RS_FILTERS; }
  else if m.error == MS_OK { why = rs_req(m, text); }
  if why != RS_ACCEPT { return ms_write_closed(sub, rs_prefix(why), rs_text(why), out); }
  if m.filters == 0 { return ms_write_closed(sub, "invalid", N45_NO_FILTER, out); }
  // together the filters must fit the work space (RS_SUB_TEXT bytes)
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = w[0].strs[..], ints = w[0].ints[..], tags = w[0].tags[..], bytes = w[0].bytes[..]};
  why = RS_SUB_SPACE;
  for k: 0..<m.filters {
    if rs_filter(text, m, k, w[0].filters[k..<m.filters], sp[..]) != FR_OK { return ms_write_closed(sub, rs_prefix(why), rs_text(why), out); }
  }
  // the relay's readers (#12631): a filter they may not ask is refused, and
  // only what they may read is counted (relay_private.coax)
  if w[0].gated {
    for k: 0..<m.filters {
      d := rp_deny(w[0].filters[k], w[0].readers);
      if d != RP_OK { return ms_write_closed(sub, RP_PREFIX[d], RP_TEXT[d], out); }
      w[0].filters[k].gate = RP_STORED; w[0].filters[k].readers = w[0].readers;
    }
  }
  slots := w[0].out[..]; seen := w[0].seen[..];
  if w[0].big_out.count > 0 { slots = w[0].big_out; seen = w[0].big_seen; }
  c := n45_count(s, ix, w[0].filters[0..<m.filters], now, slots, seen, w[0].regs[..]);
  if c.n < 0 { return ms_write_closed(sub, "error", N45_STALE, out); }
  regs := w[0].regs[0..<0]; if c.hll { regs = w[0].regs[..]; }
  return n45_write_response(sub, c.n, false, regs, out);
}
```

It writes, in this order:

1. `NOTICE` "invalid: subscription id must contain 1 to 64 characters" when
   the ID is empty or longer than 64 characters, and `NOTICE` "invalid:
   malformed message" when there is no ID at all. There is no ID to close.
2. `CLOSED` with chapter 36's reasons when the frame or a filter is refused:
   more than 16 filters, a malformed frame, bad `#e`/`#p` hex, empty arrays,
   or a query cost over 100,000.
3. `CLOSED` "invalid: COUNT needs at least one filter".
4. `CLOSED` "restricted: subscription filters exceed 8192 bytes" when the
   filters together don't fit the work space.
5. In the relay (`gated`), `CLOSED` with relay_private.coax's refusal when
   a filter asks for a private kind its readers may not ask for (chapter
   103: gift wraps need AUTH and `#p` naming only the readers), and every
   filter then counts only what those readers may read.
6. `CLOSED` "error: the store index is out of date".
6. Otherwise `COUNT`, with `hll` when the one filter has an offset.

## The example

The example is [nostr/examples/count-events.coax](../examples/count-events.coax).
It:

1. works out A's register under B's offset;
2. signs nine events with four BIP-340 test keys, admits each one through
   chapter 34's read, policy and signature steps, and builds the index;
3. sends seven `COUNT` frames through `n45_answer` and compares each count
   with a full scan of the store;
4. merges the two small sketches above;
5. builds three relays of kind 3 records from 1,000 follower pubkeys
   (`sha256("follower i")`, records without signatures): X holds followers
   0 to 599, Y holds 400 to 999, and U holds all 1,000. It compares the
   merge of X's and Y's sketches with U's;
6. reads NIP-45's two example answers and estimates their sketches;
7. shows the refusals on the relay side and on the client side.

Each frame goes through `ask`:

<!-- source: nostr/examples/count-events.coax ask -->
```jai
// One COUNT frame through the relay, and the same filters by full scan.
ask :: (r: [] Relay, text: string) {
  out: [1024] u8;
  n := n45_answer(text, r[0].s[..], r[0].ix[..], NOW, r[0].w[..], out[..]);
  print("-> %\n", text);
  show(cast(string) out[0..<n]);
  m := ms_read(text);
  if m.error != MS_OK || m.filters == 0 || rs_req(m, text) != RS_ACCEPT { return; }
  // n45_answer read the filters into the work space
  print("    full scan of % records: %\n", r[0].s[0].n, scan(r[0].s[..], r[0].w[0].filters[0..<m.filters]));
}
```

The two-relay part:

<!-- source: nostr/examples/count-events.coax relays -->
```jai
// 1,000 followers of B: pubkey i is sha256("follower i"). Records without
// signatures, admitted as verified, so that the sketches have enough
// pubkeys to estimate. Relay X holds followers 0..599, relay Y 400..999.
follower :: (i: int, out: [] u8) {
  b: [32] u8; t := "follower "; n := 0;
  for c: t { b[n] = c; n += 1; }
  d: [24] u8; k := 0; x := i;
  while true { d[k] = cast(u8) ('0' + x % 10); k += 1; x /= 10; if x == 0 { break; } }
  while k > 0 { k -= 1; b[n] = d[k]; n += 1; }
  #assert sha256_hash_checked(b[0..<n], out, false) == SHA_OK;
}
fill_followers :: (r: [] Relay, from: int, to: int) {
  relay_init(r[..]);
  tags: [96] u8; t := json_raw(tags[..], 0, "[[\"p\",\""); t = json_raw(tags[..], t, PUB_B); t = json_raw(tags[..], t, "\"]]");
  for i: from..<to {
    c: RsdRec; c.id[0] = cast(u8) (i & 255); c.id[1] = cast(u8) (i >> 8); follower(i, c.author[..]);
    c.kind = 3; c.created = NOW - i; c.expiry = -1; c.tags_to = t;
    #assert ns_store_admit(r[0].s[..], c, cast(string) tags[0..<t], true) == NSS_ACCEPT;
  }
  #assert nsi_build(r[0].s[..], r[0].ix[..]);
}
// The follower COUNT on one relay: its count and its sketch into regs.
followers :: (r: [] Relay, name: string, regs: [] u8) -> int {
  q: [256] u8; n := json_raw(q[..], 0, "[\"COUNT\",\"f\",{\"kinds\":[3],\"#p\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}]");
  out: [1024] u8;
  k := n45_answer(cast(string) q[0..<n], r[0].s[..], r[0].ix[..], NOW, r[0].w[..], out[..]);
  a := n45_read_response(cast(string) out[0..<k]);
  #assert a.error == N45_OK && a.has_hll;
  for i: 0..<256 { regs[i] = a.hll[i]; }
  print("  relay %: count %, sketch estimate %\n", name, a.count, n45_hll_estimate(regs));
  return a.count;
}
```

The whole program:

<!-- source: nostr/examples/count-events.coax example -->
```jai
#load "../../coax/lib/nostr/nip45.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The relay's clock for the whole run: 2026-11-01T00:00:00Z.
NOW :: 1793491200;
// BIP-340 vectors 0 to 3's published test keys and aux. Never sign with a
// real key in an example.
KEY_A :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_A :: "0000000000000000000000000000000000000000000000000000000000000001";
KEY_B :: "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9";
AUX_B :: "c87aa53824b4d7ae2eb035a2b5bbbccc080e76cdc6d1692c4b0b62d798e6d906";
KEY_C :: "0000000000000000000000000000000000000000000000000000000000000003";
AUX_C :: "0000000000000000000000000000000000000000000000000000000000000000";
KEY_D :: "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710";
AUX_D :: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
// Their x-only public keys (BIP-340 test-vectors.csv).
PUB_A :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
PUB_B :: "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8";
PUB_C :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
PUB_D :: "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517";

// A store of up to 1024 records, its index and the COUNT work space.
Relay :: struct {
  arena: [131072] u8; recs: [1024] RsdRec; s: [1] RsdStore;
  by_id: [1024] int; by_ak: [1024] int; by_time: [1024] int; tag_key: [2048] int; tag_rec: [2048] int;
  ix: [1] NsiIndex;
  w: [1] N45Work;
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
who :: (author: [32] u8) -> string {
  if author[0] == 0xdf { return "A"; } if author[0] == 0xdd { return "B"; } if author[0] == 0xf9 { return "C"; }
  return "D";
}
// The records a full scan finds: visible and matching any filter.
scan :: (s: [] RsdStore, filters: [] NostrFilter) -> int {
  n := 0;
  for i: 0..<s[0].n {
    hit := false;
    for f: filters { if !hit && ns_store_visible(s, i, NOW) && nf_match(f, rsd_event(s, i)) { hit = true; } }
    if hit { n += 1; }
  }
  return n;
}
// A reply, with an hll shown as its non-zero registers.
show :: (frame: string) {
  at := 0;
  while at + 7 <= frame.count && frame[at..<at + 7] != "\"hll\":\"" { at += 1; }
  if at + 7 > frame.count { print("    <- %\n", frame); return; }
  print("    <- %\"hll\":\"<512 hex digits>\"}]\n", frame[0..<at]);
  r := n45_read_response(frame);
  print("       non-zero registers (index=value):");
  for i: 0..<256 { if r.hll[i] != 0 { print(" %=%", i, r.hll[i]); } }
  print("; estimate %\n", n45_hll_estimate(r.hll[..]));
}

// book:begin ask
// One COUNT frame through the relay, and the same filters by full scan.
ask :: (r: [] Relay, text: string) {
  out: [1024] u8;
  n := n45_answer(text, r[0].s[..], r[0].ix[..], NOW, r[0].w[..], out[..]);
  print("-> %\n", text);
  show(cast(string) out[0..<n]);
  m := ms_read(text);
  if m.error != MS_OK || m.filters == 0 || rs_req(m, text) != RS_ACCEPT { return; }
  // n45_answer read the filters into the work space
  print("    full scan of % records: %\n", r[0].s[0].n, scan(r[0].s[..], r[0].w[0].filters[0..<m.filters]));
}
// book:end ask

// book:begin relays
// 1,000 followers of B: pubkey i is sha256("follower i"). Records without
// signatures, admitted as verified, so that the sketches have enough
// pubkeys to estimate. Relay X holds followers 0..599, relay Y 400..999.
follower :: (i: int, out: [] u8) {
  b: [32] u8; t := "follower "; n := 0;
  for c: t { b[n] = c; n += 1; }
  d: [24] u8; k := 0; x := i;
  while true { d[k] = cast(u8) ('0' + x % 10); k += 1; x /= 10; if x == 0 { break; } }
  while k > 0 { k -= 1; b[n] = d[k]; n += 1; }
  #assert sha256_hash_checked(b[0..<n], out, false) == SHA_OK;
}
fill_followers :: (r: [] Relay, from: int, to: int) {
  relay_init(r[..]);
  tags: [96] u8; t := json_raw(tags[..], 0, "[[\"p\",\""); t = json_raw(tags[..], t, PUB_B); t = json_raw(tags[..], t, "\"]]");
  for i: from..<to {
    c: RsdRec; c.id[0] = cast(u8) (i & 255); c.id[1] = cast(u8) (i >> 8); follower(i, c.author[..]);
    c.kind = 3; c.created = NOW - i; c.expiry = -1; c.tags_to = t;
    #assert ns_store_admit(r[0].s[..], c, cast(string) tags[0..<t], true) == NSS_ACCEPT;
  }
  #assert nsi_build(r[0].s[..], r[0].ix[..]);
}
// The follower COUNT on one relay: its count and its sketch into regs.
followers :: (r: [] Relay, name: string, regs: [] u8) -> int {
  q: [256] u8; n := json_raw(q[..], 0, "[\"COUNT\",\"f\",{\"kinds\":[3],\"#p\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}]");
  out: [1024] u8;
  k := n45_answer(cast(string) q[0..<n], r[0].s[..], r[0].ix[..], NOW, r[0].w[..], out[..]);
  a := n45_read_response(cast(string) out[0..<k]);
  #assert a.error == N45_OK && a.has_hll;
  for i: 0..<256 { regs[i] = a.hll[i]; }
  print("  relay %: count %, sketch estimate %\n", name, a.count, n45_hll_estimate(regs));
  return a.count;
}
// book:end relays

main :: () {
  r: [1] Relay; relay_init(r[..]);
  h: [64] u8; h2: [64] u8; scratch: [4096] u8;

  print("hand-worked register: pubkey A under the follower filter for B\n");
  b: [32] u8; n45_t_hex32(PUB_B, b[..]);
  print("  B's hex character 32 is '%': offset % + 8 = %\n", cast(string) PUB_B[32..<33], json_hex(PUB_B[32]), json_hex(PUB_B[32]) + 8);
  a: [32] u8; n45_t_hex32(PUB_A, a[..]); regs: [256] u8;
  print("  A's byte 8 is 0x% = %: the register; byte 9 is 0x%\n", to_hex(a[8..<9], h[..]), a[8], to_hex(a[9..<10], h2[..]));
  n45_hll_add(regs[..], a[..], 8);
  print("  register 54 = % (3 leading zero bits + 1)\n\n", regs[54]);

  print("the store: nine events from four test keys, A B C D (BIP-340 vectors 1, 2, 0, 3)\n");
  b1: [1024] u8; e1 := sign(KEY_A, AUX_A, NOW - 300, 1, "[]", "hello nostr", b1[..]);
  id1 := nostr_event_read(e1, scratch[..], false).id;
  hex1 := to_hex(id1[..], h[..]);
  react: [256] u8; t := json_raw(react[..], 0, "[[\"e\",\""); t = json_raw(react[..], t, hex1); t = json_raw(react[..], t, "\"],[\"p\",\""); t = json_raw(react[..], t, PUB_A); t = json_raw(react[..], t, "\"]]");
  fb: [96] u8; f := json_raw(fb[..], 0, "[[\"p\",\""); f = json_raw(fb[..], f, PUB_B); f = json_raw(fb[..], f, "\"]]");
  fa: [96] u8; g := json_raw(fa[..], 0, "[[\"p\",\""); g = json_raw(fa[..], g, PUB_A); g = json_raw(fa[..], g, "\"]]");
  fba: [192] u8; k := json_raw(fba[..], 0, "[[\"p\",\""); k = json_raw(fba[..], k, PUB_B); k = json_raw(fba[..], k, "\"],[\"p\",\""); k = json_raw(fba[..], k, PUB_A); k = json_raw(fba[..], k, "\"]]");
  rt := cast(string) react[0..<t];
  b2: [1024] u8; e2 := sign(KEY_B, AUX_B, NOW - 200, 1, "[]", "hi", b2[..]);
  b3: [1024] u8; e3 := sign(KEY_B, AUX_B, NOW - 100, 7, rt, "+", b3[..]);
  b4: [1024] u8; e4 := sign(KEY_C, AUX_C, NOW - 90, 7, rt, "+", b4[..]);
  b5: [1024] u8; e5 := sign(KEY_D, AUX_D, NOW - 80, 7, rt, "+", b5[..]);
  b6: [1024] u8; e6 := sign(KEY_A, AUX_A, NOW - 50, 3, cast(string) fb[0..<f], "", b6[..]);
  b7: [1024] u8; e7 := sign(KEY_C, AUX_C, NOW - 40, 3, cast(string) fba[0..<k], "", b7[..]);
  b8: [1024] u8; e8 := sign(KEY_D, AUX_D, NOW - 30, 3, cast(string) fb[0..<f], "", b8[..]);
  b9: [1024] u8; e9 := sign(KEY_B, AUX_B, NOW - 20, 3, cast(string) fa[0..<g], "", b9[..]);
  texts := [9] string.[e1, e2, e3, e4, e5, e6, e7, e8, e9];
  what := [9] string.["a note", "a note", "+ to e1", "+ to e1", "+ to e1", "follows B", "follows B and A", "follows B", "follows A"];
  for i: 0..<9 {
    result := admit(r[..], texts[i]);
    rec := r[0].recs[i]; id := rec.id;
    print("  e% % kind % id %: % -> %\n", i + 1, who(rec.author), rec.kind, to_hex(id[0..<4], h2[..]), what[i], ifx result == RS_ACCEPT then "accepted" else "refused");
  }
  #assert nsi_build(r[0].s[..], r[0].ix[..]);
  print("  e1's id is %\n  its hex character 32 is '%': the reaction filter's offset is %\n\n", hex1, cast(string) hex1[32..<33], json_hex(hex1[32]) + 8);

  print("COUNT requests\n");
  q: [512] u8; n := 0;
  n = json_raw(q[..], 0, "[\"COUNT\",\"q1\",{\"kinds\":[1,7],\"authors\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}]");
  ask(r[..], cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "[\"COUNT\",\"reactions\",{\"kinds\":[7],\"#e\":[\""); n = json_raw(q[..], n, hex1); n = json_raw(q[..], n, "\"]}]");
  ask(r[..], cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "[\"COUNT\",\"followers\",{\"kinds\":[3],\"#p\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}]");
  ask(r[..], cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "[\"COUNT\",\"q4\",{\"kinds\":[7]}]"); ask(r[..], cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "[\"COUNT\",\"q5\",{\"authors\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}]");
  ask(r[..], cast(string) q[0..<n]);
  n = json_raw(q[..], 0, "[\"COUNT\",\"q6\",{\"kinds\":[7]},{\"authors\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}]");
  ask(r[..], cast(string) q[0..<n]);
  ask(r[..], "[\"COUNT\",\"q7\",{\"kinds\":[1],\"limit\":1}]");

  print("\nmerging two small sketches by hand\n");
  x: [256] u8; y: [256] u8;
  x[54] = 4; x[73] = 3; x[60] = 2;          // A, C, D above
  y[54] = 1; y[73] = 5; y[9] = 2;           // three other pubkeys
  print("  x: 54=% 73=% 60=%, estimate %; y: 54=% 73=% 9=%, estimate %\n", x[54], x[73], x[60], n45_hll_estimate(x[..]), y[54], y[73], y[9], n45_hll_estimate(y[..]));
  n45_hll_merge(x[..], y[..]);
  print("  merged:"); for i: 0..<256 { if x[i] != 0 { print(" %=%", i, x[i]); } }
  print(", estimate %\n", n45_hll_estimate(x[..]));

  print("\ntwo relays, 1000 followers of B, 200 on both\n");
  rx: [256] u8; ry: [256] u8; ru: [256] u8;
  fill_followers(r[..], 0, 600); cx := followers(r[..], "X (0..599)", rx[..]);
  fill_followers(r[..], 400, 1000); cy := followers(r[..], "Y (400..999)", ry[..]);
  fill_followers(r[..], 0, 1000); cu := followers(r[..], "U (0..999, the union)", ru[..]);
  print("  X + Y counts % (200 twice); merged sketch estimate ", cx + cy);
  n45_hll_merge(rx[..], ry[..]);
  same := true; for i: 0..<256 { if rx[i] != ru[i] { same = false; } }
  print("%; the merged sketch equals U's register for register: %\n", n45_hll_estimate(rx[..]), same);
  hu: [600] u8; m := n45_hll_write(ru[..], hu[..], 0); hd: [32] u8;
  #assert sha256_hash_checked(ru[..], hd[..], false) == SHA_OK;
  print("  sha256 of U's 256 registers: %\n", to_hex(hd[..], h[..]));

  print("\nNIP-45's two example answers\n");
  nf := n45_read_response(NIP45_FOLLOWERS);
  nr := n45_read_response(NIP45_REACTIONS);
  print("  followers: count %, the sketch estimates %\n", nf.count, n45_hll_estimate(nf.hll[..]));
  print("  reactions: count %, the sketch estimates % (register 1 is %: 2^-239 adds nothing)\n", nr.count, n45_hll_estimate(nr.hll[..]), nr.hll[1]);

  print("\nrefusals, relay side\n");
  relay_init(r[..]);
  for i: 0..<9 { #assert admit(r[..], texts[i]) == RS_ACCEPT; }
  #assert nsi_build(r[0].s[..], r[0].ix[..]);
  ask(r[..], "[\"COUNT\",\"\",{\"kinds\":[1]}]");
  long: [128] u8; n = json_raw(long[..], 0, "[\"COUNT\",\"");
  for i: 0..<65 { n = json_raw(long[..], n, "x"); }
  n = json_raw(long[..], n, "\",{}]");
  ask(r[..], cast(string) long[0..<n]);
  ask(r[..], "[\"COUNT\",\"q\"]");
  ask(r[..], "[\"COUNT\",\"q\",{\"kinds\":[1]},2]");
  ask(r[..], "[\"COUNT\",\"q\",{\"#p\":[\"B\"]}]");
  ask(r[..], "[\"COUNT\",\"q\",{},{},{},{},{},{},{},{}]");
  bx: [1024] u8; ex := sign(KEY_C, AUX_C, NOW, 1, "[]", "after the index", bx[..]);
  print("a tenth event admitted, index not rebuilt: %\n", ifx admit(r[..], ex) == RS_ACCEPT then "accepted" else "refused");
  ask(r[..], "[\"COUNT\",\"q\",{\"kinds\":[1]}]");

  print("\nrefusals, client side\n");
  good: [700] u8; w := n45_write_response("f", 3, false, ru[..], good[..]);
  frame := cast(string) good[0..<w];
  print("  the relay's answer as written: %\n", N45_NAMES[n45_read_response(frame).error]);
  bad: [700] u8;
  for i: 0..<w { bad[i] = good[i]; }
  bad[30 + 511] = '"'; bad[30 + 512] = '}'; bad[30 + 513] = ']';   // two digits short
  print("  hll of 510 digits: %\n", N45_NAMES[n45_read_response(cast(string) bad[0..<w - 2]).error]);
  for i: 0..<w { bad[i] = good[i]; }
  bad[40] = 'g'; print("  hll with a 'g': %\n", N45_NAMES[n45_read_response(cast(string) bad[0..<w]).error]);
  bad[40] = 'A'; print("  hll with an uppercase 'A': %\n", N45_NAMES[n45_read_response(cast(string) bad[0..<w]).error]);
  print("  count -1: %\n", N45_NAMES[n45_read_response("[\"COUNT\",\"f\",{\"count\":-1}]").error]);
  print("  no count: %\n", N45_NAMES[n45_read_response("[\"COUNT\",\"f\",{\"approximate\":true}]").error]);
  print("  count twice: %\n", N45_NAMES[n45_read_response("[\"COUNT\",\"f\",{\"count\":1,\"count\":2}]").error]);
  print("  empty id: %\n", N45_NAMES[n45_read_response("[\"COUNT\",\"\",{\"count\":1}]").error]);
  print("  an EOSE: %\n", N45_NAMES[n45_read_response("[\"EOSE\",\"f\"]").error]);
}
N45_NAMES :: [8] string.["N45_OK", "N45_JSON", "N45_SHAPE", "N45_VERB", "N45_SUB", "N45_COUNT", "N45_HLL", "N45_FIELD"];

// NIP-45's "Followers count" and "Reaction counts" answers, with the
// subscription id filled in.
NIP45_FOLLOWERS :: "[\"COUNT\",\"s\",{\"count\": 16578, \"hll\": \"0607070505060806050508060707070706090d080b0605090607070b07090606060b0705070709050807080805080407060906080707080507070805060509040a0b06060704060405070706080607050907070b08060808080b080607090a06060805060604070908050607060805050d05060906090809080807050e0705070507060907060606070708080b0807070708080706060609080705060604060409070a0808050a0506050b0810060a0908070709080b0a07050806060508060607080606080707050806080c0a0707070a080808050608080f070506070706070a0908090c080708080806090508060606090906060d07050708080405070708\"}]";
NIP45_REACTIONS :: "[\"COUNT\",\"s\",{\"count\": 2044, \"hll\": \"01ef070505060806050508060707070706090d080b0605090607070b07090606060b0705070709050807080805080407060906080707080507070805060509040a0b06060704060405070706080607050907070b08060808080b080607090a06060805060604070908050607060805050d05060906090809080807050e0705070507060907060606070708080b0807070708080706060609080705060604060409070a0808050a0506050b0810060a0908070709080b0a07050806060508060607080606080707050806080c0a0707070a080808050608080f070506070706070a0908090c080708080806090508060606090906060d07050708080405070708\"}]";
```

Run it:

```shell
coax/bin/coax run nostr/examples/count-events.coax --run-steps 250000000
```

The output, in debug and in release:

```text
hand-worked register: pubkey A under the follower filter for B
  B's hex character 32 is '0': offset 0 + 8 = 8
  A's byte 8 is 0x36 = 54: the register; byte 9 is 0x18
  register 54 = 4 (3 leading zero bits + 1)

the store: nine events from four test keys, A B C D (BIP-340 vectors 1, 2, 0, 3)
  e1 A kind 1 id e0a22dd6: a note -> accepted
  e2 B kind 1 id 196dcd16: a note -> accepted
  e3 B kind 7 id 36d3b8f1: + to e1 -> accepted
  e4 C kind 7 id 42f87548: + to e1 -> accepted
  e5 D kind 7 id 73c145f4: + to e1 -> accepted
  e6 A kind 3 id e7a52e91: follows B -> accepted
  e7 C kind 3 id 4c7c47b0: follows B and A -> accepted
  e8 D kind 3 id e23fdf8b: follows B -> accepted
  e9 B kind 3 id 94f481ac: follows A -> accepted
  e1's id is e0a22dd632697fba4e68de652382d84f634b135da42311e8887476b8238c2d58
  its hex character 32 is '6': the reaction filter's offset is 14

COUNT requests
-> ["COUNT","q1",{"kinds":[1,7],"authors":["dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"]}]
    <- ["COUNT","q1",{"count":2}]
    full scan of 9 records: 2
-> ["COUNT","reactions",{"kinds":[7],"#e":["e0a22dd632697fba4e68de652382d84f634b135da42311e8887476b8238c2d58"]}]
    <- ["COUNT","reactions",{"count":3,"hll":"<512 hex digits>"}]
       non-zero registers (index=value): 82=3 150=1 183=1; estimate 3
    full scan of 9 records: 3
-> ["COUNT","followers",{"kinds":[3],"#p":["dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"]}]
    <- ["COUNT","followers",{"count":3,"hll":"<512 hex digits>"}]
       non-zero registers (index=value): 54=4 60=2 73=3; estimate 3
    full scan of 9 records: 3
-> ["COUNT","q4",{"kinds":[7]}]
    <- ["COUNT","q4",{"count":3}]
    full scan of 9 records: 3
-> ["COUNT","q5",{"authors":["dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"]}]
    <- ["COUNT","q5",{"count":3}]
    full scan of 9 records: 3
-> ["COUNT","q6",{"kinds":[7]},{"authors":["dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"]}]
    <- ["COUNT","q6",{"count":5}]
    full scan of 9 records: 5
-> ["COUNT","q7",{"kinds":[1],"limit":1}]
    <- ["COUNT","q7",{"count":2}]
    full scan of 9 records: 2

merging two small sketches by hand
  x: 54=4 73=3 60=2, estimate 3; y: 54=1 73=5 9=2, estimate 3
  merged: 9=2 54=4 60=2 73=5, estimate 4

two relays, 1000 followers of B, 200 on both
  relay X (0..599): count 600, sketch estimate 616
  relay Y (400..999): count 600, sketch estimate 566
  relay U (0..999, the union): count 1000, sketch estimate 1095
  X + Y counts 1200 (200 twice); merged sketch estimate 1095; the merged sketch equals U's register for register: true
  sha256 of U's 256 registers: e36391be961d85e585019ffa4f26d5c765392b80bb7e4d33b81b01891747c505

NIP-45's two example answers
  followers: count 16578, the sketch estimates 15070
  reactions: count 2044, the sketch estimates 13075 (register 1 is 239: 2^-239 adds nothing)

refusals, relay side
-> ["COUNT","",{"kinds":[1]}]
    <- ["NOTICE","invalid: subscription id must contain 1 to 64 characters"]
-> ["COUNT","xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",{}]
    <- ["NOTICE","invalid: subscription id must contain 1 to 64 characters"]
-> ["COUNT","q"]
    <- ["CLOSED","q","invalid: COUNT needs at least one filter"]
-> ["COUNT","q",{"kinds":[1]},2]
    <- ["CLOSED","q","invalid: malformed message"]
-> ["COUNT","q",{"#p":["B"]}]
    <- ["CLOSED","q","invalid: #p values must be 64 lowercase hexadecimal characters"]
-> ["COUNT","q",{},{},{},{},{},{},{},{}]
    <- ["CLOSED","q","restricted: query cost exceeds the configured limit"]
a tenth event admitted, index not rebuilt: accepted
-> ["COUNT","q",{"kinds":[1]}]
    <- ["CLOSED","q","error: the store index is out of date"]
    full scan of 10 records: 3

refusals, client side
  the relay's answer as written: N45_OK
  hll of 510 digits: N45_HLL
  hll with a 'g': N45_HLL
  hll with an uppercase 'A': N45_HLL
  count -1: N45_COUNT
  no count: N45_COUNT
  count twice: N45_FIELD
  empty id: N45_SUB
  an EOSE: N45_VERB
```

What the output shows:

- Every count equals the full scan's. q6 is the OR: three reactions and
  three events by B, with e3 in both, count 5.
- q7's `limit` of 1 doesn't cap its count of 2.
- The follower sketch has the hand-worked registers 54=4, 60=2 and 73=3,
  and the reaction sketch has 82=3, 150=1 and 183=1.
- X and Y count 600 each. Their sum, 1,200, counts the 200 shared followers
  twice. The merged sketch equals U's sketch register for register, and
  estimates 1,095 for the 1,000 followers: 9.5% high, within two standard
  errors.
- NIP-45's follower example estimates 15,070 for a stated count of 16,578
  (9% low). Its reaction example reuses the follower sketch with the first
  two registers changed; register 1 is 239, which adds `2^-239` to the sum,
  so the estimate is 13,075, not the stated 2,044. The NIP's examples show
  the format, not consistent values.
- The stale index refusal: the tenth event is admitted, the index isn't
  rebuilt, and the relay refuses to count rather than miss it. A full scan
  finds 3.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/count-events.coax](../examples/checks/count-events.coax),
which loads the example, so running the example doesn't run them:

- the 1,000 follower pubkeys give the sketches and estimates of a Python
  transcription of NIP-45's text and go-nostr's estimate (X 616, Y 566,
  union 1,095), the merge equals the union, and the union's registers hash
  to the transcription's SHA-256;
- NIP-45's two example sketches estimate 15,070 and 13,075, as in the
  transcription;
- the registers and the merge worked by hand above, and a pubkey with 64
  zero bits after its index byte, where NIP-45's text gives 65;
- exact counts of a four-event store worked by hand: an OR of two filters,
  and three copies of one filter counting once.

<!-- source: nostr/examples/checks/count-events.coax check -->
```jai
// Values from a Python transcription of NIP-45's text (register update and
// offset) and of go-nostr's estimate, written for this chapter. It shares no
// code with nip45.coax:
//
//   def offset(v):   # 64 lowercase hex, else an address's pubkey, else sha256
//       h = v if re.fullmatch('[0-9a-f]{64}', v) else ... hashlib.sha256(v).hexdigest()
//       return int(h[32], 16) + 8
//   def add(regs, pk, off):
//       b = bytes.fromhex(pk); bits = ''.join(f'{x:08b}' for x in b[off+1:])
//       regs[b[off]] = max(regs[b[off]], len(bits) - len(bits.lstrip('0')) + 1)
//   def est(regs):   # linear counting below 220 or while raw <= 768
//       ...
//
// It gave, for pubkey i = sha256("follower i") under B's offset 8: relay X
// (0..599) 616, relay Y (400..999) 566, the union 1095, max(X, Y) == union,
// and sha256 of the union's registers below; and for NIP-45's two example
// sketches, 15070 and 13075.
U_REGISTERS_SHA256 :: "e36391be961d85e585019ffa4f26d5c765392b80bb7e4d33b81b01891747c505";

#test "the follower sketches against the Python transcription" #steps 2000000000 {
  // The pubkeys straight into the sketches; the example builds the same
  // three through 1,000-record stores and n45_answer, and its transcript
  // holds the same estimates.
  x: [256] u8; y: [256] u8; u: [256] u8; k: [32] u8;
  for i: 0..<1000 {
    follower(i, k[..]);
    if i < 600 { n45_hll_add(x[..], k[..], 8); }
    if i >= 400 { n45_hll_add(y[..], k[..], 8); }
    n45_hll_add(u[..], k[..], 8);
  }
  #assert n45_hll_estimate(x[..]) == 616 && n45_hll_estimate(y[..]) == 566 && n45_hll_estimate(u[..]) == 1095;
  n45_hll_merge(x[..], y[..]);
  for i: 0..<256 { #assert x[i] == u[i]; }
  d: [32] u8; h: [64] u8;
  #assert sha256_hash_checked(u[..], d[..], false) == SHA_OK && to_hex(d[..], h[..]) == U_REGISTERS_SHA256;
  // merging is order-free and idempotent
  n45_hll_merge(y[..], u[..]); for i: 0..<256 { #assert y[i] == u[i]; }
}
#test "NIP-45's example sketches against the Python transcription" {
  f := n45_read_response(NIP45_FOLLOWERS); r := n45_read_response(NIP45_REACTIONS);
  #assert f.error == N45_OK && f.count == 16578 && n45_hll_estimate(f.hll[..]) == 15070;
  #assert r.error == N45_OK && r.count == 2044 && n45_hll_estimate(r.hll[..]) == 13075;
  // the two differ only in their first two registers
  for i: 2..<256 { #assert f.hll[i] == r.hll[i]; }
}
#test "registers worked by hand from the BIP-340 public keys" {
  // offset 8 (B's character 32 is '0'):
  //   A dff1d77f2a671c5f 36 18...: register 0x36 = 54, 0x18 = 00011000 -> 4
  //   C f9308a019258c310 49 34...: register 0x49 = 73, 0x34 = 00110100 -> 3
  //   D 25d1dff95105f525 3c 40...: register 0x3c = 60, 0x40 = 01000000 -> 2
  regs: [256] u8; k: [32] u8;
  n45_t_hex32(PUB_A, k[..]); n45_hll_add(regs[..], k[..], 8);
  n45_t_hex32(PUB_C, k[..]); n45_hll_add(regs[..], k[..], 8);
  n45_t_hex32(PUB_D, k[..]); n45_hll_add(regs[..], k[..], 8);
  for i: 0..<256 { #assert regs[i] == ifx i == 54 then cast(u8) 4 else ifx i == 73 then cast(u8) 3 else ifx i == 60 then cast(u8) 2 else cast(u8) 0; }
  // three registers set, 253 empty: 256 ln(256/253) = 3.02
  #assert n45_hll_estimate(regs[..]) == 3;
  // the hand merge in the example: max per register
  y: [256] u8; y[54] = 1; y[73] = 5; y[9] = 2;
  n45_hll_merge(regs[..], y[..]);
  #assert regs[9] == 2 && regs[54] == 4 && regs[60] == 2 && regs[73] == 5 && n45_hll_estimate(regs[..]) == 4;
  // where NIP-45's text and go-nostr part: 64 zero bits after the index byte.
  // NIP-45 counts on to the end of the pubkey (65); go-nostr reads 56 bits
  // and would store 57.
  z: [32] u8; z[8] = 1; z[17] = 0x80; r2: [256] u8;
  n45_hll_add(r2[..], z[..], 8); #assert r2[1] == 65;
  z[17] = 0; z[16] = 1; n45_hll_add(r2[..], z[..], 8); #assert r2[1] == 65;   // 63 zeros: 64 is smaller
}
#test "exact counts worked by hand from the example's nine events" #steps 2000000000 {
  // q1 B's kinds 1 and 7: e2 e3. reactions to e1: e3 e4 e5. followers of B:
  // e6 e7 e8. kinds 7 OR authors B: {e3 e4 e5} + {e2 e3 e9} = 5, e3 once.
  r: [1] Relay; relay_init(r[..]);
  b1: [1024] u8; e1 := sign(KEY_A, AUX_A, NOW - 300, 1, "[]", "hello nostr", b1[..]);
  b2: [1024] u8; e2 := sign(KEY_B, AUX_B, NOW - 200, 7, "[]", "+", b2[..]);
  b3: [1024] u8; e3 := sign(KEY_B, AUX_B, NOW - 100, 1, "[]", "hi", b3[..]);
  b4: [1024] u8; e4 := sign(KEY_C, AUX_C, NOW - 90, 7, "[]", "+", b4[..]);
  #assert admit(r[..], e1) == RS_ACCEPT && admit(r[..], e2) == RS_ACCEPT && admit(r[..], e3) == RS_ACCEPT && admit(r[..], e4) == RS_ACCEPT;
  #assert nsi_build(r[0].s[..], r[0].ix[..]);
  out: [256] u8; q: [256] u8;
  n := json_raw(q[..], 0, "[\"COUNT\",\"q\",{\"kinds\":[7]},{\"authors\":[\""); n = json_raw(q[..], n, PUB_B); n = json_raw(q[..], n, "\"]}]");
  k := n45_answer(cast(string) q[0..<n], r[0].s[..], r[0].ix[..], NOW, r[0].w[..], out[..]);
  #assert cast(string) out[0..<k] == "[\"COUNT\",\"q\",{\"count\":3}]";   // e2 e4 and e3
  k = n45_answer("[\"COUNT\",\"q\",{\"kinds\":[1,7]}]", r[0].s[..], r[0].ix[..], NOW, r[0].w[..], out[..]);
  #assert cast(string) out[0..<k] == "[\"COUNT\",\"q\",{\"count\":4}]";
  k = n45_answer("[\"COUNT\",\"q\",{\"kinds\":[1]},{\"kinds\":[1]},{\"kinds\":[1]}]", r[0].s[..], r[0].ix[..], NOW, r[0].w[..], out[..]);
  #assert cast(string) out[0..<k] == "[\"COUNT\",\"q\",{\"count\":2}]";
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/count-events.coax --run-steps 250000000
```

On 2026-11-01 `coax test` ran 471 checks, including the loaded units' own
checks, and all passed. The checks file has no procedures of its own, so
`coax mutate --root-only` on it has no sites.

### The library's own checks

`nip45.coax` has 8 checks: the offset for an ID, a pubkey, an address and a
hashed value, uppercase hex, the first tag filter winning, no tag filter, a
256-byte value and a 257-byte one; the address grammar at each boundary;
one register by hand, a smaller value never lowering a register, all-zero
tails at offsets 8 and 23, and the merge; estimates for empty, linear and
raw cases, the 768 boundary from both sides (raw 768 gives linear counting,
raw 769 gives 769), and registers of 40, 44, 45 and 185; hex both ways and
its refusals; the response written and read, with each refusal; the request
written and read back by `ms_read`; exact counts with OR, `limit`, a
filter that matches nothing, short arrays and a stale index; and every
reply of `n45_answer`.

On 2026-11-01 they passed, and `coax mutate coax/lib/nostr/nip45.coax
--root-only` killed 429 of 429 mutants. The first run left 46 survivors.
The fixes were checks (a 65-character ID, a count of 0, a request of
exactly one filter, a buffer that ends before a comma, a filter that
matches nothing, registers that must be cleared, the 768 boundary, a
work space overflow), and rewrites that removed code the checks could not
reach: go-nostr's 220 test, which always agrees with the 768 one; an
offset of 0 for "none" in place of -1, since a real offset is never 0;
and the register steps described above.

`store_index.coax` changed only by the split into `nsi_query_limit`, and
its 310 checks pass. `message_server.coax` and `relay_server.coax` did not
change, and their records were not rerun for this chapter:
[2026-10-29-nostr-store-r1.md](../../docs/verification/2026-10-29-nostr-store-r1.md).

## Summary

- A `COUNT` takes a `REQ`'s filters and returns one number. This relay
  counts exactly, through chapter 36's indexes, and counts an event that
  matches several filters once. `limit` doesn't cap a count.
- A HyperLogLog sketch is 256 registers. The filter's first tag value fixes
  the offset; each pubkey's byte at the offset picks a register and its
  following zero bits set the value.
- Sketches from different relays merge by taking the larger value in each
  register. The merge of two relays' sketches is exactly the sketch of
  their union, so shared events are counted once.
- The estimate is linear counting for small counts and the HyperLogLog
  formula otherwise, with about 6.5% standard error for 256 registers.
- The relay refuses with `CLOSED`, or `NOTICE` when there is no ID to close.
  The relay process doesn't route `COUNT` here yet.

## Dependencies

- [24. Relay messages](ch024-relay-messages.md)
- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)

## Sources

- [NIP-45](../nips/official/45.md)
- [NIP-01](../nips/official/01.md), for filters and `limit`
- go-nostr's `nip45/hyperloglog` package, read on 2026-11-01 for its
  estimate: <https://github.com/nbd-wtf/go-nostr/tree/master/nip45/hyperloglog>
- P. Flajolet, É. Fusy, O. Gandouet and F. Meunier, "HyperLogLog: the
  analysis of a near-optimal cardinality estimation algorithm", 2007.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
