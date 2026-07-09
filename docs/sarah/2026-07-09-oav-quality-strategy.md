# OAV Quality: Status + Improvement Strategy (2026-07-09)

Owner directive: the owned avatar video (OAV) pipeline must produce
"beautiful" audio and video. The latest enhanced take was judged
"higher def but more choppy, way less humanlike" by the owner watching it in
motion. This doc records exactly where the pipeline stands, what we measured,
the ranked defect hypotheses, and every improvement avenue we know of, so any
agent can pick up a lane and help. Related issues: openagents #8610 (OAV-1
proof/QA thread), #8611 (OAV-1), #8612 (OAV-2 realtime), #8613 (OAV-3 TTS).
Spec: `docs/sarah/2026-07-09-owned-avatar-video-pipeline-spec.md`.

## Current pipeline (offline proof lane, OAV-1)

- Source footage: owner-provided Midjourney clips (720p24, ~8s each) of the
  Sarah presenter; clip 8 is the current render base (185 usable frames — the
  last 7 frames have no face and are trimmed). Preprocessed MuseTalk avatars
  exist for 10 clips on the GPU host.
- TTS: CosyVoice2-0.5B zero-shot clone. Voice reference
  `sarah_voice_ref_v1.wav` (true voiced source is clip 4 — clip 8 has no
  speech). Spoken-form text normalization now runs before synthesis
  (hydralisk `tts/normalize.py`, commit `40b2783`): initialism lexicon
  (AI → letter-spoken, API, URL, "openagents.com" → "open agents dot com"),
  conservative all-caps heuristic.
- Lip-sync: MuseTalk 1.5 (MIT). 256×256 mouth-crop latent inpainting, pasted
  back onto the 720p frame. ~11 FPS offline on the L4 including PNG writes;
  the 24s take renders in ~2 min once the avatar is preprocessed.
- Optional enhancement: GFPGAN v1.4 applied per-frame to the 256² synthesized
  crop before paste-back.
- Encode: libx264 CRF 16 preset slow, audio loudnorm to ≈ -16 LUFS / -3 dBTP.
- Host: GCE `sarah-avatar-gpu-1` (L4 24GB, us-central1-b, ~$0.85/h, currently
  RUNNING with all venvs, weights, and preprocessed avatars staged).
  Artifacts: `gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/`
  (+ `qa/`, `qa2/` stills).

## Takes so far and their verdicts

| Take | Recipe | Verdict |
| --- | --- | --- |
| v1 `sarah_reply.mp4` (21.84s) | MuseTalk, old audio, ~1.0 Mbps encode | GO on identity/sync; mouth soft/mushy; audio said "eye" for AI, slurred "coding"→"Cody's", peaked -0.2 dBFS |
| v2 `sarah_reply_hq.mp4` (23.96s) | normalized-text audio, CRF16 (2.6 Mbps) | encode softness gone; residual MuseTalk 256² teeth softness; audio defects all fixed (STT round-trip verified) |
| v2 `sarah_reply_enhanced.mp4` | + GFPGAN per-frame on the crop | crisp teeth/lips in stills, no measured flicker — but OWNER VERDICT: choppier, plastic, less humanlike in motion |
| v3 tamed-GFPGAN take (A2+A3) | GFPGAN alpha-blend + feathered mouth mask + temporal EMA on delta | measured SMOOTHEST take so far (jerk 0.15) — current best MuseTalk-lane recipe |
| LatentSync 512² take (D1) | ByteDance LatentSync 1.6 diffusion | BEST articulation of any take, but a visible 16-frame chunk-boundary hitch (jerk in phases 14–15 ≈ 1.9× baseline) — needs chunk-seam smoothing before it wins outright |

## What we measured (facts to reason from)

- Frame counts identical (575 @ 24fps) — no dropped/duplicated frames.
- Whole-timeline inter-frame mean |diff| in the mouth region (0.5s sampling):
  orig 8.77, hq 8.88, enhanced 8.82 — statistically identical motion. The
  "choppy" percept is NOT gross frame-level jitter or dropped frames.
