# Aethyrforge: the realm creator (MVP spec)

Status: spec, 2026-11-02. The parts it builds on exist; the screen it describes does not yet.

Aethyr is the engine. **Aethyrforge** is the realm creator: the place where anyone
says what world they want, watches it appear, shapes it by voice, hand or agent,
and publishes it with a link. This spec defines the public MVP: what is on screen,
what it does, how the CLI and agents drive the same thing, and what has to be built.

It is also the subject of the next video (Episode 291). The video is made the way
the speed-run video's fourth draft was: an audio track, with screen recordings of
the real product cut to it. Every shot in that video is a real screen from this
spec, recorded headless at 1080p.

## The name

**Aethyrforge, one word.** It was decided with the engine's name
(docs/verse/engine-name.md); the code already uses it (`aethyr/forge/`, module
`aethyr.forge.edit`); one word is one search result, one URL and one wordmark.
On screen it is set in capitals, `AETHYRFORGE`, beside the power-symbol mark. In
speech it is "Aethyr Forge", and that is fine.

## What exists today (survey)

| Part | Where | State |
|---|---|---|
| Setup questions, prompt, board of features, MAKE IT | `/realms/new`, `apps/openagents-web/builder.coax` (1,930 lines), docs/verse/realm-builder.md | Live. Invite only (`GET /api/playtest/access` says `make 0` for the public). |
| The planner | `POST /api/playtest/realms/plan`, migrations 0048-0097 | Gemini under a schema built from the catalogue; 1-2 s a board; scored by `eval-builder.sh` (99% of asked things covered, 76% drawn as asked). |
| The catalogue | `verse.builder_catalogue()` | 20 features: land, buildings, water, trees, plants, lanterns, flocks, props, characters, houses, herds, mist, dwellings, village, enclosure, centre, layout, path, tools; 4 cameras, 3 styles, 6 grounds, 4 times, 4 palettes. |
| The made realm | `/realms/<slug>`, `myrealm.coax` (3,858 lines + shapes, site, things) | Live. Reads its spec once a second for its owner, so any edit shows within a second (4a0ba95164). |
| Editing in the realm | `verse/edit/edit.coax`: EDIT, FEATURES, SETTINGS, cards, lore, quests | Live for owners and admins. |
| Typed edits | `aethyr/forge/edit.coax` (one file, 15 KB) | Moves with their inverses, undo/redo, saves fenced by revision (409 on a stale one). The only code in `aethyr/forge/` today. |
| The voice | migrations 0104-0106 | Switch a feature, open a panel, ask for a thing, ask about the screen (the frame goes to Gemini). |
| The CLI and MCP | `openagents realm ...` (35 verbs), `openagents mcp` | `new`, `revise`, `switch`, `move`, `place`, `ask`, `undo`, `publish`, `fork`, `shot`, `play`/`look`/`go`/`say`, `apply FILE.jsonl`. Each verb is an MCP tool. |
| Your own Coder | "Your computer" panel, #12640-#12642 | In progress: the browser pairs with the owner's Coder over NIP-HOST, sends a job for the realm, streams its steps. |
| Publish and fork | `realm publish`, `realm fork`, migrations 0075, 0101-0102, 0107 | Revisions, visibility (private, unlisted, public), forks. A publish pins the engine build and assets with the revision (0107, `aethyr/forge/publish.coax`, #12629). |
| Asked for, not built yet | the feature list's waiting asks | Every thing the catalogue can't draw stays on the list for an agent to build. |

What is missing is not a capability. It is **one screen**: today the builder and the
realm are two pages, the board is a list you read before you see anything, the realm
appears only after MAKE IT, and the public can't get in.

## The MVP in one sentence

On one screen, you say a world, it builds itself in front of you in a few seconds,
you keep talking (or tapping, or letting your agent work) and it changes live, and
PUBLISH gives you a link anyone can walk into.

## The screen

One route: **`openagents.com/forge`**. `/realms/new` redirects there. The canvas is
the realm itself, full screen, always running; everything else is a panel over it,
in the Verse's look (thin lines, plates, capitals; `verse.coax`).

```
┌────────────────────────────────────────────────────────────────────────────┐
│ ⏻ AETHYRFORGE   MOONLIT MARSH ▾        REV 14 · SAVED      ◉ PLAY  PUBLISH │  top bar
├──────────────┬───────────────────────────────────────────┬─────────────────┤
│ IN THIS REALM│                                           │ SELECTED        │
│ ▣ Land  hills│                                           │ Lighthouse      │
│ ▣ Water pond │          the realm, live, full bleed      │ x 4.2  z -7.0   │
│ ▣ Trees 12   │       (orbiting forge camera while you    │ ROTATE  SIZE    │
│ ▢ Mist       │        talk; your camera when you PLAY)   │ REMOVE          │
│ ▣ Herd  deer │                                           │                 │
│ ...          │                                           │ LORE · QUEST    │
│ ASKED FOR    │                                           │                 │
│ ◌ koi        │                                           │                 │
│ ◌ a ferry    │                                           │                 │
├──────────────┴───────────────────────────────────────────┴─────────────────┤
│ ● HOLD TO TALK   "add a lighthouse on the cliff and make it night"   TYPE  │  the forge bar
│ ▸ placed Lighthouse · switched Time to night · asked for: a ferry   UNDO   │  what changed
└────────────────────────────────────────────────────────────────────────────┘
```

- **The canvas.** The realm, rendered by Aethyr, from the first second. Before any
  words it is an empty ground under a slow sky with a faint grid: the forge floor.
  While you build, the camera is the **forge camera**: a slow orbit at about 30
  degrees that frames whatever just changed, then settles back to the whole realm.
  **PLAY** drops you in at the start with the realm's own camera and controls;
  Escape returns to the forge camera. On a phone the panels fold into tabs under the
  canvas and the forge bar sits above the keyboard.
- **The forge bar (bottom).** HOLD TO TALK (V or Space held; a tap on a phone toggles),
  TYPE, and DROP A DOCUMENT. Under it, one line says what the last sentence did, in
  plain words, each part a chip you can tap to undo just that part. Nothing else
  talks: no "thinking", no "working on it" (docs say no filler).
- **The feature rail (left).** The board, always visible: each feature a row with its
  check and its few words; a tap switches it and the realm changes at once. Under it,
  ASKED FOR: what the catalogue can't draw yet, each with BUILD IT (sends it to your
  agent, below) or REMOVE.
