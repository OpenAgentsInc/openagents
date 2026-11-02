# 94. Differential tests

When two independent implementations agree on the same input, that is
stronger evidence than one implementation passing its own tests: a test
checks what its author thought of, and a second implementation can disagree
where neither author looked. This chapter shows the method: give the same
input to two implementations, normalize their answers, compare them,
classify every difference, and reduce a disagreement to the smallest input
that still shows it. It then runs the method in Coax on the relay's query
and cites the two relay differentials that use it against the Rust relay.

Part VII, Interop and verification. Result: Running the same inputs through
Coax and a reference implementation: the Rust relay from history in a
network-less sandbox, and independent libraries for signatures, NIP-44 and
NIP-59.

## What this chapter teaches

- The five steps of a differential: generate, render, run both, normalize,
  compare.
- Classification: every difference is a match, an explained (known)
  difference with a written reason, or unexplained. The Rust differential
  splits the explained ones into reason text only, intentional and owner
  question.
- Minimization: removing parts of a failing input, one at a time, while the
  disagreement remains.
- Seeding a fault to show that the differential sees a real bug.
- The Rust relay from `0b401561a0^` in a bwrap sandbox with a throwaway
  Postgres, and the recorded production answers, as reference
  implementations.
- Independent libraries for signing, verification and encryption, run only
  in reviewed, pinned harnesses.

## Where the code is

| File | What it does |
| --- | --- |
| [nostr/examples/differential.coax](../examples/differential.coax) | The method in Coax: the relay's query against NIP-01's filter text over 500 generated cases, a seeded fault and its minimization |
| [coax/lib/nostr/store.coax](../../coax/lib/nostr/store.coax) | `rsd_query`, the query that `coax/relay/relay.coax` answers a REQ with |
| [coax/lib/nostr/filter_read.coax](../../coax/lib/nostr/filter_read.coax), [filter.coax](../../coax/lib/nostr/filter.coax) | The REQ filter reader and `nf_match` (chapter 23) |
| [coax/tests/nostr/relay_rust_differential.py](../../coax/tests/nostr/relay_rust_differential.py) | The Coax relay against the Rust relay, 114 observations, run by `rust_relay_differential.sh` in the sandbox `rust_relay_sandbox.sh` builds |
| [coax/tests/nostr/relay_differential.py](../../coax/tests/nostr/relay_differential.py) | The Coax relay against relay.openagents.com's recorded answers to the project page's five REQs |

The Python files are test tooling: they start the relays, play a script
and compare frames. Nothing ships them. The book's example is Coax and runs
both implementations in one program, with no network and no host.

## The method

A differential has five steps. Each one has a rule.

1. **Generate.** Make inputs from a seeded generator, so that every run sees
   the same cases. Use small value sets: two authors, four times, a few
   kinds. Small sets make collisions common (two events in the same second,
   `since` equal to `created_at`), and collisions are where implementations
   disagree.
2. **Render.** Turn one generated description into each implementation's
   input format. Here the relay gets records and REQ JSON; the reference
   gets the description itself. The two inputs come from the same
   description, so a difference can't come from the inputs.
3. **Run both.** Neither implementation may call the other. The reference
   in this chapter doesn't load the filter library; the Rust relay shares no
   code with the Coax relay.
4. **Normalize.** Map both answers to one form. The relay answers record
   indexes; both answers become event names in order. The Rust relay's EOSE
   carries a third element that the Coax relay doesn't send, so the Rust
   driver folds it away and reports it once.
5. **Compare and classify.** Equal is a match. A difference that a written
   rule explains is explained, and the rule names its reason. Anything else
   is unexplained, and an unexplained difference fails the run.

### Classes

The Rust differential (2026-10-30) applies these classes in this order:

| Class | Meaning | Count |
| --- | --- | --- |
| match | identical frames | 88 |
| intentional | decided and written down in the R1 records | 7 |
| owner | reported, and left for `NEEDS_OWNER.md` | 4 |
| text | the same verdict and NIP-01 machine prefix (`invalid:`, `duplicate:`), with only the human-readable text after it differing | 15 |
| unexplained | the run fails | 0 |

Every one of the 26 differences is in the record's table, with both
answers and its reason. For example, a write before AUTH is `OK true` from
the Rust relay and `OK false "auth-required: ..."` from the Coax relay,
classed intentional because the plan requires NIP-42 for writes; a
REQ with two filters is answered in one merged order by Rust and filter by
filter by Coax, classed owner (question 2).

The example in this chapter uses three classes: match, explained (one of
three known rules) and unexplained. Its known rules are the relay rules
that NIP-01's filter section alone doesn't state:

<!-- source: nostr/examples/differential.coax known -->
```jai
// The known differences: each a rule the relay follows that the filter
// section alone does not state, with its reason.
KNOWN :: 3;
KNOWN_NAME :: [3] string.["ephemeral", "replaced", "expired"];
KNOWN_WHY :: [3] string.[
  "NIP-01: kinds 20000-29999 are not expected to be stored",
  "NIP-01: one replaceable event per pubkey and kind, the newest, then the lowest id",
  "NIP-40: relays SHOULD NOT send expired events; expired once ASK >= expiration"];
ephemeral :: (kind: int) -> bool { return kind >= 20000 && kind < 30000; }
replaceable :: (kind: int) -> bool { return kind == 0 || kind == 3 || (kind >= 10000 && kind < 20000); }
// Which known rule explains why the relay doesn't hold event i, or -1.
known_rule :: (c: Case, i: int) -> int {
  e := c.ev[i];
  if ephemeral(e.kind) { return 0; }
  if replaceable(e.kind) {
    for j: 0..<c.n {
      o := c.ev[j];
      if j != i && o.author == e.author && o.kind == e.kind && ref_before(o, e) { return 1; }
    }
  }
  if e.expiry >= 0 && ASK >= e.expiry { return 2; }
  return -1;
}
```

## Two implementations of one query

The example compares the query the relay serves with a reference written
from the filter section of [NIP-01](../nips/official/01.md).

A generated case is a list of up to six events, published in order, and one
filter. The events and the filter are plain values, not JSON:

