# Every realm edits the same way

Build your own realm ([realm-builder.md](realm-builder.md)) gave a made realm a board, its owner's
SETTINGS and the ACCEPT/DECLINE cards. Every realm now has them: Everglade, Hecate's Cemetery, the
Grid, Fairy Forest, Dead Gears, Ice Cream Man, the Haunted House and every made realm. If you can
edit a realm, EDIT opens the same layer in each one.

## Who may edit

The server decides, for any realm slug (`coax/web/api/migrations/0049_realm_edit.sql`,
`verse.edit_role`):

- **owner**: a made realm's maker (its player key or account), or a hand-built realm's owner account.
- **admin**: a site admin (`admins.conf`, logged in at /login or signing with NIP-98; `realms.c`
  asks `admin_who` and passes the answer to SQL), or an account in the realm's admins.
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
panes"): THE BOARD (a checkbox a row; HOLD TO TALK on a page with a voice, TYPE IT everywhere),
SETTINGS (what visitors get; a realm whose code honours no visitor tool says so), and N WAITING
(the first card: who, what, where, the count before and after, ACCEPT and DECLINE).

## What each realm can switch

Only what its code really switches; a feature that something else needs (Everglade's people and
quest board, a game's creatures) is not offered.

| Realm | Rows | Visitor tools | Placed |
|---|---|---|---|
| Everglade | flock, floats, undergrowth, daycycle | map, build | crate, barrel, bench, fence, stall |
| The Grid | robots, names | | |
| Hecate's Cemetery | flight, fireflies, trail, tips, others | | |
| Fairy Forest | sparkles, grass, ridges | fly | |
| Dead Gears | support, alarms, radar, sparks | | |
| Ice Cream Man | minions, weapons, backrooms | | |
| Haunted House | others, sounds, scares, chat | map, voice | |
| a made realm | its features (the builder's catalogue) | build, fly, jump, map, talk, voice | the builder's things |