- 24 consecutive frames at t=8s: enhanced 2.92 vs hq 2.82 mean diff — no
  GFPGAN boil at that window; stills show no identity drift, invisible seam.
- STT round-trip (Gemini transcription of final mux): all target words
  correct, "A I" spoken as letters 3/3.
- Render times on L4 for the 24s take: HQ 108s; +GFPGAN 254s.

## Ranked hypotheses for "choppy + less humanlike"

1. **Per-frame GAN restoration percept ("plastic" + viseme mode-snapping).**
   GFPGAN has no temporal model; each frame is pulled toward its own restored
   mode. Small per-frame appearance snaps read as choppiness in motion even
   when average pixel diffs look normal, and the over-restored skin/teeth
   texture reads as uncanny. Stills always look BETTER — this failure is
   only visible in playback, which is exactly the owner's report.
2. **Sharpening strobes at 24fps.** The original's soft mouth acted as
   natural motion blur; a crisp mouth at 24fps with fast articulation shows
   discrete mouth poses ("picket fence" percept). Same articulation + more
   detail = perceived choppier.
3. **Staccato TTS audio drives clipped visemes.** The normalized text spells
   letters ("A I … A P I") and the clone currently produces abrupt sentence
   pacing; audio-driven models translate clipped audio into clipped mouth
   motion. Less humanlike speech → less humanlike face, independent of video.
4. **MuseTalk 256² ceiling.** The inpainted region tops out below the rest
   of the 720p frame's detail; enhancement papers over it per-frame instead
   of fixing generation quality.

## Improvement avenues (pick a lane)

### A. Enhancement done right (cheap, keep MuseTalk)

- **Alpha-blend the restored crop** with the raw MuseTalk crop at ~0.4–0.6
  instead of full-strength GFPGAN. Standard wav2lip-HQ community fix: keeps
  most sharpness, kills most plastic. One-line change in the enhance step.
- **Sharpen-only variant**: skip GAN restoration entirely; unsharp-mask or
  CAS on the crop. Zero identity/temporal risk; test whether "higher def"
  survives.
- **Temporal smoothing of the enhanced crop**: EMA across frames on the
  GFPGAN output (or on its delta vs raw) to remove mode-snapping.
- **Temporally-consistent face-video restoration** instead of per-frame GAN:
  FLAIR (MIT — the P0 candidate, see the license table below),
  BasicVSR++/RealBasicVSR (Apache-2.0). KEEP (ECCV 2024) and PGTFormer are
  license-blocked; CodeFormer is S-Lab NON-commercial — do not ship it.
- **Crop-box smoothing**: verify the paste-back bbox is fixed/smoothed, not
  re-detected per frame.

### B. Motion smoothness

- **Frame interpolation 24→48fps** (RIFE / FILM, both permissive) as a final
  pass. Directly attacks the strobing percept; cheap on L4; test for
  interpolation artifacts around teeth.
- Render/encode at source fps but present at higher fps only if source
  motion supports it; do NOT resample audio timing.

### C. Audio naturalness (likely underrated)

- **Prosody-aware normalization**: "A.I." with punctuation may synthesize
  with natural rhythm vs hard letter-spacing; try variants and keep the STT
  round-trip gate as the arbiter.
- **CosyVoice2 prosody controls / instruct mode**, longer or cleaner voice
  reference, explicit pause insertion, speaking-rate tuning.
- **Script-side rule for recorded clips**: openers and canned phrases should
  simply avoid initialisms — write around "AI" in marketing-voice scripts.
- Keep loudnorm (-16 LUFS / -3 dBTP) in every mux.

### D. Model tier (quality ceiling)

- **LatentSync 1.6** (512², diffusion): rendered — best articulation of
  any take (see D1 status below); adopted as the non-realtime recording
  tier once the 16-frame chunk-boundary hitch is smoothed. License
  verified Apache-2.0; 18GB VRAM minimum fits the L4.
