# Aethyrforge for Mac: local editing by voice (experimental spec)

Status: spec, 2026-11-04. Experimental. Nothing here ships to customers or
TestFlight until the owner asks.

Aethyrforge for Mac is a desktop app that opens your Everglade on your own Mac and
lets you change it by talking. Speech recognition, the model that turns words into
edits, the room and the world document all run locally. Nothing leaves the Mac
until you press PUBLISH.

It builds on the web Aethyrforge (docs/verse/aethyrforge.md), the local room
(`coax/web/room/local.sh`, `room-dev.coax`), local Parakeet (#12667, #12669), and
gpt-oss-20b on Metal (docs/verification/psionic-gptoss-metal-2026-10-29.md).

## What you do

1. Open **Aethyrforge.app**. It opens Everglade at the South Gate, with EDIT on.
2. Hold **Space** (or the mic button) and say what you want:
   - "Put three barrels by the inn door."
   - "Make that crate twice as big."
   - "Move the well five meters toward the gate."
   - "Turn the lanterns off on Inn Street."
   - "Make it night."
   - "Undo that."
3. Let go. Your words appear in the transcript bar within about 0.3 s. The edit
   appears in the world within about 2 s, with a short line saying what changed:
   "Placed 3 barrels by The Bent Nail's door."
4. Walk, fly or click to look around. Click a thing to select it, so "that" means
   it.
5. Press **PUBLISH** to send your local version to openagents.com, or **REVERT**
   to go back to the published one.

Nothing else is on screen: the world, the transcript bar, the edit line, EDIT's
panes, PUBLISH and REVERT. No mode switches.

## Parts

| Part | What it does | Where it lives |
|---|---|---|
| App shell | An AppKit window, the menu, the mic and Space key, and the processes below. | `apps/aethyrforge-desktop/` (Swift host, Coax logic, like `apps/openagents-desktop/`) |
| World view | The same Everglade build as the site, drawn with WebGPU in a WKWebView. | The page from `apps/openagents-web/build.sh`, loaded from the local server |
| Local room | The room server in `--dev` mode: the local player is the admin, it saves the world document, and every page reloads it. | `coax/web/room/bin/room --dev DIR` (a38754d290) |
| Local page server | It serves the built page and passes `/api/` to the room. | `coax/web/serve.sh --static --api PORT` |
| Recognition | Resident Parakeet TDT v3, hold to talk, final transcripts only. | The voice host's `--recognizer parakeet` and `openagents dictate` path |
| Edit model | Turns a transcript and the world's context into one typed edit. | gpt-oss-20b on Psionic Metal |
| Edits | Typed, invertible edits with undo and redo. | `aethyr/forge/edit.coax` |
| World document | Your local copy of Everglade's world. | `~/Library/Application Support/Aethyrforge/everglade/world-everglade.json` |

### Why the world view is a web view

Firm reason: the world view must be the exact engine build that players get. A
WKWebView with WebGPU runs that build unchanged. A separate native Metal renderer
would be a second renderer to keep in step, which is the two-copies problem behind
today's room bugs. Everything around the view (voice, the model, edits, files) is
native Coax and C.

## Voice

- The app records 16 kHz mono while Space or the mic button is held, and sends the
  hold's audio to one resident Parakeet model when you let go. The model loads once
  at app start and stays loaded.
- `aethyr/voice.coax` admits each hold's final transcript once, in its turn,
  through the replay gate, as the voice host does.
- The transcript shows in the bar. You can click it to fix a word, then press
  Return to resend it.
- Typing works too: press Return with nothing held to type an instruction.

## The edit model

The model runs locally: **gpt-oss-20b** on Psionic Metal, measured at about 35
tokens a second decode on this Mac (M5 Max, 128 GB). Its 13.8 GB of weights are
resident in one GPU buffer.

- **Input:** the transcript, the selected thing, what is under the cursor, the
  camera's place and heading, the district names near you, the last 3 edits, and
  the edit schema.
- **Output:** exactly one typed verb from the schema, filled in, or a short
  question back when something is ambiguous ("Which crate? There are four on Inn
  Street.").
- **Constrained:** the output is decoded under a grammar built from the edit
  schema, so it is always a valid edit. Parsing it is bounded field parsing after
  the verb is chosen (the workspace's semantic routing rule).
- **Verbs:** `place`, `move`, `scale`, `rotate`, `delete`, `duplicate`, `switch`
  (a feature on or off), `set` (time of day, weather, light), `name`, `ask` (a
  thing the catalogue cannot draw goes on the list), `undo`, `redo`, `look`
  (answer a question about the screen), and `say` (a short reply).
- **Places and things:** the model refers to things by the world document's ids.
  The app resolves "the inn door", "that", "here" and "five meters toward the gate"
  to ids and coordinates from the world's district, building and door tables
  before the edit is applied. The model never invents coordinates.
- **Time to edit:** under 2 s from release to the edit on screen for a one-verb
  edit, measured with the transcript bar and the room's save in the path.

### The model download

- The app checks for the model at start. When it isn't present, it downloads the
  pinned `openai/gpt-oss-20b` revision
  (`6cee5e81ee83917806bbde320786a8fb61efebee`) to `~/.cache/psionic/gptoss-original/20b/`,
  with a progress bar and size shown ("13.8 GB"). It checks every file's hash
  before use.
- Parakeet downloads the same way to `~/.cache/openagents/models/parakeet/`.
- This Mac already has both. The app's first run here only verifies them.
- Weights never go in Git.

## Saving, undo and publish

- **Each edit saves** through the local room's `POST /api/playtest/world/everglade`,
  as the next local version. The room reloads it and the view updates.
- **Undo and redo** are the edit log's inverses (Cmd+Z, Cmd+Shift+Z, or saying
  it). The log lives next to the world document and survives restarts.
- **PUBLISH** sends the local document to openagents.com through the same world
  publish the web EDIT uses, signed in as you. It shows what changes first ("12
  edits: 5 placed, 4 moved, 3 resized") and needs one click to confirm. It never
  publishes on its own.
- **REVERT** fetches the published document and replaces the local one, after one
  click to confirm.
- **At start**, the app fetches the published Everglade into the local document
  when there are no local changes. If there are local changes, it keeps them and
  shows "Local changes: 12 edits not published".

## Play-testing your edits

Because it is a real local room, you can test edits as players do:

- **ADD BOTS** starts N local bot players (`coax/web/room/raid-check.coax`) in the
  same room.
- **START RAID** starts the siege in the local room.
- The app never touches openagents.com's rooms.

## Build and run

```sh
apps/aethyrforge-desktop/build.sh --open
```

It builds the page and room if their build is older than the source, builds the
app, and opens it. The app picks free local ports for the room and page server on
each start, and stops both when it quits.

The app is experimental:

- It's signed for this Mac only.
- It isn't in releases, DMGs or TestFlight.
- It isn't installed to /Applications.

## Acceptance

1. Open the app and see Everglade within 10 s, with EDIT on.
2. Each of the six example sentences in "What you do" produces the right edit, and
   the edit is on screen within 2 s of release.
3. "Undo that" reverts the last edit. Quitting and reopening keeps every edit.
4. An ambiguous sentence gets a question back, not a guess.
5. With Wi-Fi off, steps 1 to 4 still pass.
6. PUBLISH, confirmed, changes the live Everglade's document. A new browser
   session at openagents.com/worlds/everglade shows the edit.
7. A headless test drives the app's local stack end to end. It sends 20 recorded
   WAV instructions through Parakeet and the model, applies the edits through the
   room, and checks the world document and a frame for each.

## Build order

Each step lands on `main` working end to end.

1. **The shell and the local stack:** the app window, the local room and page
   server, and Everglade with EDIT on. Saving by hand works.
2. **Voice to text:** hold to talk, resident Parakeet, and the transcript bar.
3. **Text to edits:** gpt-oss-20b under the edit grammar, the resolver for places
   and things, the edit applied and saved, undo and redo.
4. **Publish and revert,** with the change summary.
5. **Bots and raid** for play-testing, and the headless end-to-end test.

## Not in this version

- Windows or Linux.
- Multiple people editing one local world. Use the web EDIT on a room for that.
- Building new kinds of things the catalogue can't draw. Those go on the list
  (`ask`) for an agent to build.
- Speaking replies aloud. Replies are text in the edit line.