<!-- source: nostr/examples/differential.coax model -->
```jai
// One generated event. author 0 is A (df00...00), 1 is B (dd00...00). id is
// the id's first byte; the other 31 bytes are 0. tag 0 is none, 1 t=nostr,
// 2 t=coax, 3 both. expiry -1 is none. name numbers it in its case (e1, e2...).
Ev :: struct { name: int; author: int; kind: int; created: int; id: int; tag: int; expiry: int; }
// One generated filter. A field is absent unless its has_ flag is set;
// authors and kinds are bit sets over A, B and KINDS, 0 when absent; tag 0
// is absent, 1 is "#t":["nostr"], 2 is "#t":["coax"].
Flt :: struct {
  has_ids: bool; id: int; authors: int; kinds: int;
  has_since: bool; since: int; has_until: bool; until: int;
  tag: int; has_limit: bool; limit: int;
}
// One case: up to six events, published in order, and one filter.
Case :: struct { n: int; ev: [6] Ev; f: Flt; }
```

### The generator

The generator is xorshift32 from seed 94. Each draw is the new state mod the
size of the value set:

<!-- source: nostr/examples/differential.coax generate -->
```jai
// xorshift32 from a fixed seed: the same cases on every run and every target.
Rng :: struct { s: u32; }
roll :: (r: [] Rng, n: int) -> int {
  h := r[0].s;
  h ^= h << 13; h ^= h >> 17; h ^= h << 5;
  r[0].s = h;
  return cast(int) (h % cast(u32) n);
}
// Small value sets on purpose: equal times, the same author twice and
// boundary values for since, until and expiry come up often.
generate :: (r: [] Rng) -> Case {
  c: Case; c.n = 1 + roll(r, 6);
  used: [256] bool;
  for i: 0..<c.n {
    e: Ev; e.name = i + 1; e.author = roll(r, 2); e.kind = KINDS[roll(r, KINDS.count)]; e.created = TIMES[roll(r, TIMES.count)];
    id := 1 + roll(r, 254); while used[id] { id = 1 + roll(r, 254); } used[id] = true; e.id = id;
    e.tag = roll(r, TAG_TEXT.count);
    x := roll(r, 6); e.expiry = -1;
    if x == 0 { e.expiry = ASK; }
    if x == 1 { e.expiry = ASK + 1; }
    c.ev[i] = e;
  }
  f: Flt;
  // id byte 255 is never generated: a filter for an id nobody has
  if roll(r, 4) == 0 { f.has_ids = true; f.id = 255; if roll(r, 4) != 0 { f.id = c.ev[roll(r, c.n)].id; } }
  if roll(r, 3) == 0 { f.authors = 1 + roll(r, 3); }
  if roll(r, 2) == 0 { f.kinds = 1 + roll(r, 15); }
  if roll(r, 3) == 0 { f.has_since = true; f.since = TIMES[roll(r, TIMES.count)]; }
  if roll(r, 3) == 0 { f.has_until = true; f.until = TIMES[roll(r, TIMES.count)]; }
  if roll(r, 3) == 0 { f.tag = 1 + roll(r, 2); }
  if roll(r, 3) == 0 { f.has_limit = true; f.limit = roll(r, 4); }
  c.f = f;
  return c;
}
```

### The relay

The relay side renders the case for the relay and runs the relay's own
code. Each event goes into a fresh store through `ns_store_admit`, so
replacement and ephemeral kinds behave as in the relay. The filter is
written as REQ JSON and read with `filter_read`. `rsd_query` answers at
`ASK`, one minute after the events were admitted, as
`coax/relay/relay.coax` does. Signatures and ids are not part of this
differential; chapter 34 covers admission, and the records hold the fields
admission reads.

<!-- source: nostr/examples/differential.coax relay -->
```jai
// Implementation 1: the relay. Each event goes into a fresh store through
// ns_store_admit (replacement, ephemeral kinds); the filter goes through
// filter_read, as a REQ does; rsd_query answers at ASK, exactly as
// coax/relay/relay.coax does. The answer is event numbers, in the relay's
// order. fault plants one bug: until compared with >= instead of >.
Relay :: struct { arena: [4096] u8; recs: [8] RsdRec; s: [1] RsdStore; }
relay_answer :: (c: Case, fault: bool, out: [] int) -> int {
  r: [1] Relay; r[0].s[0].arena = r[0].arena[..]; r[0].s[0].recs = r[0].recs[..];
  for i: 0..<c.n { stored := ns_store_admit(r[0].s[..], event_rec(c.ev[i]), TAG_TEXT[c.ev[i].tag], true); }
  text: [512] u8;
  strs: [16] string; ints: [16] int; tags: [4] NostrTagFilter; bytes: [512] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter;
  #assert filter_read(filter_json(c.f, text[..]), f[..], sp[..]) == FR_OK;
  if fault && f[0].has_until { f[0].until -= 1; }
  idx: [8] int;
  n := rsd_query(r[0].s[..], f[0], ASK, idx[..]);
  for k: 0..<n { out[k] = event_of_id(c, cast(int) r[0].recs[idx[k]].id[0]); }
  return n;
}
event_of_id :: (c: Case, id: int) -> int {
  for i: 0..<c.n { if c.ev[i].id == id { return i; } }
  return -1;
}
```

The `fault` flag is the seeded fault: it subtracts one from `until`, which
turns the relay's `created_at > until` refusal into `created_at >= until`.
That is the relational-operator mutant that `coax mutate` would make in
`nf_match`.

### The reference

The reference reads NIP-01 literally: "All of the conditions of a filter
that are specified must match", "an event matches a filter if
`since <= created_at <= until` holds", and "Newer events should appear
first, and in the case of ties the event with the lowest id (first in
lexical order) should be first". It compares ids as hex text. It doesn't
load `filter.coax` or the store.

