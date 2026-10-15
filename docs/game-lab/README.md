# Game Lab: change games by talking

Status: MVP, 2026-10-14 (#11616). Owner request: "make an equivalent UI for
Game Lab, like I can conversationally change the game, example being our ice
cream man game, locally first".

Game Lab is [Video Lab](../video-lab/README.md) for games. It is not a new
app. It is a set of chat tools, a small local service and a card that every
OpenAgents chat can show. You say "make the ice cream man 20% slower and give
the player two more hiding spots". The chat changes the game's document and
the bot plays it to check it can still be won. The new version then plays
right in the conversation, next to the numbers that changed.

The first game is [The Ice Cream Man](../verse/games/the-ice-cream-man.md).

## What already exists (and is reused, not rebuilt)

| Piece | Where | What it gives Game Lab |
| --- | --- | --- |
| The rules | `crates/icecream-rules` | A deterministic fixed-step run: the seeded house, the Ice Cream Man's state machine, tools and ammo, health, hiding, keys, the front door |
| The tuning | `crates/icecream-rules/src/tuning.rs` | Every number a playtest may change. They are now read at run time from a `Tuning` value, and the constants are its defaults |
| The bot | `crates/icecream-rules/src/bot.rs` | A player that collects the keys, fights back and escapes. Game Lab measures versions with it |
| The browser game | `crates/icecream-web`, `scripts/build-icecream-web.sh` | WebGL2 in the page. It now reads a version's numbers and words from the page address |
| Video Lab | `scripts/film/lab.py`, `crates/openagents-web/src/lab.rs` | The pattern: typed documents, versions with summaries and digests, `*_` tools offered to Gemini, an inline card, a CLI |
| Chat surfaces | the local web chat (`openagents-web`) | Where the conversation happens |

## The model: a game document, every edit a version

A game's editable surface is one typed JSON document, `game.v1`
([schema](schemas/game.v1.json)). For The Ice Cream Man its sections are:

- **layout:** the house a compare plays (`seed`; the card itself plays a
  fresh house every run), its size (`house_size`, rooms along each side),
  whether the keys go to its far ends (`key_spread`), and `extra_closets`, the
  hiding spots added beyond the rooms' own closets. A spare closet fits in
  bedrooms, kitchens and bathrooms, where it keeps the room's lanes and key
  spots clear. Every house has room for four.
- **player:** walk and sprint speed, jump, health and how it comes back.
- **man:** the Ice Cream Man's behaviour. This covers his speeds when he
  wanders, investigates, searches and chases, how far and how wide he sees,
  how long he searches, how often he sniffs you out, his grabs and
  sprinkle volleys, whether he cuts corners chasing (`corners`), and how
  many keys you hold before he guards the front door (`patrol_keys`).
- **weapons:** the four earned weapons (#11626), switched on by
  `earned_weapons` (1): the candy cane sword, the waffle cone spear, the
  cone pistol and the sugar launcher. Each one's damage, reach or range,
  rate, ammo and fridge refills, the Ice Cream Man's health against them
  (`man_health`: at none he melts, which wins the run), and what earns
  each one (`sword_task`, `pistol_task`, `sugar_task`).
- **minions:** the mini Ice Cream Men that hunt you (`minion_count`, 0 for
  none; `minion_growth`, one more allowed every so many seconds of a
  run): when they come, their speed, health, bite and sight, what fudge
  and farts do to them, the chance a melted one drops an edible cone
  (`cone_drop`), the cone's extra health and how long it lasts
  (`cone_boost` 0.5 and `cone_time` 10: 50% more for 10 s), and the
  sprinkle staff's long sprinkle and the friendly minions it makes.
- **look:** how yellowed the rooms are (`yellow`, 0 to 1), the fog
  (`fog_near`, `fog_far`), how often the lights flicker and how loud they
  buzz, and the camera a run starts with (`first_person`).
- **items:** hot fudge and the fart launcher. Ammo at the start and the most
  you can carry, fridge pickups, rate, range, how long fudge slows and
  freezes him, and how long a fart cloud confuses him. Also the key ring's
  jingle.
- **noise:** how far sprinting, fudge, fridges and the front door are heard.
- **rules:** `front_door` (`opens_with_all_keys`, or `locked`, which the
  rules refuse), `keys` (three; fixed in the code), `unlock` (seconds holding
  E at the door), `unlock_guard` (the unlock starts over while he is this
  near), `puzzle_key` (1: one key is locked in the cone cabinet behind the
  sprinkle-button puzzle), reach, and `free_roam` (1: after a win, play on
  with the sprinkle staff).
- **layout** also has `backrooms` (1: build the house as the Backrooms,
  mostly wide empty yellow rooms with wall stubs among a few of the
  Airbnb's own) and `loops` (doorways added beyond the ones that join every
  room).
- **copy:** the title, the tagline, and the lines when you escape or get
  scooped.

Each number has bounds and a plain-words description, generated from one
table in `tuning.rs`, so the schema, the validation and the game can't
disagree.

The defaults are the game the public page plays on Normal: since the
difficulty pass (#11667), version 5 of `the-ice-cream-man` (a 7 by 7
house with its keys at the far ends and one behind the button puzzle, a
faster Ice Cream Man who guards the door, a 6-second unlock he
interrupts). Version 4, the designer's revision with the Backrooms, plays
as Easy, and Hard is harder again; the game's pause menu picks one
(`icecream_rules::tuning::Difficulty`, measured in
[the game's design doc](../verse/games/the-ice-cream-man.md)). A version
saved before the code gained a setting is read with that setting as the
original game had it (`Tuning::original`, via `doc::upgrade`), so
version 1 still plays and measures exactly as it did, and version 4 still
plays as version 4.

The card plays a fresh house every run (its fragment has no seed), so a
version's route can't be learned by heart. A compare plays both versions
on the version's seed (`layout.seed`), the same house for both.

Every change makes a new **version**: the document, a one-line summary in
plain words, who made it, a digest, and the bot-run stats measured when it
was saved. Version 1 is imported from the game as it plays today. Versions
are kept, so "go back to v1's speeds" is one patch and comparing is free.
They live privately under the Lab folder, beside Video Lab's films, and never
in git:

```
~/work/film-out/lab/games/the-ice-cream-man/vN/{doc.json,version.json}
~/work/film-out/lab/games/the-ice-cream-man/notes.json
~/work/film-out/lab/games/builds/<code digest>/
```

## Talking to it: the chat tools

The chat changes games only through typed tools. The chat's model chooses
which tool runs from the tools it is offered, never from matching words in
the message (workspace rule: no ad-hoc keyword routing).

| Tool | Does |
| --- | --- |
| `game_list` | The games in the Lab |
| `game_open(game, version?)` | The version's whole document, its bot-run stats, and the card |
| `game_play(game, version?)` | The card, ready to play |
| `game_edit(game, patch, summary, version?)` | A JSON Patch on the document, as a new version. The only write |
| `game_versions(game)` | Each version's summary, author and bot win rate |
| `game_compare(game, a?, b?)` | Two versions side by side: what changed, and the measured differences over the same seeded bot runs. A version the game doesn't have, or a version with itself, is refused |
| `game_notes(game, version?, text?, seconds?, room?)` | Reads the notes, or saves one at a time and place in a run |
| `game_propose_code(game, request, version?)` | Records a change that needs the game's code changed, for Coder, once the owner agrees. It records; it doesn't change the game |
| `game_code_requests(game)` | Each recorded code request and where it stands: waiting for Coder, or made in a commit, with its link |

`game_edit` checks an edit in three steps before anything is saved:

1. **The schema.** Every section and value must exist and stay within its
   bounds. Whole numbers must be whole and words must fit on screen. A value
   the game doesn't have, like `rules.door_locked`, is refused. The reason
   says that such a change needs the game's code changed, and offers Coder.
2. **The rules.** These are the checks across values. A locked front door
   means nobody could ever escape, since the door is the only way out.
   Sprinkle volleys need a range where he'll fire. You can't start with more
   ammo than you can carry. The earned weapons need minions (the sword and
   the sugar launcher come from melting them), and the fog must start
   nearer than where it's thickest.
3. **The bot.** It plays 24 seeded houses with the new numbers, for up to 10
   minutes each. A run is won by escaping or by melting the Ice Cream Man.
   If it never wins, the edit is refused as unwinnable, with the before and
   after counts.

A refused edit says why in plain words, and the chat tells the owner in its
own words.

## Seeing it: the card

Every Game Lab answer in the web chat has a card under it. The card shows:

- **The game itself** in a frame. This is the browser build of The Ice Cream
  Man, playing this version.
- **Play**, which starts a run straight away, without the title card.
- The version's name and summary.
- **The bot-run stats:** escapes, the middle escape time, hits taken, times
  spotted, and hiding spots.
- **Compare with the original**, which turns the card into two frames side by
  side. The bot plays both on the same house, and the measured differences
  and the changed values are listed underneath.
- **Note at this time and place**, which saves a note with the run's time and
  the room you're in. The game publishes both on its canvas.
- **Play in its own window**, the Lab's own play page.

## Building per version: data, not code

Versions are data on one build of the game. The card's frame is the game's
page with the version after the `#`:
`seed=7&t.man_chase=4.48&t.extra_closets=2&c.title=...`. The game reads it
when it starts, so a new version plays as soon as it's saved, with nothing to
compile.

The Lab builds the game's WebAssembly once for each digest of the game's
code (`icecream-rules`, `icecream-web`, the build script). Only a change to
that code builds again. The first build after a code change takes a few
minutes, and the card says the game is getting ready until it's done.

A request that needs new rules code is a code change, not an edit. Examples
are a fourth key, a new item, or a door that opens another way. The edit is
refused with a reason that says so, and the chat offers to have Coder make
the change as a separate step.

When the owner agrees, the chat records it with `game_propose_code`:
`ROOT/GAME/code-requests.json` keeps the request in plain words, the version
it was about, and an id (`r1`, `r2`, ...). Nothing is built or changed by
recording one. Coder (or an agent working as Coder) makes the change the
usual way, as a commit with tests, and the commit carries a line naming the
request:

```
Game-Lab-Request: the-ice-cream-man/r1
```

When that commit is in the checkout the Lab builds from, `game_code_requests`
says the request is made and links the commit. The new code's settings are
in the document from then on (off by default), so turning them on is a
`game_edit` like any other, and the next version plays as data again on the
one new build. The designer's revision (#11626) went this way: request `r1`
recorded in the chat, the commit that made it, then versions that switched
the weapons, minions, free roam and the Backrooms on.

## Where it runs

- **The Game Lab service** is `crates/game-lab` (`game-lab serve`) on
  `127.0.0.1:8793`. It holds the documents, versions, notes, bot runs and
  build, and serves the tools at `/api/tools` and `/api/tool/{name}`, the
  card at `/api/card`, the build at `/build/{file}` and its own play page at
  `/play`. It binds to loopback only.
- **The web chat** (`crates/openagents-web/src/game_lab.rs`,
  `crates/openagents-web/src/labs.rs`). With `OPENAGENTS_GAME_LAB` set to a
  loopback URL, one chat turn gives Gemini 4 Argon (Vertex, global) both
  Labs' tools together (`game_*`, Video Lab's `lab_*`) plus `general_chat`,
  with the chat's current Lab document as context: the last card it showed,
  its kind and id. A follow-up ("compare with the original") goes to that
  document's Lab by the model's typed choice, never by matching words
  (#11619). Every game tool but `game_list` needs the game named: a call
  without one, or naming a Video Lab document, is refused and returns no
  numbers, and a compare of versions the game doesn't have is refused
  plainly. Whatever is about neither Lab goes on to the general chat. The play page
  (`/chat/{id}/game/play`) and the build (`/chat/game/build/{file}`) are
  served through the site. Only this site may frame them, and the chat
  page's policy allows same-origin frames.
- **The terminal:** `openagents game open|play|edit|versions|compare|notes`
  (`crates/openagents-cli/src/game.rs`). It starts the service when it isn't
  running.
- **The local stack:** `scripts/dev/full-local.sh start --lab` starts Game
  Lab beside Video Lab. `--game` starts Game Lab alone.

## Slices

1. **MVP (this):** `game.v1` for The Ice Cream Man, versions, the tools,
   checked edits with bot runs, the web card, the CLI, and the local stack.
2. **Coder hand-off:** "make that change" from a refused code-level request
   is recorded (`game_propose_code`) and linked to the commit that makes it
   (`game_code_requests`, #11626). Still to come: opening the Coder task
   from the chat itself, with the request, the document and the notes.
3. **Coder tools:** `game_*` in Coder's tools beside `lab_*` (as
   `crates/coder-new/src/video_lab.rs`), answering in words with the play
   link.
4. **More games:** a `game.v1` adapter per game (Grow Little Bunny next),
   each mapping its document onto its own rules crate.
5. **Better measures:** more bots (one that hides, one that fights), more
   seeds, and a playtest's own runs measured beside the bot's.
6. **Hosted:** the service behind the website's Lab tools for signed-in
   owners, with versions in the project's file store.
