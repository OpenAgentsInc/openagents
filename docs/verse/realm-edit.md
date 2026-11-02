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

An owner's or admin's page reads the spec again every second (`REDIT_WATCH`), so a row switched
from the CLI (`openagents realm switch SLUG ROW off`), Coder or another tab shows up in about a
second. An unchanged spec changes nothing (`redit_same_spec`).

Spoken and typed commands about the editing screen go to `redit_heard(ed, word, row, on, pane,
out)` (migration 0104's `switch` and `show` edits): it switches one of the realm's rows by its id,
or opens EDIT's board, SETTINGS or the cards, or closes them, and writes the sentence to show and
speak. Every realm with the layer gets it by handing the selector's answer there. The server's
selector lists the realm's own rows from `verse.realm_rights`, so a new row is switchable by voice
with no new migration.

A question about what's on screen ("what's this dark strip along the sky?") is the selector's
`look` (migration 0105). The page asks again at `POST /api/playtest/realm/<slug>/look` with the
same words and HTTP flag 32 (`coax/web/HTTP.md`): the host adds the next frame drawn, as base64
JPEG at most 1024 wide, to the body's `image`. Gemini answers from the picture in one to three
spoken sentences, `200 {edit: "chat", reply}`, shown and spoken as any chat answer. Looks are paced
and counted with voice edits; the picture is never stored. Any page can send flag 32 on a request
of its own.

## The realm's voice

Every realm with rows takes its owner's and admins' voice, not only Everglade (migration 0106,
openagents#12643). `POST /api/playtest/realm/<slug>/edit {sentence}` serves any realm with
`verse.realm_rights` rows or a made realm (its rows are its board: `verse.edit_made_board`, keyed
by `verse.edit_key` as the board is). A realm without a voice catalogue gets the shared verbs
only, under a typed enum:

| Verb | Fields | What the page does |
| --- | --- | --- |
| `switch` | `row`, `on` | switches the row (`redit_heard`, then `POST .../switch` as a tap does) |
| `show` | `panel`: edit, settings, ideas, close | opens or closes that pane; one already open stays open |
| `look` | | asks again at `.../look` with the frame on screen |
| `ask` | | puts the words on the realm's list, as FEATURES' ADD does (`POST .../asks`) |
| `chat` | `reply` | the answer, said as it is |

A realm with a catalogue (Everglade) keeps its own verbs and prompt; `ask` is only for a realm
without one. A fork speaks with its base's catalogue and switches its own rows. Who may speak is
checked as for edits: a site admin, or the realm's owner or admins by `X-Verse-Player`
(`verse.edit_may`, made realms included).

The edit layer does the whole trip, so a realm only wires it:

- `redit_hear(ed, words)` sends words said or typed to the realm itself (not to a character).
  It returns false for a visitor, whose words stay the realm's own.
- The answer is handled in the layer (`redit_heard_answer`). Every answer, a refusal too, is shown
  under EDIT for ten seconds (and in an open pane's note) and spoken as action 9.
- `redit_hears(ed, on)`, each frame, offers the voice under EDIT with the panes shut: V held or
  HOLD TO TALK on a page with a voice, and Enter to type. Pass false while a character talk has the
  keys.

Wired today:

| Page | How words reach the selector |
| --- | --- |
| Everglade | its own HOLD TO TALK (everglade-voice.coax), with its catalogue's verbs |
| Haunted House (and every page on realm.coax) | VOICE, while you face no one: the page faces the realm (`REDIT_FACING`) for its owner and admins, and `realm_hear` hands the words over |
| A made realm (myrealm.coax) | V, HOLD TO TALK under EDIT, or Enter, outside a character talk (`redit_hears`) |

EDIT opens one pane at a time (the panes rule, verse/fairy-forest/README.md "Making the world:
panes"): FEATURES (the list below; HOLD TO TALK on a page with a voice, typing everywhere),
SETTINGS (what visitors get; a realm whose code honours no visitor tool says so), and N WAITING
(the first card: who, what, where, the count before and after, ACCEPT and DECLINE).
A realm whose face carries the wire also gets COMPUTER (below, "Your computer").

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
| Everglade | flock, floats, undergrowth, daycycle, guards (off: the watch never gives chase), invincible (off by default; on: the owner and admins take no harm, visitors still do) | map, build (BUILD: edit mode's things, each a card) | crate, barrel, bench, fence, stall |
| The Grid (the homepage's module walked, `newland-walk.coax`; 0082) | walkers (everyone else walking it), names (each project's name over its tree); EDIT left of HOME, under GALAXY on a phone | | |
| Hecate's Cemetery | flight (off: a visit starts at the gate), fireflies, trail, tips, others | | |
| Fairy Forest | sparkles, grass, ridges | fly (her wing beats in the air) | |
| Dead Gears | alarms, radar, sparks (EDIT pauses the fight) | | |
| Ice Cream Man | minions, weapons (at once: minions off melts the enemy ones), backrooms (from the next house); EDIT shows while a card is up | | |
| Haunted House | others, sounds, scares, chat; on a phone the open pane has the screen to itself | map (the big map), voice (VOICE) | |
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
| `POST /asks/doc {name, data}` | a design document: a line for each thing it asks for (below) |

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

ADD FROM A DOCUMENT (`coax/web/docs.js`: action 31 opens the picker for this realm's
`/asks/doc`; message 54 brings the answer) sends a design document, .md, .txt, .pdf or .docx, at
most 360 KB: `POST /asks/doc {name, data}` (migrations/0080_realm_docs.sql). The server takes its
text out (a .docx's `word/document.xml`, inflated with zlib; a PDF goes to the model as itself),
and the model answers the board as an ADD does, naming every feature or change the document asks
for, each with the feature (or row) that makes it. Each becomes its own line on the list: APPLIED
with just what it changed (so its DELETE takes back only that), or NOT APPLIED, waiting for code.
What the answer changed that no line names is one more line ("The rest of NAME"); a document that
named nothing still waits as one line. The answer: `doc N A applied W waiting`, then the rows.

`verse/SKILL.md` ("Asks waiting on code") is how an agent works the waiting ones.

## Your computer

COMPUTER, the fourth pane, lets the realm's owner build it with their own Coder, on their own
computer, with their own Claude Code or Codex login (openagents#12640, #12642). Made realms
(`myrealm.coax`) have it; another realm gets it by calling `redit_computer(ed)` and linking the
wire into its face as `myrealm-face.c` does. It works the same on a desktop and a phone browser.

What the owner sees, one view at a time:

| View | What it says and does |
| --- | --- |
| Unpaired | "On your computer, run:" `coder host` (COPY), then a field for the key it shows, and PAIR. |
| Asking | "Allow this browser on your computer", with `coder host allow` (COPY) and this browser's name ("Chrome on Mac"). CANCEL takes the ask back. |
| Allowed | Whether the computer answers, a field for a request, SEND, and FORGET THIS COMPUTER (a second tap within 4 s). |
| Denied, expired | What happened, ASK AGAIN, or another computer. An ask lasts ten minutes. |
| Not answering | "Your computer isn't answering. On it, run:" `coder host`; a request already sent waits. |
| Running | What was asked, the job's steps as they come, and STOP. |
| Stopped, failed, done | What was asked, its steps, the reason a job failed, and NEW REQUEST. |

The realm reloads by itself as the job's edits land.

How it works:

- `verse/edit/computer.coax` is the pane's state and nothing else (checks:
  `coax/bin/coax test verse/edit/computer_test.coax`). `rc_next_op` names the one request to send
  next; `rc_reply` reads the computer's answers as plain lines from the wire.
- `verse/edit/computer-wire.coax` is its own wasm unit, linked beside the realm (its signing tables
  are computed at compile time, so `build.sh` builds `myrealm.wasm` with
  `COAXIAL_RUN_STEPS=250000000`). It keeps this browser's key, signs NIP-42 AUTH for
  `wss://relay.openagents.com`, and seals each request with the NIP-HOST browser codec
  (`coax/lib/nostr/host/browser.coax`) as a kind 3188 private artifact for the computer, NIP-44
  encrypted, after subscribing to its reply's mailbox. A reply counts only when it opens for this
  key, is signed by the computer the owner pasted, names our request and event, and answers our
  operation. Asking the same thing again within 50 s resends the exact signed event. Checks:
  `coax/bin/coax test verse/edit/computer-wire.coax --library --run-steps 250000000 --only "wire: "`.
- `apps/openagents-web/computer-face.h` moves the bytes: the record in IndexedDB (namespace
  `openagents.computer`, key `browser`: the browser's secret key, the grant, the computer's key and
  the ask), the relay socket, the browser's random bytes and the clock. The socket opens only for an
  owner or admin, once COMPUTER is open or the browser is paired.

The operations are NIP-HOST "Browser device grants" (`nostr/nips/openagents/NIP-HOST.md`):
`browser.request {key, label}` to pair, `browser.status {request}` while asking and, once allowed,
every 20 s to check that the computer answers; `thread.run {text, realm}`, `thread.read {thread,
after}` every 1.5 s while the job runs, `thread.stop {thread}`, and `browser.forget`. Only the person
at the computer allows a browser; the browser never makes a code. A request unanswered for 15 s
means the computer isn't answering. A refusal that says the browser's grant is gone (revoked,
stale, expired) pairs again.

To try it against a computer on this machine, open the realm with `?relay=ws://localhost:PORT`. The
wire takes only that form; any other `relay=` keeps `wss://relay.openagents.com`. The headless check
plays the whole flow in Chrome (no window opens) at 1440×900 and 390×844: pair, allow, a job's steps,
a reload, and FORGET. It is a Coax tool on the DevTools library (`coax/web/computer-flow.coax`). It
serves the realm from staging with this checkout's module and an owner's spec swapped in, holds the
page's writes in the browser, and reads the pane through `coaxial.guardProbe(2400 + f)`
(`redit_probe`: the buttons' boxes, the pane, the view). Run it from the repository's root:

    coax/relay/build.sh /tmp/relay && /tmp/relay --listen 127.0.0.1 --port 7777 --url ws://localhost:7777
    OPENAGENTS_NIP_HOST_RELAY=ws://localhost:7777 CODER_HOME=/tmp/host-home coder/tui/coder.sh host --engine echo
    COAXIAL_RUN_STEPS=250000000 coax/web/wasm.sh apps/openagents-web/myrealm.coax apps/openagents-web/myrealm-face.c \
      /tmp/myrealm.wasm verse/edit/computer-wire.coax coax/lib/crypto/entropy.coax
    CODER_HOME=/tmp/host-home BROWSER=/usr/bin/true coax/web/tool.sh computer-flow ws://localhost:7777 /tmp/myrealm.wasm [SHOTS_DIR]