<!-- source: nostr/examples/differential.coax reference -->
```jai
// Implementation 2: NIP-01's filter section read literally, over the model.
// "All of the conditions of a filter that are specified must match";
// "since <= created_at <= until"; a tag filter matches an event with that
// tag letter and one of the values. Nothing here calls the library.
ref_match :: (f: Flt, e: Ev) -> bool {
  if f.has_ids && f.id != e.id { return false; }
  if f.authors != 0 && f.authors & (1 << e.author) == 0 { return false; }
  if f.kinds != 0 {
    found := false; for k: 0..<KINDS.count { if f.kinds & (1 << k) != 0 && KINDS[k] == e.kind { found = true; } }
    if !found { return false; }
  }
  if f.has_since && !(f.since <= e.created) { return false; }
  if f.has_until && !(e.created <= f.until) { return false; }
  if f.tag != 0 && e.tag & f.tag == 0 { return false; }
  return true;
}
// "the event with the lowest id (first in lexical order)": compared as hex text.
lower_id :: (a: int, b: int) -> bool {
  x: [64] u8; y: [64] u8; s := id_hex(a, x[..]); t := id_hex(b, y[..]);
  for i: 0..<64 {
    if s[i] < t[i] { return true; }
    if s[i] > t[i] { return false; }
  }
  return false;
}
// "Newer events should appear first, and in the case of ties the event with
// the lowest id ... should be first."
ref_before :: (a: Ev, b: Ev) -> bool { return a.created > b.created || (a.created == b.created && lower_id(a.id, b.id)); }
// Every matching event, in that order, with no limit applied.
ref_answer :: (c: Case, out: [] int) -> int {
  n := 0;
  for i: 0..<c.n {
    if !ref_match(c.f, c.ev[i]) { continue; }
    at := n; while at > 0 && ref_before(c.ev[i], c.ev[out[at - 1]]) { out[at] = out[at - 1]; at -= 1; }
    out[at] = i; n += 1;
  }
  return n;
}
```

### Compare

`compare` runs both, then normalizes the reference: it drops each event a
known rule explains, counting the rule, and then applies the filter's
limit. (The relay's `rs_limit` caps a limit at 127, more than the six events
a case can hold.) The two lists must then be equal, in order.

<!-- source: nostr/examples/differential.coax compare -->
```jai
MATCH :: 0; EXPLAINED :: 1; UNEXPLAINED :: 2;
Verdict :: struct {
  verdict: int; why: [3] int;          // events each known rule removed
  relay: [8] int; rn: int;           // the relay's answer
  ref: [8] int; fn: int;             // the reference, normalized
}
// Run both, normalize the reference (drop what a known rule explains, then
// apply the limit the relay applies), compare the lists in order.
compare :: (c: Case, fault: bool) -> Verdict {
  v: Verdict;
  v.rn = relay_answer(c, fault, v.relay[..]);
  all: [8] int; n := ref_answer(c, all[..]);
  // rs_limit caps a limit at 127, above any case's six events
  limit := c.n; if c.f.has_limit { limit = c.f.limit; }
  for k: 0..<n {
    w := known_rule(c, all[k]);
    if w >= 0 { v.why[w] += 1; continue; }
    if v.fn < limit { v.ref[v.fn] = all[k]; v.fn += 1; }
  }
  // v.verdict starts as MATCH (0)
  if v.why[0] + v.why[1] + v.why[2] > 0 { v.verdict = EXPLAINED; }
  relay := v.relay; ref := v.ref;
  if !same_list(relay[..], v.rn, ref[..], v.fn) { v.verdict = UNEXPLAINED; }
  return v;
}
// The same events in the same order.
same_list :: (a: [] int, an: int, b: [] int, bn: int) -> bool {
  if an != bn { return false; }
  for k: 0..<an { if a[k] != b[k] { return false; } }
  return true;
}
```

The order matters: a known rule is applied before the limit. With
`"limit":1`, an expired newest event would otherwise take the one slot in
the reference's answer, and the relay's correct answer would look wrong.

## Minimization

A generated failing case often has five events and several filter fields,
and most of them are unrelated to the bug. `minimize` removes one event or
one filter field at a time and keeps the removal if the case still gives
an unexplained difference. It repeats until a full round removes nothing.
The result is 1-minimal: removing any one remaining part makes the
difference go away. A fuller minimizer would also shrink values, such as
moving a time towards `NOW`; this one removes parts only.

<!-- source: nostr/examples/differential.coax minimize -->
```jai
// Remove event i (later events keep their names) or filter field k.
without_event :: (c: Case, i: int) -> Case {
  t := c; t.n = c.n - 1;
  for j: i..<t.n { t.ev[j] = c.ev[j + 1]; }
  return t;
}
FIELD_NAME :: [7] string.["ids", "authors", "kinds", "since", "until", "#t", "limit"];
has_field :: (f: Flt, k: int) -> bool {
  if k == 0 { return f.has_ids; } if k == 1 { return f.authors != 0; } if k == 2 { return f.kinds != 0; }
  if k == 3 { return f.has_since; } if k == 4 { return f.has_until; } if k == 5 { return f.tag != 0; }
  return f.has_limit;
}
without_field :: (c: Case, k: int) -> Case {
  t := c;
  if k == 0 { t.f.has_ids = false; } if k == 1 { t.f.authors = 0; } if k == 2 { t.f.kinds = 0; }
  if k == 3 { t.f.has_since = false; } if k == 4 { t.f.has_until = false; } if k == 5 { t.f.tag = 0; }
  if k == 6 { t.f.has_limit = false; }
  return t;
}
// One try: event e<name> (field -1) or filter field <field>, and whether
// it was removed. A round removes a part or ends the search, so a case of
// six events and seven fields takes at most 14 rounds of 13 tries.
Try :: struct { round: int; name: int; field: int = -1; removed: bool; }
Trace :: struct { tries: [182] Try; n: int; }
note :: (trace: [] Trace, t: Try) { trace[0].tries[trace[0].n] = t; trace[0].n += 1; }
// Greedy one-at-a-time reduction: keep any removal after which the case
// still gives an unexplained difference; repeat until no removal does.
minimize :: (c: Case, fault: bool, trace: [] Trace) -> Case {
  cur := c; changed := true; round := 0;
  while changed {
    changed = false; round += 1;
    i := 0;
    while i < cur.n {
      t := without_event(cur, i);
      gone := compare(t, fault).verdict == UNEXPLAINED;
      note(trace, Try.{round = round, name = cur.ev[i].name, removed = gone});
      if gone { cur = t; changed = true; } else { i += 1; }
    }
    for k: 0..<FIELD_NAME.count {
      if !has_field(cur.f, k) { continue; }
      t := without_field(cur, k);
      gone := compare(t, fault).verdict == UNEXPLAINED;
      note(trace, Try.{round = round, field = k, removed = gone});
      if gone { cur = t; changed = true; }
    }
  }
  return cur;
}
```

