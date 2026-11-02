# 96. Formal properties

A test checks one input. A property says something about every input: "for
every filter and every event, the filter matches exactly when each of its
fields does". Coax can't prove a sentence like that for all 2^256 ids, but
it can check it for every input of a small domain, all of them, with no
sampling. This chapter states five properties of the stack, builds a small
domain for each from a tiny alphabet of ids, authors, kinds, times and
tags, checks every case, and plants a bug for each property to show that
the check finds it. Each counterexample becomes a regression test.

Part VII, Interop and verification. Chapter 94 compares two implementations
on generated inputs, and chapter 95 feeds readers random hostile bytes.
Both sample. This chapter enumerates: every case of a stated domain, with
a count you can work out by hand.

## What this chapter teaches

- How to state a property: a sentence over all inputs, written as a
  predicate that compares the library with a short specification.
- Small domains: a tiny alphabet whose every combination is checked, and
  case numbers that decode to one input each.
- What `#forall` does in Coax: it runs its block once for every
  combination of its bindings, at most 2^24 cases, and reports the first
  counterexample as values. It is not a theorem prover.
- Five properties: filter matching is AND across fields and OR within a
  list; a subscription's filters are a union; replacement depends on
  `(created_at, id)` only, in any arrival order; the indexed query equals
  the scan; verification refuses every one-bit change of a signed id, key
  or signature.
- Planted bugs: a deliberately wrong variant per property, to show that
  each check can fail.
- Counterexamples as regression tests.
- Which claims are checked exhaustively, which are sampled and which are
  not proven.

## Where the code is

| File | What it holds |
| --- | --- |
| [nostr/examples/formal-properties.coax](../examples/formal-properties.coax) | The alphabet, the specification, the five property checks with their planted bugs, and the output |
| [nostr/examples/checks/formal-properties.coax](../examples/checks/formal-properties.coax) | The worked table, the domain counts, every property as `#forall` on a slice of its domain, and every counterexample as a regression test |
| [coax/lib/nostr/filter.coax](../../coax/lib/nostr/filter.coax) | `nf_match` and `nf_write` (chapter 23) |
| [coax/lib/nostr/relay_server.coax](../../coax/lib/nostr/relay_server.coax) | `rs_sub_open` and `rs_sub_matches`: a subscription's filters |
| [coax/lib/nostr/store_core.coax](../../coax/lib/nostr/store_core.coax) | `ns_store_admit`: replacement (chapter 35) |
| [coax/lib/nostr/store.coax](../../coax/lib/nostr/store.coax), [store_index.coax](../../coax/lib/nostr/store_index.coax) | `rsd_query`, the scan, and `nsi_query`, the indexed query (chapter 36) |
| [coax/lib/crypto/schnorr.coax](../../coax/lib/crypto/schnorr.coax) | `nostr_verify_vartime` (Bitcoin chapter 23) |
| [coax/lib/nostr/event_sign.coax](../../coax/lib/nostr/event_sign.coax) | `nostr_event_sign_step` and the event reader's verification (Bitcoin chapter 24) |

The properties, domains and planted bugs are in the example. The library
is unchanged: every property held on the first run.

## Stating a property

A property has three parts:

1. **A domain.** The inputs it talks about. In a proof that is every
   filter and every event; here it is every filter and every event built
   from a small alphabet.
2. **A specification.** What the answer should be, written as directly as
   the source text allows, with no code shared with the implementation.
3. **A comparison.** For every input in the domain, the implementation's
   answer equals the specification's.

A good small domain keeps every distinction the code makes and drops
everything else. Two authors are enough to tell "listed" from "not
listed"; three times are enough for `since` and `until` to fall before,
on and after an event. A domain with one author couldn't catch a matcher
that ignores the author list.

### The alphabet

<!-- source: nostr/examples/formal-properties.coax alphabet -->
```jai
// The tiny alphabet every domain below is built from. Times are T0, T0+1
// and T0+2. Ids X and Y, authors A and B: only the first byte is not 0.
T0 :: 100;
ID_BYTE :: [3] u8.[0x30, 0x10, 0x20];  // X, Y and (for the store) Z
AUTHOR_BYTE :: [2] u8.[0xdf, 0xdd];    // A, B
KIND :: [2] int.[1, 0];                // 1 is regular, 0 is replaceable
TAG_TEXT :: [4] string.["[]", "[[\"t\",\"a\"]]", "[[\"t\",\"b\"]]", "[[\"t\",\"a\"],[\"t\",\"b\"]]"];
VALUE :: [2] string.["a", "b"];
ID_HEX :: [2] string.["3000000000000000000000000000000000000000000000000000000000000000",
                      "1000000000000000000000000000000000000000000000000000000000000000"];
AUTHOR_HEX :: [2] string.["df00000000000000000000000000000000000000000000000000000000000000",
                          "dd00000000000000000000000000000000000000000000000000000000000000"];
// One event: an index into each row above. tag is a bit set: bit 0 is
// t=a, bit 1 is t=b.
Ev :: struct { id: int; author: int; kind: int; time: int; tag: int; }
// One filter. ids, authors, kinds and tag are 0 when absent, else a bit
// set over the two letters of their row (1 the first, 2 the second, 3
// both). since and until are 0 when absent, else 1 + a time index.
Flt :: struct { ids: int; authors: int; kinds: int; since: int; until: int; tag: int; }
// Case numbers: the last field changes fastest.
EVENTS :: 96;     // 2 ids * 2 authors * 2 kinds * 3 times * 4 tag sets
FILTERS :: 4096;  // 4 ^ 6
event_of :: (code: int) -> Ev {
  e: Ev; c := code;
  e.tag = c % 4; c /= 4; e.time = c % 3; c /= 3; e.kind = c % 2; c /= 2; e.author = c % 2; c /= 2; e.id = c;
  return e;
}
filter_of :: (code: int) -> Flt {
  f: Flt; c := code;
  f.tag = c % 4; c /= 4; f.until = c % 4; c /= 4; f.since = c % 4; c /= 4;
  f.kinds = c % 4; c /= 4; f.authors = c % 4; c /= 4; f.ids = c;
  return f;
}
```

An event is a choice from each row: 2 ids, 2 authors, 2 kinds, 3 times and
4 tag sets, so 2 · 2 · 2 · 3 · 4 = 96 events. A filter has six fields:
`ids`, `authors`, `kinds` and `#t` are each absent or one of three
nonempty subsets of their two-letter row (4 choices), and `since` and
`until` are each absent or one of the 3 times (4 choices). That gives
4^6 = 4096 filters. Every number from 0 to 95 decodes to one event, and
every number from 0 to 4095 to one filter, so "every case" is a loop over
two ranges.

### The specification

The specification reads NIP-01's filter section literally. It doesn't
call the library.

