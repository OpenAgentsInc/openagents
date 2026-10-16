# Video Lab: see and edit Verse scenes and videos by talking

Status: spec, 2026-10-14. Owner request: "I want to be able to see inline
renders of Verse scenes including the Draconia stuff we're doing and generally
be able to edit videos and scenes, like the Episode 290 stuff we're doing. I
want to vibe it conversationally in an OpenAgents chat, here or on the website."

Video Lab on every surface, as one kind of the [Labs platform](../labs/README.md): [Video Lab on every surface](shared-primitive.md).

Video Lab is not a new app. It is a set of chat tools, a render service, and
an inline player that every OpenAgents chat surface shows: the web chat, Coder
in the terminal, and the phone. You say "make the portal walk slower and start
further out", the chat changes the scene or the cut, renders a preview, and
the result plays in the conversation, next to the previous version.

## What already exists (and is reused, not rebuilt)

| Piece | Where | What it gives Video Lab |
| --- | --- | --- |
| Cut lists | `scripts/film/cut.py`, `docs/verse/film/cut.json`, `launch.json` | A typed edit: shots with source, in, dur, xfade, `play`/`push`, titles, overlays, music, voice. `inspect` checks it; `render` encodes it |
| Shot specs | `scripts/film/studio/shots/*.json`, `studio.py` (Blender), `scripts/film/*.py` | Each generated shot is a JSON spec plus a renderer with `--preview` and `--still T` |
| The take | `crates/verse-film` (`take`, `render`, `export`), `docs/verse/film.md` | A scene recorded once as a message stream that any renderer draws, native or browser |
| Verse scenes | `crates/verse*`, zones, the Draconia set and her MetaHuman export (`~/work/film-out/draconia/metahuman/latest/`) | Real 3D scenes and characters, captured by `verse` examples |
| Review page | `scripts/film/review.py` (port 8791) | Timestamped notes on a video with exact frames, saved to `~/work/film-out/reviews/<video>-<hash>/notes.md` |
| Voice and captions | `~/work/film-out/voiceover/` (Scenario, Bella), `captions.py` | Line-synced voice and burned-in captions in our fonts |
| Parity loop | `docs/verse/draconia/parity.md`, the landmark/outline tools | Measured comparison of a render against a reference |
| Chat surfaces | web chat, Coder (`crates/coder-new`), phone | Where the conversation already happens; tools reach all three through the same API |

What is missing is the glue: one document model for "the thing being edited",
tools a chat can call on it, a place that renders, and a player that shows the
result inline.

## The model: a Lab project holds documents, every edit is a version

A **Lab** is an OpenAgents project (the same projects as everywhere else) with
one or more **documents**. A document is typed JSON, never free text:

- **Cut** (`cut.v1`): the existing cut-list format, unchanged. Episode 290 is a
  cut.
- **Shot** (`shot.v1`): one generated shot: its renderer (`studio`, `verse`,
  `film-script`, `take`), its spec JSON, and its source assets.
- **Scene** (`scene.v1`): a Verse scene: zone or set, characters and their
  export (Draconia), placements, lights, camera path, duration, and render
  settings.
- **Notes**: the review notes (time, text, frame), attached to a document
  version.

Every change makes a new **version** of the document: the JSON, a one-line
summary in plain words ("Portal walk: start 4 m further back, 20% slower"),
who made it (the person or the chat), and the renders made from it. Versions
are kept, so "go back to v1's glare" is one step and A/B is free. Documents and
versions live in the project's file store; renders live beside them.

Assets are referenced, never copied into the document: a path in the private
asset pipeline (`~/work/film-out/...`, Fab packs, MetaHuman exports) or an
object in the project's storage. The document records each asset's digest so a
render is reproducible.

## Talking to it: the chat tools

The chat edits documents only through typed tools. Which tool runs is decided
by the chat's model with these tools offered, never by matching words in the
message (workspace rule: no ad-hoc keyword routing). Bounded fields
(timestamps, durations, shot names) are parsed only after a tool is chosen.

| Tool | Does |
| --- | --- |
| `lab.open(document)` | Loads a document and its latest version; shows a poster frame and the version list |
| `lab.show(document, at?)` | Renders (or reuses) a preview and plays it inline; `at` jumps to a time or shot |
| `lab.frame(document, at)` | One still at a time, for looking closely; several times make a strip |
| `lab.edit(document, patch, summary)` | Applies a JSON patch to make a new version. The chat writes the patch; the tool validates it against the document's schema and the cut's `inspect` rules before saving |
| `lab.render(document, quality)` | `preview` (quarter resolution, few samples, seconds), `review` (1080p, minutes), `master` (full, with ProRes for cuts) |
| `lab.compare(a, b)` | Two versions side by side, or a render against a reference, with the parity measures when a reference is set (Draconia) |
| `lab.notes(document)` | Reads the review notes on a version, so "fix my notes" works without pasting them |
| `lab.voice(document, lines)` | Voice lines with the saved voice (Bella), synced to captions |
| `lab.assets(query)` | Finds clips, takes, shots, and private assets by what they show, so "use the portal walk" resolves to a file |