## Worked example: minimize case 17 by hand

With the seeded fault, the first unexplained case is case 17. `NOW` is
1793491200, so the filter's `until` 1793491000 is `NOW-200`:

| Event | Author | Kind | created_at | id | Tags |
| --- | --- | --- | --- | --- | --- |
| e1 | B | 20001 | NOW | `22...` | t=nostr, t=coax |
| e2 | A | 1 | NOW-200 | `f1...` | t=nostr, t=coax |
| e3 | A | 7 | NOW | `72...` | none |
| e4 | A | 0 | NOW-200 | `f9...` | t=nostr, t=coax |
| e5 | B | 1 | NOW-100 | `7e...` | t=nostr, t=coax |

Filter: `{"until":1793491000}`.

1. The reference keeps events with `created_at <= NOW-200`: e2 and e4.
   They have the same second, so the lower id comes first: `f1 < f9` gives
   e2, e4. No known rule applies: e4 is kind 0, but it is A's only kind 0.
2. The faulty relay compares with `until - 1 = NOW-201`. No event is that
   old, so it answers nothing.
3. e2, e4 against nothing: unexplained.

Now remove parts, one at a time, and recompute both answers each time:

| Try | Remaining events | Reference | Faulty relay | Keep the removal? |
| --- | --- | --- | --- | --- |
| drop e1 | e2 e3 e4 e5 | e2 e4 | none | yes |
| drop e2 | e3 e4 e5 | e4 | none | yes |
| drop e3 | e4 e5 | e4 | none | yes |
| drop e4 | e5 | none (NOW-100 is after `until`) | none | no: they agree |
| drop e5 | e4 | e4 | none | yes |
| drop `until` | e4 | e4 | e4 | no: they agree |

A second round tries e4 and `until` again, and both stay. The minimal case
is one event and one field: e4 at `NOW-200` and `{"until":1793491000}`.
It says exactly what the bug is: an event at the `until` second is lost.
Without the fault, the relay returns e4 and the case matches.

## The example

The example is
[nostr/examples/differential.coax](../examples/differential.coax). It
prints the known rules and case 0, runs the 500 cases against the relay as
built and against the seeded fault, then prints the first unexplained case
and its minimization. Its main part:

<!-- source: nostr/examples/differential.coax main -->
```jai
CASES :: 500;
SEED :: 94;
Tally :: struct { count: [3] int; why: [3] int; first: int = -1; }
tally :: (fault: bool) -> Tally {
  r: [1] Rng; r[0].s = SEED;
  t: Tally;
  for i: 0..<CASES {
    c := generate(r[..]); v := compare(c, fault);
    t.count[v.verdict] += 1;
    for w: 0..<KNOWN { t.why[w] += v.why[w]; }
    if v.verdict == UNEXPLAINED && t.first == -1 { t.first = i; }
  }
  return t;
}
run :: (label: string, fault: bool) -> Tally {
  t := tally(fault);
  print("%: % cases, % match, % explained, % unexplained\n", label, CASES, t.count[MATCH], t.count[EXPLAINED], t.count[UNEXPLAINED]);
  print("    events removed by a known rule:");
  for w: 0..<KNOWN { print(" % %", t.why[w], KNOWN_NAME[w]); }
  print("\n");
  return t;
}
case_at :: (i: int) -> Case {
  r: [1] Rng; r[0].s = SEED;
  c: Case; for k: 0..<i + 1 { c = generate(r[..]); }
  return c;
}

main :: () {
  print("the relay's query (rsd_query) against NIP-01's filter text, at ASK = NOW+60\n");
  print("known differences:\n");
  for w: 0..<KNOWN { print("  %: %\n", KNOWN_NAME[w], KNOWN_WHY[w]); }

  print("\ncase 0 of seed %:\n", SEED);
  c0 := case_at(0); show_case(c0); show_verdict(c0, compare(c0, false));

  print("\n");
  clean := run("the relay as built", false);
  print("\n");
  bad := run("seeded fault: until compared with >=", true);

  if bad.first < 0 { print("the seeded fault went unseen\n"); return; }
  c := case_at(bad.first);
  print("\nfirst unexplained difference: case %, % events\n", bad.first, c.n);
  show_case(c); show_verdict(c, compare(c, true));
  print("minimizing:\n");
  tr: [1] Trace; m := minimize(c, true, tr[..]); show_trace(tr[0]);
  print("minimal case:\n");
  show_case(m); show_verdict(m, compare(m, true));
  print("the same case without the fault:\n");
  show_verdict(m, compare(m, false));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/differential.coax
```

The output (the same in debug and release):

