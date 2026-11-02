# 19. Remember a fact, then revise it

> **Status: partly implemented.** A memory store run on the CPU (profile
> mm-memory-v1): facts as versioned records with a provenance, a scope and
> a validity window, written only through hash-chained events; revisions
> that supersede with a reason; conflicts flagged until resolved; facts
> whose cited passage changed refused as stale; forgetting that removes
> the text from the log and the table, with a verifier that proves it; and
> replay that rebuilds the store from the log. Coder keeps the log on disk
> (`coder remember`, `revise`, `forget`, `recall`), verifies it in every
> later process, and ranks facts for a query by cosine similarity of Vertex
> embeddings. A session's first message carries the facts a consent
> grant covers, and agents remember and recall through MCP tools. Still to
> come: checking an event-sourced fact's digest against
> [chapter 2](ch002-model-visible-means-logged.md)'s session log.

An agent that works across sessions needs to remember facts: the command
that builds the project, the editor the user prefers, a deadline. Facts
change, and some must be forgotten when the user asks. A memory that
overwrites a fact in place loses the old version and the reason it
changed; a memory that never checks its sources keeps serving a fact after
the document it came from was edited.

This chapter builds a small memory store with these rules:

- Every fact names where it came from: the digest of the session event it
  was said in, or a [Psionic chapter 57](../../psionic/book/ch057-answer-from-current-sources.md)
  citation (a passage identity, a byte span and the passage's revision
  digest) whose quoted bytes are the fact's text.
- Every fact has a scope (a session, a user or a project), the id of that
  session, user or project, and a validity window of ticks.
- The store changes only through logged events: remember, revise and
  forget. A revision creates a new version, names the versions it
  supersedes and carries a reason. Nothing is overwritten in place.
- Two different texts remembered for one key are a conflict. Neither is
  served until a revision settles it.
- A read serves only current versions inside their window, in a scope the
  reader may see, whose citation still checks current against the
  passages as they are now.
- A forget overwrites the text of every version of the key in the log and
  in the table. The log still verifies, and the verifier refuses any log in
  which a forgotten text is still readable.

The library is [harness/memory.coax](../../psionic/lib/harness/memory.coax). It is
`#authority`: it decides which facts a model may be shown. It loads only
Psionic chapter 57's [answer.coax](../../psionic/lib/book-search/answer.coax), for citations
and SHA-256.

## Run it

Run commands from the repository root.

```shell
coax/bin/coax run psionic/examples/memory.coax
coax/bin/coax run psionic/examples/memory-refuse.coax
coax/bin/coax test psionic/lib/harness/memory.coax
coax/bin/coax test psionic/tests/harness-memory-model.coax
psionic/check.sh --only harness/memory
```

## The profile

This chapter's profile is `mm-memory-v1`:

- One in-process log of caller-owned events and bytes, and one table of
  caller-owned fact versions. Every bound is the length of the caller's
  arrays; a write that doesn't fit is refused as `capacity`.
- Digests are SHA-256 over integers written as little-endian u64 values.
- Ticks are integers the caller supplies. The store has no clock.
- Keys and texts are bytes compared exactly.

## Records and events

A write names a key, a scope and its owner id, the text, the source and
the window. A logged event holds the same fields; its bytes (the key, the
text and, for a citation, the passage identity) follow the earlier
events' bytes in the log. A fact version is folded from the event that
wrote it.

<!-- source: psionic/lib/harness/memory.coax types -->
```jai
MmScope :: enum u8 { session; user; project; }
MmOp :: enum u8 { remember; revise; forget; }
MmReason :: enum u8 { none; correction; update; resolve; }
MmSource :: enum u8 { event; cite; }
MmState :: enum u8 { current; conflict; superseded; forgotten; }
MmStatus :: enum u8 { ok; capacity; shape; reason; source; same; missing; sequence; link; content; digest; redacted; exposed; }
MmServe :: enum u8 { served; missing; forgotten; conflict; early; expired; stale; }
// A write as the caller asks for it. origin: for an event source, the
// digest of the event the fact was said in; a cite source carries cite, and
// text must be its quoted bytes. The fact is valid at ticks start..<until.
MmWrite :: struct { key: string; scope: MmScope; owner: int; text: string; source: MmSource; origin: [32] u8; cite: AnCite; start: int; until: int; }
// One logged event. Its bytes follow the earlier events' bytes in the log:
// keys bytes of key, texts bytes of text, ids bytes of the cite's id. from
// and to are the cite's span. content covers those bytes; digest covers
// every field but redacted, and the previous event's digest. A key is
// never empty.
MmEvent :: struct { sequence: int; op: MmOp; scope: MmScope; owner: int; reason: MmReason; source: MmSource; origin: [32] u8; from: int; to: int; start: int; until: int; keys: int; texts: int; ids: int; redacted: bool; content: [32] u8; previous: [32] u8; digest: [32] u8; }
MmLog :: struct { events: [] MmEvent; bytes: [] u8; count: int; used: int; }
// One version of a fact, folded from the event that wrote it. by: the event
// that superseded or forgot it, -1 while it is live. reason: why this
// version was written (none for a first remember).
MmFact :: struct { event: int; key: string; scope: MmScope; owner: int; version: int; text: string; source: MmSource; origin: [32] u8; cite: AnCite; start: int; until: int; state: MmState; by: int; reason: MmReason; }
MmTable :: struct { facts: [] MmFact; count: int; }
// The current passages, as chapter 57's an_check takes them.
MmPassages :: struct { ids: [] string; heads: [] string; texts: [] string; }
// Who is reading: the session, user and project ids it may see.
MmContext :: struct { session: int; user: int; project: int; }
MmRead :: struct { serve: MmServe; fact: int; check: AnCheck; }
MmCheck :: struct { status: MmStatus; index: int; }
```