<!-- source: nostr/examples/formal-properties.coax spec -->
```jai
// The specification, read from NIP-01's filter section. A list field
// matches when it is absent or holds the event's value: "any of them may
// match" (OR within a list).
listed :: (field: int, which: int) -> bool { return field == 0 || field & (1 << which) != 0; }
// "All of the conditions of a filter that are specified must match" (AND
// across fields), and "since <= created_at <= until".
spec_match :: (f: Flt, e: Ev) -> bool {
  created := T0 + e.time;
  return listed(f.ids, e.id) && listed(f.authors, e.author) && listed(f.kinds, e.kind)
      && (f.since == 0 || T0 + f.since - 1 <= created)
      && (f.until == 0 || created <= T0 + f.until - 1)
      && (f.tag == 0 || f.tag & e.tag != 0);
}
```

`render` in the example turns an `Ev` and a `Flt` into the library's
`NostrEvent` and `NostrFilter`; it copies fields and decides nothing.

## Worked example: the kinds field by hand

Take one field alone. The event kinds are 1 and 0, and the `kinds` field
is absent, `[1]`, `[0]` or `[1,0]`. That is 4 · 2 = 8 cases. NIP-01 says a
present list matches when any entry equals the event's kind:

| `kinds` | kind 1 | kind 0 |
| --- | --- | --- |
| absent | match | match |
| `[1]` | match | no |
| `[0]` | no | match |
| `[1,0]` | match | match |

Six of the 8 cases match. Now plant a bug: a matcher that reads only the
first entry of a kinds list. On `[1]` and `[0]` the first entry is the
only one, so it is right. On `[1,0]` it compares the kind with 1 only, so
it matches kind 1 and misses kind 0. It is wrong in exactly one of the 8
cases: `[1,0]` and kind 0.

The checks file states this table as a `#forall` over `kinds: 0..<4,
kind: 0..<2` and asserts both the library's row and the planted matcher's
one wrong row. Written against the planted matcher alone, the same
`#forall` fails, and `coax test` names the case:

```shell
FAIL #forall the planted matcher against the kinds table (0 ms)
planted.coax:5:3: error[E0406]: assertion failed
    #assert match_impl(f, e, true) == spec_match(f, e);
    ^
  note: in #forall the planted matcher against the kinds table
  counterexample: kinds = 3, kind = 1
```

`kinds = 3` is the set `[1,0]`, and `kind = 1` is the second entry of
`KIND`, kind 0: the cell the table predicts. The command exits with status
1.

On the full domain the same bug first shows at a case number you can
predict. Case numbers run filter by filter, 96 events per filter. The
`kinds` digit is the fourth from the right (weight 4^3 = 64), so filters 0
to 191 have `kinds` absent, `[1]` or `[0]`, where the planted matcher is
right. Filter 192 is the first with `kinds` `[1,0]` and every other field
absent. Its events 0 to 11 have kind 1, which the planted matcher still
matches; event 12 is the first with kind 0. The counterexample is case
192 · 96 + 12 + 1 = 18,445, and the example prints exactly that.

## Five properties

Each property is a procedure that loops over its whole domain, compares
the library with the specification, and stops at the first counterexample,
as `#forall` does. Each takes `planted`, which swaps in the bug.

### 1. Matching is AND across fields, OR within a list

<!-- source: nostr/examples/formal-properties.coax match -->
```jai
// Property 1: nf_match is the conjunction of the field predicates, on
// every (filter, event) pair of the domain. planted: a matcher that reads
// only the first entry of a kinds list.
match_impl :: (f: Flt, e: Ev, planted: bool) -> bool {
  l: [1] Lists; nf := nostr_filter(f, l[..]);
  if planted && nf.has_kinds { nf.kinds = nf.kinds[0..<1]; }
  return nf_match(nf, nostr_event(e));
}
check_match :: (planted: bool) -> Outcome {
  o: [1] Outcome;
  for fc: 0..<FILTERS {
    f := filter_of(fc);
    for ec: 0..<EVENTS {
      e := event_of(ec); want := spec_match(f, e);
      o[0].cases += 1; if want { o[0].held += 1; }
      if match_impl(f, e, planted) != want { fail(o[..], fc, ec, -1); return o[0]; }
    }
  }
  return o[0];
}
```

The domain is 4096 filters times 96 events: 393,216 cases. The planted bug
truncates the `kinds` list to its first entry.

### 2. A subscription is the union of its filters

NIP-01 lets one `REQ` carry several filters: an event goes to the
subscription when any of them matches. The relay stores a subscription's
filters as JSON text and reads them back when an event arrives
(`rs_sub_matches`), so this property also checks `nf_write` and
`filter_read` on every filter of its domain. Each `REQ` goes through
`rs_req` first, as `coax/relay/relay.coax` does: `rs_sub_open` trusts its
caller to have admitted the filters, though it refuses a message `ms_read`
refused, such as an unterminated array, as `RS_FRAME`.

<!-- source: nostr/examples/formal-properties.coax union -->
```jai
// Property 2: a subscription with filters [F, G] sends an event exactly
// when F or G matches it. F and G range over the 64 filters with no ids,
// since or until; the events over the 16 with id X at T0+1. planted: a
// subscription that reads only its first filter.
SUB_FILTERS :: 64; SUB_EVENTS :: 16;
sub_filter :: (s: int) -> Flt { f: Flt; f.authors = s / 16; f.kinds = (s / 4) % 4; f.tag = s % 4; return f; }
sub_event :: (s: int) -> Ev { e: Ev; e.time = 1; e.author = s / 8; e.kind = (s / 4) % 2; e.tag = s % 4; return e; }
put :: (out: [] u8, at: int, s: string) -> int { for i: 0..<s.count { out[at + i] = s[i]; } return at + s.count; }
// The REQ a client sends: ["REQ","s",F,G].
req_text :: (f: Flt, g: Flt, out: [] u8) -> string {
  lf: [1] Lists; lg: [1] Lists; at := put(out, 0, "[\"REQ\",\"s\",");
  at += nf_write(nostr_filter(f, lf[..]), out[at..<out.count], false);
  at = put(out, at, ",");
  at += nf_write(nostr_filter(g, lg[..]), out[at..<out.count], false);
  at = put(out, at, "]");
  return cast(string) out[0..<at];
}
check_union :: (planted: bool) -> Outcome {
  o: [1] Outcome; text: [512] u8; subs: [1] RsSub;
  for a: 0..<SUB_FILTERS {
    for b: 0..<SUB_FILTERS {
      fa := sub_filter(a); fb := sub_filter(b);
      // admitted as the relay admits a REQ (rs_req), then opened
      t := req_text(fa, fb, text[..]); m := ms_read(t);
      #assert rs_req(m, t) == RS_ACCEPT && rs_sub_open(subs[..], m, t, 1) == 0 && subs[0].filters == 2;
      if planted { subs[0].filters = 1; }
      for k: 0..<SUB_EVENTS {
        e := sub_event(k);
        want := match_impl(fa, e, false) || match_impl(fb, e, false);
        o[0].cases += 1; if want { o[0].held += 1; }
        if rs_sub_matches(subs[..], 0, nostr_event(e)) != want { fail(o[..], a, b, k); return o[0]; }
      }
    }
  }
  return o[0];
}
```