```text
the relay's query (rsd_query) against NIP-01's filter text, at ASK = NOW+60
known differences:
  ephemeral: NIP-01: kinds 20000-29999 are not expected to be stored
  replaced: NIP-01: one replaceable event per pubkey and kind, the newest, then the lowest id
  expired: NIP-40: relays SHOULD NOT send expired events; expired once ASK >= expiration

case 0 of seed 94:
    e1 B kind 20001 NOW id 74... tags []
    e2 A kind 1 NOW id 50... tags [["t","nostr"]]
    e3 B kind 20001 NOW-200 id 2b... tags [["t","coax"]]
    e4 A kind 1 NOW-300 id f1... tags [["t","coax"]] expires NOW+61
    filter {"authors":["df00000000000000000000000000000000000000000000000000000000000000"],"#t":["nostr"],"limit":0}
    relay: (none); reference: (none) -> match

the relay as built: 500 cases, 342 match, 158 explained, 0 unexplained
    events removed by a known rule: 153 ephemeral 28 replaced 62 expired

seeded fault: until compared with >=: 500 cases, 326 match, 152 explained, 22 unexplained
    events removed by a known rule: 153 ephemeral 28 replaced 62 expired

first unexplained difference: case 17, 5 events
    e1 B kind 20001 NOW id 22... tags [["t","nostr"],["t","coax"]]
    e2 A kind 1 NOW-200 id f1... tags [["t","nostr"],["t","coax"]]
    e3 A kind 7 NOW id 72... tags []
    e4 A kind 0 NOW-200 id f9... tags [["t","nostr"],["t","coax"]]
    e5 B kind 1 NOW-100 id 7e... tags [["t","nostr"],["t","coax"]]
    filter {"until":1793491000}
    relay: (none); reference: e2 e4 -> UNEXPLAINED
minimizing:
  round 1: -e1 -e2 -e3 e4 stays -e5 until stays
  round 2: e4 stays until stays
minimal case:
    e4 A kind 0 NOW-200 id f9... tags [["t","nostr"],["t","coax"]]
    filter {"until":1793491000}
    relay: (none); reference: e4 -> UNEXPLAINED
the same case without the fault:
    relay: e4; reference: e4 -> match
```

What it shows:

- Case 0 matches: the filter asks for A's `#t` `nostr` events with
  `"limit":0`, and both answers are empty, as NIP-01 requires ("When
  `limit` is zero, the relay MUST NOT return stored events").
- The relay as built: 342 cases match exactly, and 158 differ only by the
  three known rules (153 ephemeral events, 28 replaced and 62 expired are
  removed from the reference's answers). No difference is unexplained.
- The seeded fault: 22 cases become unexplained. The other 478 don't
  exercise it: their filter has no `until`, or no event sits exactly on the
  `until` second. That is why the generator draws times from a set of four.
- The broken relay's refusal of e4 is shown as
  `relay: (none); reference: e4 -> UNEXPLAINED`, and the minimizer's trace
  matches the hand table above.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/differential.coax](../examples/checks/differential.coax),
which loads the example. They compare the example's parts with values
worked out by hand:

- the generator's first seven xorshift32 states from seed 94, computed
  separately, and case 0's first event decoded from them;
- cases 44 and 142 against a second implementation of the generator, a
  Python scratch script: case 44 draws an id twice and redraws it, and case
  142's filter asks for id 255, which no event has;
- the minimal case: the reference and the relay return e4, the faulty relay
  nothing, and one second either side of `until` all three agree;
- case 17's fields, the minimizer's eight tries on it in the order of the
  hand table, and its result, e4 and `until`, where dropping either removes
  the difference (1-minimal); with `"ids"` added, the minimizer removes it
  and keeps `until`;
- `without_field` clears exactly one field, and `without_event` keeps the
  other events in order;
- every `since` and `until` in the time set against every event time, for
  the reference and for the relay (through `filter_read`, `rsd_query` and
  `nf_match`);
- each known rule on its own event: ephemeral, expired at `ASK` but not at
  `ASK + 1`, two same-second kind 0 events from one author (the lower id
  stays), from two authors (both stay), and kind 1 (never replaced);
- NIP-01's kind ranges on both sides of each boundary (3 and 4, 9999 and
  10000, 19999 and 20000, 29999 and 30000), and an expiration of 0;
- `event_of_id`, which maps the relay's records back to event names, for
  the first and last event and a missing id;
- `lower_id` on hex text (`15` before `1a`, `1f` before `20`, never an id
  before itself) and `same_list` on equal, shorter and reordered lists;
- the counts of both 500-case runs, exactly as the transcript prints them;
- a `#t` filter by hand, which reaches the relay's tag matcher;
- order and limit: newest first, the lower id on a tie, `"limit":1` and
  `"limit":0`;
- 1,200 more generated cases from eight other seeds, with no unexplained
  difference;
- the REQ JSON for an empty filter, for every field at once and for single
  authors.

