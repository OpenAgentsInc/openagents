# 18. Consent before sharing

> **Status: partly implemented.** A consent gate run on the CPU (profile
> cs-consent-v1): session content as labelled nodes whose labels
> propagate from their sources, grants recorded only by the person for one
> destination, purpose, set of data classes and window of ticks, sends
> decided and logged before any byte reaches the sink, revocation that
> records what already went out, minimized and redacted alternatives, and
> replay that rebuilds the grants from the log and refuses a send no grant
> covered. In Coder, the gate stands between
> [chapter 19](ch019-remember-a-fact-then-revise-it.md)'s remembered facts
> and every engine, with `coder consent` to grant and revoke and a ledger
> on disk that each process replays. It also stands between a quiet
> Worker's log and System One, which the watchdog asks about it, and
> between System One and the text that routing and the Router's batch
> decisions send. Still to come:
> [chapter 16](ch016-speak-the-protocols.md)'s outbound calls, mesh peer
> links and [Psionic chapter 247](../../psionic/book/ch247-outcomes-become-training-rows.md)'s
> export, and labels for [chapter 2](ch002-model-visible-means-logged.md)'s
> log events.

The [outbound inventory](CONTRACT.md#outbound-authorization-and-current-enforcement)
states the authorization boundary and current enforcement for inference,
fallback, embeddings, handoff, tools and export. This chapter's ledger
guards selected paths, not every byte of ordinary engine inference.

Session content is private by default. A harness sends bytes outward in
many ways: a call to a remote tool, a task handed to another agent, work
sent to a mesh peer, a training row exported for later learning. Each of
those can carry what the person typed, files from the project, facts the
agent remembered about the person, or keys. DeepSeek Harness sends
session-log suffixes to its API by default. Psionic's harness sends
nothing unless the person said yes to that data, that destination and that
purpose, and the yes is still in force.

This chapter builds that gate with these rules:

- Every piece of session content is a node with a **label**: the set of
  data classes it carries. A source node's label comes from where it came
  from; a node made from other nodes carries their labels as well as its
  own. A summary of a file is still project data.
- An outbound message is a destination, a purpose and a list of nodes. Its
  label is the union of their labels.
- Consent is a **grant**: one destination, one purpose, a set of classes, a
  window of ticks, and the person who gave it. `cs_grant` is the only way to
  make one. Consent is never inferred from an earlier send, from a grant
  for another destination or purpose, or from a message having no label.
- A message is sent only when the grants active at its tick, for its
  destination and purpose, cover every class of its label. Even a message
  with an empty label needs one active grant for its destination.
- The decision is logged before any byte reaches the sink. A refusal is
  logged too, with its reason and the classes no grant covered.
- A revocation stops later sends under the grant. Its event records how
  many messages and bytes already went out under it. They stay sent; the
  ledger never claims to recall them.
- When consent is missing, the gate offers two alternatives: the parts of
  the message that are covered, or a copy with the other parts replaced by
  `[withheld]`. Either is a new message that goes through the gate again.

The library is [harness/consent.coax](../../psionic/lib/harness/consent.coax). It is
`#authority`: it decides what may leave the harness. It loads only the
SHA-256 unit from `coax/lib/crypto`.

## Run it

Run commands from the repository root.

```shell
coax/bin/coax run psionic/examples/consent.coax
coax/bin/coax run psionic/examples/consent-refuse.coax
coax/bin/coax test psionic/lib/harness/consent.coax
coax/bin/coax test psionic/tests/harness-consent-model.coax
psionic/check.sh --only harness/consent
```

## The profile

This chapter's profile is `cs-consent-v1`:

- Five data classes, one bit each in a label: `session` (typed in the
  session), `user` (a fact about the user, such as chapter19's
  user-scope facts), `project` (project files and facts), `tool` (a
  tool's output) and `identity` (ids and keys).
- Four kinds of destination (a tool, an agent, a mesh peer, an export),
  each with an integer id, and four purposes (call, delegate, compute,
  train).
- One in-process ledger, graph and sink, all in caller-owned arrays. Every
  bound is the length of an array; a call that doesn't fit is refused as
  `capacity` and logs nothing.
- Ticks are integers the caller supplies, and they never go back: an event
  at an earlier tick than the last one is refused as `tick`.
- Digests are SHA-256 over integers written as little-endian u64 values.

## Records

<!-- source: psionic/lib/harness/consent.coax types -->
```jai
// The classes of data, one bit each in a label: session (typed in the
// session), user (a fact about the user), project (project files and
// facts), tool (a tool's output), identity (ids and keys).
CsClass :: enum u8 { session; user; project; tool; identity; }
CS_CLASSES :: 5;
CS_ALL :: 31;
// Where bytes go, and why.
CsKind :: enum u8 { tool; agent; peer; export; }
CsPurpose :: enum u8 { call; delegate; compute; train; }
CsDest :: struct { kind: CsKind; id: int; purpose: CsPurpose; }
CsOp :: enum u8 { grant; revoke; send; refuse; }
// ok, then a send's refusals (none: no grant for the destination and
// purpose; classes: grants exist but none covers a class; early, expired,
// revoked: the covering grant is not active), then refused calls and
// refused logs.
CsWhy :: enum u8 { ok; none; classes; early; expired; revoked; shape; capacity; unknown; person; twice; tick; sequence; link; digest; uncovered; replay; }
// The session graph: node i holds texts[i], its source's classes own[i]
// and its label labels[i].
CsGraph :: struct { texts: [] string; own: [] int; labels: [] int; count: int; }
CsMessage :: struct { dest: CsDest; parts: [] int; }
// A grant, folded from the event that recorded it. sends and bytes count
// what went out under it; at is the tick it was revoked.
CsGrant :: struct { event: int; dest: CsDest; classes: int; start: int; until: int; person: int; revoked: bool; at: int; sends: int; bytes: int; }
// One logged event. grant: the grant made or revoked, or the first active
// grant for a send's destination (-1 when none). classes: a grant's
// classes, or a message's label. cover, for a send or a refusal: per
// class, the grant that covers it (-1 when none or not in the label). sends and bytes: what a revoked
// grant had already carried, or a message's byte count. content: SHA-256
// of a message's bytes. digest covers every field and previous.
CsEvent :: struct { sequence: int; op: CsOp; tick: int; dest: CsDest; classes: int; start: int; until: int; person: int; grant: int; why: CsWhy; cover: [5] int; sends: int; bytes: int; content: [32] u8; previous: [32] u8; digest: [32] u8; }
CsLedger :: struct { events: [] CsEvent; grants: [] CsGrant; count: int; granted: int; }
// What reached the outside: each message's bytes end at ends[i], and
// events[i] is the send event that let it out.
CsSink :: struct { bytes: [] u8; ends: [] int; events: [] int; count: int; used: int; }
// A send's decision. missing: classes of the label no active grant covers.
CsDecision :: struct { why: CsWhy; labels: int; missing: int; first: int; cover: [5] int; }
CsCheck :: struct { why: CsWhy; index: int; }
```

The ledger holds the events and the grants folded from them. The sink
stands for everything outside the harness: a transport would read its
bytes, and nothing else may write there. The send event stores the SHA-256
of the bytes it let out, not the bytes. The consent log is not another
copy of the session.

## Labels

A node's label is fixed when the node is made: its own classes, plus the
label of every parent. A parent must already exist, so the graph has no
cycles and a label never changes after it is computed.

<!-- source: psionic/lib/harness/consent.coax label -->
```jai
// Add a node: its label is its own classes and every parent's label.
// Returns its index, or -1 when the classes or a parent are not valid or
// the graph is full.
cs_node :: (g: [] CsGraph, text: string, own: int, parents: [] int) -> int {
  n := g[0].count;
  if own < 0 || own > CS_ALL || n == g[0].texts.count { return -1; }
  label := own;
  for p: parents {
    if p < 0 || p >= n { return -1; }
    label = label | g[0].labels[p];
  }
  g[0].texts[n] = text; g[0].own[n] = own; g[0].labels[n] = label; g[0].count = n + 1;
  return n;
}
// A message's label: every part's label.
cs_label :: (g: CsGraph, parts: [] int) -> int {
  label := 0;
  for p: parts { label = label | g.labels[p]; }
  return label;
}
```

The gate is as good as the labels. Every node the harness makes from a
source must carry that source's class, and every node it writes from other
nodes must name them as parents. A node with no class and no parents is
text the harness wrote itself, such as a tool's name.

## The decision

<!-- source: psionic/lib/harness/consent.coax decide -->
```jai
cs_to :: (g: CsGrant, d: CsDest) -> bool { return g.dest.kind == d.kind && g.dest.id == d.id && g.dest.purpose == d.purpose; }
cs_active :: (g: CsGrant, d: CsDest, now: int) -> bool { return cs_to(g, d) && !g.revoked && g.start <= now && now < g.until; }
// May a message with this label go to d at tick now? Each class of the
// label needs an active grant for d that covers it, and even an empty
// label needs one active grant for d. A refusal names the strongest reason
// among d's grants that could have covered the missing classes: revoked,
// then expired, then early; classes when d has grants but none of them
// could; none when d has no grant at all.
cs_decide :: (l: CsLedger, d: CsDest, labels: int, now: int) -> CsDecision {
  r := CsDecision.{why = CsWhy.none, labels = labels, first = -1};
  for c: 0..<CS_CLASSES { r.cover[c] = -1; }
  for g, i: l.grants[0..<l.granted] {
    if !cs_active(g, d, now) { continue; }
    if r.first < 0 { r.first = i; }
    for c: 0..<CS_CLASSES { if r.cover[c] < 0 && (labels & g.classes & (1 << c)) != 0 { r.cover[c] = i; } }
  }
  for c: 0..<CS_CLASSES { if r.cover[c] < 0 { r.missing = r.missing | (labels & (1 << c)); } }
  if r.first >= 0 && r.missing == 0 { r.why = CsWhy.ok; return r; }
  // A grant for d that could have covered the missing classes is not
  // active: it was revoked, is over, or has not started.
  any := false; gone := false; over := false; soon := false;
  for g: l.grants[0..<l.granted] {
    if !cs_to(g, d) { continue; }
    any = true;
    if r.missing != 0 && (g.classes & r.missing) == 0 { continue; }
    if g.revoked { gone = true; }
    else if now >= g.until { over = true; }
    else { soon = true; }
  }
  if gone { r.why = CsWhy.revoked; }
  else if over { r.why = CsWhy.expired; }
  else if soon { r.why = CsWhy.early; }
  else if any { r.why = CsWhy.classes; }
  return r;
}
```

A grant is active for a message when its destination kind, id and purpose
all match, it is not revoked, and the tick is inside its window. Each class
of the label is covered by the first active grant that includes it; the
first active grant also opens the destination for an empty label. When some
class is missing, the reason looks at the destination's grants that could
have covered a missing class. If one was revoked, the reason is `revoked`;
if one is over, `expired`; if one has not started, `early`. If the
destination has grants but none of them could cover the missing classes,
the reason is `classes`. If it has no grant at all, `none`.

## The ledger and the gate

Every change goes through one append, which chains the event to the one
before it.

<!-- source: psionic/lib/harness/consent.coax encode -->
```jai
cs_u64 :: (b: [] u8, at: int, x: int) -> int {
  v := cast,trunc(u64) x;
  for i: 0..<8 { b[at + i] = cast(u8) (v & 255); v = v >> 8; }
  return at + 8;
}
cs_feed :: (state: [] u8, s: string) {
  buf: [64] u8; at := 0;
  while at < s.count {
    n := 0;
    while n < buf.count && at + n < s.count { buf[n] = s[at + n]; n += 1; }
    r := sha256_update_checked(state, buf[0..<n], false);
    at += n;
  }
}
// SHA-256 of a message's bytes: its parts' texts in order.
cs_content :: (g: CsGraph, parts: [] int) -> [32] u8 {
  s: [112] u8; out: [32] u8;
  r := sha256_init(s[..]);
  for p: parts { cs_feed(s[..], g.texts[p]); }
  r = sha256_final_checked(s[..], out[..], false);
  return out;
}
cs_digest :: (e: CsEvent) -> [32] u8 {
  b: [216] u8; s: [112] u8; out: [32] u8;
  for i: 0..<32 { b[i] = e.content[i]; b[32 + i] = e.previous[i]; }
  f := [19] int.[e.sequence, cast(int) e.op, e.tick, cast(int) e.dest.kind, e.dest.id, cast(int) e.dest.purpose, e.classes, e.start, e.until, e.person, e.grant, cast(int) e.why, e.cover[0], e.cover[1], e.cover[2], e.cover[3], e.cover[4], e.sends, e.bytes];
  at := 64;
  for x: f { at = cs_u64(b[..], at, x); }
  r := sha256_init(s[..]);
  r = sha256_update_checked(s[..], b[..], false);
  r = sha256_final_checked(s[..], out[..], false);
  return out;
}
cs_same :: (a: [32] u8, b: [32] u8) -> bool {
  for i: 0..<32 { if a[i] != b[i] { return false; } }
  return true;
}
```

<!-- source: psionic/lib/harness/consent.coax ledger -->
```jai
// May one more event be logged at tick now?
cs_room :: (l: CsLedger, now: int) -> CsWhy {
  if l.count == l.events.count { return CsWhy.capacity; }
  if l.count > 0 && now < l.events[l.count - 1].tick { return CsWhy.tick; }
  return CsWhy.ok;
}
// Chain and append an event.
cs_append :: (l: [] CsLedger, x: CsEvent) {
  n := l[0].count; e := x;
  e.sequence = n;
  if n > 0 { e.previous = l[0].events[n - 1].digest; }
  e.digest = cs_digest(e);
  l[0].events[n] = e; l[0].count = n + 1;
}
// The only way consent is given: the person's explicit grant to one
// destination and purpose, for some classes, at ticks start..<until.
cs_grant :: (l: [] CsLedger, d: CsDest, classes: int, start: int, until: int, person: int, now: int) -> CsWhy {
  if classes < 0 || classes > CS_ALL || d.id < 0 || person < 0 || until <= start || until <= now { return CsWhy.shape; }
  w := cs_room(l[0], now);
  if w == CsWhy.ok && l[0].granted == l[0].grants.count { w = CsWhy.capacity; }
  if w != CsWhy.ok { return w; }
  k := l[0].granted;
  cs_append(l, CsEvent.{op = CsOp.grant, tick = now, dest = d, classes = classes, start = start, until = until, person = person, grant = k});
  l[0].grants[k] = CsGrant.{event = l[0].count - 1, dest = d, classes = classes, start = start, until = until, person = person};
  l[0].granted = k + 1;
  return CsWhy.ok;
}
// Withdraw a grant. Only the person who gave it may; later sends under it
// stop. The event records what it already carried: that stays sent.
cs_revoke :: (l: [] CsLedger, grant: int, person: int, now: int) -> CsWhy {
  if grant < 0 || grant >= l[0].granted { return CsWhy.unknown; }
  g := l[0].grants[grant];
  if person != g.person { return CsWhy.person; }
  if g.revoked { return CsWhy.twice; }
  w := cs_room(l[0], now);
  if w != CsWhy.ok { return w; }
  cs_append(l, CsEvent.{op = CsOp.revoke, tick = now, dest = g.dest, classes = g.classes, start = g.start, until = g.until, person = person, grant = grant, sends = g.sends, bytes = g.bytes});
  l[0].grants[grant].revoked = true; l[0].grants[grant].at = now;
  return CsWhy.ok;
}
// Log a decided message; a sent one counts once under every grant it used.
cs_record :: (l: [] CsLedger, d: CsDest, r: CsDecision, bytes: int, content: [32] u8, now: int) {
  e := CsEvent.{op = ifx r.why == CsWhy.ok then CsOp.send else CsOp.refuse, tick = now, dest = d, classes = r.labels, grant = r.first, why = r.why, cover = r.cover, bytes = bytes, content = content};
  cs_append(l, e);
  if r.why != CsWhy.ok { return; }
  for i: 0..<l[0].granted {
    used := i == r.first;
    for c: r.cover { if c == i { used = true; } }
    if used { l[0].grants[i].sends += 1; l[0].grants[i].bytes += bytes; }
  }
}
```

`cs_revoke` takes the grant's counts into its event before it marks the
grant revoked. That is the record of what already left: a revocation can
stop the next send but can't call back the last one.

The gate checks the message, decides, logs the decision, and only then
copies bytes to the sink.

<!-- source: psionic/lib/harness/consent.coax send -->
```jai
// A call refused before any decision: no grant, no cover.
cs_fail :: (w: CsWhy) -> CsDecision {
  r := CsDecision.{why = w, first = -1};
  for c: 0..<CS_CLASSES { r.cover[c] = -1; }
  return r;
}
// The gate. Decide, log the decision, and only then copy the bytes to the
// sink. Nothing is logged for a malformed message or one that does not fit.
cs_send :: (l: [] CsLedger, g: CsGraph, s: [] CsSink, m: CsMessage, now: int) -> CsDecision {
  n := 0;
  for p: m.parts {
    if p < 0 || p >= g.count { return cs_fail(CsWhy.shape); }
    n += g.texts[p].count;
  }
  if m.parts.count == 0 || m.dest.id < 0 { return cs_fail(CsWhy.shape); }
  w := cs_room(l[0], now);
  if w != CsWhy.ok { return cs_fail(w); }
  r := cs_decide(l[0], m.dest, cs_label(g, m.parts), now);
  if r.why == CsWhy.ok && (s[0].count == s[0].ends.count || n > s[0].bytes.count - s[0].used) { return cs_fail(CsWhy.capacity); }
  cs_record(l, m.dest, r, n, cs_content(g, m.parts), now);
  if r.why != CsWhy.ok { return r; }
  at := s[0].used;
  for p: m.parts { t := g.texts[p]; for i: 0..<t.count { s[0].bytes[at + i] = t[i]; } at += t.count; }
  s[0].ends[s[0].count] = at; s[0].events[s[0].count] = l[0].count - 1;
  s[0].count += 1; s[0].used = at;
  return r;
}
```

## Alternatives when consent is missing

<!-- source: psionic/lib/harness/consent.coax offer -->
```jai
// What could go instead: the parts whose labels the active grants for the
// message's destination cover, in order, to out. None when no grant for
// the destination is active. Returns their count, or -1 when out is too
// small.
cs_minimize :: (l: CsLedger, g: CsGraph, m: CsMessage, now: int, out: [] int) -> int {
  r := cs_decide(l, m.dest, CS_ALL, now);
  if r.first < 0 { return 0; }
  k := 0;
  for p: m.parts {
    if (g.labels[p] & r.missing) != 0 { continue; }
    if k == out.count { return -1; }
    out[k] = p; k += 1;
  }
  return k;
}
// The message's bytes with every part that is not covered replaced by
// [withheld], to out. Returns the byte count, or -1 when out is too small.
// Its node, made with the kept parts as parents, has their label only.
cs_redact :: (l: CsLedger, g: CsGraph, m: CsMessage, now: int, out: [] u8) -> int {
  keep: [64] int;
  k := cs_minimize(l, g, m, now, keep[..]);
  if k < 0 { return -1; }
  at := 0; j := 0;
  for p: m.parts {
    t := CS_WITHHELD;
    if j < k && keep[j] == p { t = g.texts[p]; j += 1; }
    if t.count > out.count - at { return -1; }
    for i: 0..<t.count { out[at + i] = t[i]; }
    at += t.count;
  }
  return at;
}
```

Neither alternative sends anything. The minimized parts and the redacted
copy are new messages, and they pass through `cs_send` like any other.
The redacted copy is made into a node whose parents are only the kept
parts, so its label is exactly theirs: the withheld classes are gone from
the bytes and from the label together.

## Replay

<!-- source: psionic/lib/harness/consent.coax replay -->
```jai
// Check the chain alone: sequence, link and digest of every event.
cs_verify :: (l: CsLedger) -> CsCheck {
  previous: [32] u8;
  for e, i: l.events[0..<l.count] {
    if e.sequence != i { return CsCheck.{why = CsWhy.sequence, index = i}; }
    if !cs_same(e.previous, previous) { return CsCheck.{why = CsWhy.link, index = i}; }
    if !cs_same(e.digest, cs_digest(e)) { return CsCheck.{why = CsWhy.digest, index = i}; }
    previous = e.digest;
  }
  return CsCheck.{why = CsWhy.ok, index = l.count};
}
// Rebuild a ledger from the log src alone, in the empty ledger to: verify the
// chain, then redo every event and require the same event back. A send the
// grants of its moment do not cover is uncovered, even in a chain that
// verifies.
cs_replay :: (src: CsLedger, to: [] CsLedger) -> CsCheck {
  v := cs_verify(src);
  if v.why != CsWhy.ok { return v; }
  for e, i: src.events[0..<src.count] {
    w := CsWhy.ok;
    if e.op == CsOp.grant { w = cs_grant(to, e.dest, e.classes, e.start, e.until, e.person, e.tick); }
    else if e.op == CsOp.revoke { w = cs_revoke(to, e.grant, e.person, e.tick); }
    else {
      w = cs_room(to[0], e.tick);
      r := cs_decide(to[0], e.dest, e.classes, e.tick);
      if w == CsWhy.ok && e.op == CsOp.send && r.why != CsWhy.ok { return CsCheck.{why = CsWhy.uncovered, index = i}; }
      if w == CsWhy.ok { cs_record(to, e.dest, r, e.bytes, e.content, e.tick); }
    }
    if w != CsWhy.ok || !cs_same(to[0].events[i].digest, e.digest) { return CsCheck.{why = CsWhy.replay, index = i}; }
  }
  return v;
}
```

Replay does not trust the log's own account. It runs every grant and
revocation again, decides every send and refusal again against the grants
of that moment, and requires the event it rebuilds to have the recorded
digest. Someone who can rewrite the whole log and recompute every digest
gets a chain that verifies, but not a send that no grant covered.

## A trace by hand

Work the example through before you run it. The session has six nodes.

<!-- source: psionic/tests/harness-consent-checks.coax fixture -->
```jai
// The hand trace's session: what the person typed, a project file, a fact
// about the user from chapter 214's memory, the owner's key, a summary
// written from the first two, and a remote linter's reply to the summary.
CT_TEXTS :: [6] string.["Fix the flaky test in parser.rs.", "fn parse(s: &str) -> Ast", "editor = helix", "owner npub1alice", "Summary: parse() fails on empty input.", "lint: 2 warnings in parse()"];
CT_SESSION :: 1; CT_USER :: 2; CT_PROJECT :: 4; CT_TOOL :: 8; CT_IDENTITY :: 16;
CT_LINT :: CsDest.{kind = CsKind.tool, id = 1, purpose = CsPurpose.call};
CT_LINT_TRAIN :: CsDest.{kind = CsKind.tool, id = 1, purpose = CsPurpose.train};
CT_AGENT :: CsDest.{kind = CsKind.agent, id = 2, purpose = CsPurpose.delegate};
CT_PEER :: CsDest.{kind = CsKind.peer, id = 3, purpose = CsPurpose.compute};
CT_EXPORT :: CsDest.{kind = CsKind.export, id = 4, purpose = CsPurpose.train};
```

| Node | Text | Own | Parents | Label |
| --- | --- | --- | --- | --- |
| 0 | `Fix the flaky test in parser.rs.` (32 bytes) | session | — | session |
| 1 | `fn parse(s: &str) -> Ast` (24 bytes) | project | — | project |
| 2 | `editor = helix` (14 bytes) | user | — | user |
| 3 | `owner npub1alice` (16 bytes) | identity | — | identity |
| 4 | `Summary: parse() fails on empty input.` (38 bytes) | — | 0, 1 | session+project |
| 5 | `lint: 2 warnings in parse()` (27 bytes) | tool | 4 | session+project+tool |

Node 4's label is node 0's and node 1's: 1 | 4 = 5. Node 5 adds `tool`
(8) to node 4's 5, which is 13. Now the steps, at the tick shown.

1. t1: send node 4 to the linter (tool 1, call). The linter has no grant:
   `none`, missing session+project.
2. t2: person 1 grants session+project to the linter for calls, ticks
   2..<20. It is grant 0.
3. t3: send node 4 to the linter. Grant 0 is active and covers both
   classes: `ok`. The sink holds 38 bytes.
4. t4: send nodes 4 and 2. The label is 5 | 2 = 7, session+user+project.
   No active grant covers `user`, and the linter's one grant doesn't
   include it: `classes`, missing user. Minimizing keeps node 4 alone.
   Redacting gives `Summary: parse() fails on empty input.[withheld]`
   (38 + 10 = 48 bytes), node 6, whose only parent is node 4, so its label
   is 5. Sending node 6 is `ok`.
5. t5: send node 5 to agent 2 to delegate: `none`. Send node 4 to the
   linter for training: `none` too. Grant 0 is for calls.
6. t6: grant session+project+tool to agent 2, ticks 6..<9 (grant 1).
   Sending node 5 is `ok`, 27 bytes.
7. t7: person 2 revokes grant 0: `person`, only the person who gave a
   grant may withdraw it. Person 1 revokes it: `ok`. Grant 0 carried 2
   messages, 38 + 48 = 86 bytes, and the revocation event says so.
8. t8: send node 4 to the linter: `revoked`. Revoking again: `twice`.
9. t9: send node 5 to agent 2. Grant 1's window ended at tick 9:
   `expired`. Grant session to peer 3 for compute, ticks 20..<30 (grant
   2).
10. t10: send node 0 to peer 3: `early`. Grant project to the export for
    training, ticks 10..<12 (grant 3). Sending node 1 is `ok`, 24 bytes.
11. t11: send node 3 (identity) to the export: `classes`, missing
    identity.
12. t12: send node 1 to the export again: `expired`.

The ledger holds 18 events: 4 grants, 1 revocation (the two refused
revocations are not logged, because nothing changed), 4 sends and 9
refusals. The sink holds 4 messages, 38 + 48 + 27 + 24 = 137 bytes. A
replay into an empty ledger rebuilds the same four grants.

## The program

[consent.coax](../../psionic/examples/consent.coax) runs those steps and prints each
decision, the grants, and what reached the sink.

```text
session nodes:
  0 "Fix the flaky test in parser.rs." own session label session
  1 "fn parse(s: &str) -> Ast" own project label project
  2 "editor = helix" own user label user
  3 "owner npub1alice" own identity label identity
  4 "Summary: parse() fails on empty input." own - label session+project
  5 "lint: 2 warnings in parse()" own tool label session+project+tool
t1: send node 4 [session+project] to linter (tool 1, call): none (missing session+project)
t2: person 1 grants session+project to linter (tool 1, call), ticks 2..<20: ok
t3: send node 4 [session+project] to linter (tool 1, call): ok
t4: send nodes 4,2 [session+user+project] to linter (tool 1, call): classes (missing user)
    minimized: 1 part(s), node 4
    redacted: node 6 "Summary: parse() fails on empty input.[withheld]" label session+project
t4: send node 6 [session+project] to linter (tool 1, call): ok
t5: send node 5 [session+project+tool] to agent 2 (delegate): none (missing session+project+tool)
t5: send node 4 [session+project] to linter (tool 1, train): none (missing session+project)
t6: person 1 grants session+project+tool to agent 2 (delegate), ticks 6..<9: ok
t6: send node 5 [session+project+tool] to agent 2 (delegate): ok
t7: person 2 revokes grant 0: person
t7: person 1 revokes grant 0: ok
    already sent under grant 0, not recalled: 2 messages, 86 bytes
t8: send node 4 [session+project] to linter (tool 1, call): revoked (missing session+project)
t8: person 1 revokes grant 0 again: twice
t9: send node 5 [session+project+tool] to agent 2 (delegate): expired (missing session+project+tool)
t9: person 1 grants session to peer 3 (compute), ticks 20..<30: ok
t10: send node 0 [session] to peer 3 (compute): early (missing session)
t10: person 1 grants project to export 4 (train), ticks 10..<12: ok
t10: send node 1 [project] to export 4 (train): ok
t11: send node 3 [identity] to export 4 (train): classes (missing identity)
t12: send node 1 [project] to export 4 (train): expired (missing project)
grants:
  0 session+project to linter (tool 1, call), ticks 2..<20, 2 sends, 86 bytes, revoked at 7
  1 session+project+tool to agent 2 (delegate), ticks 6..<9, 1 sends, 27 bytes
  2 session to peer 3 (compute), ticks 20..<30, 0 sends, 0 bytes
  3 project to export 4 (train), ticks 10..<12, 1 sends, 24 bytes
sink: 4 messages, 137 bytes
  event 2 to linter (tool 1, call): "Summary: parse() fails on empty input."
  event 4 to linter (tool 1, call): "Summary: parse() fails on empty input.[withheld]"
  event 8 to agent 2 (delegate): "lint: 2 warnings in parse()"
  event 15 to export 4 (train): "fn parse(s: &str) -> Ast"
verify: ok after 18 events; head 159dffe01868dd3e
replay: ok, 4 grants, same grants: true
```

Every line matches the trace. The book's example check runs the program
in debug and release builds and compares both outputs with
[consent.out](../../psionic/fixtures/consent/consent.out), exit status 0.

## Refusals

[consent-refuse.coax](../../psionic/examples/consent-refuse.coax) makes calls the gate
must refuse, then edits a finished log.

<!-- source: psionic/examples/consent-refuse.coax refuse -->
```jai
main :: () {
  st: CtStore; l := [1] CsLedger.[ct_open(st.events[..], st.grants[..])];
  g := [1] CsGraph.[ct_graph(st.texts[..], st.own[..], st.labels[..])]; s := [1] CsSink.[ct_sink(st.bytes[..], st.ends[..], st.sent[..])];
  ct_session(g[..]); nb: [1] int; one := [1] int.[4]; far := [1] int.[9];
  print("node with an unknown class bit (32): %\n", cs_node(g[..], "x", 32, nb[0..<0]));
  print("node whose parent does not exist yet: %\n", cs_node(g[..], "x", 0, far[..]));
  print("grant with an empty window: %\n", name(cs_grant(l[..], CT_LINT, CT_SESSION, 5, 5, 1, 2)));
  print("grant already over: %\n", name(cs_grant(l[..], CT_LINT, CT_SESSION, 2, 4, 1, 4)));
  print("grant from no person: %\n", name(cs_grant(l[..], CT_LINT, CT_SESSION, 2, 20, -1, 2)));
  print("revoke a grant never given: %\n", name(cs_revoke(l[..], 0, 1, 2)));
  print("send of no parts: %\n", name(cs_send(l[..], g[0], s[..], ct_message(CT_LINT, nb[0..<0]), 2).why));
  print("send of a node that does not exist: %\n", name(cs_send(l[..], g[0], s[..], ct_message(CT_LINT, far[..]), 2).why));
  print("events logged: %\n", l[0].count);
  print("grant at tick 6: %, then a send at tick 5: %\n", name(cs_grant(l[..], CT_LINT, CT_SESSION | CT_PROJECT, 6, 20, 1, 6)), name(cs_send(l[..], g[0], s[..], ct_message(CT_LINT, one[..]), 5).why));
  small := [1] CsSink.[ct_sink(st.bytes[0..<37], st.ends[..], st.sent[..])];
  print("a covered 38-byte send to a 37-byte sink: %, events logged: %\n", name(cs_send(l[..], g[0], small[..], ct_message(CT_LINT, one[..]), 6).why), l[0].count);
  // The hand trace in a new ledger, then its log edited after the fact.
  st = CtStore.{}; l[0] = ct_open(st.events[..], st.grants[..]); g[0] = ct_graph(st.texts[..], st.own[..], st.labels[..]); s[0] = ct_sink(st.bytes[..], st.ends[..], st.sent[..]);
  ct_session(g[..]); out: [24] CsWhy; n := ct_trace(l[..], g[..], s[..], st.redacted[..], out[..]);
  e2 := st.events; f := CsLedger.{events = e2[..], grants = st.grants[..], count = l[0].count, granted = l[0].granted};
  st2: CtStore; u := [1] CsLedger.[ct_open(st2.events[..], st2.grants[..])];
  e2[4].classes = CT_SESSION; print("the redacted send's label edited: verify %\n", check(cs_verify(f)));
  e2 = st.events; e2[1].classes = CT_SESSION; ct_rechain(f);
  r := cs_replay(f, u[..]);
  print("the linter's grant narrowed to session, chain rewritten: verify %, replay % at event %\n", check(cs_verify(f)), check(r), r.index);
  e2 = st.events; e2[0].op = CsOp.send; ct_rechain(f); u[0] = ct_open(st2.events[..], st2.grants[..]);
  r = cs_replay(f, u[..]);
  print("the first refusal written as a send: replay % at event %\n", check(r), r.index);
  e2 = st.events; e2[9].sends = 1; ct_rechain(f); u[0] = ct_open(st2.events[..], st2.grants[..]);
  r = cs_replay(f, u[..]);
  print("the revocation claims one message already sent, not two: replay % at event %\n", check(r), r.index);
}
```

```text
node with an unknown class bit (32): -1
node whose parent does not exist yet: -1
grant with an empty window: shape
grant already over: shape
grant from no person: shape
revoke a grant never given: unknown
send of no parts: shape
send of a node that does not exist: shape
events logged: 0
grant at tick 6: ok, then a send at tick 5: tick
a covered 38-byte send to a 37-byte sink: capacity, events logged: 1
the redacted send's label edited: verify digest
the linter's grant narrowed to session, chain rewritten: verify ok, replay uncovered at event 2
the first refusal written as a send: replay uncovered at event 0
the revocation claims one message already sent, not two: replay replay at event 9
```

None of the malformed calls logs an event. The send at tick 5 after an
event at tick 6 is refused as `tick`, and the covered send that doesn't fit
the sink is refused as `capacity` before it is logged, so the count stays
at 1, the grant. The last three lines are rewritten logs whose chain
verifies. Narrowing the linter's grant to `session` leaves the send at
event 2, which carried project data, uncovered. Turning the first refusal
into a send claims bytes went out with no grant at all. Lowering the count
on the revocation hides a message that already left; replay counts the
sends again and finds two.
The output is
[consent-refuse.out](../../psionic/fixtures/consent/consent-refuse.out), exit
status 0, in both builds.

## Checks

The checks are in [harness-consent-checks.coax](../../psionic/tests/harness-consent-checks.coax),
apart from the gate:

- **The hand trace**, step by step: every decision, every event's
  operation, the revocation's counts, each send's covering grant per
  class, the sink's bytes and the events that let them out, and a replay
  that gives the same grants and digests.
- **The decision table**: an empty label, kind, id and purpose each
  mattering, every reason at its tick boundaries, two grants covering one
  message together, and the order revoked, expired, early, classes, none.
- **Every refused call**, with nothing logged: bad classes, ids, persons
  and windows, unknown and doubled revocations, the wrong person, ticks
  going back, a full log, a full grant table and a full sink.
- **Accounting**: a send counts once under every grant it used, and each
  class goes to the first grant that covers it.
- **The encoding** against SHA-256 values computed with Python's
  `hashlib` and `struct`: the first event of the trace, an event with every
  field distinct, and message contents split across parts and across the
  feeding buffer.
- **Forged logs**: an edited field, a wrong sequence, swapped events, a
  broken link, and rechained lies (a narrowed grant, a refusal turned into
  a send and back, a revocation by another person, lowered counts, a
  rewritten cover, a tick moved back), each refused at its event.
- **A taint model.** Label propagation is compared with a model that
  walks every node's ancestors and takes the classes of every source it
  reaches, on random graphs of up to 24 nodes with up to 3 parents each,
  and on random messages over them.

<!-- source: psionic/tests/harness-consent-checks.coax taint -->
```jai
// The taint model: a node's label is the classes of every source it can
// reach through its parents, found by walking the graph, not by carrying
// labels forward.
ct_reach :: (own: [] int, starts: [] int, ends: [] int, parents: [] int, node: int) -> int {
  seen: [32] bool; stack: [32] int; top := 1; stack[0] = node; seen[node] = true; label := 0;
  while top > 0 {
    top -= 1; v := stack[top];
    for c: 0..<CS_CLASSES { if (own[v] >> c) % 2 == 1 { label = label | (1 << c); } }
    for k: starts[v]..<ends[v] { p := parents[k]; if !seen[p] { seen[p] = true; stack[top] = p; top += 1; } }
  }
  return label;
}
```

- **A consent model.** On random sessions of grants, revocations and
  sends to three destinations that share a kind or an id, a model that
  keeps only the list of grants decides each send class by class, and must
  agree with the gate on every decision and refusal reason. Then every
  message in the sink is checked against the model's grants: each class
  of its label had a grant that included it and was in its window, and no
  revocation of that grant came before the send in the log. The log must
  replay to the same grants, and whatever minimizing keeps must be
  sendable.

The checks file runs 12 random graphs and 8 random sessions, which reach
every send decision, `shape`, `unknown`, `person` and `twice`. [harness-consent-model.coax](../../psionic/tests/harness-consent-model.coax)
runs 400 graphs and 120 sessions; `psionic/check.sh` runs it beside the
unit. `coax mutate` on `consent.coax` kills all 401 mutants; none
survive.

## In Coder

The first outbound path the gate guards is the one chapter 19 opened:
the facts you remember go in front of a session's first message, and so
to the engine's provider. In Coder, each fact is a node labelled with its
scope's class, `user` or `project`. The message is the list of facts, and
its destination is the engine with the purpose `delegate`.

```shell
coder/tui/coder.sh consent                                   # the grants, with what each carried
coder/tui/coder.sh consent grant claude --classes user,project
coder/tui/coder.sh consent grant all --classes user --days 30
coder/tui/coder.sh consent revoke 0
```

- **No grant, no facts.** With no grant for the engine, `cs_send` refuses
  the message and logs the refusal. The session starts without the facts.
  It logs `{"kind":"memory","text":"","withheld":2}` and shows one line
  with the command that sends them.
- **Some classes.** When the grants cover only some classes,
  `cs_minimize` picks the covered facts. Those go out as a second, logged
  send, and the rest are counted as withheld.
- **Revoke.** `coder consent revoke N` stops grant N. Its revoke event
  records how many sends and bytes the grant already carried.
- **The ledger.** `~/.openagents/coder/consent/events.jsonl` holds one
  event per line with every field and its digests. Every `coder consent`
  and every gated session replays the whole ledger with `cs_replay` before
  it decides. A ledger that does not replay, including one with a send no
  grant covered, sends nothing. Ticks are wall-clock seconds and never go
  back. The ledger holds 2,048 events; when it is full, sends are refused
  as `capacity`.
- **Echo is not gated.** The echo engine is offline: nothing leaves the
  machine.

In a live run on 2026-11-02, Microcoder was asked which editor to use.
With no grant, it got no facts and suggested VS Code. With
`consent grant micro --classes user`, it got the user fact (the
project-scoped `deploy` fact stayed local) and answered "You should open
the file in Helix." After `consent revoke 0`, the next session got none.
The grant listed `1 sends, 46 bytes`.

The smoke checks the same sequence on recorded Microcoder responses:
refused with no grant, refused when only a `user` grant exists for a
`project` fact, sent under a `project` grant, refused after the revoke,
and the ledger's counts. The code is in [coder/tui/gate.coax](../tui/gate.coax).

### Session content to System One

The second path is the Router's watchdog (chapter 20). When a Worker's log
stops growing, the watchdog asks System One, on Baseten, whether the Worker
is still making progress, and sends it the task and the last 12,000 bytes
of the Worker's log, tool output included. That is session content, so it
is one node labelled `session`, and its destination is System One as a
remote tool with the purpose `call`:

```shell
coder/tui/coder.sh consent grant system-one --classes session
```

With no grant, `cs_send` refuses and logs it, nothing is sent, and the
watchdog reports the Worker as stuck with the command above. The gate runs
only when a send could happen: with no System One key, the Worker is
reported stuck for that reason and the ledger is untouched.

In a live run on 2026-11-02, a quiet stand-in Worker with no grant was
reported as "no consent to send its log to System One". After the grant,
System One judged two such Workers as still making progress, and the
grant listed `2 sends, 792 bytes`. The smoke checks the refusal offline,
with a key that is never used.

The same grant covers System One's other two calls that carry session
content:

- **Routing.** `--engine auto` and `coder route` send the task's text so
  System One can pick an engine (chapter 13). With no grant, routing has no
  decision: the default engine runs, and the route line says "no consent to
  send the task to System One" with the grant command.
- **The Router's batch.** The Router sends the text of each candidate event
  so System One can decide whether it asks for work (chapter 17). With no
  grant, each candidate is dropped with "no consent to send the events".

`coder decide` is not gated. It's an explicit request to send the question
and its state to System One.

## What this does not do yet

- **The gate guards two destinations.** Remembered facts go to engines,
  and session content (the watchdog's log excerpts, routing's task text
  and the Router's event text) goes to System One. Chapter 16's `tools/call`
  requests, mesh peer links and Psionic chapter 247's training-row export
  still need to build a `CsMessage` and go through `cs_send`.
- **Labels come only from facts.** Coder labels each chapter 19 fact by
  its scope. The text you type in a session and tool output (`session`
  and `tool`) are not yet nodes, so the engine receives them without a
  decision. They are the conversation itself.

## Dependencies

- [Chapter2](ch002-model-visible-means-logged.md): the hash-chained log
  this ledger is shaped like.
- [Chapter5](ch005-a-sandbox-and-approvals.md): approvals that are
  explicit, bound to what they approve and limited in time.
- [Chapter16](ch016-speak-the-protocols.md): the protocols that carry
  content to other services.
- [Chapter19](ch019-remember-a-fact-then-revise-it.md): facts with a
  scope and an owner, the sources of `user` and `project` labels.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- FIPS 180-4, Secure Hash Standard (SHA-256).