The domain is 64 · 64 pairs of filters times 16 events: 65,536 cases. The
planted bug makes the subscription read only its first filter. Its first
counterexample is `F = {"#t":["a"]}`, `G = {}` and an untagged event: `G`
matches it, `F` doesn't. Pairs with `F = {}` come first and pass, because
`{}` matches everything.

### 3. Replacement depends on `(created_at, id)` only

NIP-01 keeps one replaceable event per author and kind: the newest, and on
a tie in `created_at` the lowest id. A relay receives events in any order,
so the property is "for every set of three events and every one of the 6
arrival orders, the store keeps what NIP-01 keeps". The tag set varies
too, and the specification doesn't read it, so a store whose choice
depended on the tags would fail.

<!-- source: nostr/examples/formal-properties.coax replace -->
```jai
// Property 3: after admitting three events in any order, the store holds
// exactly the events NIP-01 keeps: every regular one, and for each author's
// kind 0 the newest, then the lowest id. Event slot i has id ID_BYTE[i];
// its author, kind, time (T0 or T0+1) and tag set (none or t=a) range over
// 16 values. planted: a store where the later arrival wins a tie.
Rep :: struct { author: int; kind: int; time: int; tag: int; }
REP :: 16; TRIPLES :: 4096;
rep_of :: (code: int) -> Rep { return Rep.{author = code / 8, kind = (code / 4) % 2, time = (code / 2) % 2, tag = code % 2}; }
PERM :: [18] int.[0, 1, 2,  0, 2, 1,  1, 0, 2,  1, 2, 0,  2, 0, 1,  2, 1, 0];
// The events NIP-01 keeps, as a bit set over the slots.
spec_kept :: (r: [3] Rep) -> int {
  kept := 0;
  for i: 0..<3 {
    gone := false;
    for j: 0..<3 {
      // j == i is never newer than itself
      same := r[i].kind == 1 && r[j].kind == 1 && r[j].author == r[i].author;
      newer := r[j].time > r[i].time || (r[j].time == r[i].time && ID_BYTE[j] < ID_BYTE[i]);
      if same && newer { gone = true; }
    }
    if !gone { kept |= 1 << i; }
  }
  return kept;
}
Store :: struct { arena: [256] u8; recs: [4] RsdRec; s: [1] RsdStore; }
rep_rec :: (r: Rep, slot: int) -> RsdRec {
  c: RsdRec; c.id[0] = ID_BYTE[slot]; c.author[0] = AUTHOR_BYTE[r.author];
  c.kind = KIND[r.kind]; c.created = T0 + r.time; c.expiry = -1; c.tags_to = TAG_TEXT[r.tag].count;
  return c;
}
admit :: (s: [] RsdStore, c: RsdRec, text: string, planted: bool) -> int {
  if planted && ns_class(c.kind) == NS_REPLACEABLE {
    for i: 0..<s[0].n {
      if s[0].recs[i].live && ns_store_same_address(s[0].recs[i], c) && s[0].recs[i].created == c.created {
        ns_store_retire(s, i); ns_store_append(s, c, text); return NSS_ACCEPT;
      }
    }
  }
  return ns_store_admit(s, c, text, true);
}
// The slots the store holds after admitting r in the order of permutation p.
store_kept :: (r: [3] Rep, p: int, planted: bool) -> int {
  st: [1] Store; st[0].s[0].arena = st[0].arena[..]; st[0].s[0].recs = st[0].recs[..];
  for k: 0..<3 {
    slot := PERM[3 * p + k];
    admitted := admit(st[0].s[..], rep_rec(r[slot], slot), TAG_TEXT[r[slot].tag], planted);
    #assert admitted == NSS_ACCEPT || admitted == NSS_SUPERSEDED;
  }
  kept := 0;
  for i: 0..<st[0].s[0].n {
    if !st[0].recs[i].live { continue; }  // retired: replaced
    for slot: 0..<3 { if st[0].recs[i].id[0] == ID_BYTE[slot] { kept |= 1 << slot; } }
  }
  return kept;
}
triple :: (code: int) -> [3] Rep { r: [3] Rep; r[0] = rep_of(code / 256); r[1] = rep_of((code / 16) % 16); r[2] = rep_of(code % 16); return r; }
check_replace :: (planted: bool) -> Outcome {
  o: [1] Outcome;
  for t: 0..<TRIPLES {
    r := triple(t); want := spec_kept(r);
    if want != 7 { o[0].held += 1; }  // triples where something is replaced
    for p: 0..<6 { o[0].cases += 1; if store_kept(r, p, planted) != want { fail(o[..], t, p, -1); return o[0]; } }
  }
  return o[0];
}
```

The three slots have fixed ids X = 0x30, Y = 0x10 and Z = 0x20, so the id
order Y < Z < X differs from the slot order. Each slot ranges over 2
authors, 2 kinds, 2 times and 2 tag sets: 16^3 = 4096 triples, and 4096 ·
6 = 24,576 cases. In 1280 of the triples at least one event is replaced.

The planted store gives a tie to the later arrival. Its first
counterexample is triple 68 in the order X, Y, Z: X is author A's kind 1,
and Y and Z are author A's kind 0 at the same second. NIP-01 keeps Y, the
lower id; the planted store keeps Z because it arrived last. In the order
X, Z, Y the planted store keeps Y and happens to be right, which is why a
property over every order catches what a single test order can miss.

### 4. The indexed query equals the scan

Chapter 36 builds an index and checks it against the scan on 500 generated
stores. Here the comparison runs on every store of at most two events from
an alphabet of 32 events, and every filter from a set of 256.

