# Video Lab on every surface

Status: proposed, 2026-10-16; the `video` kind crate is done. Video Lab becomes one kind on the
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

## The `video` kind (#11879, done)

`crates/video-lab` implements `LabKind` and owns every rule:

- **Schemas:** the `cut.v1`, `shot.v1` and `scene.v1` schemas, read from
  `docs/video-lab/schemas/`.
- **`check`:** every rule of `cut.py check` and `lab.py check`.
- **The patch:** `@name` picking shots.
- **The edit pipeline:** captions follow their shots, and removed shots take
  their captions and music hits. An edit that adds a problem is refused, with
  beat-grid runs merged into one sentence.
- **Times and voiceover pacing:** the times the chat passes, and caption
  pacing for voiceover.
- **The timeline:** moved from `verse-film`, which re-exports it.

Facts that need the files (probes, animation beats, still stretches, speech,
title and caption layout in our fonts, audio lengths) are `Measured`. Today
`cut.py measure` computes them; job runners will later.

`lab.py` and `cut.py` call the rules through the `video-lab` program
(`scripts/film/video_lab_bridge.py`). Their Python copies of the rules are
deleted. The tools stay where they were.

Parity: before the switch, every saved version on the owner's Mac was checked
both ways with identical problems and notes. That was 66 documents: 56 cuts,
9 scenes and 1 shot. All 27 `test_lab.py` tests pass, with the
temporary-path test fixed, and the crate has its own tests.

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