- **The inspector (right).** Appears when you tap a thing in the canvas: its name,
  where it stands, ROTATE, SIZE, REMOVE, drag to move it on the ground; for a
  character, its LORE and QUEST. Hidden when nothing is selected.
- **The top bar.** The mark and AETHYRFORGE, the realm's name (a tap renames; ▾ opens
  your realms and NEW), the revision and whether it is saved, PLAY, PUBLISH.
- **PUBLISH.** A sheet: visibility (unlisted or public), the line a shared link shows,
  the realm's card (a frame from the forge camera), and the link with COPY. Publishing
  freezes the revision.
- **YOUR AGENT.** A tab beside the inspector: your paired Coder (the "Your computer"
  panel, #12642) or, without one, our hosted builder. BUILD IT on an asked thing, or
  words typed here, become a job; its steps stream in this tab and its edits land in
  the canvas as they are saved.

## How it looks when things change (the visuals)

The difference between a form and a forge is that you **see** every change happen.
Each change animates in the canvas, driven by the engine's fixed step (never by the
host's clock), so a recording is the same every time:

| Change | What you see (about 0.6-1.2 s) |
|---|---|
| A realm is made | The forge floor's grid draws out from the middle; the ground rises and takes its colour; the sky turns to the realm's time of day. |
| A thing is placed | A ring of light marks the spot, the thing rises out of the ground with a short overshoot, and a few sparks settle. |
| A thing is removed | It sinks and dissolves into motes that drift up. |
| Trees, plants, rocks | They grow in a wave from the middle outward, a few frames apart. |
| Water | The basin cuts in, then fills from its lowest point. |
| A herd, a flock, a village | They walk or fly in from the edge to their places. |
| A switch off / on | The feature's things fade to outlines, then out; on, the reverse. |
| Time, palette, style | The sky and the light cross-fade over a second; a style change wipes across the realm. |
| A move or resize | The thing slides or scales with easing; a ghost shows where it was for half a second. |
| An agent's edit | The same, plus a thin line from the agent tab to the thing for a moment. |

Each of these is one engine feature: a per-entity **arrival** (kind, start step,
duration) that the shared renderer reads, so every realm built in the forge gets it,
and a recording with the same edits is the same frames (book ch005, replay).

## The flow, start to finish

1. **Arrive** at `/forge`, logged in (one tap to log in; the invite gate opens, see
   Limits). The forge floor, and in the forge bar: "Say the world you want."
2. **Say it.** "A foggy fishing village on a cliff, a lighthouse, gulls, and an old
   sailor who tells stories." The planner answers in 1-2 s; the realm is **made at
   once** (no MAKE IT step) and builds itself on the canvas: ground, cliffs, houses
   one by one, the lighthouse, gulls, the sailor. The rail fills as it builds.
   The setup questions are gone from the first run: style, camera, ground and time
   are chosen by the planner from the words and shown as rows you can change.
3. **Keep talking.** "Make it night and light the lighthouse." "Move the sailor to the
   dock." "No gulls." Each sentence is one revision, applied live, with its line of
   chips under the bar.
4. **Shape by hand.** Tap a thing, drag it, rotate it, remove it; tap a row to switch
   it. Each is a revision too. UNDO walks back one at a time.
5. **Ask for what isn't there.** "Add a ferry that crosses to an island." The catalogue
   has no ferry: it goes to ASKED FOR. BUILD IT sends it to your agent, which writes
   it, and the ferry rises out of the water when the agent saves.
6. **PLAY.** Walk it, talk to the sailor (character chat), come back.
7. **PUBLISH.** Unlisted or public, a link, a card. Anyone with the link walks in.

## The CLI and agents drive the same thing

Everything on the screen is a verb, and every verb is on the screen. The CLI already
has the verbs; the MVP adds what the screen needs and makes the screen show agent
work as it happens:

| On screen | CLI / MCP |
|---|---|
| Say or type a world | `openagents realm new "WORDS"` |
| Keep talking | `openagents realm revise SLUG "WORDS"` |
| Tap a row | `openagents realm switch SLUG ROW on|off` |
| Drag, rotate, size, remove | `realm move`, `realm place`, `realm apply FILE.jsonl` (rotate, resize, remove ops added) |
| UNDO | `openagents realm undo SLUG` |
| ASKED FOR, BUILD IT | `openagents realm asks SLUG`, then the agent's own edits, `realm ask-done` |
| PLAY | `openagents realm play SLUG` and `look`, `go`, `say`, `use` |
| A picture for the card | `openagents realm shot SLUG` |
| PUBLISH | `openagents realm publish SLUG --listed` |
| Open the forge on a realm | `openagents forge SLUG` (new: opens `/forge/SLUG` in the browser, or prints the link) |

An agent working through the CLI on a realm that is open in someone's forge shows up
live: the forge reads the realm's spec every second already, and its edits animate in
like anyone's. Coder gets the realm tools (`openagents mcp`) by default.

## Limits (to make it public)

- **Who can make realms.** The invite gate (`make 0`) opens for any logged-in account.
  Each account: 5 realms a day, 200 planner calls a day, 20 looks a day. Admins and
  approved creators: no limit. Our own sessions are excluded from usage numbers.
- **Cost.** Each planner call is one Gemini Flash call with thinking off, and a look
  sends one picture. Before opening, measure the cost of a call and of a look from the
  billing export, and set the per-account and server-wide daily caps so a full month
  at the caps stays under the $1,500 GCP ceiling with everything else running. The
  caps already in place stay (`OA_EDIT_DAILY_CAP` and the planner's), and a day's cap
  reached says so plainly.
- **What a realm can hold.** The catalogue's limits (40 placed things a sentence,
  the feature counts) and the engine's budgets (ch016) refuse past them with the reason.
- **Names and lines** shown publicly go through the existing moderation on names; a
  public realm can be reported from its page.

## What to build (slices, in order)

1. **One screen.** `/forge` and `/forge/SLUG`: the made-realm module with the rail,
   inspector, forge bar and top bar over it; `/realms/new` redirects; made at the first
   sentence, no MAKE IT; setup questions folded into rows. (builder.coax's planner
   calls and board move into the forge; myrealm.coax is the canvas.)
2. **Arrivals.** The per-entity arrival in the shared renderer, and the table above for
   every catalogue feature; the forge camera that frames the latest change.
3. **Edits by hand in the canvas.** Select, drag, rotate, size, remove, each a typed
   op in `aethyr/forge/edit.coax` with its inverse; `realm apply` gains rotate, resize,
   remove.
   The model, the document (migration 0109), the page's drawing and the CLI
   (`realm rotate|resize|remove`, `realm apply`) are done (#12629); the canvas gestures
   are not.
4. **The change line.** Each revision's summary as chips, each undoable on its own.
5. **Open to the public.** The invite gate becomes the per-account limits above; LOG IN
   is the only gate.
6. **Publish sheet** with the card from the forge camera; `openagents forge SLUG`.
7. **Your agent tab.** The paired Coder (#12642) or the hosted builder; BUILD IT on an
   asked thing.
8. **Pin the published build** (#12629): a published realm plays the engine build and
   assets it was published with. Done for made realms (migration 0107,
   `aethyr/forge/publish.coax`, `coax/web/engine-served.coax` for the served bytes'
   hashes, book chapter 54, checked end to end by `realm_walkthrough.coax`); open:
   built-in realms' forks and the release gates per version.

Slices 1-5 are the MVP. 6 and 7 make the video's last act. 8 is what makes a link
keep working.

## The video (Episode 291)

Cut to the audio track, every shot a real recording of `/forge` at 1080p
(headless Chrome through `realm shot`/the film pipeline, never a mock-up):

1. The empty forge floor; the words typed or said; the realm building itself, wide.
2. A run of sentences, each with its change landing: night falls, the lighthouse lights,
   a herd walks in, gulls lift off.
3. Hands: a drag, a rotate, a removal dissolving.
4. An ask the catalogue doesn't have, sent to an agent; split screen with the agent's
   terminal (Coder) and the thing rising in the realm when it saves.
5. PLAY: walking the realm in first person, a conversation with the sailor.
6. PUBLISH: the link, then a second browser walking into it.
7. The same realm built again from the CLI alone, `openagents realm new` and `apply`,
   while the forge shows it build.

## Done means

- A logged-out visitor goes from `openagents.com/forge` to a published realm with a
  link, on a laptop and on a phone, in under three minutes, with no help.
- Every action on the screen has its CLI verb and MCP tool, and an agent's edit to an
  open realm shows in the forge within a second.
- The same sequence of edits replayed gives the same frames (the video can be re-cut).
- Each slice has its `#test`s; the forge has a headless journey in the release checks.