<!-- source: nostr/examples/formal-properties.coax index -->
```jai
// Property 4: the indexed query answers exactly what the scan answers,
// in the same order, for every store of at most two events and every
// filter below. An event is any of 32 (author, kind, time T0 or T0+1, tag set) with
// id X in slot 0 and Y in slot 1. A filter is authors, kinds and tag (4
// each), since absent or T0+1, and limit absent or 1. planted: an index
// whose time range starts one second late.
IX_EVENTS :: 32; IX_FILTERS :: 256; STORES :: 1057;  // 1 + 32 + 32 * 32
ix_event :: (code: int, slot: int) -> Ev { return Ev.{id = slot, author = code / 16, kind = (code / 8) % 2, time = (code / 4) % 2, tag = code % 4}; }
IxFlt :: struct { f: Flt; limit: bool; }
ix_filter :: (code: int) -> IxFlt {
  x: IxFlt; c := code;
  x.limit = c % 2 == 1; c /= 2; x.f.since = ifx c % 2 == 1 then 2 else 0; c /= 2;
  x.f.tag = c % 4; c /= 4; x.f.kinds = c % 4; c /= 4; x.f.authors = c;
  return x;
}
Indexed :: struct {
  arena: [256] u8; recs: [4] RsdRec; s: [1] RsdStore;
  by_id: [4] int; by_ak: [4] int; by_time: [4] int; tag_key: [8] int; tag_rec: [8] int; ix: [1] NsiIndex;
}
// Store number k: 0 is empty, 1..32 one event, then every ordered pair.
ix_store :: (k: int, t: [] Indexed) -> int {
  t[0].s[0].arena = t[0].arena[..]; t[0].s[0].recs = t[0].recs[..];
  t[0].ix[0] = NsiIndex.{by_id = t[0].by_id[..], by_ak = t[0].by_ak[..], by_time = t[0].by_time[..], tag_key = t[0].tag_key[..], tag_rec = t[0].tag_rec[..]};
  codes: [2] int; n := 0;
  if k > IX_EVENTS {
    pair := k - 1 - IX_EVENTS; codes[0] = pair / IX_EVENTS; codes[1] = pair - codes[0] * IX_EVENTS; n = 2;
  } else if k >= 1 { codes[0] = k - 1; n = 1; }
  for i: 0..<n {
    e := ix_event(codes[i], i);
    c: RsdRec; c.id[0] = ID_BYTE[i]; c.author[0] = AUTHOR_BYTE[e.author]; c.kind = KIND[e.kind];
    c.created = T0 + e.time; c.expiry = -1; c.tags_to = TAG_TEXT[e.tag].count;
    admitted := ns_store_admit(t[0].s[..], c, TAG_TEXT[e.tag], true);
  }
  #assert nsi_build(t[0].s[..], t[0].ix[..]);
  return n;
}
check_index :: (planted: bool) -> Outcome {
  o: [1] Outcome;
  for k: 0..<STORES {
    t: [1] Indexed; events := ix_store(k, t[..]);
    for q: 0..<IX_FILTERS {
      x := ix_filter(q); l: [1] Lists; f := nostr_filter(x.f, l[..]);
      if x.limit { f.has_limit = true; f.limit = 1; }
      a: [4] int; b: [4] int;
      na := rsd_query(t[0].s[..], f, T0 + 2, a[..]);
      g := f; if planted && g.has_since { g.since += 1; }
      r := nsi_query(t[0].s[..], t[0].ix[..], g, T0 + 2, b[..]);
      o[0].cases += 1; o[0].held += na;
      same := r.n == na;
      if same { for i: 0..<na { if a[i] != b[i] { same = false; } } }
      if !same { fail(o[..], k, q, -1); return o[0]; }
    }
  }
  return o[0];
}
```

There are 1 + 32 + 32 · 32 = 1057 stores (empty, one event, an ordered
pair) and 4 · 4 · 4 · 2 · 2 = 256 filters: 270,592 queries. The answers
must be equal in length and order, so the index's order and its `limit`
cut are checked too. Pairs of two kind 0 events by one author exercise
replacement before the index is built.

The planted index starts its time range one second late. The first
counterexample is store 5, one kind 1 event at `T0+1`, and the filter
`{"since":T0+1}`: `since <= created_at` holds with equality, the scan
serves the event, and the planted index doesn't.

### 5. Verification refuses every one-bit change

No small domain covers secp256k1: the group has about 2^256 elements. The
property here is narrower and exact: for BIP-340 vector 0 (a 32-byte
message, so a Nostr id), the verifier accepts the published triple and
refuses every triple that differs from it in one bit.

<!-- source: nostr/examples/formal-properties.coax signature -->
```jai
// Property 5: verification never accepts a changed id, key or signature.
// The domain is every one-bit change of BIP-340 vector 0 (its message is a
// 32-byte id): 256 id bits, 256 key bits and 512 signature bits, plus the
// unchanged triple, which must verify. planted: a verifier that copies
// only the first 31 bytes of the id.
V0_KEY :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
V0_ID :: "0000000000000000000000000000000000000000000000000000000000000000";
V0_SIG :: "e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca821525f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0";
FLIPS :: 1024;
decode :: (text: string, out: [] u8) { #assert hex_decode_checked(text, out, false).written == out.count; }
verify_impl :: (id: [32] u8, key: [32] u8, sig: [64] u8, planted: bool) -> bool {
  m := id; k := key; s := sig; if planted { m[31] = 0; }
  return nostr_verify_vartime(m[..], k[..], s[..]);
}
// Flip f: 0..255 an id bit, 256..511 a key bit, 512..1023 a signature bit;
// bit b is byte b / 8, mask 1 << (b % 8).
flipped :: (f: int, id: [] [32] u8, key: [] [32] u8, sig: [] [64] u8) {
  if f < 256 { id[0][f / 8] ^= cast(u8) (1 << (f % 8)); }
  else if f < 512 { key[0][(f - 256) / 8] ^= cast(u8) (1 << (f % 8)); }
  else { sig[0][(f - 512) / 8] ^= cast(u8) (1 << (f % 8)); }
}
check_signature :: (planted: bool) -> Outcome {
  o: [1] Outcome; id: [1] [32] u8; key: [1] [32] u8; sig: [1] [64] u8;
  decode(V0_ID, id[0][..]); decode(V0_KEY, key[0][..]); decode(V0_SIG, sig[0][..]);
  o[0].cases = 1;
  if verify_impl(id[0], key[0], sig[0], planted) { o[0].held = 1; } else { fail(o[..], -1, -1, -1); }
  for f: 0..<FLIPS {
    i := id; k := key; s := sig; flipped(f, i[..], k[..], s[..]);
    o[0].cases += 1;
    if verify_impl(i[0], k[0], s[0], planted) { fail(o[..], f, -1, -1); return o[0]; }
  }
  return o[0];
}
// Sign then verify, sampled: two published BIP-340 test keys, four
// contents each, signed as kind 1 events and verified from their JSON.
KEYS :: [2] string.["b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
                    "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9"];
CONTENT :: [4] string.["", "a", "hello", "{\"x\":1}"];
check_round_trip :: () -> Outcome {
  o: [1] Outcome;
  for k: 0..<2 {
    for c: 0..<4 {
      key: [32] u8; aux: [32] u8; decode(KEYS[k], key[..]);
      work: [1] NostrSignWork; scratch: [4096] u8; out: [512] u8;
      n := EVENT_MORE;
      while n == EVENT_MORE { n = nostr_event_sign_step(key[..], aux[..], T0, 1, "[]", CONTENT[c], work[..], scratch[..], out[..], false); }
      #assert n > 0;
      o[0].cases += 1;
      v := nostr_event_verify_vartime(cast(string) out[0..<n], scratch[..], false);
      if v.error == NE_OK && v.verified { o[0].held += 1; } else { fail(o[..], k, c, -1); return o[0]; }
    }
  }
  return o[0];
}
```

