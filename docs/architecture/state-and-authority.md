# Three kinds of state, three kinds of authority

Issue [#13431](https://github.com/OpenAgentsInc/openagents/issues/13431).
The rule is in [INVARIANTS.md](../../INVARIANTS.md), "Authoring never writes
durable state". This page says what each kind is, who decides it in each
realm, and how to add a realm. The idea comes from the Spawn analysis
([what to adapt](../research/spawn-what-to-adapt.md), N3).

## The kinds of state

| Kind | What it is | Where it lives | What writes it |
| --- | --- | --- | --- |
| Authored | What a maker wrote: the realm document, its edit log, content tables and their revisions | `metaverse-engine/editor/edit.coax`, `doc.coax`, `metaverse-engine/content/tables.coax`; `forge.realm_docs` on the server | An edit, undo, redo, a content rollback, a world's save or load, a publish |
| Running | What a session holds while it plays: the World and a page's state | `metaverse-engine/core/world.coax`; a page's state, kept across a code reload by `coax/web/state-keep.js`; a room's memory | A simulation step, a code reload, and an authoring operation applying what it authored |
| Durable | What outlives every session and belongs to a player: the character record and the item ledger | `verse.characters`; `verse.items`, `verse.inventories`, `verse.item_ops`; rules in `everglade/model/character.coax` and `metaverse-engine/session/ledger.coax` | Only the record's and the ledger's own routes, from a room-key-signed post |

The table as code is `state_op_writes` in
`metaverse-engine/editor/authority.coax`. What follows from it:

- Undo never gives an item back and never takes XP away.
- Rolling content back to an earlier revision changes what a creature drops
  from now on. It doesn't change what a player already holds.
- Loading a world replaces the running World. The players' records stay as
  they are.
- A code reload keeps running state and touches neither of the others.

## Who decides, per realm

`realm_authority(NAME)` in the same file is the table the editor and the API
read. `realm_authority_line` gives a row as `REALM|CONTENT|SIMULATION|DURABILITY`.

| Realm | Content: who may change authored state | Simulation: what decides running state | Durability: what holds durable state |
| --- | --- | --- | --- |
| `splice` | `owner-admins`: the realm's owner and the site's admins | `page`: the player's own page; nothing is checked by a server | `none`: a run is over when the page closes |
| `everglade` | `admins`: the site's admins | `room`: the server's room decides combat, movement, loot and travel; a page predicts and is corrected | `ledger`: the character record and the item ledger |

A realm with no row has declared nothing. `realm_op_may_write` refuses every
write for it, so a made realm can't be treated as having a durable store
until it declares one.

## How it is checked

`coax/web/tool.sh layer-check` prints a `durable` line and fails on any of:

1. An authoring or running context (`realm-authoring`, `engine`, `splice`,
   `realm-tools`, `realm-seat`) that can reach `characters` or `items`
   through `contexts.txt`'s "may load" lists.
2. A line in `metaverse-engine/layer-allowlist.txt` that excuses a load from
   one of them into either.
3. A non-Coax source they own that names a durable table (`verse.characters`,
   `verse.character_*`, `verse.inventories`, `verse.items`, `verse.item_*`),
   the rules' generated headers or `room_items`.

It runs in the pre-push check. There is no allowlist for it.

## Adding a realm

1. Add its row to `realm_authority` and its name to `REALM_AUTHORITY_REALMS`,
   with a check.
2. Write the same statement in the realm's README.
3. If it holds durable state, it uses the character record and the ledger.
   It doesn't add a second store ([reuse](../reuse.md)).
