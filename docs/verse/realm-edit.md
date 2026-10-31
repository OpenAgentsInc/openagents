# Every realm edits the same way

Build your own realm ([realm-builder.md](realm-builder.md)) gave a made realm a board, its owner's
SETTINGS and the ACCEPT/DECLINE cards. Every realm now has them: Everglade, Hecate's Cemetery, the
Grid, Fairy Forest, Dead Gears, Ice Cream Man, the Haunted House and every made realm. If you can
edit a realm, EDIT opens the same layer in each one.

## Who may edit

The server decides, for any realm slug (`coax/web/api/migrations/0050_realm_edit.sql`,
`verse.edit_role`):

- **owner**: a made realm's maker (its player key or account), or a hand-built realm's owner account.
- **admin**: a site admin (`admins.conf`: logged in at /login, signed in at /api/admin/login,
  whose cookie reaches /api/playtest too, or signing with NIP-98; `realms.c` asks `admin_who` and
  passes the answer to SQL), or an account in the realm's admins.
- **visitor**: anyone else.

Each hand-built realm in `apps/openagents-web/realms.json` has an `edit` block: `owner` (an
account), `admins` (accounts), `tools` (the visitor tools its code honours), `things` (what may be
placed there) and `rows` (what its code can switch: `id`, `label`, `says`). The migration seeds
`verse.realm_rights` from it; `test_realm_rights.py` fails when the two disagree. A new hand-built
realm, or a new row, takes a migration that adds or updates its row.

Owners and admins switch rows, change the settings, place things at once and settle cards. A
visitor's thing, while BUILD is on, waits as a card. Everyone else gets 403.

## The routes

All under `/api/playtest/realms/<slug>`, for a made or a hand-built realm:

| | |
|---|---|
| `GET` | the spec, a line each: `role`, `owner 0/1`, `tool NAME 0/1`, `thing NAME`, `row KEY 0/1 LABEL`, `placed THING X Z YAW`, `waiting N` (owners and admins) |
| `POST /switch {row, on}` | one row on or off, for everyone |
| `POST /revise {prompt}` | talking or typing revises the board: the model answers under a schema whose enums are this realm's row ids (a made realm: the builder's catalogue), and its answer is the realm now. `?as=lines` answers the rows as lines |
| `POST /settings {tools}` | the tools visitors get (only those the realm honours) |
| `POST /place {thing, x, z, yaw}` | an owner's or admin's goes in; a visitor's becomes a card |
| `GET /proposals`, `POST /proposals/<id>/accept\|decline` | the cards |
| `GET /panes` | everyone: `lore WHO\|TEXT` and `quest GIVER\|ASK\|STEP\|STEP\|STEP\|REWARD`, a line each |
| `POST /lore {who, text}` | a character's lore (400 characters; `""` forgets it) |
| `POST /quest {giver, ask, steps, reward}` | the quest a character gives, one each (`""` ask takes it away) |

A made realm keeps the rows switched off in `made_realms.off`, so they can come back on.
Revisions are rows in `verse.realm_plans` (with its `slug`), under the builder's daily cap and its
one-a-player-every-2-seconds pace. No words are matched anywhere: the model picks under a schema.

## The layer

`verse/edit/edit.coax` (checks: `coax/bin/coax test verse/edit/edit_test.coax`). A realm keeps one
`RealmEdit` (`ed: [1] RealmEdit`) and wires small hooks:

- each frame: `redit_start(ed, SLUG)`, `redit_step(ed, time, width, height, dpr)`;
- input: `redit_key` (0 not ours, 1 used, 2 typed text for the host), `redit_pointer`;
- drawing: `redit_hud(ed, out, at, anchor)` draws EDIT in the box the realm gives and the open pane
  under it; the realm draws `plate_box[0..<plate_n]` with its own `plate`;
- actions: `redit_take_action(ed, outbox)` first in the realm's action (push to talk);
- the C face: `edit-face.h` (`edit_face_drain`, `edit_face_message`).

The realm reads `redit_row_on(ed, "robots")` where its code draws the thing, `redit_tool(ed,
"map")` for a visitor's tool, `placed_*` for what was accepted, and `redit_changed(ed)` when the
server's answer changed it.

