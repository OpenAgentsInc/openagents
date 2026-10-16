# Video Lab MVP: try it on the Mac

This is the local-first MVP of [Video Lab](README.md), slices 1–4 (#11581).
You chat in the OpenAgents web app running on this Mac. The chat opens,
edits, renders and compares films with the Lab's tools, and the video plays
in a card under the answer.

## Start it

One command, from a checkout of `main`. The one `coderdev` keeps current
works:

```
~/work/openagents-coderdev/scripts/dev/full-local.sh start --lab
```

Then open **http://127.0.0.1:4301**. You can chat with the Lab signed in
or not: only this Mac reaches the Lab. To be yourself rather than a guest
(your chats and projects), open http://127.0.0.1:4301/login and choose
**Continue with GitHub**.

The film review page on 8791 keeps running beside it.

`--no-build` skips the server builds, but the chat page's browser code is
rebuilt when it is older than the checkout. A stale build kept a sent
message on "Working" after the page moved to the new chat.

The first start builds the whole stack into
`~/work/openagents-target-fulllocal`, which takes several minutes. Later
starts reuse that build. Add `--no-build` to skip it.

- `scripts/dev/full-local.sh stop` stops the stack, including the Lab it
  started.
- `scripts/dev/full-local.sh status` shows what's running.

`--lab` starts these, beside the usual website, gateway and chat worker:

- **The Lab service** on http://127.0.0.1:8792 (`scripts/film/lab.py serve`).
  Its log is `~/.openagents/full-local/logs/lab.log`.
- **The chat's Lab tools.** Gemini 4 Argon on Vertex (global) calls them. Its credential is
  `$GOOGLE_APPLICATION_CREDENTIALS`, or else
  `~/work/.secrets/gcp-mvp-automation.json`. One turn sees Video Lab's and
  Game Lab's tools together, with the chat's current Lab document (the
  last card shown) as context, so "compare the latest with v2 at 0:25"
  after a film goes to the film and "compare with the original" after the
  game goes to the game (`crates/openagents-web/src/labs.rs`, #11619).
- **The gateway on 8790**, because the film review page holds 8791. Your
  review page and its notes are left alone.

## Edit on the timeline

Open the **Lab player** to see video, audio, and text tracks. Drag video
clips to reorder them. Select an audio or text clip and choose **Link to
video** to keep its offset when that video moves; choose **Unlinked** to
move it independently. Clip controls also provide start time, video speed,
text and text length, and a single dissolve style. Text uses Geist.

Each edit saves a version. Select **Render preview** to watch the result.
Video snaps into the continuous sequence; audio and text snap to frames.
Original speech stays with its video. Speed changes apply to video without
original speech or held frames. Older versions remain available for review.

## Talk to it

Start a new chat and send, one at a time:

1. **open episode 290**
   - The answer lists take 2's shots and the problem the checks find: the
     delegation shot cuts the report card off mid-animation.
   - The card shows the take 2 preview. Rendering it the first time took
     6.5 minutes; after that it's cached.
2. **show me 1:20 to 1:35**
   - The answer names the shots and captions in that range.
   - The card plays the preview from 1:20 to 1:35, with a strip of frames
     under it.
3. **make the delegation report card play to the end of its animation before the cut**
   - The chat lengthens `s4-delegation` from 10 s to 12.5 s. Lengths stay on
     the 120 BPM beat grid, and the captions move with their shots.
   - It saves the change as v3. The music is remade to the new length.
   - The v3 preview renders in about 1.5 minutes, because only the changed
     shot renders again, and then plays in the card. The card shows
     "Rendering the preview…" until then.
4. **render a preview and compare with the previous version at 1:26**
   - The card shows v2 and v3 side by side at 1:26, with **Play both** and the
     two frames.
   - In v2 the report card is cut; in v3 it finishes.

Every card has **Open in the Lab player** (http://127.0.0.1:8792/player). The
player has the documents, every version, compare side by side, and the notes.

**Note at this time** under a video saves a note at the playing time. These
are the same notes the review page keeps, under
`~/work/film-out/reviews/<video>-<hash>/notes.md`. "Read my notes" in the chat
reads them with `lab_notes`.

## Splice in a clip of someone talking (#11617)

Ask in the same chat, for example:

1. **add the whiteboard clip IMG_4135 as a source**
   - `lab_import` finds it in Downloads, Movies, Desktop or
     `~/work/film-out`, and reads its size as it plays (a phone's rotation
     flag applied), length, a strip of frames, what's said with each word's
     time (whisper.cpp, `whisper-cli` with `~/work/ggml-small.en.bin`), and
     the pauses measured from its sound.
   - Names it mishears are fixed from `scripts/film/lab_words.json`
     ("Cloud Code" becomes Claude Code).
2. **what does he say from 0:40 to 1:20, and where are the pauses?**
   (`lab_sources`)
3. **splice 0:00 to 0:19 of it in after s1-macbook-chat as wb-hook**
   (`lab_splice`)
   - The clip keeps its own sound, and the music ducks under it
     (`"sound"` on the shot; `"music_duck"` on the cut, default 0.2).
   - The in and out move to the nearest pauses that are a whole number of
     beats apart, so no word is cut and the beat grid holds.
   - Captions of what's said are made in the film's caption style. Later
     shots and their captions move along, and the music is remade to the new
     length.
4. **show me both ways to frame it** (`lab_framing`)
   - A clip that isn't the film's shape sits whole over a blurred, dimmed
     copy of itself (`"fit": "blur"`) or is cropped to fill (`"crop"`).
5. **remake the captions over wb-hook** (`lab_captions`), after its in or
   length changed.
6. **remove every caption except where I'm speaking** (`lab_keep_captions`
   with `over: speech`, or shot names): one edit keeps only the captions over
   the shots that keep their own sound. Titles and text inside the shots'
   pictures aren't captions and stay.

The checks refuse a shot that keeps its sound but starts or ends mid-word,
and a caption over it that doesn't match what's said.

Sources live in `~/work/film-out/lab/sources/<id>/` (`source.json`,
`speech.json`, the poster and strip). `cut.py check` also reads a
`<clip>.speech.json` beside a clip.

## A new cut, with voiceover lines to read

1. **start a new document episode-290-metaharness from episode-290**
   (`lab_new`): v1 is a copy of the latest version; episode-290 and its
   versions are left as they are.
2. **put these voiceover lines over s3-routing-beat1: ...** (`lab_lines`):
   lines that aren't recorded yet become captions, one sentence each (a long
   one split at a comma), timed at a speaking pace (150 words a minute; say
   another pace if you like) from a time or just after a shot's start.
   Numbers count as the words they take to say. Lines that would run into a
   shot where someone speaks, over another caption, or past the end are
   refused with how much time they need. They're marked `"vo": true`, so
   they can be swapped for the recorded voice later.
3. **put the Bella clip 290-bella-s3 over those lines** (`lab_voice`): a
   recorded voiceover clip goes on from a time (or just after a shot's
   start) at its own length, kept in the cut's `voices`, with the music
   ducked under it as under the whiteboard. The captions of the lines it says
   are re-timed to when each is said (from whisper.cpp's word times, or
   spread over the clip without them), and the answer says how much longer or
   shorter the voice is than the captions were. A clip that would run into a
   shot where someone speaks is refused with how much more picture its lines
   need.

## An edit that's refused

Ask for something the checks won't take, for example **now make the
delegation shot exactly 12.25 seconds**. Nothing is saved. The Lab tells the
chat:

> s4-tray and the 15 shots after it would start off the 120 BPM beat grid
> (s4-tray at 89.25s); keep shot lengths to whole beats (multiples of 0.5 s)

The chat then says so in its own words and offers 12.0 s or 12.5 s.

These edits are refused in the same way:

- a shot ending inside an animation;
- a shot that pauses: it holds a still frame (`play`) for more than 0.4 s, or
  the part of its clip it plays has a still stretch that long (measured once per
  clip, cached in `~/work/film-out/lab/motion/`). A shot that should rest sets
  `"still_ok"`. After a cut renders, the Lab measures the same rule on the
  render and `lab_open` and `lab_render` name any pause left;
- a missing source, or one that's too short;
- a title outside the safe area;
- music shorter than the cut;
- a field a shot doesn't take, such as `duration` instead of `dur`.

## The same tools elsewhere

**In the terminal:** each command prints the Lab player link.

```
openagents lab list
openagents lab open episode-290
openagents lab show episode-290 --from 1:20 --to 1:35
openagents lab frame episode-290 1:26
openagents lab edit episode-290 --patch '[{"op":"replace","path":"/shots/@s4-delegation/dur","value":12.5}]' --summary "Delegation plays through its report card"
openagents lab versions episode-290
openagents lab render episode-290 --quality review
openagents lab compare episode-290 --at 1:26
openagents lab notes episode-290
```

**In Coder** (`coderdev`): ask the same things as in the web chat. The
`lab_*` tools are in Coder's media tools. Coder answers in words with the
player link, and the terminal doesn't play video.

Both start the Lab service if it isn't running.

## In Coder Desktop

Coder Desktop (now in this repository at `apps/openagents/`; the Lab work
landed as `84d10a9554`) offers the Lab's tools to every agent it runs: Claude
Code, Codex, Cursor, and the other harnesses. It reads all 18 tools from the
running Lab, so it has the newer ones (`lab_new`, `lab_lines`, `lab_voice`,
`lab_keep_captions`) that the frozen Coder 1.0 terminal lacks. Ask in any chat,
the same way as in the web chat.

Each result shows a card above the answer, drawn natively:

- the document, version and summary;
- the frame, frame strip, side-by-side compare or poster;
- the video itself, playing inline with sound, with play and pause, time, and
  a seek bar; for a compare, **Play v2** and **Play v3** switch between the
  versions;
- the render state, which follows a render in progress (the card asks the Lab
  again every two seconds) and swaps the video in when it lands;
- whether the film stays on this Mac.

**Open in the Lab player and timeline** opens the player in a Coder Browser
tab, with the versions, compare, notes and timeline editing.

Build and run it from this repository:

```
cd apps/openagents && scripts/run-macos-dev.sh
```

A Coder Dev that is already running keeps its old build until you quit and
rerun it. Like the web chat, it needs a Lab on this Mac: `VIDEO_LAB_URL`, a
Lab already answering on 8792, or `~/work/openagents/scripts/film/lab.py`,
which it starts. It works on this Mac only, through the loopback Lab, until
the [Labs platform](../labs/README.md) serves Labs from the account.

## Where things are

- **Documents and versions:**
  `~/work/film-out/lab/films/<document>/vN/{doc.json,version.json}`. They're
  private and never in git.
- **Renders:** `.../renders/<digest>-<quality>.mp4`, with a poster and a
  `.json` describing the render.
- **Not-for-training marker:** a render that uses private assets (MetaHuman,
  Draconia, Fab packs, Berkeley Mono) has a `.no-training` file beside it.
  The Lab binds to 127.0.0.1 only and has no upload path.
- **The first documents:**
  - `episode-290` (`cut.v1`: v1 is the draft, v2 is take 2);
  - `portal-walk` (`shot.v1`: `scripts/film/portal-walk.py`);
  - `portal-walk-scene` (`scene.v1`: the Verse walk for the `film_shot`
    example).
- **Schemas:** `docs/video-lab/schemas/{cut,shot,scene}.v1.json`.
- **Animation beats:** `cut.py check` reads `<clip>.beats.json` beside a
  source. `scripts/film/coder-segments.py` writes it for the delegation
  segment.

## Not in this MVP

- The phone card.
- Cloud render boxes (slice 5).
- The live WebGPU scene view (slice 6).
- Draconia and castle documents (slice 7).
- `lab.voice` and `lab.assets`.
- The chat's model seeing the frames as images. It reads the shot and caption
  list instead.
- Terminal inline images.

A `scene.v1` render runs `cargo run --release -p verse --example film_shot`.
It works, but its first build is long.
