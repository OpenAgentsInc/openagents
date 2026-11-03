# 51. A realm is a document

> **Status: partly implemented.** A made realm is a versioned JSON document
> the server keeps (`aethyr.realm`, version 1, with a revision and a SHA-256
> hash), and [aethyr/forge/doc.coax](../forge/doc.coax) reads it into
> Aethyrforge's edit model: moves, turns, sizes and removals as entity
> overrides, and the setup, tools, placed things, lore and quests as named
> slots. The reader knows every version the server has written, moves an
> older one forward, and refuses a newer, malformed or out-of-bounds
> document whole, so a client keeps the last document it could read. The
> example below runs natively in debug and release. Not built yet: the
> catalogue as one Coax table that generates the SQL catalogue and the
> planner's schema, a pure `layout` in Aethyr (the page still lays a realm
> out from its slug's seed in `myrealm.coax`), and `want` lines in the
> document. See [Still to do](#still-to-do).

A realm someone makes is not code. It is a small record: the camera, the art
style, the ground, the time of day, a list of features with bounded values,
the tools visitors get and the things placed since. Everything a visitor sees
must follow from that record and a seed, the same on every device. This part
of the book is Aethyrforge, the tools that make and change such records; this
chapter defines the record and the reader every client uses.

## What this chapter teaches

- A realm as a versioned document with an identity: a kind, a version, a
  revision and a hash.
- Reading a document into typed state with bounds, in whole units, so the
  same document is the same bytes on every device.
- Moving an older version forward, and refusing a newer one whole instead of
  guessing at it.
- Two documents compared as one canonical text.

## The document

`GET /api/playtest/realms/<slug>/doc` answers a made realm's document and its
hash. Lamp Cove at revision 4:

```text
{"doc": {"doc": "aethyr.realm", "version": 1, "slug": "lamp-cove", "rev": 4,
         "name": "Lamp Cove",
         "setup": {"camera": "third", "style": "lowpoly", "ground": "meadow", "time": "night", "palette": "warm"},
         "features": [], "off": [], "tools": {"fly": false, "jump": true},
         "placed": [{"thing": "barrel", "x": 2, "z": -3.5, "yaw": 90}],
         "moved": [{"entity": "trees:3", "x": 1.25, "z": -2.5}, {"entity": "rocks:0", "x": -40, "z": 0.07}],
         "base": null, "forked_from": null},
 "sha256": "..."}
```

The server builds it from the realm's row (`verse.realm_doc` in
[0111_realm_edit_log.sql](../../coax/web/api/migrations/0111_realm_edit_log.sql),
first written in
[0093_realm_documents.sql](../../coax/web/api/migrations/0093_realm_documents.sql))
and hashes it (`verse.realm_doc_sha`). Any change to what the realm is (its
features, the rows switched off, its tools, things placed, moves, shapes,
lore, quests) makes a new revision, and each saved, published, forked or made
revision is kept in `verse.realm_revisions` with its hash.

Every entity a layout makes has a stable name: its feature and its index in
that feature's layout (`trees:3`), because the layout comes from the slug's
seed. `moved` holds an override per moved entity. Two later fields are in the
document only when they have entries: `shaped` (turns, sizes and removals,
[0109_realm_shapes.sql](../../coax/web/api/migrations/0109_realm_shapes.sql)),
and `lore` and `quests` (0111). A document written before them keeps its
bytes and its hash.

## Reading it

The reader names the one kind and the newest version it knows:

<!-- source: aethyr/forge/doc.coax read -->
```jai
FORGE_DOC_KIND :: "aethyr.realm";
FORGE_DOC_NEWEST :: 1;   // the newest version this reader knows
FORGE_DOC_BYTES :: 262144;
FORGE_SLUG :: 64;        // a realm's slug: [a-z0-9][a-z0-9-]{0,63}, the server's rule
// What a read found: its version, and for a fork the realm and revision it was copied from
// (from_n 0 and from_rev 0 when it isn't one).
ForgeRead :: struct { ok: bool; why: string; version: int; from: [FORGE_SLUG] u8; from_n: int; from_rev: int; }
```

