# 52. Edit a realm in place

> **Status: unimplemented — design draft.** Every realm already loads one
> edit layer ([verse/edit/edit.coax](../../verse/edit/edit.coax)): the board,
> SETTINGS, visitor proposals and the asks list, with the server deciding
> roles. This chapter plans edits as typed, invertible operations over the
> realm document. Nothing here runs yet.

Editing a realm while people walk in it is the same problem as replaying it:
each change must be a recorded, ordered operation over the state, applied the
same way everywhere, and able to come back out. An owner switches a row, a
visitor proposes a crate, an admin asks by voice for snow; each is an
operation with an author, a target and an inverse.

## What this chapter teaches

- Edits as operations on chapter 51's document, each with its inverse.
- Roles decided by the server, never by the client: owner, admin, visitor.
- Proposals as pending operations, accepted or declined exactly once.
- An asks ledger whose DELETE reverts only what is still as the ask left it.

## The method

An operation is `(id, author, kind, target, before, after)`. Applying it
checks that the target still holds `before`; its inverse swaps the two:

```jai
EditOp :: struct { id: u64; author: Key; kind: OpKind; target: Target; before: Value; after: Value; }
apply   :: (doc: *RealmDoc, op: EditOp) -> ApplyStatus;   // refused if target != before
inverse :: (op: EditOp) -> EditOp;                          // before and after swapped
```

Undo is applying the inverse; a log of operations replays to the same
document (chapter 5's rule, over edits instead of inputs). Placed things keep
the stable entity IDs of chapter 6, so moving or removing one names it, not
its position in a list.

Reverting an ask follows one rule: for each row it changed, if the row still
holds the ask's `after`, put back `before`; if a later ask changed it since,
leave it, because that change owns it now. An ask the code cannot do yet
waits (NOT APPLIED) until an agent builds it and marks it done with the
pushed commit.

The model that turns "make it snow" into a change answers under a schema
whose enums are this realm's row ids; no words are matched.

## What exists today

- [docs/verse/realm-edit.md](../../docs/verse/realm-edit.md): who may edit
  (`verse.edit_role`, `0050_realm_edit.sql`), the routes (`/switch`,
  `/revise`, `/settings`, `/place`, `/proposals`, `/panes`, `/lore`,
  `/quest`, `/asks`), what each realm can switch, and how DELETE cleans up.
- [verse/edit/edit.coax](../../verse/edit/edit.coax): `RealmEdit` and its
  hooks (`redit_start`, `redit_step`, `redit_key`, `redit_pointer`,
  `redit_hud`, `redit_take_action`, `redit_row_on`, `redit_tool`,
  `redit_near`), with checks in `verse/edit/edit_test.coax`.
- Everglade's own edit mode (`apps/openagents-web/everglade-edit.coax`):
  the hotbar, UNDO, REDO, ORIGINAL, picking and moving things, voice edits.
- Each hand-built realm's `edit` block in `apps/openagents-web/realms.json`
  (rows, tools, things), checked against `verse.realm_rights` by
  `test_realm_rights.py`.

Undo exists only in Everglade's mode; elsewhere a row's history is the asks
ledger in SQL. The two are not one model.

## What it builds

- `aethyr/forge/edit.coax`: `EditOp`, `apply`, `inverse`, the edit log and
  its replay, proposals as pending operations.
- The asks ledger's revert rule as a Coax function the server's SQL is
  checked against.
- Everglade's UNDO/REDO and the shared layer on the same log.

## Acceptance checks and oracles

- For every operation kind, `apply(inverse(op))` after `apply(op)` restores
  the document byte for byte.
- An operation whose `before` no longer holds is refused, never applied over
  someone else's change.
- Replaying a realm's edit log from its first document reproduces the
  current document; native and browser agree.
- Revert: an ask's rows still as it left them go back; a row changed again
  since is left alone (the cases in `realm-edit.md`, as fixtures).
- A visitor's placement with BUILD off is refused by the server; with BUILD
  on it waits as one card, decided once.
- `verse/edit/edit_test.coax` keeps passing through the move.

## Dependencies

[Chapter 5](ch005-replay-a-session-exactly.md) (replay),
[chapter 6](ch006-entities-are-born-and-die.md) (stable IDs),
[chapter 15](ch015-ui-and-the-hud.md) (panes) and
[chapter 51](ch051-a-realm-is-a-document.md) (the document).