The domain is 1 + 256 + 256 + 512 = 1025 cases, every one of them checked.
The planted verifier copies only 31 bytes of the id, so byte 31 is always
0. Vector 0's id is all zeros, so the 8 flips in byte 31 are accepted;
the first is flip 248, byte 31, mask 1, at case 1 + 249 = 250.

Sign then verify is sampled: 8 events signed with two published BIP-340
test keys. It is not checked over a domain, because a domain of secret
keys can't be enumerated.

## The example

The example is
[nostr/examples/formal-properties.coax](../examples/formal-properties.coax).
For each property it runs the library on the whole domain, then the
planted variant, and prints the first counterexample.

<!-- source: nostr/examples/formal-properties.coax main -->
```jai
main :: () {
  print("1. nf_match is the AND of the field predicates, OR within each list\n");
  o := check_match(false);
  show_outcome("library", o); print(" (% pairs match)\n", o.held);
  o = check_match(true);
  show_outcome("planted: kinds list reads its first entry only", o); print("\n");
  print("  first counterexample: filter #% ", o.a); show_filter(filter_of(o.a));
  print(", event #% ", o.b); show_event(event_of(o.b));
  print("\n    spec % library % planted %\n", spec_match(filter_of(o.a), event_of(o.b)),
    match_impl(filter_of(o.a), event_of(o.b), false), match_impl(filter_of(o.a), event_of(o.b), true));

  print("\n2. a subscription [F, G] sends exactly what F or G matches\n");
  o = check_union(false);
  show_outcome("library", o); print(" (% sent)\n", o.held);
  o = check_union(true);
  show_outcome("planted: only the first filter is read", o); print("\n");
  print("  first counterexample: F "); show_filter(sub_filter(o.a)); print(", G "); show_filter(sub_filter(o.b));
  print(", event "); show_event(sub_event(o.c)); print("\n");

  print("\n3. replacement keeps the newest, then the lowest id, in every arrival order\n");
  o = check_replace(false);
  show_outcome("library", o); print(" (% of % triples replace something)\n", o.held, TRIPLES);
  o = check_replace(true);
  show_outcome("planted: a tie goes to the later arrival", o); print("\n");
  r := triple(o.a);
  print("  first counterexample: triple #%, order", o.a);
  for k: 0..<3 { print(" %", SLOT[PERM[3 * o.b + k]]); } print("\n");
  for i: 0..<3 { print("    id % author % kind % created T0+% tags %\n", SLOT[i], LETTER_AUTHOR[r[i].author], KIND[r[i].kind], r[i].time, TAG_TEXT[r[i].tag]); }
  print("    NIP-01 keeps "); show_slots(spec_kept(r));
  print(", library keeps "); show_slots(store_kept(r, o.b, false));
  print(", planted keeps "); show_slots(store_kept(r, o.b, true)); print("\n");

  print("\n4. the indexed query answers what the scan answers\n");
  o = check_index(false);
  show_outcome("library", o); print(" (% events served)\n", o.held);
  o = check_index(true);
  show_outcome("planted: the time range starts one second late", o); print("\n");
  x := ix_filter(o.b);
  print("  first counterexample: store #%, filter #% ", o.a, o.b); show_filter(x.f); if x.limit { print(" limit 1"); } print("\n");
  print("    store: "); show_event(ix_event(o.a - 1, 0)); print("\n");

  print("\n5. verification refuses every one-bit change\n");
  o = check_signature(false);
  show_outcome("library", o); print(" (% unchanged accepted)\n", o.held);
  o = check_signature(true);
  show_outcome("planted: only 31 id bytes are read", o); print("\n");
  print("  first counterexample: flip %, id byte % mask %\n", o.a, o.a / 8, 1 << (o.a % 8));
  o = check_round_trip();
  print("  sign then verify, sampled: % events, % verified\n", o.cases, o.held);
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/formal-properties.coax --run-steps 250000000
```

`--run-steps` raises the compile-time budget for the signing tables that
`event_sign.coax` builds. The output, which is
[nostr/fixtures/formal-properties.out](../fixtures/formal-properties.out):

```text
1. nf_match is the AND of the field predicates, OR within each list
  library: 393216 cases, all hold (59400 pairs match)
  planted: kinds list reads its first entry only: counterexample at case 18445
  first counterexample: filter #192 {"kinds":[1,0]}, event #12 id X author A kind 0 created T0+0 tags []
    spec true library true planted false

2. a subscription [F, G] sends exactly what F or G matches
  library: 65536 cases, all hold (39348 sent)
  planted: only the first filter is read: counterexample at case 1025
  first counterexample: F {"#t":["a"]}, G {}, event id X author A kind 1 created T0+1 tags []

3. replacement keeps the newest, then the lowest id, in every arrival order
  library: 24576 cases, all hold (1280 of 4096 triples replace something)
  planted: a tie goes to the later arrival: counterexample at case 409
  first counterexample: triple #68, order X Y Z
    id X author A kind 1 created T0+0 tags []
    id Y author A kind 0 created T0+0 tags []
    id Z author A kind 0 created T0+0 tags []
    NIP-01 keeps {X,Y}, library keeps {X,Y}, planted keeps {X,Z}

4. the indexed query answers what the scan answers
  library: 270592 cases, all hold (132990 events served)
  planted: the time range starts one second late: counterexample at case 1283
  first counterexample: store #5, filter #2 {"since":T0+1}
    store: id X author A kind 1 created T0+1 tags []

5. verification refuses every one-bit change
  library: 1025 cases, all hold (1 unchanged accepted)
  planted: only 31 id bytes are read: counterexample at case 250
  first counterexample: flip 248, id byte 31 mask 1
  sign then verify, sampled: 8 events, 8 verified
```

Every library run holds on its whole domain. Every planted bug is found,
at the case number worked out above. The counts in parentheses are the
cases where the property's subject is true (pairs that match, events
sent, triples that replace, events served); a Python model gives the same
numbers. The program exits with status 0.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/formal-properties.coax](../examples/checks/formal-properties.coax):

