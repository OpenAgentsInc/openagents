# Video Lab on every surface

Status: proposed, 2026-10-16. Video Lab becomes one kind on the
[Labs platform](../labs/README.md). That document owns the API, storage,
jobs, cards, SDK and surfaces for every Lab. This page covers only what is
particular to video. Issues: #11879 (kind crate), #11880 (platform), #11886
(desktop timeline), plus the platform issues listed there.

## Today

Video Lab runs only on the owner's Mac. `scripts/film/lab.py` (2,741 lines,
Python stdlib HTTP, 127.0.0.1:8792, no accounts) provides:

- 18 tools at `POST /api/tool/{name}`;
- a version store under `~/work/film-out/lab/`;
- renders through `cut.py` (ffmpeg), Blender `studio.py` and the Verse
  `film_shot` example;
- speech import with whisper.cpp;
- the 20 KB vanilla-JS player and timeline (`scripts/film/lab_player.html`).

The edit rules live in three places: `cut.py check` and `lab.py` in Python,
and the timeline in Rust (`crates/verse-film/src/timeline.rs`).

Three clients use it:

- **The local web chat:** `crates/openagents-web/src/lab.rs`, through the
  Labs door. Loopback only.
- **Coder 1.0.0:** `crates/coder-new/src/video_lab.rs`, with a copied list
  of 14 tools. `lab_keep_captions`, `lab_new`, `lab_lines` and `lab_voice`
  are missing.
- **`openagents lab` 1.0.0:** `crates/openagents-cli/src/lab.rs`.

openagents.com in production, Coder Desktop and the iPhone app have none of
it. Cards are not saved in the history API, so #11869's four-surface
recording shows words everywhere, and the film was cut in the Mac's player.

## The `video` kind (#11879)

`crates/video-lab` implements `LabKind`:

- **Schemas:** `cut.v1`, `shot.v1`, `scene.v1` (moved from
  `docs/video-lab/schemas/`).
- **`check`:** every rule of `cut.py check` and `lab.py check()`:
  - the 120 BPM beat grid;
  - animation beats from `<clip>.beats.json`;
  - held frames and still stretches over 0.4 s, unless `still_ok`;
  - shots that keep their speech start and end in pauses, and their
    captions match the words;
  - the title safe area;
  - sources exist and are long enough;
  - music covers the cut;
  - unknown fields are refused.

  Motion, pause and word timings are **measurements**. A job computes them
  once per source digest, so `check` stays pure.
- **`apply`:** JSON Patch with `/shots/@name` addressing; captions move with
  their shots. Timeline intents come from `verse-film/src/timeline.rs`:
  move, link and unlink, speed, text, snap to frame. Each tool's own change
  is an intent too: `splice`, `lines` (150 wpm pacing), `voice` (re-time
  captions to word times), `keep_captions`, `captions`, `framing`.
- **Job types:**
  - `render` at `preview`, `review` or `master` quality;
  - `frames` and `strip`;
  - `measure` (motion, pauses);
  - `transcribe` (whisper.cpp with `lab_words.json` fixes);
  - `score` (music remade to length);
  - `import` (probe, rotation, poster).
- **Card:** `view: Video` with the poster, the render, the frame strip, and
  `Compare` for version pairs at a time. The `detail` lists shots and
  captions for a time range.
- **Tools:** the 18 current `lab_*` tools, unchanged in name and arguments,
  each mapped onto platform operations.

Refusal wording is kept word for word ("s4-tray and the 15 shots after it
would start off the 120 BPM beat grid…"). `test_lab.py`'s 27 cases are
ported as Rust tests. `lab.py` calls the crate through a bridge binary, as
`timeline_bridge.py` does today, until #11880 replaces it.

## Video on the platform (#11880)

- **Runners.** The job runners are Rust wrappers around the same tools
  (ffmpeg, Blender, Verse, whisper.cpp, `music.py` until it is ported),
  packaged for two placements:
  - an **enrolled host** on the owner's Mac for any version whose asset
    manifest has a `private-local` asset (MetaHuman, Draconia, Fab packs,
    Berkeley Mono);
  - a **cloud worker image** for everything else.
- **Caching.** Renders stay cached by version digest plus quality. One
  render at a time per host.
- **Loopback mode.** The same service on 127.0.0.1:8792 with the
  filesystem store, serving the 1.0 routes (`/healthz`, `/api/tools`,
  `/api/tool/{name}`, `/api/card`, `/media/*`, `/api/timeline`,
  `/api/note`), so the owner's current workflow, `full-local.sh --lab` and
  frozen Coder work unchanged. Existing documents under
  `~/work/film-out/lab/films/` are imported with their versions and
  digests.
- **Pages.** The web player and timeline become `/labs/video/{doc}` on
  openagents.com. The vanilla JS stays and calls `/v1/labs`. Timeline drags
  send intents.

The `no_training` marker becomes the artifact field. The `.no-training`
files stay in loopback mode.

## Desktop timeline (#11886)

After the standard views (#11883), Coder Desktop gets the first native
editor: the video, audio and text tracks from the screenshot, in GPUI,
over `video-lab` intents, previewed locally and saved as versions through
the SDK. Posters and the frame strip stand in for live decoding while
scrubbing, and the hosted AVPlayer plays the render. The phone keeps chat
edits, trims and notes.

## Measures

- A 10-second preview plays within 60 s from the enrolled host, and within
  3 minutes from a cold cloud worker.
- A refused edit never reaches a render, and the refusal reads the same on
  every surface.
- Every version re-renders to the same digest.
- No `private-local` asset appears in any bucket, dataset or trace.
- A card saved in a web chat plays on desktop and phone through
  `/v1/history`.
- A cached render starts within 2 s, and seeking uses Range reads.