- MuseTalk parameter passes: bbox_shift sweep, v1.5 config review.
- Watch the space: Hallo3 / EMO-class audio-driven portrait models for a
  future quality tier; evaluate strictly with the same QA protocol.

### E. Non-realtime HQ recording lane (owner-authorized)

Owner: "if you need to create a higher gpu high quality pipeline for
recording, non realtime just to make sure we can — try it. I would like to
record some clips of the standard phrases for beginning of convo so it's
perfect to start."

- Bigger GPU is allowed (A100/H100 spot in `openagentsgemini`) if the L4 is
  the bottleneck for diffusion-tier renders.
- Deliverable: a small library of PERFECT pre-recorded clips of standard
  opening phrases, starting with the live opener
  "Hello! I'm Sarah. What's on your mind today?" plus early-conversation
  standards (acknowledgments, "tell me more about that", pricing deflection
  to the human loop, closing/next-step lines).
- Each clip must pass the QA protocol below before it counts.
- Serving integration target: the owned renderer lane (OAV-4,
  `SARAH_AVATAR_RENDERER=owned`) and the semantic-answer cache (KHS-6) so
  cached/canned answers can play a recorded clip instead of a live render.

### F. Realtime lane implications (OAV-2/3, later)

Everything above is offline-tier. The realtime lane inherits whichever
recipe wins ONLY if it fits the frame budget (MuseTalk ~20 FPS on L4;
GFPGAN per-frame does not fit realtime on L4 today). Pre-recorded openers
buy latency headroom at session start regardless.

## QA protocol (what "beautiful" means operationally)

A take passes only if ALL of:

1. **Motion naturalness in playback** — a human (owner or agent watching
   rendered motion, not stills) sees smooth, humanlike articulation. Stills
   are NOT sufficient; per-frame GAN failures hide in stills.
2. Mouth-crop sharpness comparable to the rest of the frame (480×360 crop at
   `crop=480:360:400:250` on the 720p frame is the standard inspection view).
3. No temporal boil/flicker on 6+ consecutive-frame sequences and no
   identity drift; paste-back seam invisible.
4. STT round-trip: independent transcription of the final mux matches the
   source script verbatim (initialisms spoken as intended, no word slurs).
5. Loudness ≈ -16 LUFS integrated, true peak ≤ -3 dBTP.
6. A/V sync within one frame at start, middle, end.

## Open questions for contributing agents

- Which single change most improves the owner's motion percept: GFPGAN
  alpha-blend, sharpen-only, RIFE 48fps, or more natural audio pacing?
  (Testable independently; the takes and crops are all in GCS.)
- LatentSync license + L4 render economics for a ~10-clip opener library.
- Can CosyVoice2 prosody controls produce natural letter-speech, or should
  recorded scripts simply avoid initialisms?
- Is there a permissive temporally-consistent restorer (KEEP et al.) that
  runs at acceptable speed on L4 for 600-frame clips?

## Research addendum (2026-07-09, web-verified)

Root causes confirmed against literature and upstream repos:

- Per-frame single-image restorers on video cause "severe identity
  flickering" and boil (KEEP paper, arXiv 2408.05205); GFPGAN hallucinates
  different high-frequency detail every frame — looks sharper in stills,
  boils in motion. Matches the owner's report exactly.
- MuseTalk's own README admits "there exists some jitter as the current
  pipeline adopts single-frame generation" — no temporal module in 1.0/1.5.
  The un-enhanced soft output was acting as anti-jitter blur.
- "Mechanical mouth": per-phoneme pose targeting from clipped/letter-spaced
  audio reads as robotic; MuseTalk conditions on a short Whisper window
  (`--audio_padding_length_left/right`, default 2 — raise to 3–4 for more
  coarticulation).
- fps: MuseTalk trains at 25fps and recommends 25fps input; our footage is
  24fps — ensure `--fps 24` is explicit end-to-end or resample.