`forge_read_doc` parses the text, checks the kind, the version and the
revision, then reads each part into a fresh model. Only when every part is in
bounds does it replace the caller's model; any refusal returns a reason and
leaves the model as it was. A version 1 document without `shaped`, `lore` or
`quests` is the same realm with none: that is how an older document moves
forward. A version above `FORGE_DOC_NEWEST` is refused, never read in part.

Positions are read as whole centimetres, never as floating point, so the same
document gives the same model everywhere:

<!-- source: aethyr/forge/doc.coax metres -->
```jai
// Metres with at most two decimals, as the server writes them, to whole centimetres; 4001 when not.
forge_cm_of :: (raw: string) -> int {
  neg := raw != "" && raw[0] == '-';
  s := ifx neg then raw[1..<raw.count] else raw;
  dot := 0; while dot < s.count && s[dot] != '.' { dot += 1; }
  if dot == 0 || dot > 2 || (dot < s.count && (s.count - dot - 1 < 1 || s.count - dot - 1 > 2)) { return 4001; }
  v := 0;
  for i: 0..<dot { if !forge_digit(s[i]) { return 4001; } v = v * 10 + cast(int) (s[i] - '0'); }
  frac := 0; places := 0;
  for i: dot + 1..<s.count { if !forge_digit(s[i]) { return 4001; } frac = frac * 10 + cast(int) (s[i] - '0'); places += 1; }
  if places == 1 { frac *= 10; }
  cm := v * 100 + frac;
  return ifx neg then -cm else cm;
}
```

### Worked by hand

Take Lamp Cove's three positions and one out of bounds. An entity may stand at
most `FORGE_REACH`, 4000 cm, from the middle on each axis
([chapter 52](ch052-edit-a-realm-in-place.md)).

| Raw | Whole metres | Fraction | Centimetres | In reach |
| --- | --- | --- | --- | --- |
| `1.25` | 1 | `25`, two places | 1 × 100 + 25 = 125 | yes |
| `-2.5` | 2 | `5`, one place, so 50 | −(200 + 50) = −250 | yes |
| `-40` | 40 | none | −4000 | yes, at the edge |
| `0.07` | 0 | `07` | 7 | yes |
| `40.01` | 40 | `01` | 4001 | no: refused |