`lab.edit` is the only write. A patch that fails `inspect` (a shot that ends
inside an animation beat, a missing source, a title outside the safe area, a
freeze) is refused with the reason in plain words, and the chat fixes it before
anyone renders. This is what stops cutting animations off mid-move, which the
owner flagged on the Episode 290 take 2.

## Seeing it: the inline player

Each surface shows the same player card: poster, play/pause/scrub, the
version name, Compare, and "Note at this time". Notes made in the card are the
same notes the review page keeps.

- **Web chat:** an inline `<video>` for rendered previews. For Verse scenes, a
  live view as well: the scene's take streams to the browser and draws with
  WebGPU, so orbiting the camera or scrubbing doesn't need a new render. Draconia
  close-ups use the rendered video, since her MetaHuman assets never leave the
  owner's machines (below).
- **Coder in the terminal:** inline images and video where the terminal can
  show them (the Kitty graphics protocol, iTerm2 inline images: Ghostty, Kitty,
  WezTerm, iTerm2 all can), as a poster plus a play key that opens the local
  player. Where the terminal can't, one line with the link to the local player
  page (the review page, which becomes the Lab player). Never a dead control.
- **Phone:** the same card with the platform video player; notes by voice
  become the same timestamped notes.

## Rendering: where the pixels come from

Two places, chosen per document by what it uses:

1. **This Mac (local render host).** Anything that touches private assets
   renders here: MetaHuman (Draconia), Fab packs, Berkeley Mono. The Lab render
   service is a small local process (the `coder host` already runs here) that
   takes render jobs from the chat, runs `cut.py`, `studio.py`, Blender, or the
   `verse` capture examples, and uploads the result to the project privately.
   One job at a time per GPU, previews first.
2. **Google Cloud (render boxes).** Everything else renders on a GPU box made
   through the Box SDK the same way Solver runs (`docs/cloud/solver-on-gcp.md`):
   a render image with Blender, ffmpeg, and Verse, a job, a result, the box
   deleted. This is what lets the website and phone edit when the Mac is off.

The private-asset rule is binding: MetaHuman exports are licensed for use but
must not be redistributed in raw form, and must never be used to train any
model. Lab never uploads them to a render box, never puts them in a dataset,
trace, or Gym run, and marks every render that contains them `no-training`.
Renders of her are fine to watch, share, and cut into films.

Caching: a render is keyed by the document version's digest plus quality, so
asking to see something already rendered plays it immediately.

## The loop, end to end (Episode 290 example)

1. "Open Episode 290." The card shows take 2's poster, its 5 notes, and the
   version list.
2. "Fix my notes and keep it under 5 minutes." The chat reads the notes, writes
   one patch per note, and `inspect` refuses one (a cut inside the delegation
   card's animation). The chat moves the cut point to the end of the beat. New
   version v3, with each change listed in a sentence.
3. Preview renders (local, because the cut uses the Draconia shots) and plays
   inline. "Compare with v2 at 1:26" shows both at that moment.
4. "The glare's overdone, go back to v1's glare." One patch from v1's value.
   v4.
5. "Render the review version." 1080p, link in the card; notes on it land in the
   same document.

## Draconia in Lab

Her parity loop becomes a Lab document with a reference: each round is a
version, the grid is `lab.compare` against `hICimQqsWFwwDPcE.mp4`, and the
owner's notes ("hair is coming out of her forehead") are notes on that version
the loop reads first. The castle world (#11576) is a `scene.v1`, so "move her
to the courtyard and have her turn to the camera" is an edit plus a preview.

## Measures (what "working" means)

- A preview of a 10-second shot plays in the chat within 60 s of the request
  (local), 3 minutes (cloud, cold box).
- An edit that `inspect` would refuse never reaches a render.
- Every version can be re-rendered to the same pixels from its digest.
- No MetaHuman asset ever appears in a cloud bucket, dataset, or trace; a check
  in the render service enforces it.

## Slices

1. **Documents and versions.** `cut.v1`/`shot.v1`/`scene.v1` schemas, version
   store in projects, Episode 290 and the portal walk imported. API and CLI
   (`openagents lab open|edit|versions|render`).
2. **Local render service** on the `coder host`: the job queue, previews,
   caching, the private-asset guard.
3. **Chat tools** (`lab.*`) in the web chat and Coder, through the same API.
4. **Inline player:** web `<video>` card, terminal inline images with the local
   player fallback, phone card; notes in the card.
5. **Cloud render boxes** via the Box SDK, for documents without private assets.
6. **Live scene view:** take streaming and the WebGPU renderer in the web
   card.
7. **Draconia and the castle** as Lab documents; the parity loop reads Lab
   notes.

Each slice ships on its own and is usable when it lands: slice 1 plus 2 plus
the review page already gives "edit by talking, see it locally".