<!-- source: nostr/examples/checks/formal-properties.coax check -->
```jai
// The worked example: the kinds field alone, on its whole domain of 4
// filters times 2 event kinds. The chapter's table, row by row: absent
// matches both kinds, [1] only kind 1, [0] only kind 0, [1,0] both.
WORKED :: [8] bool.[true, true,  true, false,  false, true,  true, true];
#forall "the kinds table by hand" kinds: 0..<4, kind: 0..<2 {
  f: Flt; f.kinds = kinds; e: Ev; e.kind = kind;
  #assert spec_match(f, e) == WORKED[2 * kinds + kind];
  #assert match_impl(f, e, false) == WORKED[2 * kinds + kind];
  // the planted matcher is wrong in exactly one row: [1,0] and kind 0
  #assert match_impl(f, e, true) == (WORKED[2 * kinds + kind] && !(kinds == 3 && kind == 1));
}

// Domain sizes and counts, computed separately by a Python model of NIP-01
// (scratch script, 2026-11-01) that shares no code with this file.
#test "domain sizes and the Python model's counts" {
  #assert EVENTS == 2 * 2 * 2 * 3 * 4 && FILTERS == 4 * 4 * 4 * 4 * 4 * 4;
  #assert SUB_FILTERS * SUB_FILTERS * SUB_EVENTS == 65536;
  #assert TRIPLES == REP * REP * REP && STORES == 1 + IX_EVENTS + IX_EVENTS * IX_EVENTS;
  replacing := 0;
  for t: 0..<TRIPLES { if spec_kept(triple(t)) != 7 { replacing += 1; } }
  #assert replacing == 1280;
}
fp_one :: (b: bool) -> int { return ifx b then 1 else 0; }
// The case numbers decode one to one: re-encoding each decoded case, last
// field fastest, gives its number back, and every field stays in range.
#forall "event and filter numbers decode one to one" n: 0..<FILTERS {
  f := filter_of(n);
  #assert ((((f.ids * 4 + f.authors) * 4 + f.kinds) * 4 + f.since) * 4 + f.until) * 4 + f.tag == n;
  if n < EVENTS {
    e := event_of(n);
    #assert (((e.id * 2 + e.author) * 2 + e.kind) * 3 + e.time) * 4 + e.tag == n && e.id < 2 && e.time < 3;
  }
  if n < SUB_FILTERS { g := sub_filter(n); #assert g.authors * 16 + g.kinds * 4 + g.tag == n && g.ids == 0 && g.since == 0 && g.until == 0; }
  if n < SUB_EVENTS { e := sub_event(n); #assert e.author * 8 + e.kind * 4 + e.tag == n && e.id == 0 && e.time == 1; }
  if n < REP { r := rep_of(n); #assert r.author * 8 + r.kind * 4 + r.time * 2 + r.tag == n; }
  if n < IX_EVENTS { e := ix_event(n, 1); #assert e.author * 16 + e.kind * 8 + e.time * 4 + e.tag == n && e.id == 1; }
  if n < IX_FILTERS {
    x := ix_filter(n);
    #assert (((x.f.authors * 4 + x.f.kinds) * 4 + x.f.tag) * 2 + x.f.since / 2) * 2 + fp_one(x.limit) == n;
    #assert (x.f.since == 0 || x.f.since == 2) && x.f.ids == 0 && x.f.until == 0;
  }
  if n < TRIPLES { r := triple(n); #assert ((r[0].author * 8 + r[0].kind * 4 + r[0].time * 2 + r[0].tag) * 16 + r[1].author * 8 + r[1].kind * 4 + r[1].time * 2 + r[1].tag) * 16 + r[2].author * 8 + r[2].kind * 4 + r[2].time * 2 + r[2].tag == n; }
}
// The library types hold exactly the decoded fields, for every filter and
// event of the domain.
#forall "rendering copies every field" n: 0..<FILTERS {
  f := filter_of(n); l: [1] Lists; r := nostr_filter(f, l[..]);
  #assert r.has_ids == (f.ids != 0) && r.has_authors == (f.authors != 0) && r.has_kinds == (f.kinds != 0);
  #assert r.has_since == (f.since != 0) && r.has_until == (f.until != 0) && !r.has_limit && r.tags.count == fp_one(f.tag != 0);
  for w: 0..<2 {
    #assert listed(f.ids, w) == (f.ids == 0 || r.ids.count == 2 || r.ids[0] == ID_HEX[w]);
    #assert f.ids == 0 || r.ids.count == fp_one(f.ids == 3) + 1;
    #assert f.authors == 0 || (r.authors.count == fp_one(f.authors == 3) + 1 && listed(f.authors, w) == (r.authors[0] == AUTHOR_HEX[w] || r.authors[r.authors.count - 1] == AUTHOR_HEX[w]));
    #assert f.kinds == 0 || (r.kinds.count == fp_one(f.kinds == 3) + 1 && listed(f.kinds, w) == (r.kinds[0] == KIND[w] || r.kinds[r.kinds.count - 1] == KIND[w]));
    #assert f.tag == 0 || (r.tags[0].name == 't' && r.tags[0].values.count == fp_one(f.tag == 3) + 1 && listed(f.tag, w) == (r.tags[0].values[0] == VALUE[w] || r.tags[0].values[r.tags[0].values.count - 1] == VALUE[w]));
  }
  #assert f.since == 0 || r.since == T0 + f.since - 1;
  #assert f.until == 0 || r.until == T0 + f.until - 1;
  if n < EVENTS {
    e := event_of(n); v := nostr_event(e);
    #assert v.error == NE_OK && v.id[0] == ID_BYTE[e.id] && v.public_key[0] == AUTHOR_BYTE[e.author] && v.id[1] == 0;
    #assert v.kind == KIND[e.kind] && v.created_at == T0 + e.time && v.original == TAG_TEXT[e.tag] && v.tags_from == 0 && v.tags_to == TAG_TEXT[e.tag].count;
  }
}

// Property 1 as #forall over two slices of its domain the evaluator runs
// in seconds, against every event: filters with ids and until absent, and
// filters with kinds and since absent. The example runs all 4096 filters
// natively.
#forall "nf_match is the conjunction of the field predicates" f: 0..<256, ev: 0..<EVENTS #steps 2000000000 {
  // f's digits are authors, kinds, since and tag; ids and until stay 0
  code := (f / 64) * 256 + ((f / 16) % 4) * 64 + ((f / 4) % 4) * 16 + f % 4;
  #assert filter_of(code).ids == 0 && filter_of(code).until == 0;
  #assert match_impl(filter_of(code), event_of(ev), false) == spec_match(filter_of(code), event_of(ev));
}
#forall "nf_match: ids, authors, until and tag" f: 0..<256, ev: 0..<EVENTS #steps 2000000000 {
  code := (f / 64) * 1024 + ((f / 16) % 4) * 256 + ((f / 4) % 4) * 4 + f % 4;
  #assert filter_of(code).kinds == 0 && filter_of(code).since == 0;
  #assert match_impl(filter_of(code), event_of(ev), false) == spec_match(filter_of(code), event_of(ev));
}
// Property 2 on every filter F, with G its mirror (63 - F) and every event.
#forall "a subscription is the union of its filters" a: 0..<SUB_FILTERS, k: 0..<SUB_EVENTS #steps 2000000000 {
  t: [512] u8; subs: [1] RsSub; fa := sub_filter(a); fb := sub_filter(63 - a); e := sub_event(k);
  text := req_text(fa, fb, t[..]); m := ms_read(text);
  #assert rs_req(m, text) == RS_ACCEPT && rs_sub_open(subs[..], m, text, 1) == 0 && subs[0].filters == 2;
  #assert rs_sub_matches(subs[..], 0, nostr_event(e)) == (spec_match(fa, e) || spec_match(fb, e));
  subs[0].filters = 1;
  #assert rs_sub_matches(subs[..], 0, nostr_event(e)) == spec_match(fa, e);
}
// Property 3 in the evaluator, on the triples whose first slot is author
// A's kind 0 at T0 with no tags (256 triples, all six orders). The example
// runs all 4096 natively; the evaluator's aggregate-copy budget for one
// check stops near this size.
#forall "replacement is order-independent" t: 0..<256, p: 0..<6 #steps 2000000000 {
  r := triple(4 * 256 + t);
  #assert r[0].kind == 1 && r[0].author == 0 && r[0].time == 0 && r[0].tag == 0;
  #assert store_kept(r, p, false) == spec_kept(r);
}
// The planted store, on the slice whose first slot is at T0+1: it differs
// from NIP-01 only when two kind 0 events by one author tie in created_at.
#forall "the planted store fails only on a tie" t: 0..<256, p: 0..<6 #steps 2000000000 {
  r := triple(6 * 256 + t); want := spec_kept(r);
  #assert r[0].kind == 1 && r[0].author == 0 && r[0].time == 1 && r[0].tag == 0;
  tie := false;
  for i: 0..<3 { for j: 0..<3 { if i < j && r[i].kind == 1 && r[j].kind == 1 && r[i].author == r[j].author && r[i].time == r[j].time { tie = true; } } }
  #assert tie || store_kept(r, p, true) == want;
}
// Property 4 on every store, with four filters: {}, since T0+1, kinds [0]
// with limit 1, and authors [B] with #t ["a","b"].
IX_SOME :: [4] int.[0, 2, 65, 236];
#forall "the index answers what the scan answers" k: 0..<STORES, q: 0..<4 #steps 2000000000 {
  t: [1] Indexed; n := ix_store(k, t[..]);
  #assert n == ifx k == 0 then 0 else ifx k <= IX_EVENTS then 1 else 2;
  // every record is its slot's decoded event (a superseded one is not
  // stored; a replaced one stays, retired)
  #assert t[0].s[0].n >= 1 || n == 0;
  for i: 0..<t[0].s[0].n {
    rec := t[0].recs[i]; slot := ifx rec.id[0] == ID_BYTE[0] then 0 else 1;
    code := ifx n == 1 then k - 1 else ifx slot == 0 then (k - 1 - IX_EVENTS) / IX_EVENTS else (k - 1 - IX_EVENTS) % IX_EVENTS;
    e := ix_event(code, slot);
    #assert rec.id[0] == ID_BYTE[slot] && rec.author[0] == AUTHOR_BYTE[e.author] && rec.kind == KIND[e.kind] && rec.created == T0 + e.time && rec.expiry == -1;
    #assert rsd_text(t[0].s[..], i) == TAG_TEXT[e.tag] && rec.tags_to - rec.tags_from == TAG_TEXT[e.tag].count;
  }
  x := ix_filter(IX_SOME[q]); l: [1] Lists; f := nostr_filter(x.f, l[..]);
  if x.limit { f.has_limit = true; f.limit = 1; }
  a: [4] int; b: [4] int;
  na := rsd_query(t[0].s[..], f, T0 + 2, a[..]);
  r := nsi_query(t[0].s[..], t[0].ix[..], f, T0 + 2, b[..]);
  #assert r.n == na;
  for i: 0..<na { #assert a[i] == b[i]; }
}
// Each flip changes exactly one bit of the 128 bytes id || key || sig:
// bit f % 8 of byte f / 8.
#forall "flip f changes bit f of id, key and signature" f: 0..<FLIPS {
  id: [1] [32] u8; key: [1] [32] u8; sig: [1] [64] u8;
  flipped(f, id[..], key[..], sig[..]);
  for b: 0..<128 {
    v := ifx b < 32 then id[0][b] else ifx b < 64 then key[0][b - 32] else sig[0][b - 64];
    #assert cast(int) v == ifx b == f / 8 then 1 << (f % 8) else 0;
  }
}

// Each counterexample the example printed, kept as a regression test: the
// library passes it and the planted bug fails it.
#test "regression: kinds [1,0] matches a kind 0 event" {
  f := filter_of(192); e := event_of(12);
  #assert f.kinds == 3 && f.ids == 0 && f.authors == 0 && f.since == 0 && f.until == 0 && f.tag == 0;
  #assert e.kind == 1 && KIND[e.kind] == 0 && e.tag == 0 && e.time == 0;
  #assert spec_match(f, e) && match_impl(f, e, false) && !match_impl(f, e, true);
}
#test "regression: [{\"#t\":[\"a\"]},{}] sends an untagged event" {
  t: [512] u8; subs: [1] RsSub; fa := sub_filter(1); fb := sub_filter(0); e := sub_event(0);
  #assert fa.tag == 1 && fb.authors == 0 && fb.kinds == 0 && fb.tag == 0 && e.tag == 0;
  text := req_text(fa, fb, t[..]);
  #assert text == "[\"REQ\",\"s\",{\"#t\":[\"a\"]},{}]";
  #assert rs_sub_open(subs[..], ms_read(text), text, 1) == 0 && rs_sub_matches(subs[..], 0, nostr_event(e));
  subs[0].filters = 1; #assert !rs_sub_matches(subs[..], 0, nostr_event(e));
}
#test "regression: a kind 0 tie keeps the lower id in both orders" {
  r := triple(68);
  #assert r[1].kind == 1 && r[2].kind == 1 && r[1].time == r[2].time && r[1].author == r[2].author;
  #assert spec_kept(r) == 3;
  for p: 0..<6 { #assert store_kept(r, p, false) == 3; }
  // the planted store keeps whichever of Y and Z arrives last: Z (bit 2)
  // in the orders X Y Z, Y X Z and Y Z X, and Y in the other three
  for p: 0..<6 { #assert store_kept(r, p, true) == ifx p == 0 || p == 2 || p == 3 then 5 else 3; }
}
#test "regression: since equal to created_at is served by the index" {
  t: [1] Indexed; #assert ix_store(5, t[..]) == 1;
  x := ix_filter(2); #assert x.f.since == 2 && !x.limit && x.f.authors == 0 && x.f.kinds == 0 && x.f.tag == 0;
  #assert t[0].recs[0].created == T0 + 1;
  l: [1] Lists; f := nostr_filter(x.f, l[..]); a: [4] int; b: [4] int;
  #assert rsd_query(t[0].s[..], f, T0 + 2, a[..]) == 1;
  #assert nsi_query(t[0].s[..], t[0].ix[..], f, T0 + 2, b[..]).n == 1;
  f.since += 1; #assert nsi_query(t[0].s[..], t[0].ix[..], f, T0 + 2, b[..]).n == 0;
}
// A since of T0 lets every event of the alphabet through, so only an event
// before T0 shows that the specification reads it.
#test "since and until bound an event outside the alphabet's times" {
  f: Flt; f.since = 1; e: Ev; e.time = -1;
  #assert !spec_match(f, e); e.time = 0; #assert spec_match(f, e);
  f.since = 0; f.until = 3; e.time = 3; #assert !spec_match(f, e); e.time = 2; #assert spec_match(f, e);
}
// A store record holds exactly its slot's fields.
#forall "store records copy every field" n: 0..<REP, slot: 0..<3 {
  r := rep_of(n); c := rep_rec(r, slot);
  #assert c.id[0] == ID_BYTE[slot] && c.author[0] == AUTHOR_BYTE[r.author] && c.author[1] == 0 && c.id[1] == 0;
  #assert c.kind == KIND[r.kind] && c.created == T0 + r.time && c.expiry == -1 && c.tags_from == 0 && c.tags_to == TAG_TEXT[r.tag].count;
}
#test "the flip numbering by hand" {
  id: [1] [32] u8; key: [1] [32] u8; sig: [1] [64] u8;
  flipped(248, id[..], key[..], sig[..]); #assert id[0][31] == 1;
  flipped(256, id[..], key[..], sig[..]); #assert key[0][0] == 1;
  flipped(1023, id[..], key[..], sig[..]); #assert sig[0][63] == 0x80;
}
```