<!-- source: nostr/examples/checks/differential.coax check -->
```jai
dv_ev :: (name: int, author: int, kind: int, created: int, id: int, tag: int, expiry: int) -> Ev {
  return Ev.{name = name, author = author, kind = kind, created = created, id = id, tag = tag, expiry = expiry};
}
single :: (e: Ev, f: Flt) -> Case { c: Case; c.n = 1; c.ev[0] = e; c.f = f; return c; }

// xorshift32 from 94, computed separately (Python, 2026-11-01): states
// 24365883, 2055631121, 539321123, 74048867, 544651553, 487502728,
// 2760184023. Mod 6, 2, 4, 4, 254, 4, 6 they give 3, 1, 3, 3, 115, 0, 3:
// four events; e1 is B, kind KINDS[3] = 20001, at TIMES[3] = NOW, id
// 1 + 115 = 0x74, no tags, no expiry. That is case 0 as the example prints it.
#test "case 0 decoded by hand from the generator" {
  r: [1] Rng; r[0].s = 94;
  #assert roll(r[..], 6) == 3 && r[0].s == 24365883;
  c := case_at(0);
  #assert c.n == 4;
  e := c.ev[0];
  #assert e.author == 1 && e.kind == 20001 && e.created == NOW && e.id == 0x74 && e.tag == 0 && e.expiry == -1;
  #assert c.ev[3].expiry == ASK + 1;
}
// The same generator rewritten in Python (scratch script, 2026-11-01) gives
// the same cases. Case 44 draws an id twice and redraws it; case 142's
// filter asks for id 255, which no event has.
IDS_44 :: [6] int.[0x20, 0xae, 0x49, 0x38, 0x9d, 0x52];
IDS_142 :: [5] int.[0x40, 0x78, 0x08, 0x92, 0xaa];
#test "cases 44 and 142 match a second implementation of the generator" {
  c := case_at(44);
  #assert c.n == 6 && c.f.authors == 1 && c.f.tag == 2 && !c.f.has_ids;
  for i: 0..<6 { #assert c.ev[i].id == IDS_44[i]; }
  c = case_at(142);
  #assert c.n == 5 && c.f.has_ids && c.f.id == 255 && c.f.authors == 0 && c.f.kinds == 0;
  for i: 0..<5 { #assert c.ev[i].id == IDS_142[i]; }
}

// The minimal case, worked by hand: NIP-01 says created_at <= until, and
// NOW-200 <= NOW-200 holds, so the reference returns e4. The relay as built
// agrees; with until compared with >= it returns nothing.
#test "the minimal case by hand" {
  f: Flt; f.has_until = true; f.until = NOW - 200;
  c := single(dv_ev(4, 0, 0, NOW - 200, 0xf9, 3, -1), f);
  out: [8] int;
  #assert ref_answer(c, out[..]) == 1 && out[0] == 0;
  #assert relay_answer(c, false, out[..]) == 1 && out[0] == 0;
  #assert relay_answer(c, true, out[..]) == 0;
  #assert compare(c, false).verdict == MATCH && compare(c, true).verdict == UNEXPLAINED;
  // one second either side: both implementations agree, with or without the fault
  c.f.until = NOW - 201; #assert compare(c, false).verdict == MATCH && compare(c, true).verdict == MATCH;
  c.f.until = NOW - 199; #assert compare(c, false).verdict == MATCH && compare(c, true).verdict == MATCH;
}

// The minimizer on case 17 makes exactly the tries of the hand table, and
// its result is 1-minimal: dropping its event or its field makes the
// difference go away.
TRY_ROUND :: [8] int.[1, 1, 1, 1, 1, 1, 2, 2];
TRY_NAME :: [8] int.[1, 2, 3, 4, 5, 0, 4, 0];
TRY_FIELD :: [8] int.[-1, -1, -1, -1, -1, 4, -1, 4];
TRY_GONE :: [8] bool.[true, true, true, false, true, false, false, false];
#test "the minimizer follows the hand table to a 1-minimal case" {
  c := case_at(17);
  #assert c.n == 5 && c.f.has_until && c.f.until == NOW - 200;
  #assert !c.f.has_ids && c.f.authors == 0 && c.f.kinds == 0 && !c.f.has_since && c.f.tag == 0 && !c.f.has_limit;
  #assert c.ev[3].kind == 0 && c.ev[3].created == NOW - 200 && c.ev[3].id == 0xf9 && c.ev[1].id == 0xf1;
  #assert compare(c, true).verdict == UNEXPLAINED;
  tr: [1] Trace;
  m := minimize(c, true, tr[..]);
  // round 1: -e1 -e2 -e3, e4 stays, -e5, until stays; round 2: e4 and until stay
  #assert tr[0].n == 8;
  for k: 0..<8 {
    t := tr[0].tries[k];
    #assert t.round == TRY_ROUND[k] && t.field == TRY_FIELD[k] && t.removed == TRY_GONE[k];
    if TRY_FIELD[k] < 0 { #assert t.name == TRY_NAME[k]; }
  }
  #assert m.n == 1 && m.ev[0].name == 4 && m.f.has_until && m.f.until == NOW - 200;
  #assert compare(without_event(m, 0), true).verdict != UNEXPLAINED;
  #assert compare(without_field(m, 4), true).verdict != UNEXPLAINED;
  // with "ids":[f9...] added, the minimizer removes ids and keeps until
  c = m; c.f.has_ids = true; c.f.id = 0xf9;
  tr[0] = Trace.{};
  m = minimize(c, true, tr[..]);
  #assert tr[0].n == 5 && tr[0].tries[1].field == 0 && tr[0].tries[1].removed && !tr[0].tries[2].removed;
  #assert !m.f.has_ids && m.f.has_until && m.n == 1;
}
// Removing field k clears k and nothing else; removing an event keeps the
// others' order and names.
#forall "without_field clears one field" k: 0..<FIELD_NAME.count {
  f := Flt.{has_ids = true, id = 3, authors = 1, kinds = 2, has_since = true, since = NOW, has_until = true, until = NOW, tag = 1, has_limit = true, limit = 2};
  c: Case; c.f = f;
  t := without_field(c, k);
  for j: 0..<FIELD_NAME.count { #assert has_field(t.f, j) == (j != k); }
}
#test "without_event keeps the rest in order" {
  c := case_at(17);
  t := without_event(c, 1);
  #assert t.n == 4 && t.ev[0].name == 1 && t.ev[1].name == 3 && t.ev[2].name == 4 && t.ev[3].name == 5;
  t = without_event(c, 4);
  #assert t.n == 4 && t.ev[3].name == 4;
}

// The reference against the relay (filter_read, rsd_query, nf_match)
// on NIP-01's boundaries, every since and until in TIMES against every time.
#test "the reference and the relay agree on the since and until boundaries" {
  for t: 0..<4 {
    for b: 0..<4 {
      e := dv_ev(1, 0, 1, TIMES[t], 9, 0, -1);
      f: Flt; f.has_since = true; f.since = TIMES[b];
      c := single(e, f); out: [8] int;
      #assert (ref_answer(c, out[..]) == 1) == (TIMES[b] <= TIMES[t]);
      #assert (relay_answer(c, false, out[..]) == 1) == (TIMES[b] <= TIMES[t]);
      f = Flt.{}; f.has_until = true; f.until = TIMES[b]; c = single(e, f);
      #assert (ref_answer(c, out[..]) == 1) == (TIMES[t] <= TIMES[b]);
      #assert (relay_answer(c, false, out[..]) == 1) == (TIMES[t] <= TIMES[b]);
    }
  }
}

// The kind ranges of NIP-01: ephemeral 20000 to 29999; replaceable 0, 3
// and 10000 to 19999. A newer copy from the same author explains the old one
// only for a replaceable kind.
RULE_KIND :: [12] int.[19999, 20000, 29999, 30000, 0, 3, 2, 4, 9999, 10000, 19999, 1];
RULE_WANT :: [12] int.[1, 0, 0, -1, 1, 1, -1, -1, -1, 1, 1, -1];
#test "the known rules' kind ranges" {
  for k: 0..<12 {
    c: Case; c.n = 2;
    c.ev[0] = dv_ev(1, 0, RULE_KIND[k], NOW - 100, 0x40, 0, -1);
    c.ev[1] = dv_ev(2, 0, RULE_KIND[k], NOW, 0x41, 0, -1);
    #assert known_rule(c, 0) == RULE_WANT[k];
  }
  #assert ephemeral(20000) && ephemeral(29999) && !ephemeral(19999) && !ephemeral(30000);
  #assert replaceable(10000) && replaceable(19999) && !replaceable(20000) && !replaceable(9999);
  // an expiration of 0 is long past
  c := single(dv_ev(1, 0, 1, NOW, 5, 0, 0), Flt.{});
  #assert known_rule(c, 0) == 2;
}
// Ids compare as text, first character first.
#test "lower_id compares the hex text" {
  #assert lower_id(0x15, 0x1a) && !lower_id(0x1a, 0x15);
  #assert lower_id(0x1f, 0x20) && !lower_id(0x20, 0x1f);
  #assert !lower_id(5, 5);
}
#test "event_of_id finds the first and the last event" {
  c := case_at(17);
  #assert event_of_id(c, 0x22) == 0 && event_of_id(c, 0x7e) == 4 && event_of_id(c, 0xff) == -1;
}
#test "same_list wants the same events in the same order" {
  a := [3] int.[1, 2, 3]; b := [3] int.[1, 2, 3];
  #assert same_list(a[..], 3, b[..], 3) && same_list(a[..], 0, b[..], 0);
  #assert !same_list(a[..], 3, b[..], 2);
  b[0] = 9; #assert !same_list(a[..], 3, b[..], 3);
  b[0] = 1; b[2] = 9; #assert !same_list(a[..], 3, b[..], 3);
}
// The example's counts, as its transcript prints them.
#test "the two runs give the transcript's counts" #steps 400000000 {
  t := tally(false);
  #assert t.count[MATCH] == 342 && t.count[EXPLAINED] == 158 && t.count[UNEXPLAINED] == 0 && t.first == -1;
  #assert t.why[0] == 153 && t.why[1] == 28 && t.why[2] == 62;
  t = tally(true);
  #assert t.count[MATCH] == 326 && t.count[EXPLAINED] == 152 && t.count[UNEXPLAINED] == 22 && t.first == 17;
  #assert t.why[0] == 153 && t.why[1] == 28 && t.why[2] == 62;
}
// Each known rule, by hand.
#test "each known rule explains its event and no other" {
  f: Flt;
  c := single(dv_ev(1, 0, 20001, NOW, 5, 0, -1), f);
  v := compare(c, false); #assert v.verdict == EXPLAINED && v.why[0] == 1 && v.rn == 0;
  // NIP-40: expired once ASK >= expiration; one second later it is still sent
  c = single(dv_ev(1, 0, 1, NOW, 5, 0, ASK), f);
  v = compare(c, false); #assert v.verdict == EXPLAINED && v.why[2] == 1 && v.rn == 0;
  c.ev[0].expiry = ASK + 1;
  v = compare(c, false); #assert v.verdict == MATCH && v.rn == 1;
  // two kind 0 events from A in the same second: the lower id (0x05) stays
  c.n = 2; c.ev[0] = dv_ev(1, 0, 0, NOW, 0x30, 0, -1); c.ev[1] = dv_ev(2, 0, 0, NOW, 0x05, 0, -1);
  v = compare(c, false);
  #assert v.verdict == EXPLAINED && v.why[1] == 1 && v.rn == 1 && v.relay[0] == 1;
  #assert known_rule(c, 0) == 1 && known_rule(c, 1) == -1;
  // from different authors, both stay
  c.ev[1].author = 1; v = compare(c, false); #assert v.verdict == MATCH && v.rn == 2;
  // kind 1 is never replaced
  c.ev[0].kind = 1; c.ev[1] = dv_ev(2, 0, 1, NOW, 0x05, 0, -1);
  #assert known_rule(c, 0) == -1 && compare(c, false).verdict == MATCH;
}

// Order and limit: NOW-100 before NOW-200; ties by the lower id; limit 1.
#test "order and limit by hand" {
  c: Case; c.n = 3;
  c.ev[0] = dv_ev(1, 0, 1, NOW - 200, 0x10, 0, -1);
  c.ev[1] = dv_ev(2, 1, 1, NOW - 100, 0x90, 0, -1);
  c.ev[2] = dv_ev(3, 0, 1, NOW - 100, 0x20, 0, -1);
  v := compare(c, false);
  #assert v.verdict == MATCH && v.rn == 3 && v.relay[0] == 2 && v.relay[1] == 1 && v.relay[2] == 0;
  c.f.has_limit = true; c.f.limit = 1;
  v = compare(c, false); #assert v.verdict == MATCH && v.rn == 1 && v.relay[0] == 2;
  c.f.limit = 0; v = compare(c, false); #assert v.verdict == MATCH && v.rn == 0;
}

// Other seeds: the relay as built has no unexplained difference.
#forall "no unexplained difference on other seeds" s: 1..<9 #steps 400000000 {
  r: [1] Rng; r[0].s = cast(u32) (s * 7919);
  for i: 0..<150 { #assert compare(generate(r[..]), false).verdict != UNEXPLAINED; }
}
// The filter JSON for the hand case, and every field at once.
#test "the filter as REQ JSON" {
  f: Flt; out: [512] u8;
  #assert filter_json(f, out[..]) == "{}";
  f.has_until = true; f.until = NOW - 200;
  #assert filter_json(f, out[..]) == "{\"until\":1793491000}";
  f.has_ids = true; f.id = 0xf9; f.authors = 3; f.kinds = 6; f.has_since = true; f.since = NOW; f.tag = 2; f.has_limit = true; f.limit = 0;
  want := "{\"ids\":[\"f900000000000000000000000000000000000000000000000000000000000000\"],\"authors\":[\"df00000000000000000000000000000000000000000000000000000000000000\",\"dd00000000000000000000000000000000000000000000000000000000000000\"],\"kinds\":[1,7],\"since\":1793491200,\"until\":1793491000,\"#t\":[\"coax\"],\"limit\":0}";
  #assert filter_json(f, out[..]) == want;
  g: Flt; g.authors = 2; g.kinds = 9; g.tag = 1;
  #assert filter_json(g, out[..]) == "{\"authors\":[\"dd00000000000000000000000000000000000000000000000000000000000000\"],\"kinds\":[0,20001],\"#t\":[\"nostr\"]}";
  g = Flt.{}; g.authors = 1; g.has_limit = true; g.limit = 120;
  #assert filter_json(g, out[..]) == "{\"authors\":[\"df00000000000000000000000000000000000000000000000000000000000000\"],\"limit\":120}";
}
// A #t filter reaches the relay's tag matcher: only events with that tag.
#test "a tag filter by hand" {
  c: Case; c.n = 4;
  for i: 0..<4 { c.ev[i] = dv_ev(i + 1, 0, 1, NOW - 100 * i, 0x10 + i, i, -1); }
  c.f.tag = 2;
  v := compare(c, false);
  #assert v.verdict == MATCH && v.rn == 2 && v.relay[0] == 2 && v.relay[1] == 3;
  c.f.tag = 1; v = compare(c, false);
  #assert v.verdict == MATCH && v.rn == 2 && v.relay[0] == 1 && v.relay[1] == 3;
}
```