The event digest covers every field except `redacted`, the content
digest, and the previous event's digest, so the events form a chain. The
content digest covers the three lengths and the bytes. Forgetting
overwrites bytes and sets `redacted` but leaves both digests alone, so the
chain still verifies after a forget; the same rule as chapter2's
redaction.

<!-- source: psionic/lib/harness/memory.coax encode -->
```jai
mm_u64 :: (b: [] u8, at: int, x: int) -> int {
  v := cast,trunc(u64) x;
  for i: 0..<8 { b[at + i] = cast(u8) (v & 255); v = v >> 8; }
  return at + 8;
}
mm_sha :: (b: [] u8) -> [32] u8 {
  s: [112] u8; out: [32] u8;
  r := sha256_init(s[..]);
  an_feed(s[..], cast(string) b);
  r = sha256_final_checked(s[..], out[..], false);
  return out;
}
// The content digest covers the three lengths and the bytes.
mm_content :: (e: MmEvent, bytes: [] u8) -> [32] u8 {
  b: [24] u8; s: [112] u8; out: [32] u8;
  at := mm_u64(b[..], mm_u64(b[..], mm_u64(b[..], 0, e.keys), e.texts), e.ids);
  r := sha256_init(s[..]);
  r = sha256_update_checked(s[..], b[0..<at], false);
  an_feed(s[..], cast(string) bytes);
  r = sha256_final_checked(s[..], out[..], false);
  return out;
}
// The event digest covers every field except redacted.
mm_digest :: (e: MmEvent) -> [32] u8 {
  b: [256] u8;
  at := mm_u64(b[..], 0, e.sequence);
  for i: 0..<32 { b[at + i] = e.previous[i]; b[at + 32 + i] = e.origin[i]; b[at + 64 + i] = e.content[i]; }
  at += 96;
  f := [11] int.[cast(int) e.op, cast(int) e.scope, e.owner, cast(int) e.reason, cast(int) e.source, e.from, e.to, e.start, e.until, e.keys, e.texts];
  for x: f { at = mm_u64(b[..], at, x); }
  return mm_sha(b[0..<mm_u64(b[..], at, e.ids)]);
}
```

You can check the layout outside Coax. An event with every field zero is
200 zero bytes (8 for the sequence, 96 for three digests, 96 for twelve
integers), and Python's `hashlib.sha256(bytes(200))` gives
`6d9c54de...c3a33a`, the value the check expects. The content digest of a
one-byte key `k` and a two-byte text `vv` is
`sha256(struct.pack('<3Q', 1, 2, 0) + b'kvv')`, `21ed4f33...f4feec`.

## The rules

Each write is checked against the table before its event is appended.
In order:

| Check | Refused as |
| --- | --- |
| The key is empty, the owner or start is negative, the window is empty, or a forget has text or a write has none | `shape` |
| A cited fact's citation is not current against the passages now | `source` |
| A revise has no reason, or a remember or forget has one | `reason` |
| A revise or forget names a key with no live version | `missing` |
| A remember repeats a live text, or a revise repeats the current text | `same` |
| The table, the event array or the byte store is full | `capacity` |

A version is **live** while it is current or in conflict. Folding an
event into the table:

- **remember** with no live version: a new current version. With live
  versions: every live version and the new one become `conflict`.
- **revise**: every live version becomes `superseded`, with `by` set to
  the revision's event; the new version is current and keeps the reason.
  A revise in a conflict is how the conflict is resolved: newer explicit
  correction wins.
- **forget**: every version of the key becomes `forgotten` and its text is
  dropped; live ones get `by` set to the forget's event.

Version numbers count every version the key has had, so a key remembered
again after a forget starts after its old versions, never at 1 again.

