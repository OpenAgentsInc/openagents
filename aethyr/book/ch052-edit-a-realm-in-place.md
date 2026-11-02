# 52. Edit a realm in place

> **Status: partly implemented.** Every revision path a made realm has is a
> typed, invertible edit in one log with undo, redo and replay:
> [aethyr/forge/edit.coax](../forge/edit.coax). Moves, turns, sizes and
> removals of any entity, and slot edits for rows, tools, setup fields,
> lore, quests and placed things. A made realm's page
> ([apps/openagents-web/myrealm.coax](../../apps/openagents-web/myrealm.coax))
> uses it for MOVE, UNDO and SAVE and draws turns, sizes and removals
> (migration 0109); `openagents realm rotate|resize|remove` and `realm apply`
> make them. The page has no hand gestures for turning, sizing or removing
> yet, and the server still undoes rows, placements and cards as whole
> revisions (`0094_realm_live_and_undo.sql`); see [Still to do](#still-to-do).

Editing a realm while people walk in it is the same problem as replaying it.
Each change must be a recorded, ordered operation over the state. It must be
applied the same way everywhere, and it must be possible to take it back. An
owner moving a tree, switching a row or placing a crate each makes an
operation with a target, a before and an after.

## What this chapter teaches

- An edit as an operation over chapter 51's document, with its inverse.
- Why an edit is refused when its target no longer holds its before.
- Undo and redo as a position in a log of operations.
- Why a save names the revision it was read at.

## The document the page edits

A made realm's layout comes from its slug's seed, so every entity in it has a
stable name: its feature and its index in that feature's layout, such as
`trees:3` (and `placed:N` for the N-th thing placed). The document keeps an
override for each entity that was moved, turned, resized or taken away: its
place in whole centimetres, its turn in whole degrees, its size in whole
percent, so the same document is the same bytes on every device. Every other
revision path a made realm has is a named slot: a board row (`row:KEY`), a
visitor tool (`tool:NAME`), a setup field (`set:time`), a character's lore
(`lore:WHO`), a quest (`quest:GIVER`) and each thing placed (`place:N`). Slot
values live in one text the document and its history share, packed when it
fills:

<!-- source: aethyr/forge/edit.coax doc -->
```jai
FORGE_KEY :: 24;       // an entity's name: "trees:3"
FORGE_MOVES :: 64;     // entities one document overrides (the server's limit too)
FORGE_OPS :: 64;       // undo steps kept; the oldest goes first
FORGE_REACH :: 4000;   // centimetres from the middle an entity may stand (the server's limit too)
FORGE_TURN :: 180;     // degrees an entity turns either way, at most
FORGE_SCALE_MIN :: 25; // percent of its laid-out size, at least
FORGE_SCALE_MAX :: 400;
FORGE_SLOT_KEY :: 48;  // a slot's name: "lore:mara"
FORGE_VAL :: 480;      // a slot's value bytes at most (lore: 400 characters)
FORGE_SLOTS :: 96;     // slots one document keeps
FORGE_TEXT :: 32768;   // bytes of slot values the document and its history keep together (64 steps of 400-byte lore fit)

// An entity as the document has it: where it stands instead (`at`), its turn and size, and gone.
// The plain shape (nothing overridden) is the entity as laid out.
ForgeShape :: struct { at: bool; x: int; z: int; yaw: int; scale: int = 100; gone: bool; }
ForgeMove :: struct { key: [FORGE_KEY] u8; key_n: int; x: int; z: int; at: bool = true; yaw: int; scale: int = 100; gone: bool; }
// A slot's value is a span of the document's `text`; so are an edit's before and after values.
ForgeSlot :: struct { key: [FORGE_SLOT_KEY] u8; key_n: int; at: int; n: int; }
FORGE_ENTITY :: 0;
FORGE_SLOT :: 1;
// One edit: what it changes, as it was before and after. An entity's shapes (`b`, `a`), or a
// slot's values (`had` false: absent before; `has` false: absent after).
ForgeOp :: struct {
  kind: int;
  key: [FORGE_SLOT_KEY] u8; key_n: int;
  b: ForgeShape; a: ForgeShape;
  had: bool; bv: int; bv_n: int;
  has: bool; av: int; av_n: int;
}
ForgeDoc :: struct {
  rev: int;                        // the revision this document was read at, or last saved as
  moves: [FORGE_MOVES] ForgeMove; move_n: int;
  slots: [FORGE_SLOTS] ForgeSlot; slot_n: int;
  text: [FORGE_TEXT] u8; text_n: int;   // the values slots and edits name
  ops: [FORGE_OPS] ForgeOp; op_n: int; done: int;   // the log, and how many of it are applied
  saved: int;                      // `done` when it was last read or saved; -1 when that state is gone
}
```

`rev` is the revision the page read from the server, or the revision its last
save was kept as. `done` is how much of the log is applied, and `saved` is
the value of `done` when the server last kept the document.

## Operations and their inverses

An operation names what it changes and how it was before and after: an
entity's whole shape (`b`, `a`), or a slot's value (`had` false: absent).
Applying an operation is refused unless what it changes is still as the
operation says it was; that is how a stale edit is rejected, on the page and
in the CLI's batches:

<!-- source: aethyr/forge/edit.coax ops -->
```jai
// Apply an edit: refused unless what it changes is still as the edit says it was.
forge_apply :: (d: [] ForgeDoc, o: [] ForgeOp) -> bool {
  key := cast(string) o[0].key[0..<o[0].key_n];
  if o[0].kind == FORGE_ENTITY {
    if !forge_same_shape(forge_shape_of(d, key), o[0].b) { return false; }
    return forge_set_shape(d, key, o[0].a);
  }
  now := forge_slot_at(d, key);
  if now.found != o[0].had { return false; }
  if now.found && forge_slot_text(d, key) != forge_span(d, o[0].bv, o[0].bv_n) { return false; }
  return forge_put_slot(d, key, o[0].has, o[0].av, o[0].av_n);
}
// The edit that takes it back: before and after swapped.
forge_inverse :: (o: ForgeOp) -> ForgeOp {
  r := o;
  r.b = o.a; r.a = o.b;
  r.had = o.has; r.bv = o.av; r.bv_n = o.av_n;
  r.has = o.had; r.av = o.bv; r.av_n = o.bv_n;
  return r;
}
```

Every edit a made realm has is one such operation, so one log holds them all
(GitHub #12629):

<!-- source: aethyr/forge/edit.coax edits -->
```jai
// The edits a made realm has, each one step with its inverse.
forge_move :: (d: [] ForgeDoc, key: string, x: int, z: int) -> bool {
  s := forge_shape_of(d, key); s.at = true; s.x = x; s.z = z;
  return forge_reshape(d, key, s);
}
forge_rotate :: (d: [] ForgeDoc, key: string, deg: int) -> bool {
  s := forge_shape_of(d, key); s.yaw = forge_turn(deg);
  return forge_reshape(d, key, s);
}
forge_resize :: (d: [] ForgeDoc, key: string, percent: int) -> bool {
  s := forge_shape_of(d, key); s.scale = percent;
  return forge_reshape(d, key, s);
}
forge_remove :: (d: [] ForgeDoc, key: string) -> bool {
  s := forge_shape_of(d, key); s.gone = true;
  return forge_reshape(d, key, s);
}
forge_restore :: (d: [] ForgeDoc, key: string) -> bool {
  s := forge_shape_of(d, key); s.gone = false;
  return forge_reshape(d, key, s);
}
// A board row on or off; a visitor tool on or off; one of the setup fields.
forge_switch :: (d: [] ForgeDoc, row: string, on: bool) -> bool { return forge_slot_edit(d, "row:", row, true, ifx on then "1" else "0"); }
forge_tool :: (d: [] ForgeDoc, tool: string, on: bool) -> bool { return forge_slot_edit(d, "tool:", tool, true, ifx on then "1" else "0"); }
forge_setting :: (d: [] ForgeDoc, what: string, value: string) -> bool {
  if what != "style" && what != "ground" && what != "time" && what != "camera" && what != "palette" { return false; }
  return forge_slot_edit(d, "set:", what, true, value);
}
// A character's lore ("" forgets it); a giver's quest (ask "" takes it away). No bar in a quest's words.
forge_lore :: (d: [] ForgeDoc, who: string, text: string) -> bool { return forge_slot_edit(d, "lore:", who, text.count > 0, text); }
forge_quest :: (d: [] ForgeDoc, giver: string, asked: string, steps: [3] string, reward: string) -> bool {
  b: [FORGE_VAL] u8; n := 0;
  parts := [5] string.[asked, steps[0], steps[1], steps[2], reward];
  for p: 0..<5 {
    for c: parts[p] { if c == '|' { return false; } }
    if p > 0 { n = forge_put(b[..], n, "|"); }
    n = forge_put(b[..], n, parts[p]);
  }
  if n < 0 { return false; }
  return forge_slot_edit(d, "quest:", giver, asked.count > 0, cast(string) b[0..<n]);
}
// A thing placed: the next "place:N" slot, THING X Z YAW (centimetres, degrees).
forge_place :: (d: [] ForgeDoc, thing: string, x: int, z: int, yaw: int) -> bool {
  if !forge_in_reach(x, z) || thing.count == 0 { return false; }
  for c: thing { if !forge_lower(c) { return false; } }
  kb: [24] u8; n := 0; while forge_slot_at(d, forge_place_key(n, kb[..])).found { n += 1; }
  b: [64] u8; m := forge_put(b[..], 0, thing); m = forge_put(b[..], m, " "); m = forge_put_int(b[..], m, x);
  m = forge_put(b[..], m, " "); m = forge_put_int(b[..], m, z); m = forge_put(b[..], m, " "); m = forge_put_int(b[..], m, forge_turn(yaw));
  if m < 0 { return false; }
  return forge_reslot(d, forge_place_key(n, kb[..]), true, cast(string) b[0..<m]);
}
```

Replaying the applied steps of one log onto the document it started from
gives the same document, byte for byte in `forge_doc_text`; a replay onto a
document that changed under it stops at the first step that no longer fits:

<!-- source: aethyr/forge/edit.coax replay -->
```jai
// Replay: the applied steps of `from`, in order, onto `d` as steps of its own. False at the first
// step whose before no longer matches (the document changed under it); the steps before stay.
forge_replay :: (d: [] ForgeDoc, from: [] ForgeDoc) -> bool {
  for i: 0..<from[0].done {
    o: [1] ForgeOp; o[0] = from[0].ops[i];
    if o[0].kind == FORGE_SLOT {   // its values move into this document's text
      if o[0].has { o[0].av = forge_keep(d, forge_span(from, o[0].av, o[0].av_n)); if o[0].av < 0 { return false; } }
      if o[0].had { o[0].bv = forge_keep(d, forge_span(from, o[0].bv, o[0].bv_n)); if o[0].bv < 0 { return false; } }
    }
    if !forge_push(d, o[..]) { return false; }
  }
  return true;
}
```

Undo applies the inverse of the last applied operation and moves `done`
back by one. Redo applies the next operation again. A new move after an undo
drops the operations past `done`, so there is no redo after it. The log keeps
the last `FORGE_OPS` steps; the oldest goes first.

Because undo and redo only move `done`, unsaved changes are a comparison:
the document is dirty when `done` differs from `saved`. Undoing back to the
saved state makes the page clean again.

## Saving names its revision

A save sends the revision the page read, every move and (to a server with migration 0109, which says `shapes 1`) every turn, size and removal:

<!-- source: aethyr/forge/edit.coax save -->
```jai
// The body of POST /api/playtest/realms/<slug>/save: the revision read, every move in metres, and
// (only when there are any: a server before migration 0109 knows moves only) every turn, size and removal.
forge_save_body :: (d: [] ForgeDoc, buf: [] u8) -> int {
  n := forge_put(buf, 0, "{\"rev\":"); n = forge_put_int(buf, n, d[0].rev); n = forge_put(buf, n, ",\"moved\":[");
  first := true;
  for i: 0..<d[0].move_n {
    m := d[0].moves[i];
    if !m.at { continue; }
    if !first { n = forge_put(buf, n, ","); }
    first = false;
    n = forge_put(buf, n, "{\"entity\":\""); n = forge_put(buf, n, forge_move_key(d, i));
    n = forge_put(buf, n, "\",\"x\":"); n = forge_put_cm(buf, n, m.x);
    n = forge_put(buf, n, ",\"z\":"); n = forge_put_cm(buf, n, m.z); n = forge_put(buf, n, "}");
  }
  n = forge_put(buf, n, "]");
  if forge_any_shaped(d) {
    n = forge_put(buf, n, ",\"shaped\":[");
    first = true;
    for i: 0..<d[0].move_n {
      m := d[0].moves[i];
      if m.yaw == 0 && m.scale == 100 && !m.gone { continue; }
      if !first { n = forge_put(buf, n, ","); }
      first = false;
      n = forge_put(buf, n, "{\"entity\":\""); n = forge_put(buf, n, forge_move_key(d, i));
      n = forge_put(buf, n, "\",\"yaw\":"); n = forge_put_int(buf, n, m.yaw);
      n = forge_put(buf, n, ",\"scale\":"); n = forge_put_int(buf, n, m.scale);
      n = forge_put(buf, n, ",\"gone\":"); n = forge_put(buf, n, ifx m.gone then "true" else "false"); n = forge_put(buf, n, "}");
    }
    n = forge_put(buf, n, "]");
  }
  return forge_put(buf, n, "}");
}
```

The server keeps the save as the next revision only when the realm is still
at the named revision. Otherwise it answers `409` with the revision there
now, and the page keeps its changes unsaved:

```text
POST /api/playtest/realms/forge-court/save  {"rev":1,"moved":[...]}  -> 200 {"rev":2,"sha256":"..."}
POST /api/playtest/realms/forge-court/save  {"rev":1,"moved":[]}     -> 409 {"error":"changed since","rev":2}
```

Any change to the realm makes a new revision, including a row switched on
the board, so a save read before that change is refused too. The newer write
never simply wins. The rule lives in
[coax/web/api/migrations/0093_realm_documents.sql](../../coax/web/api/migrations/0093_realm_documents.sql)
(`verse.realm_save`).

## On the page

In a made realm, the owner sees a row of controls above the tools. Each one
is shown only when it applies:

| Control | Shown when | What it does |
| --- | --- | --- |
| MOVE | the realm has something to move | Picks the thing nearest you: a tree, a rock, a prop, a building, a house, a lantern or a thing placed. A tap on another picks that one. |
| PUT IT HERE | a thing is picked | Stands it in front of you (2.5 m, further for a big thing), as one undo step. Where any of it would be in a rock or another thing, off the ground or in the water, it says so and keeps the thing picked. Esc lets it go. |
| UNDO | there is a step to undo | Takes the last unsaved move back. With none left, takes back the server's newest step: a save, a row switched, a thing placed or a card settled. |
| SAVE | there are unsaved changes | Saves from the revision read. |

A thing's pieces, solids, lights, fires and turning parts move with it, and
nothing else does. This browser keeps the unsaved moves (host action 4), so
a reload of the same revision brings them back, still undoable. A save
becomes one server step.

The server keeps the undo steps (`made_realms.undo`, at most 64): each edit
that makes a revision pushes the revision before it. `POST .../undo {rev}`
restores the fields of that revision as the next revision, so history is
never rewritten, and a card settled goes back to waiting.

On a 390 px phone the row spans the screen width and PUT IT HERE is
shortened to PUT HERE.

## Checks

- `coax/bin/coax test aethyr/forge/edit.coax --library`: a move, its undo and
  redo; an undo of a second move puts back the first; an edit whose entity
  moved since is refused; apply then inverse restores the document; saved is
  a state of the log; the log and the document keep their limits; the save's
  body; rotate, resize and remove with their inverses; every slot path with
  its inverse; a stale slot edit is refused; a replayed log gives the same
  document and a replay onto a changed one stops; slot text packs and refuses
  what can't fit.
- `coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax` with
  `PIN_SQL=coax/web/api/tests/realm_shapes.sql`: the server's side of shapes
  (bounds, a save keeps them, refusals change nothing, visitors, forks and
  undo carry them).
- `coax/bin/coax test apps/openagents-web/myrealm.coax`: MOVE moves only
  the picked thing's pieces and UNDO restores every piece; a rock, a prop, a
  building and a lantern each move with their solids and lights; a big thing
  is refused where any of it would stand in a rock or off the ground; unsaved
  moves come back after a reload of the same revision and not another; UNDO
  with nothing unsaved asks the server; SAVE names the
  revision it was read at; a 409 keeps the changes unsaved, and reading the
  spec again does not drop them; a visitor gets none of the owner's
  controls; the row fits a 390 px phone.
- `coax/web/api/test.sh` (`test_realm_fork.py`): only the owner saves; a save
  from an older revision is refused with the revision there now; moves are
  validated and rounded to the centimetre; a save, a placement, a row switch,
  an accepted and a declined card are each undone as one step.
- `coax/web/tool.sh forge-journey URL [--phone]`: the whole maker journey in
  headless Chrome, steps 2 to 9 (move, undo, reload before saving, move a
  thing that isn't a tree, save, reload, build and undo).

## Still to do

> **Design draft.** Nothing below runs yet.

- The server keeping the log: today the server undoes rows, placements,
  cards and revisions by voice as whole revisions, while the model's slot
  edits replay them on the client. Revisions by voice and visitor proposals
  are server-side model calls, not slot edits.
- Hand gestures for rotate, resize and remove on the page (slice 3's canvas).
- Redo across reloads, and redo of a server step.
- The asks ledger's revert rule as a Coax function the server's SQL is
  checked against: for each row an ask changed, put back `before` only if
  the row still holds the ask's `after`.
- Everglade's own edit mode
  (`apps/openagents-web/everglade-edit.coax`) on the same log.
- Moving figures and animals.

The older edit layer is documented in
[docs/verse/realm-edit.md](../../docs/verse/realm-edit.md): who may edit
(`verse.edit_role`, `0050_realm_edit.sql`), its routes and how DELETE cleans
up, with the shared layer in [verse/edit/edit.coax](../../verse/edit/edit.coax).

## Dependencies

[Chapter 5](ch005-replay-a-session-exactly.md) (replay),
[chapter 6](ch006-entities-are-born-and-die.md) (stable IDs),
[chapter 15](ch015-ui-and-the-hud.md) (panes) and
[chapter 51](ch051-a-realm-is-a-document.md) (the document).