Run them:

```shell
coax/bin/coax test nostr/examples/checks/differential.coax
```

All 17 of this file's checks pass, with the 302 library checks the file
loads (319 in all, 2026-11-01). `coax mutate --root-only` on a temporary
file that joins the example and its checks, so that the example's
procedures are mutated: 572 mutants, 572 killed (the print-only procedures `show_*` and `run` are not reached by a check). On the checks file alone, 5 of 5.

### Against the Rust relay

The Rust relay is the reference for the Coax relay's R1 behaviour.
[2026-10-30-coax-relay-r1-rust-differential.md](../../docs/verification/2026-10-30-coax-relay-r1-rust-differential.md)
records how it runs:

1. `rust_relay_sandbox.sh extract` copies `crates/nostr-relay`, its two path
   crates, the migrations and the files they read at compile time from
   `0b401561a0^` with `git archive`, outside the repository. Nothing is
   restored into the tree.
2. `fetch` is the only step with network: `cargo fetch`, then a check that
   all 144 packages have the old `Cargo.lock`'s versions and checksums.
3. `build` and `run` have no network: the relay is built offline and run in
   bwrap against a throwaway Postgres 17.11 on loopback, created with
   `initdb` in the scratch directory.
4. `relay_rust_differential.py` signs 169 events with the test-only key and
   plays the same script against both relays: NIP-11, AUTH, admission,
   replacement, REQ, limits, CLOSE and other messages.