- The kinds table is the worked example, row by row, for the
  specification, the library and the planted matcher.
- The domain sizes are the products worked out above.
- The counts come from a separate Python model of NIP-01 (a scratch script
  run on 2026-11-01, sharing no code with the example). It enumerated the
  same domains and gave 393,216 cases with 59,400 matches; 65,536 cases
  with 39,348 sent; 24,576 cases with 1280 replacing triples; 270,592
  queries with 132,990 events served; and 1025 signature cases. The
  example prints the same numbers.
- Every case number decodes one to one (re-encoding gives the number
  back), and rendering copies every field into the library's types, for
  all 4096 filters and 96 events.
- Each property also runs as `#forall` in the evaluator, on slices the
  evaluator finishes in seconds: property 1 on two slices of 256 filters
  times 96 events (24,576 cases each); property 2 on every filter `F`
  with `G = 63 - F` and every event, with and without the planted bug;
  property 3 on 256 triples times 6 orders, and the planted store on
  another 256, where it may differ only on a tie; property 4 on all 1057
  stores with four filters, which also checks each stored record against
  its decoded event; and the flip numbering on all 1024 flips. The
  evaluator runs about 40 million steps a second, so the full domains run
  natively in the example instead.
- Each counterexample the example found is a `#test`: the library passes
  it and the planted variant fails it.