A third decimal (`1.255`), a quoted number or three whole-metre digits all give
4001 and are refused the same way. The JSON reader's `jv_raw` gives a string's
contents without its quotes, so `"1.25"` would read as `1.25`; every numeric
field (the version, the revision, a fork's revision, a move's `x` and `z`, a
shape's turn and size, a placed thing's `x`, `z` and turn) is read through
`forge_num`, which gives nothing unless the value is a JSON number (#12720).

## The model as one text

Two models are the same realm exactly when their texts are the same bytes.
The text lists every override and slot in name order, so the order a
document wrote them in doesn't matter:

<!-- source: aethyr/forge/edit.coax text -->
```jai
// The whole document as text, every override and slot in name order: two documents are the same
// realm exactly when their texts are the same bytes (replays are compared this way).
forge_doc_text :: (d: [] ForgeDoc, buf: [] u8) -> int {
  n := forge_put(buf, 0, "rev "); n = forge_put_int(buf, n, d[0].rev); n = forge_put(buf, n, "\n");
  last := "";
  for _: 0..<d[0].move_n {   // the next override after `last`, by name
    best := -1;
    for i: 0..<d[0].move_n {
      k := forge_move_key(d, i);
      if forge_after(k, last) && (best < 0 || forge_after(forge_move_key(d, best), k)) { best = i; }
    }
    m := d[0].moves[best]; last = forge_move_key(d, best);
    n = forge_put(buf, n, "entity "); n = forge_put(buf, n, last);
    if m.at { n = forge_put(buf, n, " at "); n = forge_put_int(buf, n, m.x); n = forge_put(buf, n, " "); n = forge_put_int(buf, n, m.z); }
    n = forge_put(buf, n, " yaw "); n = forge_put_int(buf, n, m.yaw); n = forge_put(buf, n, " scale "); n = forge_put_int(buf, n, m.scale);
    n = forge_put(buf, n, ifx m.gone then " gone\n" else "\n");
  }
  last = "";
  for _: 0..<d[0].slot_n {
    best := -1;
    for i: 0..<d[0].slot_n {
      k := forge_slot_key(d, i);
      if forge_after(k, last) && (best < 0 || forge_after(forge_slot_key(d, best), k)) { best = i; }
    }
    last = forge_slot_key(d, best);
    n = forge_put(buf, n, "slot "); n = forge_put(buf, n, last); n = forge_put(buf, n, " ");
    n = forge_put(buf, n, forge_slot_text(d, last)); n = forge_put(buf, n, "\n");
  }
  return n;
}
```

## Read Lamp Cove

The program reads the document above, prints the model's text, then gives
the reader three broken documents: a version this reader doesn't know, a move
past the reach and a placed thing with no name. The program is
[aethyr/examples/read-a-realm.coax](../examples/read-a-realm.coax):

<!-- source: aethyr/examples/read-a-realm.coax example -->
```jai
#load "../forge/doc.coax";
#import "Basic";

// Lamp Cove's document at revision 4, as GET .../lamp-cove/doc answers it, read into the edit
// model and written back as the model's text. Then three documents the reader refuses whole:
// each refusal names why, and the last document read stays exactly as it was.
main :: () {
  d: [1] ForgeDoc; forge_reset(d[..], 1);
  r := forge_read_doc(d[..], FORGE_DOC_V1);
  print("read: % (version %)\n", r.ok, r.version);
  last: [8192] u8; ln := forge_doc_text(d[..], last[..]);
  print("%", cast(string) last[0..<ln]);
  b: [4096] u8;
  froms := [3] string.["\"version\": 1", "\"x\": 1.25", "\"thing\": \"barrel\""];
  tos := [3] string.["\"version\": 2", "\"x\": 40.01", "\"thing\": \"\""];
  for i: 0..<3 {
    c: [4096] u8; cn := forge_put(c[..], 0, forge_doc_with(froms[i], tos[i], b[..]));
    no := forge_read_doc(d[..], cast(string) c[0..<cn]);
    now: [8192] u8; nn := forge_doc_text(d[..], now[..]);
    print("% -> refused: %; the last document stays: %\n", tos[i], no.why, cast(string) now[0..<nn] == cast(string) last[0..<ln]);
  }
}
```

```shell
coax/bin/coax run aethyr/examples/read-a-realm.coax
```

```text
read: true (version 1)
rev 4
entity rocks:0 at -4000 7 yaw 0 scale 100
entity trees:3 at 125 -250 yaw 0 scale 100
slot place:0 barrel 2 -3.5 90
slot set:camera third
slot set:ground meadow
slot set:palette warm
slot set:style lowpoly
slot set:time night
slot tool:fly 0
slot tool:jump 1
"version": 2 -> refused: a newer document than this reader knows; the last document stays: true
"x": 40.01 -> refused: a move out of bounds; the last document stays: true
"thing": "" -> refused: a placed thing out of bounds; the last document stays: true
```

The moves are the hand-worked centimetres: `trees:3` at 125, −250 and
`rocks:0` at −4000, 7. A placed thing keeps the document's numbers as written
(`barrel 2 -3.5 90`). `rocks:0` sorts before `trees:3`, and `set:` before
`tool:`, whatever order the document used. The rows switched off (`off`) are
not in the model: the server owns them, and `row:KEY` slots carry only the
edits a page makes to them.

## Checks

- `coax/bin/coax test aethyr/forge/doc.coax --library`: a version 1 document
  from before shapes reads as the same realm with none; shapes, lore and
  quests from 0109 and 0111 are read as the slots the server's undo log names,
  and six broken lore and quest fields are refused; 22 broken documents (a
  newer, zero, fractional or missing version, another kind, a revision of 0, a
  move past the reach, with three decimals or quoted, an entity moved twice
  or misnamed, moves that aren't a list, shapes out of bounds, plain or twice,
  shapes that aren't a list, a tool that isn't a boolean, a setup field with a
  line break, a placed thing with no name or a missing or non-number place,
  and a top level that isn't an object) are each refused and leave the last
  document's text byte-identical; each of the ten numeric fields quoted is
  refused with its own reason while the same field as a number reads; and every
  bound (revision, turn, size, the 256 KiB limit, slug and key lengths) is
  checked at its edge and one past it. `coax/bin/coax mutate
  aethyr/forge/doc.coax --library --root-only`: 409 mutants, 0 survivors.
- The independent check is the server. `coax/web/tool.sh
  coax/web/api/tests/realm_walkthrough.coax` runs the made-realm route against
  PostgreSQL with every migration applied
  ([chapter 54](ch054-publish-a-realm.md#the-route-checked-end-to-end)): it
  hashes the document a visitor gets and compares that with the hash the SQL
  rule stated, then reads it back with this reader and finds exactly the
  moves, turns, sizes and removals its edits made. The SQL that writes the
  document and the Coax that reads it are separate implementations; the
  walkthrough fails when they disagree.

## Where the rest lives today

- [docs/verse/realm-builder.md](../../docs/verse/realm-builder.md): the
  builder's flow (setup questions, prompt, board, toggles, MAKE IT), the
  catalogue (`verse.builder_catalogue()` in
  `coax/web/api/migrations/0048_realm_builder.sql`, extended by 0097's
  `enclosure`, `centre`, `layout` and `path`), limits (200 placed things, 50
  waiting cards) and the composition rules.
- [apps/openagents-web/builder.coax](../../apps/openagents-web/builder.coax):
  the board page. It never reads the words; it shows the board and sends back
  which rows are checked.
- [apps/openagents-web/myrealm.coax](../../apps/openagents-web/myrealm.coax)
  and [myrealm-shapes.coax](../../apps/openagents-web/myrealm-shapes.coax): the
  line spec `GET /api/playtest/realms/<slug>` answers, laid out from the slug
  (`SLOT_A`, `NEAR_A`, `owl_home`, `roofs_out`). The spec carries the
  document's identity: `rev N`, `published N`, `listed 0|1`, `from SLUG
  REV|NAME` for a fork, `moved ENTITY X Z` and `shaped ENTITY YAW SCALE 0|1`.
- A save names the revision it was read at, and the server refuses a stale
  one with `409` and the revision there now
  ([chapter 52](ch052-edit-a-realm-in-place.md),
  [chapter 81](ch081-save-and-come-back.md)).

## Still to do

The document has an identity and one reader; its types and its layout are
still split between SQL and the page.

- **The catalogue as one table.** `aethyr/forge/catalogue.coax`: each feature's
  id, enumerated values and count range, from which the SQL catalogue and the
  planner's JSON schema are generated. Today the catalogue lives in SQL
  (0048, 0097) and the page's `#test`s and the API tests are the only check
  that the two sides agree.
- **Features in the model.** The reader keeps `features` and `off` out of the
  model. A typed `Feature` read with its bounds (an unknown feature, a count
  out of range or a value outside its enum refused with its field named)
  needs the catalogue first.
- **A pure layout.** `aethyr/forge/layout.coax`: `layout(doc, seed)` giving
  chapter 6 entities under chapter 16's budgets, byte-identical natively and
  in the browser, with adding a feature leaving every other feature's
  entities unchanged. Today `myrealm.coax` lays the realm out on the page.
- **Wants.** Whatever a prompt asks for that no feature draws becomes a card
  on the builder's board; it is not a `want` line carried in the document
  through revisions.

### Acceptance checks still open

- Encode then decode is the identity for every catalogue feature at its
  bounds.
- `layout` of the same document and seed gives byte-identical entity tables
  natively and in the browser.
- Every asked thing either maps to a feature or stays as a `want` line
  through ten revisions (the builder's `eval-builder.sh` corpus as fixtures).
- The generated SQL catalogue equals `verse.builder_catalogue()`; drift fails.
- A document over its budgets (placed things, vertices) is refused at
  layout, never clipped.

## Dependencies

[Chapter 3](ch003-state-starts-from-its-defaults.md) (defaults and
versions), [chapter 6](ch006-entities-are-born-and-die.md) (stable IDs) and
[chapter 16](ch016-budgets-that-refuse.md) (budgets) suffice for the typed
document and deterministic layout. A physics-bearing realm later maps that
document into [chapter 17](ch017-a-scene-owns-the-state.md)'s scene and checks
that mapping; constructing the document does not require the physics engine.