EDIT opens one pane at a time (the panes rule, verse/fairy-forest/README.md "Making the world:
panes"): FEATURES (the list below; HOLD TO TALK on a page with a voice, typing everywhere),
SETTINGS (what visitors get; a realm whose code honours no visitor tool says so), and N WAITING
(the first card: who, what, where, the count before and after, ACCEPT and DECLINE).

The panes for what you work on (ff-panes, migration 0055): a realm says, each frame, which of its
characters you are by (`redit_near(ed, key, name)`). With EDIT open, coming to one brings up its
LORE (written, SAVE, and it comes back the next time); QUEST turns that pane into the form (ASK,
STEP 1-3, REWARD) and SAVE QUEST makes the quest. A realm that plays them calls
`redit_read_panes_too(ed)` and reads `redit_lore(ed, key)`, `redit_quest(ed, key)` and
`redit_quest_part(ed, q, part)`. Fairy Forest is the first (apps/openagents-web/fairyquest.coax):
its creatures know their lore when you talk to them, and give their quests when you come near.

## What each realm can switch

Only what its code really switches; a feature that something else needs (Everglade's people and
quest board, a game's creatures, Dead Gears' support engineer, whom the fight's rules reach for) is
not offered (migration 0052 took the engineer's row back off).

| Realm | Rows | Visitor tools | Placed |
|---|---|---|---|
| Everglade | flock, floats, undergrowth, daycycle | map, build (BUILD: edit mode's things, each a card) | crate, barrel, bench, fence, stall |
| The Grid | robots, names | | |
| Hecate's Cemetery | flight (off: a visit starts at the gate), fireflies, trail, tips, others | | |
| Fairy Forest | sparkles, grass, ridges | fly (her wing beats in the air) | |
| Dead Gears | alarms, radar, sparks (EDIT pauses the fight) | | |
| Ice Cream Man | minions, weapons (at once: minions off melts the enemy ones), backrooms (from the next house); EDIT shows while a card is up | | |
| Haunted House | others, sounds, scares, chat | map (the big map), voice (VOICE) | |
| a made realm | its features (the builder's catalogue) | build, fly, jump, map, talk, voice | the builder's things |

## Everglade

Everglade's own edit mode (everglade-edit.coax: the hotbar, UNDO, REDO, ORIGINAL, picking and
moving things, HOLD TO TALK voice edits) stays; the layer's EDIT stands where its EDIT was and opens
both, the board and SETTINGS under EDIT and the hotbar along the bottom. DONE closes both. The
board is typed there (TYPE IT): Everglade's HOLD TO TALK is its voice edits. The realm's owner and
admin accounts get edit mode as a site admin does, and the server lets them publish its world
(player.c: a hand-built realm's world routes). A visitor the owner lets build gets BUILD: the
hotbar's things only, each placed where an admin's would go and sent as a card; ACCEPT on the
owner's page puts the thing in the city and publishes it as an edit by hand. Its owner and admin
accounts make voice edits (HOLD TO TALK, `/api/playtest/realm/everglade/edit`, npc.c) as a site
admin does; theirs are paced and counted under their key's hash.

## What you asked for: the feature list

FEATURES, the first pane EDIT opens in every realm, is a bulleted list: first the board's rows (what
the realm's code switches; a tap on one switches it), then every ask its owner and admins made,
newest first. Each says APPLIED or NOT APPLIED (REMOVING while its code comes out), with DELETE
(tapped once it reads SURE?; a second tap within 4 s sends it). ADD (HOLD TO TALK, or typing) asks
for a new feature or a change. A phone gets the same list a page at a time (MORE).

The server keeps the list per realm, for its owner and admins only (`migrations/0078_realm_asks.sql`,
`verse.realm_asks`). An ask goes the way talking to the board always went: the model answers under
a schema whose enums are this realm's rows (a made realm's: the builder's catalogue), under the
builder's daily cap and pace, and no words are matched anywhere. What the answer changed is kept
with the ask, each changed row as it was before and after (and a made realm's ground and time).

| | |
|---|---|
| `GET /asks` | `ask ID applied\|waiting\|removing ask\|remove WORDS`, newest first; `""` for a visitor |
| `POST /asks {words}` | ADD: lines `ask ID applied\|waiting`, `plan N`, then the board's rows |
| `POST /asks/<id>/delete` | DELETE an ask: `{deleted, reverted}`, or `{state: removing}` for one built in code |
| `POST /rows/<key>/delete` | DELETE a board's row: `{deleted}` (a made realm), or `{ask, state: waiting}` (a hand-built one) |
| `POST /asks/<id>/done {commit}` | a site admin (an agent signing NIP-98): built in code, or its code taken out |

How DELETE cleans up:

- **An applied ask**: each row it changed that is still as it left it goes back to what it was
  before (a hand-built row's entry leaves `realm_edit.switched` when it had none; a made realm's
  feature it added leaves `features` and `off` altogether), and the ask is deleted. A row changed
  again since belongs to that later change and is left alone.
- **A waiting ask** (NOT APPLIED: the realm's code can't do it yet): it is deleted.
- **A made realm's row**: its feature is data, so it leaves the realm's `features` and `off` at
  once, and an ask whose only change it was goes with it.
- **A hand-built realm's row** is code: it is switched off now and a `remove` ask waits ("Take out
  ..."). The agent that takes its code out (and the row out of `realms.json` and `realm_rights`, in
  a migration) marks it done with the pushed commit, and the ask is gone. DELETE on that remove ask
  instead puts the row back on.
- **An ask built in code** (marked done with its commit): DELETE makes it REMOVING; its code comes
  out the way it went in (reverting that commit, with tests), and done with that commit removes it.

`verse/SKILL.md` ("Asks waiting on code") is how an agent works the waiting ones.
