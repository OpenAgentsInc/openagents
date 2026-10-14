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

- **layout:** the house the card plays (`seed`), and `extra_closets`, the
  hiding spots added beyond the rooms' own closets. A spare closet fits in
  bedrooms, kitchens and bathrooms, where it keeps the room's lanes and key
  spots clear. Every house has room for four.
- **player:** walk and sprint speed, jump, health and how it comes back.
- **man:** the Ice Cream Man's behaviour. This covers his speeds when he
  wanders, investigates, searches and chases, how far and how wide he sees,
  how long he searches, how often he sniffs you out, and his grabs and
  sprinkle volleys.
- **items:** hot fudge and the fart launcher. Ammo at the start and the most
  you can carry, fridge pickups, rate, range, how long fudge slows and
  freezes him, and how long a fart cloud confuses him. Also the key ring's
  jingle.
- **noise:** how far sprinting, fudge, fridges and the front door are heard.
- **rules:** `front_door` (`opens_with_all_keys`, or `locked`, which the
  rules refuse), `keys` (three; fixed in the code), `unlock` (seconds holding
  E at the door), and reach.
- **copy:** the title, the tagline, and the lines when you escape or get
  scooped.

Each number has bounds and a plain-words description, generated from one
table in `tuning.rs`, so the schema, the validation and the game can't
disagree.

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

`game_edit` checks an edit in three steps before anything is saved:

1. **The schema.** Every section and value must exist and stay within its
   bounds. Whole numbers must be whole and words must fit on screen. A value
   the game doesn't have, like `rules.door_locked`, is refused. The reason
   says that such a change needs the game's code changed, and offers Coder.
2. **The rules.** These are the checks across values. A locked front door
   means nobody could ever escape, since the door is the only way out.
   Sprinkle volleys need a range where he'll fire. You can't start with more
   ammo than you can carry.
3. **The bot.** It plays 24 seeded houses with the new numbers, for up to 10
   minutes each. If it never escapes, the edit is refused as unwinnable,
   with the before and after counts.

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
refused with a reason that says so. The chat offers to have Coder make the
change as a separate step. Coder does this the usual way, with a branch and
tests. The next version then builds once and plays as data again.

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
   opens a Coder task with the request, the document and the notes, and the
   next build follows from its merge.
3. **Coder tools:** `game_*` in Coder's tools beside `lab_*` (as
   `crates/coder-new/src/video_lab.rs`), answering in words with the play
   link.
4. **More games:** a `game.v1` adapter per game (Grow Little Bunny next),
   each mapping its document onto its own rules crate.
5. **Better measures:** more bots (one that hides, one that fights), more
   seeds, and a playtest's own runs measured beside the bot's.
6. **Hosted:** the service behind the website's Lab tools for signed-in
   owners, with versions in the project's file store.