The result was 114 observations: 88 match, 15 text, 7 intentional, 4 owner,
0 unexplained, with no sanitizer report. The run also found one bug: a REQ
with no filter opened a subscription in the Coax relay, where the Rust relay
answers CLOSED. The fix came with its check first,
`#test "a REQ needs at least one filter"` in `relay_route.coax`. The
differential was rerun at `bffe9c0d04` with the same counts.

It was not rerun for this chapter. The fetch step needs network access, and
the owner approved that for the 2026-10-30 run; the relay code it compares
hasn't changed since the rerun.

### Against the production recording

`relay_differential.py` publishes the 237 events relay.openagents.com sent
for the project page's five REQs (recorded 2026-10-28) to a fresh Coax relay
built with ASan and UBSan, sends the same REQs, and compares ids, order,
event bytes and EOSE. It never connects to a public relay. It was run
again on 2026-11-01 for this chapter: all 237 events were published and
verified, and each of the five subscriptions returned the Rust relay's
events in its order (2, 105, 121, 8 and 1 events). The only difference is
the Rust relay's EOSE third element, which the driver notes. There was no
sanitizer report, and the run took 62 s.
Chapter 36's example runs the same five REQs inside Coax.

### Signatures and encryption

The same method checks the cryptography against independent libraries. The
signature differential
[2026-10-29-schnorr-fast-differential.json](../../docs/verification/2026-10-29-schnorr-fast-differential.json)
compares the fast and the original BIP-340 verifiers with pinned
libsecp256k1 and an independent Python affine reference over 790 cases (the
19 pinned vectors, seeded valid signatures, mutated messages, keys and
signatures, and r and s boundaries), in checked and release native and wasm
builds, and records the SHA-256 of every source file it ran
([2026-10-29-schnorr-fast.md](../../docs/verification/2026-10-29-schnorr-fast.md)).
The relay drivers sign their test events with `@noble/curves` and a
test-only key. These libraries run only in test harnesses, at pinned
versions, and nothing ships them.

## Summary

- A differential gives one generated input to two independent
  implementations, normalizes both answers, compares them and classifies
  every difference.
- Every explained difference names a written rule and its reason; any other
  difference fails the run.
- The example finds no unexplained difference between the relay's query
  and NIP-01's filter text over 500 cases; its three known rules explain
  the 158 that differ.
- A seeded `until` fault gives 22 unexplained cases, and minimization
  reduces the first to one event and one filter field.
- The Rust differential classified all 26 of its differences, and the
  production recording matches.

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [93. Test vectors and their
  provenance](ch093-test-vectors-and-their-provenance.md)
- [23. Filters](ch023-filters.md) and [36. Match filters with
  indexes](ch036-match-filters-with-indexes.md), for `nf_match` and
  `rsd_query`.

## Sources

- [NIP-01](../nips/official/01.md): filters, `since` and `until`, `limit`
  and order, replaceable and ephemeral kinds.
- [NIP-40](../nips/official/40.md): "Relays SHOULD NOT send expired events
  to clients, even if they are stored."
- The records:
  [2026-10-30-coax-relay-r1-rust-differential.md](../../docs/verification/2026-10-30-coax-relay-r1-rust-differential.md)
  and [2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md).
- The pinned vectors and libraries:
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md), section 17.