On 2026-11-01, `coax test nostr/examples/checks/formal-properties.coax
--run-steps 250000000` passed 478 of 478 checks in 98 seconds (the file
loads the libraries' own checks too) with `coax0 0.46.0`. The checks file has no procedures of its own, so `coax mutate --root-only`
on it finds no sites. To mutate the example's procedures, a scratch file
joined the example and the checks into one root and ran root-only
mutation on it: all 463 mutants were killed (336 seconds). The
`check_*`, `show_*` and verification procedures are reached only by the
example's run, whose output the book compares with the fixture, so the
mutation run reports them as not reached. Earlier rounds of the same run
left 142 and then 16 survivors; each was killed by a new check above
(decoding, rendering, record fields, and a `since` bound outside the
alphabet), and none was marked equivalent. The example
gives the fixture in debug and release.

### Checked, sampled, not proven

| Claim | How it is checked | Covers |
| --- | --- | --- |
| `nf_match` is AND across fields, OR within a list | Exhaustive, native | Every filter and event of the 2-letter alphabet: 393,216 cases |
| A subscription is the union of its filters | Exhaustive, native | 64 × 64 filter pairs × 16 events: 65,536 cases |
| Replacement is a function of `(created_at, id)`, in any order | Exhaustive, native | 4096 triples × 6 orders: 24,576 cases |
| Indexed query = scan, with order and `limit` | Exhaustive, native; sampled in chapter 36 | Stores of at most 2 events: 270,592 queries; chapter 36's 500 generated stores of 4 to 14 events |
| Verification refuses a changed id, key or signature | Exhaustive over one-bit changes of one signature | 1025 cases; not multi-bit changes or other signatures |
| Verify accepts exactly what BIP-340 accepts | Sampled | The 19 published vectors (Bitcoin chapter 23) and the one-hour fuzz record cited in chapter 95 |
| Sign then verify succeeds | Sampled | 8 events here; Bitcoin chapter 24's vectors |
| Any property on larger inputs: more events, longer lists, other kinds, other tag letters | Not proven | The small domains assume the code treats a third author or a fourth event like the first two. That is an argument about the code, not a check |
| Constant-time signing | Not proven | Timing measurements in Bitcoin chapter 3 are evidence, not proof |

A small-domain check is complete for its domain and says nothing outside
it. Its strength is the argument that the domain keeps every distinction
the code makes; the planted bugs are evidence for that argument, because
each one is caught inside the domain.

## Summary

- A property is a domain, a specification and a comparison. Coax checks
  it by enumerating the domain: `#forall` over ranges in the evaluator, or
  a loop in a native program for larger domains.
- A tiny alphabet (2 ids, 2 authors, 2 kinds, 3 times, 4 tag sets) gives
  96 events and 4096 filters, small enough to check every pair.
- Five properties hold on every case: matching is AND across fields and OR
  within a list; a subscription is a union; replacement depends on
  `(created_at, id)` in any order; the index equals the scan; verification
  refuses every one-bit change.
- Each property has a planted bug that its check finds, at a case number
  you can work out by hand, and each counterexample is a regression test.
- Signatures over all keys, larger stores and timing are sampled or not
  proven; the table says which.

## Dependencies

- [Bitcoin chapter 23. Verify a Schnorr signature](../../bitcoin/book/ch023-verify-a-schnorr-signature.md)
- [23. Filters](ch023-filters.md)
- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)
- [94. Differential tests](ch094-differential-tests.md) and
  [95. Fuzzing](ch095-fuzzing.md), the sampled methods this chapter
  complements.

## Sources

- [NIP-01](../nips/official/01.md): filters, several filters in one `REQ`,
  replaceable events and the tie rule.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  and its test vectors, pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- Coax's checks: [coax/spec.md](../../coax/spec.md) section 10.1.1, and
  [what the gates don't prove](../../coax/book/ch23-verification-boundaries.md).
- The verification plan:
  [2026-10-27-formal-verification-brainstorm.md](../../docs/verification/2026-10-27-formal-verification-brainstorm.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