<!-- source: psionic/lib/harness/memory.coax fold -->
```jai
mm_slot :: (f: MmFact, key: string, scope: MmScope, owner: int) -> bool { return f.key == key && f.scope == scope && f.owner == owner; }
mm_live :: (f: MmFact) -> bool { return f.state == MmState.current || f.state == MmState.conflict; }
// May this event apply to the table? A revise needs a live version and a
// reason, a forget needs a live version, and a remember or revise that
// would only repeat a live text is refused as the same.
mm_admit :: (t: MmTable, op: MmOp, reason: MmReason, key: string, scope: MmScope, owner: int, text: string) -> MmStatus {
  if (op == MmOp.revise) != (reason != MmReason.none) { return MmStatus.reason; }
  live := false; same := false;
  for f: t.facts[0..<t.count] {
    if !mm_slot(f, key, scope, owner) || !mm_live(f) { continue; }
    live = true;
    if f.text == text && (op == MmOp.remember || f.state == MmState.current) { same = true; }
  }
  if !live && op != MmOp.remember { return MmStatus.missing; }
  if same && op != MmOp.forget { return MmStatus.same; }
  if op != MmOp.forget && t.count == t.facts.count { return MmStatus.capacity; }
  return MmStatus.ok;
}
// Fold event index of the log into the table. A remember next to a live
// version makes both conflict; a revise supersedes every live version; a
// forget marks every version of the key forgotten and drops its text.
mm_apply :: (t: [] MmTable, l: MmLog, index: int) {
  e := l.events[index]; at := mm_at(l, index);
  key := cast(string) l.bytes[at..<at + e.keys];
  text := cast(string) l.bytes[at + e.keys..<at + e.keys + e.texts];
  g := t[0]; version := 1; live := false;
  for f, i: g.facts[0..<g.count] {
    if !mm_slot(f, key, e.scope, e.owner) { continue; }
    version = f.version + 1;
    if e.op == MmOp.forget {
      if mm_live(f) { g.facts[i].by = index; }
      g.facts[i].state = MmState.forgotten; g.facts[i].text = ""; continue;
    }
    if !mm_live(f) { continue; }
    live = true;
    if e.op == MmOp.revise { g.facts[i].state = MmState.superseded; g.facts[i].by = index; }
    else { g.facts[i].state = MmState.conflict; }
  }
  if e.op == MmOp.forget { return; }
  c := AnCite.{id = cast(string) l.bytes[at + e.keys + e.texts..<at + e.keys + e.texts + e.ids], from = e.from, to = e.to, digest = e.origin};
  state := ifx live && e.op == MmOp.remember then MmState.conflict else MmState.current;
  g.facts[g.count] = MmFact.{event = index, key = key, scope = e.scope, owner = e.owner, version = version, text = text, source = e.source, origin = e.origin, cite = c, start = e.start, until = e.until, state = state, by = -1, reason = e.reason};
  t[0].count = g.count + 1;
}
mm_at :: (l: MmLog, index: int) -> int { at := 0; for e: l.events[0..<index] { at += e.keys + e.texts + e.ids; } return at; }
```

`mm_write` is the only function that appends to the log. It checks,
appends, folds, and for a forget overwrites the text bytes of every
earlier write of the key.

<!-- source: psionic/lib/harness/memory.coax write -->
```jai
// The only way to change memory: check the write, append its event, fold it.
mm_write :: (l: [] MmLog, t: [] MmTable, op: MmOp, reason: MmReason, w: MmWrite, p: MmPassages) -> MmStatus {
  g := l[0];
  if w.key.count == 0 || w.owner < 0 || w.start < 0 || w.until <= w.start || (op == MmOp.forget) != (w.text.count == 0) { return MmStatus.shape; }
  if op != MmOp.forget && w.source == MmSource.cite && an_check(w.cite, w.text, p.ids, p.heads, p.texts) != AnCheck.current { return MmStatus.source; }
  id := ifx w.source == MmSource.cite then w.cite.id else "";
  s := mm_admit(t[0], op, reason, w.key, w.scope, w.owner, w.text);
  if s != MmStatus.ok { return s; }
  n := w.key.count + w.text.count + id.count;
  if g.count == g.events.count || n > g.bytes.count - g.used { return MmStatus.capacity; }
  end := an_put(g.bytes, an_put(g.bytes, an_put(g.bytes, g.used, w.key), w.text), id);
  e := MmEvent.{sequence = g.count, op = op, scope = w.scope, owner = w.owner, reason = reason, source = w.source, origin = w.origin, start = w.start, until = w.until, keys = w.key.count, texts = w.text.count, ids = id.count};
  if w.source == MmSource.cite { e.origin = w.cite.digest; e.from = w.cite.from; e.to = w.cite.to; }
  if g.count > 0 { e.previous = g.events[g.count - 1].digest; }
  e.content = mm_content(e, g.bytes[g.used..<g.used + n]);
  e.digest = mm_digest(e);
  g.events[g.count] = e; l[0].count = g.count + 1; l[0].used = g.used + n;
  mm_apply(t, l[0], g.count);
  if op == MmOp.forget { mm_redact(l[0], w.key, w.scope, w.owner); }
  return MmStatus.ok;
}
mm_remember :: (l: [] MmLog, t: [] MmTable, w: MmWrite, p: MmPassages) -> MmStatus { return mm_write(l, t, MmOp.remember, MmReason.none, w, p); }
mm_revise :: (l: [] MmLog, t: [] MmTable, w: MmWrite, reason: MmReason, p: MmPassages) -> MmStatus { return mm_write(l, t, MmOp.revise, reason, w, p); }
// A forget names the key, its scope and owner, and nothing else.
mm_forget :: (l: [] MmLog, t: [] MmTable, key: string, scope: MmScope, owner: int, p: MmPassages) -> MmStatus {
  return mm_write(l, t, MmOp.forget, MmReason.none, MmWrite.{key = key, scope = scope, owner = owner, until = 1}, p);
}
// Overwrite the text bytes of every write of the key and mark it redacted.
mm_redact :: (g: MmLog, key: string, scope: MmScope, owner: int) {
  for e, i: g.events[0..<g.count] {
    at := mm_at(g, i);
    if e.op == MmOp.forget || cast(string) g.bytes[at..<at + e.keys] != key || e.scope != scope || e.owner != owner { continue; }
    for k: at + e.keys..<at + e.keys + e.texts { g.bytes[k] = MM_HIDDEN; }
    g.events[i].redacted = true;
  }
}
```

