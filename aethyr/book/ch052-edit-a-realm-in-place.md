# 52. Edit a realm in place

> **Status: partly implemented.** Moving a laid-out entity is a typed,
> invertible edit with undo and redo:
> [aethyr/forge/edit.coax](../forge/edit.coax), used by a made realm's page
> ([apps/openagents-web/myrealm.coax](../../apps/openagents-web/myrealm.coax))
> for MOVE A TREE, UNDO and SAVE. Switches, placements, proposals and the
> asks ledger still use the older edit layer and are not yet operations of
> this kind; see [Still to do](#still-to-do).

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
`trees:3`. The document keeps an override for each moved entity, in whole
centimetres, so the same document is the same bytes on every device:

<!-- source: aethyr/forge/edit.coax doc -->
```jai
FORGE_KEY :: 24;       // an entity's name: "trees:3"
FORGE_MOVES :: 64;     // moved entities one document keeps (the server's limit too)
FORGE_OPS :: 64;       // undo steps kept; the oldest goes first
FORGE_REACH :: 4000;   // centimetres from the middle an entity may stand (the server's limit too)

ForgeMove :: struct { key: [FORGE_KEY] u8; key_n: int; x: int; z: int; }   // where it stands instead, in cm
// One edit: the entity, where it stood before (or `had` false: at its laid-out place) and after.
ForgeOp :: struct {
  key: [FORGE_KEY] u8; key_n: int;
  had: bool; bx: int; bz: int;     // before
  has: bool; ax: int; az: int;     // after
}
ForgeDoc :: struct {
  rev: int;                        // the revision this document was read at, or last saved as
  moves: [FORGE_MOVES] ForgeMove; move_n: int;
  ops: [FORGE_OPS] ForgeOp; op_n: int; done: int;   // the log, and how many of it are applied
  saved: int;                      // `done` when it was last read or saved; -1 when that state is gone
}
```

`rev` is the revision the page read from the server, or the revision its last
save was kept as. `done` is how much of the log is applied, and `saved` is
the value of `done` when the server last kept the document.

## Operations and their inverses

An operation names its entity and where the entity stood before and after.
`had` false means the entity stood at its laid-out place. Applying an
operation is refused unless the entity still stands where the operation says
it stood:

<!-- source: aethyr/forge/edit.coax ops -->
```jai
// Apply an edit: refused unless the entity still stands where the edit says it stood.
forge_apply :: (d: [] ForgeDoc, o: [] ForgeOp) -> bool {
  key := cast(string) o[0].key[0..<o[0].key_n];
  now := forge_where(d, key);
  if now.found != o[0].had || (o[0].had && (now.x != o[0].bx || now.z != o[0].bz)) { return false; }
  return forge_set(d, key, o[0].has, o[0].ax, o[0].az);
}
// The edit that takes it back: before and after swapped.
forge_inverse :: (o: ForgeOp) -> ForgeOp {
  r := o;
  r.had = o.has; r.bx = o.ax; r.bz = o.az;
  r.has = o.had; r.ax = o.bx; r.az = o.bz;
  return r;
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

A save sends the revision the page read and every move:

<!-- source: aethyr/forge/edit.coax save -->
```jai
// The body of POST /api/playtest/realms/<slug>/save: the revision read, and every move in metres.
forge_save_body :: (d: [] ForgeDoc, buf: [] u8) -> int {
  n := forge_put(buf, 0, "{\"rev\":"); n = forge_put_int(buf, n, d[0].rev); n = forge_put(buf, n, ",\"moved\":[");
  for i: 0..<d[0].move_n {
    m := d[0].moves[i];
    if i > 0 { n = forge_put(buf, n, ","); }
    n = forge_put(buf, n, "{\"entity\":\""); n = forge_put(buf, n, forge_move_key(d, i));
    n = forge_put(buf, n, "\",\"x\":"); n = forge_put_cm(buf, n, m.x);
    n = forge_put(buf, n, ",\"z\":"); n = forge_put_cm(buf, n, m.z); n = forge_put(buf, n, "}");
  }
  return forge_put(buf, n, "]}");
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
| MOVE A TREE | the realm has trees | Picks the tree nearest you. A tap on another tree picks that one. |
| PUT IT HERE | a tree is picked | Stands it 2.5 m in front of you, as one undo step. Esc lets it go. |
| UNDO | there is a step to undo | Takes the last move back. |
| SAVE | there are unsaved changes | Saves from the revision read. |

On a 390 px phone the row spans the screen width and its labels are
shortened (MOVE TREE, PUT HERE).

## Checks

- `coax/bin/coax test aethyr/forge/edit.coax --library`: a move, its undo and
  redo; an undo of a second move puts back the first; an edit whose entity
  moved since is refused; apply then inverse restores the document; saved is
  a state of the log; the log and the document keep their limits; the save's
  body.
- `coax/bin/coax test apps/openagents-web/myrealm.coax`: MOVE A TREE moves
  only that tree's pieces and UNDO restores every piece; SAVE names the
  revision it was read at; a 409 keeps the changes unsaved, and reading the
  spec again does not drop them; a visitor gets none of the owner's
  controls; the row fits a 390 px phone.
- `coax/web/api/test.sh` (`test_realm_fork.py`): only the owner saves; a save
  from an older revision is refused with the revision there now; moves are
  validated and rounded to the centimetre.
- `coax/web/tool.sh forge-journey URL [--phone]`: the whole maker journey in
  headless Chrome, steps 2 to 6.

## Still to do

> **Design draft.** Nothing below runs yet.

- Row switches, revisions by voice, placements and visitor proposals as
  operations of the same kind, so that one log replays to the current
  document (chapter 5's rule, over edits instead of inputs).
- Undo and redo kept across reloads. Today the log lives in the page; a
  reload starts it empty at the saved revision.
- The asks ledger's revert rule as a Coax function the server's SQL is
  checked against: for each row an ask changed, put back `before` only if
  the row still holds the ask's `after`.
- Everglade's own edit mode
  (`apps/openagents-web/everglade-edit.coax`) on the same log.
- Moving entities other than trees, and turning or removing them.

The older edit layer is documented in
[docs/verse/realm-edit.md](../../docs/verse/realm-edit.md): who may edit
(`verse.edit_role`, `0050_realm_edit.sql`), its routes and how DELETE cleans
up, with the shared layer in [verse/edit/edit.coax](../../verse/edit/edit.coax).

## Dependencies

[Chapter 5](ch005-replay-a-session-exactly.md) (replay),
[chapter 6](ch006-entities-are-born-and-die.md) (stable IDs),
[chapter 15](ch015-ui-and-the-hud.md) (panes) and
[chapter 51](ch051-a-realm-is-a-document.md) (the document).