License verdicts (commercial use):

| Model | License | Usable? |
| --- | --- | --- |
| MuseTalk | MIT | yes |
| GFPGAN | Apache-2.0 | yes |
| LatentSync 1.6 | Apache-2.0 | yes (18GB VRAM fits L4; slow diffusion) |
| BasicVSR++ / RealBasicVSR | Apache-2.0 | yes (temporally consistent VSR) |
| FLAIR (WACV, arXiv 2311.15445) | MIT (official repo `wustl-cig/FLAIR`, pretrained models available) | yes — P0 temporal face restorer |
| RIFE (ECCV 2022) | MIT | yes (24→48fps final pass) |
| CosyVoice2 | Apache-2.0 | yes |
| Hallo2 | MIT | yes (offline opener candidates) |
| Hallo3 | code MIT; model weights derivative of CogVideo-5B | model license chain needs review |
| CodeFormer | S-Lab non-commercial | **NO** |
| KEEP (ECCV'24) | S-Lab non-commercial | **NO** |
| PGTFormer | non-commercial w/o permission | **NO** |
| StableVSR | unverified | blocked until checked |

CORRECTION (2026-07-09, `research.md`): an earlier draft of this section
claimed every good temporally-consistent face restorer is license-blocked.
That is now wrong — **FLAIR** (arXiv 2311.15445, WACV; official repo
`wustl-cig/FLAIR`, MIT, pretrained models available) converts an image
diffusion restorer into a video diffusion model with recurrent refinement
and temporal self-attention, and it is the **P0 candidate to replace
per-frame GFPGAN**. Until FLAIR is evaluated, the community-standard recipe
for our stack remains "tame GFPGAN": alpha-blend ~0.45 with the raw crop +
feathered mouth-only mask + temporal EMA on pixels and bbox coords. That,
plus punctuation-prosody audio (drop hard letter spacing; CosyVoice2 has
pronunciation-inpainting and instruct modes), is the v3 recipe now in
flight. Local PDF copies of the underlying papers live in the workspace
root repo under `projects/papers/` (see its manifest).

## Adopted plan (from `research.md`, 2026-07-09)

The owner's prioritized research doc `docs/sarah/research.md` is now the
experiment authority for this lane. Its triage:

- **P0 implement now:** LatentSync 1.6 (Apache-2.0, offline quality tier),
  FLAIR (MIT, temporal face restorer — GFPGAN replacement candidate),
  RIFE (MIT, 24→48fps final pass), CosyVoice2/3 prosody variants
  (Apache-2.0), MuseTalk 1.5 tuning (fps lock, bbox sweep, audio padding),
  BasicVSR++/RealBasicVSR (Apache-2.0, conservative temporal VSR), and
  GFPGAN kept ONLY tamed (alpha 0.35–0.55 + feathered mask + EMA delta).
- **P1 prerecorded openers:** Hallo2 (MIT); Hallo3 code is MIT but the
  model weights inherit CogVideo-5B terms — review before shipping.
- **Research-only/blocked:** KEEP, PGTFormer, CodeFormer (non-commercial),
  BFVR-STC (CodeFormer-derived, unclear), StableVSR (repo MIT but the
  StableSR/SD weight chain needs review).

Experiments run in the A0–F2 matrix order defined in `research.md`
(same script, same source clip, same crop inspection, same loudnorm, same
owner playback protocol). Empirical status:

| Test | Status |
| --- | --- |
| A2+A3 (tamed GFPGAN — the v3 recipe) | DONE — measured smoothest take (jerk 0.15) |
| D1 (LatentSync 1.6) | DONE — best articulation, but a 16-frame chunk-boundary hitch (jerk in phases 14–15 ≈ 1.9× baseline) |
| B1/E1 (RIFE 48fps over best A-take / LatentSync) | queued |
| F1 (FLAIR over raw MuseTalk / LatentSync) | queued |
| C1 (CosyVoice prosody variants) | queued |
| Hallo2 opener comparison | queued |

Local reference clones for all of the above now exist in the workspace
root repo under `projects/repos/`: `LatentSync`, `GFPGAN`, `ECCV2022-RIFE`,
`Practical-RIFE`, `BasicVSR_PlusPlus`, `RealBasicVSR`, `hallo2`,
`StableVSR` — alongside the already-present `MuseTalk`, `CosyVoice`,
`livetalking`, and `OpenAvatarChat`. FLAIR is `wustl-cig/FLAIR` on GitHub.
Papers are mirrored in the workspace `projects/papers/`.

Host access, run scripts, and receipts: see #8610/#8611 comments and
`docs/sarah/2026-07-09-oav1-offline-proof-receipt.md`. Do not print secrets;
host is billed hourly — coordinate before stopping it.

## Round 3 (2026-07-09 late): opener library v1 FAILED owner playback — short-clip program

Owner verdict on the full v1 opener library (5 scripts × v3/LatentSync):
**all clips fail** ("all the sarah-openers were shit"), despite passing
stills, jerk metrics, and word-level STT. Two QA lessons harden into rules:

1. Stills + motion statistics + word-level STT are NOT sufficient gates.
   Playback verdicts and prosody judgment must gate every take (SQ-1 #8618).
2. Short clips (4–6s) are a HARDER problem than long takes, not an easier
   one: prosody instability dominates, and there is no time for the viewer
   to acclimate to any visual artifact.

Root-cause hypotheses for the short-clip failure, ranked:

- **Zero-shot TTS prosody instability on short texts.** CosyVoice zero-shot
  conditions on one short reference; 4–6s scripts routinely come out with
  wrong intonation ("Hello?" rising on the greeting), robotic pacing, or
  flat affect. A long take averages out; an opener IS its prosody.
- **Source-motion conflict.** Openers 01/02 used big-smile, high-motion
  source clips (mouth-region motion 3.7–4.2 vs 1.2 on the long take).
  Replacing the mouth on a smiling face reads uncanny: cheeks/eyes say one
  thing, the synthesized mouth says another.
- **Model-tier ceiling on expressiveness.** Both MuseTalk (inpaint-only)
  and LatentSync (lip-region diffusion) keep the source head motion; neither
  can generate greeting-appropriate head/face motion for a scripted line.

Active lanes (parallelized 2026-07-09, all pushing to main + GCS):

| Lane | What | Where |
| --- | --- | --- |
| Audio bake-off | per-script candidate pools: CosyVoice seeds + instruct-mode + 30–60s concatenated voice ref + Chirp 3 HD benchmark; gates = per-segment STT + **LLM audio judge** (Gemini rates naturalness/warmth/confidence 1–10 with defect notes) | L4 prod host, openers-v2 |
| Calm-clip re-render | v3 recipe over neutral low-motion source clips only (<1.5 mouth-motion), winning audio from the bake-off | L4, openers-v2 |
| Still-animation tier | **Hallo2** (MIT) animates a single still with generated head/face motion matched to speech — eliminates source-motion conflict entirely; the strongest paper candidate for scripted openers | fresh A100 spot instance |
| RIFE 48fps | presentation-layer strobing test on the best take | L4 |
| FLAIR | temporally-consistent crop restoration replacing tamed GFPGAN | queued after the above |

Production note: `/sarah` flipped to the OWNED pipeline on 2026-07-09
(`f5f9cb3725`): hydralisk-avatar (MuseTalk realtime + WebRTC WHEP) and
hydralisk-tts live on the GPU host behind caddy/sslip.io; e2e verified from
the public internet (ICE completed, idle+speak video flowing, full
brain→TTS→render spoken turn). Fixes landed during bring-up: aiortc answerer
track pairing (hydralisk `efb17d1`), keepalive-vs-warmup race (`f291823`).
Hardening list lives in SQ-4 (#8621); the quality program issues are
SQ-1..SQ-8 (#8618–#8625).