## Reads

A reader names its session, user and project. `mm_look` answers for one
key in one scope: `served` with the current version, or why not:
`missing`, `forgotten`, `conflict`, `early` (before the window), `expired`
(at or after its end) or `stale` (the citation is not current, with
Psionic chapter 57's reason). `mm_read` lists every fact a reader may be shown: the
current versions in its three scopes that `mm_serve` would serve, in write
order. A fact owned by another project, user or session is never listed.

<!-- source: psionic/lib/harness/memory.coax read -->
```jai
mm_owner :: (c: MmContext, s: MmScope) -> int {
  if s == MmScope.session { return c.session; }
  if s == MmScope.user { return c.user; }
  return c.project;
}
// What a read of one key in one scope gets at tick now: the current version
// when it is in its window and its citation is current, or why not.
mm_look :: (t: MmTable, key: string, scope: MmScope, c: MmContext, now: int, p: MmPassages) -> MmRead {
  r := MmRead.{serve = MmServe.missing, fact = -1, check = AnCheck.current};
  for f, i: t.facts[0..<t.count] {
    if !mm_slot(f, key, scope, mm_owner(c, scope)) { continue; }
    if f.state == MmState.forgotten { r.serve = MmServe.forgotten; }
    if f.state == MmState.conflict { r = MmRead.{serve = MmServe.conflict, fact = i}; }
    if f.state == MmState.current { r = mm_serve(f, i, now, p); }
  }
  return r;
}
mm_serve :: (f: MmFact, i: int, now: int, p: MmPassages) -> MmRead {
  r := MmRead.{serve = MmServe.served, fact = i, check = AnCheck.current};
  if now < f.start { r.serve = MmServe.early; }
  else if now >= f.until { r.serve = MmServe.expired; }
  else if f.source == MmSource.cite {
    r.check = an_check(f.cite, f.text, p.ids, p.heads, p.texts);
    if r.check != AnCheck.current { r.serve = MmServe.stale; }
  }
  return r;
}
// Every fact the reader may be shown, in write order: current, in a scope
// the context names, and served. Their indexes go to out; -1 when out is
// too small.
mm_read :: (t: MmTable, c: MmContext, now: int, p: MmPassages, out: [] int) -> int {
  n := 0;
  for f, i: t.facts[0..<t.count] {
    if f.state != MmState.current || f.owner != mm_owner(c, f.scope) || mm_serve(f, i, now, p).serve != MmServe.served { continue; }
    if n == out.count { return -1; }
    out[n] = i; n += 1;
  }
  return n;
}
```

## Verify and replay

`mm_verify` rebuilds the chain from the log alone. Beyond the sequence,
links, content and digests, it checks the forget rule both ways: a
redacted event must be a write whose key a later forget names and whose
text bytes are all zero, and a write whose key a later forget names must
be redacted. A log in which a forgotten text is readable fails as
`exposed`; a log with a redaction no forget explains fails as `redacted`.
That is the proof that a forgotten fact is no longer visible: the table
drops its text, reads never serve it, and the only other copy, the log, is
checked byte by byte.

`mm_replay` verifies the log, then applies the same admission checks and
fold to every event in order, into an empty table. It returns the first
event the checks refuse. Redacted events are folded without the `same`
check, since their texts are all zero bytes.

<!-- source: psionic/lib/harness/memory.coax verify -->
```jai
// Rebuild the chain and check every event: sequence, link, content (unless
// redacted), digest; a redacted event must be a write whose text is all
// hidden bytes and whose key a later event forgot; a write whose key a later
// event forgot must be redacted.
mm_verify :: (g: MmLog) -> MmCheck {
  at := 0; previous: [32] u8;
  for e, i: g.events[0..<g.count] {
    n := e.keys + e.texts + e.ids;
    if e.keys < 1 || e.texts < 0 || e.ids < 0 || n > g.used - at { return MmCheck.{status = MmStatus.shape, index = i}; }
    if e.sequence != i { return MmCheck.{status = MmStatus.sequence, index = i}; }
    if !an_same_digest(e.previous, previous) { return MmCheck.{status = MmStatus.link, index = i}; }
    gone := mm_forgotten(g, i, at);
    if e.redacted != gone { return MmCheck.{status = ifx gone then MmStatus.exposed else MmStatus.redacted, index = i}; }
    if gone { for k: at + e.keys..<at + e.keys + e.texts { if g.bytes[k] != MM_HIDDEN { return MmCheck.{status = MmStatus.exposed, index = i}; } } }
    else if !an_same_digest(e.content, mm_content(e, g.bytes[at..<at + n])) { return MmCheck.{status = MmStatus.content, index = i}; }
    if !an_same_digest(e.digest, mm_digest(e)) { return MmCheck.{status = MmStatus.digest, index = i}; }
    previous = e.digest; at += n;
  }
  if at != g.used { return MmCheck.{status = MmStatus.shape, index = g.count}; }
  return MmCheck.{status = MmStatus.ok, index = g.count};
}
// Is event i, at byte at, a write whose key a later forget names?
mm_forgotten :: (g: MmLog, i: int, at: int) -> bool {
  e := g.events[i];
  if e.op == MmOp.forget { return false; }
  key := cast(string) g.bytes[at..<at + e.keys]; b := at;
  for k: i..<g.count {
    f := g.events[k];
    if f.op == MmOp.forget && f.scope == e.scope && f.owner == e.owner && cast(string) g.bytes[b..<b + f.keys] == key { return true; }
    b += f.keys + f.texts + f.ids;
  }
  return false;
}
// Rebuild a table from the log alone: verify it, then admit and fold every
// event in order.
mm_replay :: (g: MmLog, t: [] MmTable) -> MmCheck {
  v := mm_verify(g);
  if v.status != MmStatus.ok { return v; }
  t[0].count = 0;
  for e, i: g.events[0..<g.count] {
    at := mm_at(g, i);
    s := mm_admit(t[0], e.op, e.reason, cast(string) g.bytes[at..<at + e.keys], e.scope, e.owner, cast(string) g.bytes[at + e.keys..<at + e.keys + e.texts]);
    if s != MmStatus.ok && !e.redacted { return MmCheck.{status = s, index = i}; }
    mm_apply(t, g, i);
  }
  return v;
}
```

## A trace by hand

Work the example through before you run it. The reader is session 3,
user 1, project 7. The README passage `README.md#Build#0` (heading
`Build`) starts as `Run coax/build.sh test. It takes ten minutes.`

1. Remember project 7's `build`, quoting bytes 0..<23, `Run coax/build.sh
   test.`. The citation is current, so this is `ok`: version 1, current.
   The event holds the key (5 bytes), the text (23) and the identity
   (17): 45 bytes.
2. Remember user 1's `editor` = `vim`, said in event `a1a1...`. No live
   version: `ok`, version 1, current. 6 + 3 = 9 bytes.
3. Remember `editor` = `helix`, said in event `a2...`. `vim` is live and
   different, so both become `conflict`; `helix` is version 2. A look at
   `editor` answers `conflict` with version 2, and a read lists only
   `build`. 11 bytes.
4. Revise `editor` = `helix`, reason `resolve`. Versions 1 and 2 become
   superseded by event 3; version 3 is current. The same revision again is
   refused as `same`: `helix` is now the current text. The read at tick 4
   serves `build` v1 and `editor` v3. 11 bytes.
5. The README's first sentence becomes `Run coax/build.sh test --fast.`.
   The passage digest changes, so `an_check` answers `changed`: `build` is
   stale and the read serves only `editor`. Revising `build` from the old
   bytes is refused as `source`, since the citation no longer checks.
   Revising it from the new bytes 0..<30, reason `update`, is `ok`: version
   2, and version 1 is superseded by event 4. 5 + 30 + 17 = 52 bytes.
6. Remember session 3's `deadline` = `ship Friday`, valid at ticks 0..<10:
   `ok`. The read at tick 6 serves three facts in write order: `editor` v3
   (event 3), `build` v2 (event 4) and `deadline` v1 (event 5). At tick 10
   the deadline is expired. 8 + 11 = 19 bytes.
7. Forget user 1's `editor`. All three versions become forgotten; version
   3 is ended by event 6, versions 1 and 2 keep event 3. The text bytes of
   events 1, 2 and 3 are overwritten, so the log no longer holds `vim` or
   `helix`, while `ship Friday` is still there. The forget event holds
   only its key: 6 bytes.

The log is 7 events and 45 + 9 + 11 + 11 + 52 + 19 + 6 = 153 bytes. It
verifies, and a replay into an empty table gives the same six versions. A
reader in project 8, user 2, session 4 is shown nothing.

## The program

[memory.coax](../../psionic/examples/memory.coax) runs those steps and prints each
result, every read, and the versions at the end. The provenance column
shows the first four bytes of the cited passage's digest or of the event
digest.

```text
README.md#Build#0, before: "Run coax/build.sh test. It takes ten minutes."
1. remember project build, quoting README bytes 0..<23: ok
2. remember user editor = vim, said in event a1: ok
3. remember user editor = helix, said in event a2: ok
look user editor at tick 3: conflict, version 2
4. revise user editor = helix, reason resolve: ok
4. revise user editor = helix again: same
read at tick 4 (session 3, user 1, project 7), facts served: 2
  project 7 build v1 "Run coax/build.sh test." from README.md#Build#0 bytes 0..<23 (7eb39f18) [current]
  user 1 editor v3 "helix" said in event a3a3a3a3 [current, reason resolve]
README.md#Build#0, after: "Run coax/build.sh test --fast. It takes ten minutes."
look project build at tick 5: stale (changed: the cited passage has a new revision; ask again), version 1
read at tick 5 (session 3, user 1, project 7), facts served: 1
  user 1 editor v3 "helix" said in event a3a3a3a3 [current, reason resolve]
5. revise project build from the old bytes: source
5. revise project build, quoting new bytes 0..<30, reason update: ok
6. remember session deadline = ship Friday, ticks 0..<10: ok
read at tick 6 (session 3, user 1, project 7), facts served: 3
  user 1 editor v3 "helix" said in event a3a3a3a3 [current, reason resolve]
  project 7 build v2 "Run coax/build.sh test --fast." from README.md#Build#0 bytes 0..<30 (37d203d8) [current, reason update]
  session 3 deadline v1 "ship Friday" said in event a4a4a4a4 [current]
look session deadline at tick 10: expired, version 1
7. forget user editor: ok
look user editor at tick 7: forgotten
read at tick 7 (session 3, user 1, project 7), facts served: 2
  project 7 build v2 "Run coax/build.sh test --fast." from README.md#Build#0 bytes 0..<30 (37d203d8) [current, reason update]
  session 3 deadline v1 "ship Friday" said in event a4a4a4a4 [current]
every version:
  project 7 build v1 "Run coax/build.sh test." from README.md#Build#0 bytes 0..<23 (7eb39f18) [superseded, ended by event 4]
  user 1 editor v1 "" said in event a1a1a1a1 [forgotten, ended by event 3]
  user 1 editor v2 "" said in event a2a2a2a2 [forgotten, ended by event 3]
  user 1 editor v3 "" said in event a3a3a3a3 [forgotten, ended by event 6, reason resolve]
  project 7 build v2 "Run coax/build.sh test --fast." from README.md#Build#0 bytes 0..<30 (37d203d8) [current, reason update]
  session 3 deadline v1 "ship Friday" said in event a4a4a4a4 [current]
log bytes hold "vim": false, "helix": false, "ship Friday": true
verify: ok after 7 events, 153 bytes; head b35b81ecdf2ae833
replay: ok, 6 versions, same table: true
read at tick 7 (session 4, user 2, project 8), facts served: 0
```

Every line matches the trace. The book's example check runs the program
in debug and release builds and compares both outputs with
[memory.out](../../psionic/fixtures/memory/memory.out), exit status 0.

## Refusals

[memory-refuse.coax](../../psionic/examples/memory-refuse.coax) sends writes the
store must refuse, then edits a finished log the ways an attacker or a
bug would.

<!-- source: psionic/examples/memory-refuse.coax refuse -->
```jai
main :: () {
  ev: [16] MmEvent; by: [512] u8; fs: [16] MmFact;
  l := [1] MmLog.[MmLog.{events = ev[..], bytes = by[..]}]; t := [1] MmTable.[MmTable.{facts = fs[..]}];
  ids := MC_IDS; hd := MC_HEADS; old := MC_OLD; new := MC_NEW;
  p := mc_passages(ids[..], hd[..], old[..]); q := mc_passages(ids[..], hd[..], new[..]);
  w := mc_event("editor", MmScope.user, 1, "vim", 161, 0, 1000);
  print("revise a fact never remembered: %\n", name(mm_revise(l[..], t[..], w, MmReason.correction, p)));
  print("forget a fact never remembered: %\n", name(mm_forget(l[..], t[..], "editor", MmScope.user, 1, p)));
  print("remember with an empty window: %\n", name(mm_remember(l[..], t[..], mc_event("editor", MmScope.user, 1, "vim", 161, 5, 5), p)));
  print("remember a quote the README does not hold: %\n", name(mm_remember(l[..], t[..], mc_cite(new[0], 0, 30), p)));
  print("remember: %, then the same text again: %\n", name(mm_remember(l[..], t[..], w, p)), name(mm_remember(l[..], t[..], w, p)));
  print("revise with no reason: %\n", name(mm_revise(l[..], t[..], mc_event("editor", MmScope.user, 1, "helix", 162, 0, 1000), MmReason.none, p)));
  print("events logged: % (the one remember)\n", l[0].count);
  // The hand trace in a new store, then its log edited after the fact.
  l[0] = MmLog.{events = ev[..], bytes = by[..]}; t[0].count = 0;
  mc_trace(l[..], t[..]);
  e2 := ev; b2 := by; g := MmLog.{events = e2[..], bytes = b2[..], count = l[0].count, used = l[0].used};
  b2[6] = 'r'; print("one byte of the build quote edited: %\n", name(mm_verify(g).status));
  b2 = by; e2[5].redacted = true; print("the deadline hidden with no forget: %\n", name(mm_verify(g).status));
  e2 = ev; e2[2].redacted = false; print("a forgotten version marked readable: %\n", name(mm_verify(g).status));
  e2 = ev; g.count -= 1; g.used -= 6; print("the forget event cut off: %\n", name(mm_verify(g).status));
  g = l[0]; e2 = ev; e2[4].reason = MmReason.none; g.events = e2[..]; g.bytes = b2[..]; mc_rechain(g);
  gs: [16] MmFact; u := [1] MmTable.[MmTable.{facts = gs[..]}];
  print("a revision's reason removed and the chain rewritten: verify %, replay %\n", name(mm_verify(g).status), name(mm_replay(g, u[..]).status));
}
```

```text
revise a fact never remembered: missing
forget a fact never remembered: missing
remember with an empty window: shape
remember a quote the README does not hold: source
remember: ok, then the same text again: same
revise with no reason: reason
events logged: 1 (the one remember)
one byte of the build quote edited: content
the deadline hidden with no forget: redacted
a forgotten version marked readable: exposed
the forget event cut off: redacted
a revision's reason removed and the chain rewritten: verify ok, replay reason
```

The last line is the case the chain can't catch: someone who can write
the whole log edits an event and recomputes every digest. The chain then
verifies, but replay applies the write rules again and refuses event 4, a
revise with no reason. Cutting off the forget event fails for the same
kind of reason: the redactions it explained are left without one. The
output is [memory-refuse.out](../../psionic/fixtures/memory/memory-refuse.out), exit
status 0, in both builds.

## Checks

The checks are in [harness-memory-checks.coax](../../psionic/tests/harness-memory-checks.coax),
apart from the store:

- **The hand trace**, step by step, with every version's state, `by`,
  reason and text, every read and look, the redaction flags, the log's
  byte count, the verifier's result and replay into a new table.
- **Every refusal**, at its boundary: owner 0 and a start of 0 are
  accepted, a one-tick window is served at its start and expired at its
  end, and no refused write logs anything.
- **Scopes**: one key in a session, a user and two projects is four facts,
  and each reader sees only its own.
- **The encoding** against SHA-256 values computed with Python's
  `hashlib`.
- **Forged logs**: every verifier status, at the event where it happens,
  including keys edited next to a forgotten text and a rechained log that
  only replay refuses.
- **A brute-force model.** The model keeps, per key, only the list of
  live texts, where each came from and its version, and whether the key
  was ever forgotten; it knows nothing of events, digests or the table.
  Random sequences of remember, revise and forget on four keys (two
  projects, a session), with event and cited sources, interleaved with
  passage edits and deletions, are run through both. After every step,
  every key's look must be the model's answer, its version and text the
  model's, and the listing exactly the model's served facts; at the end
  the log verifies, every forgotten version is redacted and replay
  rebuilds the same table. No read in any sequence returns a superseded,
  conflicting, stale or forgotten version. Six sequences that reach every
  write outcome and every read outcome run with the store's checks;
  [harness-memory-model.coax](../../psionic/tests/harness-memory-model.coax) runs
  200 more.

`coax mutate` on `memory.coax` kills all 310 mutants; none survive.

## In Coder

Coder exposes the store as four verbs. Each reads the log back from disk,
verifies the whole chain with `mm_replay` and folds it into a table before
it does anything; a log that does not verify is refused, with the event
where it breaks.

```shell
coder/tui/coder.sh remember build "Build Coder with coder/tui/coder.sh."
coder/tui/coder.sh remember deploy "Ship openagents.com only through coax/web/release.sh." --project
coder/tui/coder.sh revise build "Build with coder/tui/coder.sh." --reason correction
coder/tui/coder.sh forget build
coder/tui/coder.sh recall "how do I put the website live?" --top 3
```

- **Scope.** A fact is yours (user scope) unless you pass `--project`,
  which scopes it to the folder you run Coder in. The project's owner id is
  48 bits of SHA-256 of the folder's physical path, so a fact remembered in
  one checkout is not shown in another. `recall` shows your facts and this
  folder's.
- **Window.** A fact holds from the second it was written until
  `--days N` later, or for good. `recall` shows only facts inside their
  window.
- **Conflicts and revisions.** A second `remember` of a key that holds a
  different text makes both versions conflict; neither is shown until
  `revise` settles it. `revise` takes `--reason correction`, `update` (the
  default) or `resolve`, and prints the new version number.
- **Forget.** The forget event is appended, and the text bytes of every
  earlier version of the key are overwritten in the file. Their content
  digests stay, so the chain still verifies.
- **Recall.** With no query, `recall` lists the facts in the order they
  were written. With a query, it ranks them by cosine similarity of
  Vertex `gemini-embedding-001` embeddings (256 dimensions): each fact as a
  retrieval document (`KEY: TEXT`), the query as a retrieval query.
  Microcoder's worker (`coder/tui/mcworker.coax`, request
  `{"embed":"TEXT","query":true}`) makes the calls, so Coder itself holds
  no credentials. A fact's vector is fetched the first time it is ranked
  and kept; the query's is fetched every time. When Vertex can't be
  reached, `recall` says so and lists the facts unranked. `--json` prints
  the facts with their scope, version and score (thousandths of the
  cosine).

- **Sessions.** The first message of every session starts with the facts
  `recall` would list in its folder that a `coder consent` grant lets the
  engine receive ([chapter 18](ch018-consent-before-sharing.md)), then a
  blank line, then your message:

  ```text
  Facts I asked you to remember:
  - deploy: Ship openagents.com only through coax/web/release.sh.
  - editor: I use Helix; never suggest VS Code.

  Which editor should I open this file in?
  ```

  The facts are logged as a `memory` line before the `request` line, so
  `coder replay` rebuilds the same bytes after the memory changes. At most
  8,192 bytes of facts go in; `--no-memory` leaves them out. Facts no
  grant covers stay on the machine, and the log counts them as
  `withheld`. In a live
  Microcoder run, that message got the answer "You should open this file
  in Helix."

- **Agents.** `coder mcp` serves two more tools, `coder_remember` (a key
  and a text; with `reason` it revises; `project` and `cwd` scope it) and
  `coder_recall` (an optional `query` and `top`). They run the verbs, so
  the same log, checks and consent apply. On 2026-11-02, Claude Code was
  told "I only deploy on Fridays" with only `coder_remember` allowed; it
  stored `deploy-day: The user only deploys on Fridays.`, and `coder
  recall` listed it.

The files are in `~/.openagents/coder/memory/`:

| File | What it holds |
|---|---|
| `events.jsonl` | The log, one event a line: every field of the event, its key, text and citation bytes in hex, and its digests. The `key` field is a readable copy and is not loaded. |
| `vectors.jsonl` | One vector for each live fact, keyed by the digest of the event that wrote it, as integers (each value times 1,000,000). A write drops the vectors of facts that are no longer live. |

Both files are rewritten through a temporary file and a rename. The verbs
are in [coder/tui/memory.coax](../tui/memory.coax).

In a live run on 2026-11-02, the query "how do I put the website live?"
ranked the `deploy` fact at 71% and `build` at 63%, and "compile the
coding agent" ranked `build` first at 75%. The second query fetched only
its own embedding.

The smoke (`coder/tui/smoke.py`) remembers, conflicts, revises and forgets
in a fresh HOME, then replays three recorded embedding responses
([fixtures/memory-replay](../tui/fixtures/memory-replay/README.md)) with
`GW_REPLAY` and no credentials. It checks that `deploy` ranks first, that
the forgotten text is not in the file, that `forget` drops the vector, and
that a log with one edited field is refused. It also checks that a
session's first message carries the facts, that replay rebuilds it, and
that `--no-memory` leaves them out.

## What this does not do yet

- **Event sources are not checked against a session log.** An
  event-sourced fact carries the digest of the event it was said in, but
  nothing yet confirms that chapter2's log holds an event with that
  digest. Cited facts are checked against their passage on every read.
- **The agent writes through MCP only.** An agent that has Coder's MCP
  tools can propose a fact with `coder_remember`, and the host's tool
  approval is your consent. An engine Coder drives directly does not get
  those tools unless you start it with `--coder-tools`.
- **Ranking is by embeddings only.** Psionic chapter 54's lexical ranking
  and reranking are not combined with the cosine score.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- FIPS 180-4, Secure Hash Standard (SHA-256).
