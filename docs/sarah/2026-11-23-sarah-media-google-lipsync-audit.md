# Sarah Media, Google Stack & Lip-Sync Historical Audit + Completion Plan

- **Date:** 2026-11-23
- **Baseline:** `origin/main` at `eaa6a19155`
- **Status:** Historical audit + actionable engineering plan (no live cloud mutations or spend executed)
- **Sources inspected:**
  - Deleted `docs/sarah/` corpus (59 files) and `scripts/sarah-avatar/sarah-direction.json` at `dabc08102f~1` (deleted by the 2026-09-18 Nuke `dabc08102f`)
  - Deleted Sarah episode video runners at `a098b3e29f~1` (`scripts/sarah-avatar/*.mjs`, deleted 2026-08-28)
  - Deleted `apps/sarah/` live avatar surface and `packages/sarah-take-scoreboard/` at `13bc1e7443~1` (removed 2026-07-10)
  - Deleted desktop/Sarah voice receipts and transcripts at `dabc08102f~1` (`docs/voice/`, `docs/sol/`, `docs/omega/`, `docs/transcripts/260–271.md`)
  - Cross-repo voice audit at `0b401561a0^:docs/audits/2026-10-21-voice-infrastructure.md` (deleted by the 2026-10-27 Nuke `0b401561a0`)
  - Current Coax / Metaverse / Psionic voice, Google gateway, and speaking-character code on `origin/main`

---

## Executive Summary

1. **Image & visual origin (not Imagen/Veo):** Sarah's face and source footage were **owner-created Midjourney synthetic sci-fi assets** (one canonical portrait `v2_1783572530279-62463022.png` plus ten uniform 8-second 720p/24 fps H.264 clips). Because she is synthetic rather than a first-person human likeness, HeyGen LiveAvatar's likeness policy blocked uploading her as a custom avatar — which directly triggered the Owned Avatar Video (OAV) pipeline. Google Cloud Storage (`gs://openagentsgemini-oa-artifacts/sarah-avatar/`) hosted the portrait, clips, voice references, and renders, using locally signed V4 read URLs to feed external talking-avatar APIs (Segmind). Cinematic video models (including Veo, Seedance, Wan, Luma, PixVerse) were explicitly excluded for talking-head lip sync.
2. **How Google powered Sarah across seven layers:**
   - **LLM / Brain:** Google Generative Language API (`gemma-4-31b-it`, `gemma-4-26b-a4b-it`), then Khala Gateway's persona-neutral lane (`openagents/internal-neutral`, Gemma-4-led with Vertex Gemini overflow), plus Vertex Gemini (`gemini-2.5-flash` / `gemini-4-*`).
   - **Speech-to-Text (STT):** Google Cloud Speech-to-Text V2 (`chirp_3`, `us` recognizer) for voice gateways and round-trip verification (2.2–6.2 s final wall time on Cloud Run in Jul 2026; 292 ms local REST on Mac in Oct 2026).
   - **Text-to-Speech (TTS):** Google Cloud **Chirp 3 HD** bidirectional streaming (`en-US-Chirp3-HD-Sulafat`, measured **163–198 ms** first byte — fastest live voice OpenAgents ever measured) and owner-locked **Leda** voice (`sarah-direction.json` & `pro.openagents.com` live persona). In Aug 2026, `OpenAgentsInc/voice` also ran **Gemini Live** (`gemini-2.5-flash-native-audio`, `gemini-3.1-flash-live-preview`) and **Gemini Flash TTS** in a Google cascade.
   - **Automated Audio QA & Judging:** Independent Gemini STT round-trip checks and a **Gemini 3.5 Flash** LLM audio judge scoring 100+ TTS bake-off takes 1–10 on naturalness, warmth, and confidence.
   - **Storage, Compute & Secrets:** GCP project `openagentsgemini` — GCS (`openagentsgemini-oa-artifacts`), GCE L4 (`sarah-avatar-gpu-1`), GCE A100 SPOT (`sarah-hallo2-exp-1`), Cloud Run, Secret Manager, and a later GKE LiveKit cluster (`oa-livekit-prod`, torn down 2026-10-28).
3. **Lip-syncing to audio research & hard-won laws:** The July 2026 OAV program (`research.md`, `2026-07-09-oav-quality-strategy.md`, 13 scored takes) empirically established:
   - **MuseTalk 1.5 (MIT)** proved realtime 2D mouth-region inpainting over owned 720p footage (20 FPS stock / 24 FPS strided over WebRTC on one L4), with pixel-perfect identity outside the mouth — but a 256² teeth-softness ceiling and source-motion conflict on smiling clips.
   - **Full-strength per-frame GFPGAN is banned:** it passed stills QA and **failed owner playback** ("higher def but more choppy, way less humanlike") due to per-frame GAN mode-snapping. **Tamed GFPGAN (`v3`)** (alpha 0.45 + feathered mouth mask + temporal EMA on delta) measured smoothest on the MuseTalk lane (`jerk mean 0.15`).
   - **LatentSync 1.6 (Apache-2.0)** delivered the best mouth articulation and natural teeth at 512², but ran ~31× slower than realtime on an L4 and suffered a rhythmic **16-frame chunk-boundary hitch** (every 0.64 s).
   - **Hallo2 (MIT)** full-portrait still animation on A100 eliminated source-motion conflict and earned the **only owner playback approval** in the OAV program (`opener-05-show-you-hallo2.mp4` called "close to shippable"), at native 512². Both Hallo2's `video_sr.py` (×4) and **FLAIR** (`wustl-cig/FLAIR`) were **rejected** — both pull in S-Lab non-commercial CodeFormer weights, and FLAIR measured ~33 min per second of video on an A100.
   - **Segmind Async V2 (`p-video-avatar`)** became the proven default for **produced episode videos** (1080p, 24 fps, up to 60 s continuous takes, interleaved with live product screenshares and ElevenLabs `music_v2` beds).
4. **We do not start from zero today (`origin/main`):** The Coax rewrite already has Google Chirp 3 STT + Chirp 3 HD TTS + Vertex Gemini wired (`metaverse/voice-host/speech.c`, `coax/web/speak.coax`, `coax/lib/gateway/vertex.coax`), `#authority` VAD/turn/barge-in/floor/metering (`metaverse/voice*.coax`, `pay/voice-meter.coax`), local GPU **Psionic Parakeet TDT 0.6B v3** STT, a **working 14-viseme phoneme-aligned talking-character pipeline** (`coax/web/tool.sh speak` + `metaverse-engine/audio/speak.coax` + MetaHuman Lab), and Coax ElevenLabs + Vertex Gemini audio judging + EBU R128 loudness (`metaverse-engine/audio/sfx-tool.coax`, `measure.coax`). Even Foreman's current voice (`en-US-Chirp3-HD-Sulafat`) was explicitly carried over from the Sarah era (`045bcc80d6`).
5. **What is needed to complete Sarah now:** Verify survival of `gs://openagentsgemini-oa-artifacts/sarah-avatar/`, restore Sarah's direction profile (`Leda` + `voice_anchor` + register), add Sarah (`en-US-Chirp3-HD-Leda`) to `metaverse/voice-host` with streaming Chirp 3 HD, and then execute only the chosen visual track(s): **Track A** (Coax CLI for produced 1080p Segmind/audio-first episodes, 2–4 days), **Track B** (owned GPU realtime 2D lip-sync sidecar benchmarking Ditto / SoulX-FlashHead-Lite / MuseTalk v3 + RIFE, 1–2 weeks), **Track C** (3D Coax/Metaverse speaking character at 60 fps WebGPU with zero GPU rent, 3–5 days), and/or **Track D** (sovereign neural TTS weights in `psionic/lib/speak.coax`).

---

## 1. Historical Timeline & Deleted Corpus Inventory

Sarah evolved across six distinct eras between July and November 2026. Most voice and avatar systems did not end because the underlying media tech stopped working; they were removed during repo resets, owner product pivots, or provider likeness/voice refusals.

| Era | Dates | Surface / Stack | Key Commits | Outcome |
| --- | --- | --- | --- | --- |
| **1. Sales Avatar & OAV Program** | 2026-07-07 – 2026-07-10 | Private `OpenAgentsInc/sarah` (`openai/gpt-realtime-2`) → monorepo `apps/sarah` (Bun + Effect Native) → HeyGen LiveAvatar FULL+Custom LLM → Owned pipeline (Gemma 4 / Khala gateway + Chirp 3 HD & CosyVoice2 + MuseTalk 1.5 WebRTC on GCE L4 + Hallo2 MIT openers on A100) | `627e303a6a`, `974490e7ba`, `2778ac5a81`, `934bb44c09`, `ebb024d9da`, `53b5137f1e`, `f5f9cb3725`, `f50bbcbeb8`, `fc4e8e1bce`, `653550af07` | Removed at owner direction in `13bc1e7443` (192 files); `#8610` closed WONTDO; GPU host stopped. |
| **2. Desktop Voice & Orchestrator Reboot** | 2026-07-12 – 2026-07-19 | Persistent Desktop Voice Mode (`#8733`: Rust `cpal` helper + Cloud Run Effect gateway + Google Chirp 3 STT + Chirp 3 HD `Sulafat` streaming TTS, 2 ms p95 barge-in). Sarah rebooted as owner orchestrator (`packages/sarah`) with mobile OpenAI `gpt-4o-mini-tts` (`marin`) listen button. | `e0f3612345`, `385820682b`, `a9c50fa401` | Electron deleted (`a299a2ef43`); mobile retired (`090b7a4a9a`); TS audio deleted (`d7574f6669`). |
| **3. Produced Video Pipeline (Segmind + RC Assembly)** | 2026-07-22 – 2026-07-24 | Programmatic produced talking-head videos via Segmind Async V2 (`p-video-avatar`, GCS V4 signed portrait URL, persistent `sarah-direction.json` locking `Leda`), Omega live screenshare interleaving, cut-point Finder/Desktop QC, `Zed` British TTS lint, ElevenLabs `music_v2` bed. | `f907f5f172`, `39f354c5ec`, `409f5c3c57`, `6d29c3fe26`, `dc435014fa`, `74654db48b`, `ae9b8aa12a`, `ba432028ca` | Node runners deleted in Wave 4 (`a098b3e29f`, 2026-08-28); `sarah-direction.json` + `docs/sarah/` deleted in Nuke (`dabc08102f`, 2026-09-18). |
| **4. Realtime Gateways, LiveKit & `pro.openagents.com`** | 2026-07-28 – 2026-08-19 | Managed OpenAI Realtime (`gpt-realtime-2.1`) → Self-hosted GKE LiveKit (`13.7–15.2 s` first audio) → `OpenAgentsInc/voice` (Rust LiveKit + Gemini Live + Google cascade: Gemini Live STT + Gemma 4 31B + Gemini Flash TTS, `Leda` voice on `pro.openagents.com`, military register retired in `761e3bd12e`) → Elixir Phoenix LiveView (`OpenAgentsInc/sarah`, OpenAI Realtime WebRTC). | `82e9f00fdb`, `0ec4ea19b6`, `c4906fbd89`, `761e3bd12e`, `19a362b11a`, `9a292b987c` | OpenAI refused Sarah's custom voice (Ep 270); GKE LiveKit code deleted (`d7574f6669`) and cluster torn down 2026-10-28. |
| **5. Repository Resets ("Nukes")** | 2026-09-18 & 2026-10-27 | Legacy TS/Rust workspaces removed cleanly to rebuild on Coax, Metaverse Engine, and Psionic. | `dabc08102f`, `0b401561a0` | Full history preserved in git (`dabc08102f~1`, `0b401561a0^`). |
| **6. Coax-Native Voice & Speaking Characters** | 2026-10-28 – 2026-11-23 | Owner locks Google Chirp 3 STT + Chirp 3 HD TTS behind Coax `#authority` voice engine (`3319b93cd9`), Foreman adopts Sarah-era `Sulafat` (`045bcc80d6`), Psionic Parakeet TDT v3 GPU STT lands (`faee883a01`, `67aadf9e2c`), and Coax Speaking Characters MVP (`6d511975f5`, `b28103de59`) delivers Chirp 3 HD + STT word timing + 14-viseme MetaHuman lip sync. | `3319b93cd9`, `045bcc80d6`, `faee883a01`, `6d511975f5`, `b28103de59` | **Active on `origin/main` today.** |

### Inventory of the Deleted `docs/sarah/` Folder (`dabc08102f~1`)

All 59 files that existed in `docs/sarah/` before `dabc08102f` fall into five groups:

| Category | Historical Paths (`dabc08102f~1:docs/sarah/...`) | Core Content Recovered |
| --- | --- | --- |
| **Avatar / Lip-Sync / Voice Research & Specs (9 files)** | `2026-07-09-owned-avatar-video-pipeline-spec.md`, `2026-07-09-oav-quality-strategy.md`, `research.md`, `2026-07-09-oav1-offline-proof-receipt.md`, `2026-07-09-oav2-render-service-closeout.md`, `2026-07-09-liveavatar-integration-assessment.md`, `2026-07-09-pipecat-voice-infra-audit.md`, `2026-07-09-sarah-quality-next-steps-assessment.md`, `2026-07-09-sq4-hardening-receipt.md` | Full OAV-1..6 architecture, MuseTalk 1.5 / LatentSync 1.6 / Hallo2 / FLAIR / RIFE / CosyVoice2 experiments, license clearance table, LiveAvatar FULL vs LITE teardown, Pipecat pattern extraction, SQ-1..8 quality bar. |
| **Produced Video & Persona Runbooks (4 files)** | `2026-07-22-segmind-talking-avatar-pipeline.md`, `ACTING_AS_SARAH_RUNBOOK.md`, `2026-07-22-episode-261-script.md`, `2026-07-22-sarah-introduction.md` | Segmind Async V2 talking-avatar runner, GCS V4 signed URLs, Episode 261/262 RC assembly & lessons, ElevenLabs music bed, voice register rules (`Leda`, retirement of military framing), `pro.openagents.com` persona linkage. |
| **Media Quality Scoreboards & GPU Receipts (31 files)** | `QUALITY_SCOREBOARD.md`, `GPU_MEDIA_RUN_CLOSEOUT.md`, `receipts/2026-07-10-openers-v2-{hallo2,r2}-a100.json`, `scoreboards/README.md`, `scoreboards/index.ndjson`, plus `.json`/`.md` pairs for `sarah-reply-{v1,hq,enhanced,v3,latentsync}`, `sarah-openers-v1`, and `openers-v2-opener-01..05` (+ `r2` on 02 & 05) | Machine-validated `sarah-take-scoreboard.v1` and `openagents.sarah.gpu_media_run_closeout.v1` receipts with measured jerk, motion, LUFS, STT round-trip, Gemini judge scores, GPU wall time, USD cost, and verbatim owner playback verdicts. |
| **Brain, Contracts, Memory & Ops (12 files)** | `README.md`, `2026-07-09-khs1-khala-gateway-migration.md`, `2026-07-09-blueprint-map-surface-audit.md`, `SARAH_CONTRACTS.md`, `SARAH_KNOWLEDGE_BASE.md`, `SARAH_TWEET_QUEUE.md`, `EN-GAPS.md`, `2026-07-19-sarah-{program-assessment,activation-gap-analysis,activation-status}.md`, `2026-07-22-sarah-{autonomous-operations-program,company-command-analysis}.md`, `2026-07-22-note-to-sarah-memory-upgrade.md`, `2026-07-24-sarah-memory-on-nostr-audit.md` | Gemma 4 → Khala `openagents/internal-neutral` inference migration (persona-bleed fix), behavior contracts, Blueprint KB, activation & Nostr memory specs. |
| **Migration & Pre-Monorepo History (3 files)** | `MIGRATION.md`, `historical/private-sarah-readme.md`, `historical/vercel-production-runbook.md` | Consolidation from private `OpenAgentsInc/sarah` (Vercel + AI SDK `openai/gpt-realtime-2` + Eve). |

**Companion deleted code outside `docs/sarah/`:**
- `scripts/sarah-avatar/` (`a098b3e29f~1`): `segmind-talking-avatar.mjs`, `segmind-paste-lint-lib.mjs`, `assemble-rc.mjs`, `assemble-rc-lib.mjs`, `elevenlabs-music-bed.mjs`, `elevenlabs-music-bed-lib.mjs`, tests, and `sarah-direction.json` (kept until `dabc08102f`).
- `apps/sarah/` (`13bc1e7443~1`): `owned-renderer.ts`, `opener-clips.ts`, `liveavatar.ts`, `google-inference.ts`, `voice-stream-coordinator.ts`, `voice-fragment-coalescer.ts`, `avatar-clip-layer.ts`, `avatar-media-health.ts`, `gpu-media-run-closeout.mjs`.
- `packages/sarah-take-scoreboard/` (`13bc1e7443~1`): schema, validator, and Markdown/NDJSON renderer for `sarah-take-scoreboard.v1`.
- Related cross-docs (`dabc08102f~1`): `docs/transcripts/SARAH_VIDEO_SCREENSHARE.md`, `docs/voice/2026-07-12-persistent-desktop-voice-mode-audit-and-plan.md`, `docs/voice/evidence/2026-07-12-audio-{2,7,8}-*.md`, `docs/sol/2026-07-18-sarah-openai-voice-api-audit.md`, `docs/omega/2026-07-28-managed-sarah-realtime-voice-gateway.md`.

---

## 2. Image & Visual Asset Pipeline Forensics

### 2.1 Source Assets (Synthetic Midjourney Character, Not Imagen/Veo)

Sarah's visual identity was created offline in **Midjourney** (a stylized, sci-fi presenter inspired by StarCraft's Sarah Kerrigan — Episode 260, `dabc08102f~1:docs/transcripts/260.md`). Google Imagen and Google Veo were **not** used to generate her face or talking-head video; the Segmind runbook explicitly warned against using Veo or other cinematic video generators (`Seedance`, `Veo`, `Wan`, `Luma`, `PixVerse`) because they animate scenes rather than synchronizing lips to speech.

Because Sarah is fully synthetic, OpenAgents owns the visual asset outright with zero human-likeness consent restrictions — which paradoxically caused HeyGen LiveAvatar's first-person human-likeness policy to reject her (`2026-07-09-owned-avatar-video-pipeline-spec.md`).

| Asset | Historical Location / URI | Format & Specs | Role & Recorded Caveats |
| --- | --- | --- | --- |
| **Canonical Portrait Still** | Local `~/Downloads/sarah/v2_1783572530279-62463022.png`; staged in GCS `gs://openagentsgemini-oa-artifacts/sarah-avatar/src/<portrait>.png` | PNG still (sci-fi portrait) | Master input for Segmind produced-clip generators (`p-video-avatar`, `kling-v2-*-avatar`) via 6h GCS V4 signed URL. |
| **Calm Square Portrait Crop** | `source_square.jpg` (derived from the calm Sarah source set) | Square face crop (`512×512` input) | Single-still input for Hallo2 `openers-v2` renders (eliminated source-motion conflict). |
| **10 Animated Source Clips (`clip0..clip9`)** | Local `~/Downloads/sarah/`; GCS `gs://openagentsgemini-oa-artifacts/sarah-avatar/footage/` | Uniform H.264, `1280×720`, `24 fps`, `yuv420p`, `8.000 s` each | Pre-graded expression state library for OAV realtime MuseTalk rendering (stream-copy stitchable without re-encode). |
| — `clip6` / `clip3` | Inside `footage/` | `-46.9 dB` quiet idle (`clip6`), idle/filler (`clip3`) | Idle state loop when Sarah is silent. |
| — `clip4` / `clip5` | Inside `footage/` | Listening/animated with smile ending (`clip4`), subtle smile transition (`clip5`) | Listening state; **`clip4` was the true voiced source** for `sarah_voice_ref_v1.wav` (5.40 s, STT confidence 1.00). |
| — `clip8` (`v2_traced-df7d3b47…`) / `clip0` | Inside `footage/` | Talking poses; `clip8` trimmed to **frames 0–184 (7.71 s)** | Primary OAV-1 render base. **Two traps documented:** (1) frames 185–191 drop her head with no detectable face, crashing MuseTalk's `blending.face_seg` (`ValueError: height and width must be > 0`); (2) its `-11.1 dB` audio track is non-speech (0 s billed by Google STT, empty Whisper). |
| — `clip7` / `clip9` | Inside `footage/` | Stylized UI-overlay shots | Intro/outro visual flair only. |

### 2.2 How Google Cloud Storage Hosted & Signed Portraits

Segmind's talking-avatar endpoints fetch the portrait server-side and reject base64 data URIs. Because buckets in GCP project `openagentsgemini` enforce **uniform bucket-level access** (no per-object ACLs), the pipeline hosted images privately in GCS and generated short-lived V4 read URLs locally with the automation service account key (requiring no IAM `signBlob` call):

- **Bucket:** `gs://openagentsgemini-oa-artifacts`
- **Prefix:** `sarah-avatar/` (`src/`, `footage/`, `voice-ref/`, `oav1/`, `openers/`, `openers-v2/`, `transfer/`)
- **Auth paths (gitignored):** `CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config` (`oa-mvp-automation` SA) for `gcloud storage cp`, and `~/work/.secrets/gcp-mvp-automation.json` for local 6-hour V4 URL signing.

### 2.3 Canonical Visual Direction & Episode Assembly Rules

Owner director notes were persisted in `scripts/sarah-avatar/sarah-direction.json` (schema `openagents.sarah.avatar_direction.v1`, last updated `2026-07-23`) so character appearance and framing stayed locked across every generated video:

- **Visual prompt (`video_prompt`):** *"Composed, still, and direct to camera. Neutral closed-lip expression; no persistent smile. Steady eye contact, restrained natural head movement and blinking. ... Cinematic sci-fi lighting that matches the source portrait. Fixed camera."*
- **Negative prompt (`negative_prompt`):** Explicitly suppresses `big toothy grin, wide open-mouth smile, persistent smile, presenter smile, smirking during warning, over-cheerful, bubbly, laughing, giddy, subtitles, text, watermark, blurry, distorted face, extra fingers, frozen static face`.
- **Resolution & Take Length:** `1080p`; prefer continuous takes **up to 60 seconds** rather than splitting per sentence, hiding any necessary performance joins under full-screen cutaways.
- **Episode A/B/C Screenshare Interleaving (`assemble-rc.mjs`):** Continuous Sarah spoken audio from `0 → D`, cutting picture only: `A` (Sarah master) → `B` (live product screenshare at `1280×900` non-maximized with FIT-decrease dark pad `0x0E0E10` + optional second mid cutaway still) → `C` (Sarah master). Cut point `T1` derived from silence detection after a spoken cue; `T2` near `0.78 * D` verified on audio; automated cut-point pixel QC at `T2 - 1 s`, `T2 - 0.5 s`, and `T2 - 1 frame` rejecting Finder/Desktop leak frames; SIGTERM-only window quit (never `Cmd+Q`).

---

## 3. Audio, Voice Identity & Google Stack Forensics

### 3.1 Complete Map of Google Usage Across Sarah

Google Cloud (`project: openagentsgemini`, primarily `us-central1` and `us` multi-region) was the backbone of Sarah's inference, live speech, automated media QA, storage, and GPU compute:

| Layer | Google Product / Endpoint | Exact Models / Voices Used | Measured Results & Historical Notes |
| --- | --- | --- | --- |
| **Sarah Brain (Direct)** | Generative Language API (`generativelanguage.googleapis.com/v1beta`) | `gemma-4-31b-it` (default dense), `gemma-4-26b-a4b-it` (MoE) | Filtered `thought: true` scratchpad parts before speech/transcripts; no native tool calling on direct Gemma 4 lane (`apps/sarah/src/services/google-inference.ts`). |
| **Sarah Brain (Gateway)** | Khala Gateway (`POST /api/v1/chat/completions`) → Vertex AI overflow | `openagents/internal-neutral` (Gemma 4 primary, Vertex Gemini overflow) | Created after `openagents/khala` injected "We are Khala" over Sarah's prompt (`#8600` FC-BRAIN). Metered via `x-openagents-demand-kind: internal` / `x-openagents-demand-source: sarah` with 5M/day token cap. |
| **Realtime Voice Brain & Cascade (Aug 2026)** | Vertex AI / Gemini Live (`OpenAgentsInc/voice`) | `gemini-2.5-flash-native-audio`, `gemini-3.1-flash-live-preview`; cascade: Gemini Live STT + Gemma 4 31B + Gemini Flash TTS | Local 505 ms agent audio after admission (324 ms Rust output); ~0.9 s prod first spoken text; required explicit English lock (Gemini STT misheard `"no"` as Portuguese `"Não"`). |
| **Streaming & Batch STT** | Google Cloud Speech-to-Text V2 (`us-speech.googleapis.com/v2/projects/openagentsgemini/locations/us/recognizers/_:recognize`) | `chirp_3` (`en-US`, `LINEAR16` 16 kHz / 24 kHz mono) | Used for Desktop Voice Mode (`#8735` AUDIO-2), OAV-3 voice-ref verification (0.92 confidence on CosyVoice2 clone, proved clip 8 had 0 s speech), and today's `metaverse/voice-host`. Measured 2,207–6,153 ms final wall time on Jul 2026 Cloud Run; 292 ms local REST on Mac in Oct 2026. |
| **Streaming & Batch TTS** | Google Cloud Text-to-Speech (`texttospeech.googleapis.com`, bidirectional `streaming_synthesize` & REST `v1/text:synthesize`) | **Chirp 3 HD:** `en-US-Chirp3-HD-Sulafat`, `Leda`, `Kore`, `Zephyr`, `Charon`, `Aoede` (`LINEAR16`, 24 kHz / 16 kHz) | **163–198 ms** time to first byte (AUDIO-7/8 & OAV-3 live voice); **2 ms p95** server barge-in ACK (`#8740`). Benchmarked **6–8/10** in openers-v2 audio bake-off. Instant custom voice was allow-list gated, so stock Chirp 3 HD voices shipped. |
| **Automated QA & Audio Judge** | Gemini multimodal API / Vertex AI | `gemini-3.5-flash` (openers-v2 bake-off), Gemini STT round-trip | Scored 100+ TTS candidates (waves 1–3) on a 1–10 naturalness / warmth / confidence rubric with structured defect notes; ran independent verbatim STT checks on final muxes. |
| **Artifact Storage & URL Signing** | Google Cloud Storage (`gs://openagentsgemini-oa-artifacts/sarah-avatar/`) | Uniform bucket-level access + local SA V4 signed URLs | Held all 10 clips, portrait still, voice references, OAV-1 renders, openers v1/v2, bake-off JSONs, and a 16.4 GiB CosyVoice env tarball (`transfer/cosy_env_20260710.tar`). |
| **GPU & Hosting Compute** | GCE GPU VMs, Cloud Run, GKE, Secret Manager | `sarah-avatar-gpu-1` (`g2-standard-8`, 1× L4 24 GB, `us-central1-b`, `~$0.85/h`); `sarah-hallo2-exp-1` (`a2-highgpu-1g`, 1× A100-40GB SPOT, `us-central1-f`, `~$1.30/h`); `openagents-audio-staging` (Cloud Run); `oa-livekit-prod` (6-node GKE, **deleted 2026-10-28**) | L4 ran Hydralisk MuseTalk + CosyVoice2 (`~$6` for OAV-1); A100 ran Hallo2 openers-v2 + FLAIR test (note: 22 h idle stall burned `~$29` on Jul 9–10, motivating SQ-8 closeout law). |

### 3.2 Sarah's Voice Identity Evolution & Direction Lock

Across July and August 2026, owner feedback converged on **one canonical voice everywhere**:

1. **`Zephyr (Female)` (2026-07-22, first Segmind test):** Worked cleanly, but owner judged it too bright/cheery.
2. **`Kore (Female)` (2026-07-22):** Deeper and calmer, but a per-clip *"low and dry"* voice direction accidentally pushed one generated clip into a masculine voice.
3. **Gender Lock (`voice_anchor`, 2026-07-22):** Added a profile-level anchor prepended to *every* `voice_prompt` regardless of per-clip overrides:
   > `"A woman's voice, distinctly feminine and never masculine."`
4. **Owner Lock to `Leda` (2026-07-23 & 2026-08-10):**
   - Locked **`Leda (Female)`** (`English (US)`) in both `scripts/sarah-avatar/sarah-direction.json` (produced videos) and `pro/lib/openagents-voice/persona.ts` (`openagents.public.voice.v5-pro.1` on `pro.openagents.com`, documented in `ACTING_AS_SARAH_RUNBOOK.md` §4a).
   - Owner retired the Episode 261 tactical/military register on 2026-08-10 (`761e3bd12e`): Sarah's ordinary register is **assertive, composed, direct, dry warmth, plain professional English, one or two short spoken sentences, one question at a time** (no combat/mission/deployment allegory unless explicitly directed for a single episode).

*(Note: The `Zephyr`, `Kore`, `Leda`, and `Sulafat` names are shared across Google's Chirp 3 HD / Gemini TTS voice family and Pruna's 30-voice catalog on Segmind. When Foreman's voice was set in `metaverse/voice-host/speech.c` on 2026-10-29 in commit `045bcc80d6`, it explicitly reused the Sarah-era `en-US-Chirp3-HD-Sulafat` voice.)*

### 3.3 Non-Google Audio Providers Tested & Why Each Hit Limits

| Provider / Model | Where Used | Measured Performance | Limitations / Verdict |
| --- | --- | --- | --- |
| **CosyVoice2-0.5B** (Apache-2.0, FunAudioLLM, self-hosted L4/A100) | OAV-1 / OAV-3 / openers-v1 & v2 zero-shot voice clone from clip 4 (`sarah_voice_ref_v1.wav`, 5.40 s) and 30–60 s concatenated long ref | Warm RTF `0.81–0.897`; cold load `20.2 s`; **3.0–5.8 s time to first chunk** (`#8613`). Openers-v2 Gemini judge: `9/10` on opener-02 (`w2-slow92-s31415`) & opener-04 (`lr-s555`), `8/10` on opener-05 (`w2-pb-s777`), **plateaued at `7/10`** on opener-01 & opener-03 across 60+ candidates. | Too slow for live first-chunk latency (Chirp 3 HD stayed live). Pronounced `"AI"` as `"eye"` without `tts/normalize.py`; letter-spaced `"A I"` caused staccato visemes (fixed via punctuation-prosody `"A.I."` or no-initialism scripts). Wave-1 `instruct2` read prompts aloud until terminated with `<\|endofprompt\|>` (still scored only 3–6/10). Escalation path named: CosyVoice3 / stronger TTS tier. |
| **OpenAI TTS & Realtime** (`gpt-4o-mini-tts` `marin`, `gpt-realtime-2` / `2.1`) | Private `sarah-prev`, mobile Sarah listen (`a9c50fa401`), managed voice gateway (`#9272`), GKE LiveKit (`#9283`), Elixir Phoenix (`openagents.com`) | Managed/Elixir staging WebRTC: call setup p50 `865 ms`, first remote audio p50 `969 ms`. GKE LiveKit: `13.7–15.2 s` first audio. | **Refused Sarah's custom voice** (Ep 270, line 33); frequent session drops (`"voice session failed"`); tool-call visibility issues in voice mode; provider lock-in. |
| **Segmind Pruna Built-in TTS** (`p-video-avatar`) | Produced episodes (Ep 261, Ep 262) | Single-call 1080p MP4 with built-in 30 voices (`Leda`, `Kore`, `Zephyr`), `voice_prompt` + `voice_anchor`. | Convenient for offline single-call episodes, but required `segmind-paste-lint-lib.mjs` to block standalone uppercase `Zed` under British voices (mispronounced `"Zeed"`). No decoupled audio reuse unless `--audio <url>` is passed. |
| **ElevenLabs Music (`music_v2`)** | `scripts/sarah-avatar/elevenlabs-music-bed.mjs` (`#9235`) | `POST /v1/music` with `force_instrumental=true`; mixed via ffmpeg `loudnorm → volume=-3dB → fades → amix → alimiter` preserving `*-rc-no-music.mp4` clean plate. | Proven for episode background beds (`~/work/.secrets/elevenlabs.env`). Not used for Sarah's speaking voice. |

### 3.4 Audio QA Engineering Standards Established

Every Sarah audio/video pipeline converged on four non-negotiable audio rules:
1. **Loudness normalization:** integrated `-16` to `-18 LUFS`, true peak `≤ -3 dBTP` (`loudnorm` + `alimiter`).
2. **Per-segment STT round-trip gate:** whole-clip STT produced false artifacts; per-segment Whisper/Google STT must match the script verbatim before rendering video.
3. **Word-level STT is necessary but insufficient:** `openers-v1` passed verbatim STT while rising on `"Hello?"` with robotic flat affect. An **LLM audio judge** (Gemini) plus **owner playback** became mandatory.
4. **Spoken-form hygiene:** avoid initialisms (`AI`, `API`) in recorded opener scripts where possible; normalize abbreviations (`openagents.com` → `open agents dot com`, punctuation-prosody `A.I.`) before neural TTS; lint risky product names (`zed`).

---

## 4. Lip-Syncing to Audio Research & Experimental Results

OpenAgents investigated and benchmarked **four distinct lip-syncing paradigms** across July–November 2026: (1) realtime 2D mouth-region inpainting, (2) offline 2D diffusion lip-sync & still-portrait animation + face restoration/interpolation, (3) hosted commercial avatar APIs, and (4) deterministic 3D parametric viseme lip sync in Coax.

### 4.1 Paradigm 1 — Realtime 2D Mouth-Region Inpainting (Owned OAV-1..4)

- **Architecture (`2026-07-09-owned-avatar-video-pipeline-spec.md`):** An idle/listen/speak clip state machine (ported from `lipku/livetalking`, Apache-2.0) running on a dedicated GPU node (`hydralisk-avatar` on GCE L4 `sarah-avatar-gpu-1`), outputting WebRTC (WHEP / `aiortc`). When Sarah is silent or listening, untouched source clips loop (`clip6`/`clip3`, `clip4`/`clip5`). When TTS PCM (`s16le`, 24 kHz mono) arrives, **MuseTalk 1.5 (MIT)** inpaints only a `256×256` lower-face mouth crop and pastes it back onto the `1280×720` frame.
- **One-time preprocessing:** `914 s` (~15.2 min) on L4 for all 10 clips (DWPose landmarks, BiSeNet face parsing masks, VAE latents). Live inference touches none of this cost — critical because unpreprocessed MuseTalk runs at RTF `2.248` (Ditto paper), whereas preprocessed MuseTalk hits `42 FPS` (RTX 3080Ti) / `72 FPS` (RTX 4090) in LiveTalking.
- **Measured on our L4 (`sarah-avatar-gpu-1`):**
  - Offline OAV-1 (`ebb024d9da`): `20.0 FPS` (`--skip_save_images`, video RTF `1.20`), `11.2 FPS` including PNG disk writes, `1.4 s` warm Whisper-tiny audio feature extraction for 21.84 s speech.
  - Realtime OAV-2 simulator pass (`53b5137f1e`): `23.96–24.03 FPS` across 2 utterances + 10 s idle (`910` frames, `0` failures, max gap `0.746 s` on utterance 1 / `0.170 s` on utterance 2) using `HYDRALISK_AVATAR_MUSETALK_FRAME_STRIDE` and moving inference off the asyncio loop (`asyncio.to_thread`, `d3a8fdb`).
  - Capacity & warmup: **1 concurrent stream per L4**, ~60 s cold warmup; solved cold-start UX in Round 4b (`fc4e8e1bce`) by playing a pre-rendered MIT Hallo2 opener clip immediately in the browser (`greeting: "client_clip"`) while WebRTC warmed underneath.
- **Hard defects discovered:**
  1. `256²` teeth smearing/softness at zoom; single-frame generation with no temporal model.
  2. Source-motion conflict: replacing only the mouth on high-motion smiling clips (`mouth motion 3.7–4.2`) looked uncanny because cheeks/eyes smiled while the mouth spoke calmly; required calm source clips (`motion < 1.5`).
  3. Eyes-closed mid-speech when cycling clips forward/reverse (requires per-clip eyes-open frame windows).
  4. Placeholder-bbox crash on no-face frames (patched fail-closed in SQ-4).

### 4.2 Paradigm 2 — Enhancement, Diffusion Lip-Sync & Still Animation (Matrix A0–F2)

When the owner rejected full-strength GFPGAN enhancement (*"higher def but more choppy, way less humanlike"*), the team compiled `docs/sarah/research.md` (mirroring papers in `projects/papers/` and repos in `projects/repos/`) and executed the A0–F2 experiment matrix under the canonical scoreboard schema (`sarah-take-scoreboard.v1`):

| Take / Experiment | Lip-Sync & Post-Process Recipe | Audio Source & Judge | Motion / Jerk (mouth crop) | Render Time & Cost | Verdict & Root-Cause Finding |
| --- | --- | --- | --- | --- | --- |
| **`sarah-reply-v1`** (OAV-1 baseline) | Raw MuseTalk 1.5 (`256²`), libx264 ~1.0 Mbps | CosyVoice2 pre-normalizer (peaked `-0.2 dBFS`) | Smooth in tiles; soft mouth | `119 s` wall (L4) | **NO.** Identity/sync GO, but audio said `"eye"` for `"AI"`, slurred `"coding"` → `"Cody's"`; encode softness. |
| **`sarah-reply-hq`** (A0 baseline) | Raw MuseTalk 1.5 (`256²`), CRF 16 slow (`2.6 Mbps`) | CosyVoice2 + `tts/normalize.py` (`-18.4 LUFS`, Gemini STT PASS) | Mean diff `8.88` vs orig `8.77` | `108 s` (L4) | **NO.** Encode softness & audio words fixed; staccato `"A I"` pacing; residual `256²` teeth softness. |
| **`sarah-reply-enhanced`** | `hq` + **full-strength per-frame GFPGAN v1.4** | Same as `hq` | Mean diff `8.82` (no metric spike!) | `254 s` (L4) | **OWNER FAIL.** Stills crisp, yet **choppy & plastic in motion** due to per-frame GAN mode-snapping + 24 fps strobing. **Full-strength per-frame GFPGAN banned.** |
| **`sarah-reply-v3`** (**A2+A3**) | MuseTalk 1.5 (`audio_padding 3/3`) + **tamed GFPGAN** (alpha `0.45` + feathered mouth mask + temporal EMA on delta & bbox) | CosyVoice2 punctuation-prosody (`"A.I."`, per-segment STT PASS) | Motion `1.26 / 2.13`; **Jerk `0.15 / 0.44`** | `252 s` (L4, `$0.06`) | **Smoothest MuseTalk-lane recipe** (beats even raw source jerk). Owner playback head-to-head pending. |
| **`sarah-reply-latentsync`** (**D1**) | **ByteDance LatentSync 1.6** (Apache-2.0, `512²` diffusion, 20 steps, DeepCache, 25 fps, 18 GB VRAM) | Same `hq` audio (note: muxed 16 kHz copy; needs 24 kHz remux) | Motion `1.48 / 2.49`; **Jerk `0.37 / 1.10`** (mod-16 phases 14–15: `0.62 / 0.58`) | `756 s` (L4, ~31× RT, `$0.18`) | **Best articulation & natural teeth**, zero temporal boil — **blocked by periodic 16-frame chunk-boundary hitch** every `0.64 s` (upstream `inference.py` lacked chunk overlap-blending). |
| **`sarah-openers-v1`** (10 clips) | 5 short scripts (3.6–5.8 s) × both `v3` and `LatentSync 1.6` | CosyVoice2 seed 1986 (passed word-level STT) | `v3` jerk `0.14–0.80`; LS `0.34–1.27` | `1,324 s` (L4, `$0.35`) | **OWNER FAIL ON ALL 10 CLIPS.** Proved short clips are *harder* than long takes: rising-intonation `"Hello?"`, smiling-clip source-motion conflict (`3.7–4.2`), flat affect. |
| **`openers-v2`** (5 clips, Round 4) | **Hallo2 (MIT)** `512²` still animation (`inference_long`, 16-frame chunks × 40 DDIM steps) over single calm still | CosyVoice2 long-ref bake-off winners (`7–9/10` Gemini 3.5 Flash judge; Chirp 3 HD `6–8/10`) | Full-face `512²` motion `2.0–2.3`; mod-16 phase spike watch on 01/02 | `140–250 s`/clip (A100 SPOT, `$0.12`/clip; `$29` idle waste booked) | **OWNER PLAYBACK PASS (2026-07-10):** *"those v2s are much better - opener-05-show-you-hallo2.mp4 is for example close to shippable so proceed in that direction."* Shipped behind `opener-clips.ts` closed allowlist; `*-sr.mp4` blocked on S-Lab license. |
| **`openers-v2-r2`** (02 & 05, Round 5) | Fresh Hallo2 `512²` renders over wave 2–3 audio winners | Opener-02 `9/10` verbatim (`w2-slow92-s31415`, fixed `did→do` slur); Opener-05 `8/10` (`w2-pb-s777`); 01 & 03 plateaued at `7/10` | Clean temporal strips | `1.16 h` total A100 run (`$1.60`, cleanly stopped) | Owner playback pending; confirmed CosyVoice2-0.5B hits a ~7/10 short-clip prosody ceiling on 01/03. |
| **FLAIR (`F1`)** | `wustl-cig/FLAIR` (WACV diffusion temporal face restorer, repo MIT) | Tested on A100 in Round 5 | Capped at `512²` | **`~4.9 s/step` ≈ 33 min per 1 s of video** on A100 | **EMPIRICALLY REJECTED.** Unconditionally loads S-Lab non-commercial `codeformer.pth`, cannot upscale past `512²`, ~2,000× slower than realtime. |
| **Queued / Unrun Matrix Items** | **RIFE** (`B1`/`E1`, MIT, 24→48 fps), **BasicVSR++ / RealBasicVSR** (`F2`, Apache-2.0 temporal VSR), **Ditto** (Apache-2.0, TensorRT, 385 ms TTFF, full-head motion), **SoulX-FlashHead-Lite** (Apache-2.0, 96 FPS @ 4090, 3 streams/GPU) | — | — | — | Ready to evaluate cleanly whenever Track B resumes (none have license blockers). |

### 4.3 Paradigm 3 — Hosted Commercial Talking-Avatar Platforms

- **HeyGen LiveAvatar (`2026-07-09-liveavatar-integration-assessment.md`):**
  - Embed (`POST /v2/embeddings`): sealed iframe, zero events/tools — rejected.
  - FULL Mode + Custom LLM (`@heygen/liveavatar-web-sdk`, `2 credits/min`): worked end-to-end with Gemma 4 (`2778ac5a81`) using a stock preset (`June HR`), giving LiveKit data-channel utterance events + SSE popup cards. Blocked for custom synthetic Sarah likeness; kept only as fallback flag (`SARAH_AVATAR_OWNED_FALLBACK`).
  - LITE Mode (`1 credit/min`, WebSocket `agent.speak` 24 kHz PCM16): sovereignty stepping stone bypassed directly to OAV.
- **Segmind AI Gateway Async V2 (`2026-07-22-segmind-talking-avatar-pipeline.md`):**
  - Recommended evaluation order: (1) **VEED Fabric 1.0**, (2) **Kling Avatar V2** (`kling-v2-standard-avatar` / `pro`, audio-driven `image_url` + `audio_url`), (3) HeyGen Avatar V (skipped — too corporate), (4) **Pruna P Video Avatar (`p-video-avatar`)**.
  - **Proven production result:** `p-video-avatar` rendered Sarah's portrait at **1920×1088, 24 fps** H.264+AAC (~96 s inference, `0.225 credits` for 3.4 s intro; later scaled to continuous ~45–60 s masters for Episodes 261 & 262). Note: an early ~28 s take froze lip sync around 20 s, later superseded by Episode 261's long-take proof, though formal A/B scoreboards against Kling Avatar V2 and VEED Fabric 1.0 were never completed.

### 4.4 Paradigm 4 — 3D Parametric Viseme Lip Sync (Built Nov 4, 2026 in Coax)

After the Coax reset, OpenAgents built a completely different, deterministic lip-sync pipeline in [`coax/web/speak.coax`](../../coax/web/speak.coax) and [`metaverse-engine/audio/speak.coax`](../../metaverse-engine/audio/speak.coax) (spec: [`docs/metaverse/2026-11-04-speaking-characters.md`](../metaverse/2026-11-04-speaking-characters.md), `#12798`):

1. **Voice:** Google Cloud TTS Chirp 3 HD (`texttospeech.googleapis.com/v1/text:synthesize`) → Google STT (`speech.googleapis.com/v1/speech:recognize`) with word time offsets and strict verbatim check (fails if transcript diverges).
2. **Align (pure Coax):** CMUdict-derived table (BSD) + per-character ARPAbet `say` overrides → phoneme duration spread across each word's STT span + loudness-envelope phrase splitting → **14 ARKit-style visemes** (`rest`, `mbp`, `fv`, `th`, `dtnl`, `kg`, `sz`, `chj`, `r`, `aa`, `ee`, `ih`, `oh`, `oo`).
3. **Perform (pure Coax):** 60 ms coarticulation lookahead/overlap, forced lip closure on `mbp` and teeth contact on `fv`, jaw opening scaled by audio loudness, 6 base moods (`calm`, `worried`, `urgent`, `angry`, `cheerful`, `grim`) eased over 0.25 s, and deterministic seeded blinks.
4. **Render & Composite:** Headless WebGPU 60 fps portrait capture via MetaHuman Lab (`?stage=speak&track=...`) + `ffmpeg` panel composite, 8 dB audio ducking, burned-in word-by-word subtitles, `.srt`, and `.json`.

**Tradeoff vs. 2D neural lip sync:** Zero cloud GPU cost, 60 fps native smoothness, zero GAN flicker or 16-frame chunk hitch, 100% deterministic caching — but drives a **3D rigged character** (and MetaHuman packs carry Epic licensing constraints in [`docs/metaverse/metahuman-support.md`](../metaverse/metahuman-support.md)) rather than Sarah's 2D Midjourney sci-fi portrait.

### 4.5 Commercial License Clearance Matrix

| Model / Component | Role | License | Commercial Status | Notes |
| --- | --- | --- | --- | --- |
| **MuseTalk 1.5** | Realtime `256²` mouth inpainting | MIT | **CLEAR** | Preprocessed clips required for realtime. |
| **LatentSync 1.6** | Offline `512²` diffusion lip sync | Apache-2.0 | **CLEAR** | Needs 16-frame chunk overlap-blending fix. |
| **Hallo2** (base `inference_long`) | Offline `512²` still animation | MIT | **CLEAR** | **ONLY raw `512²` output** — never `video_sr.py`. |
| **Ditto** / **SoulX-FlashHead-Lite** | Realtime full-head / multi-stream lip sync | Apache-2.0 | **CLEAR** | Untested v2 candidates. |
| **RIFE** (ECCV 2022) | 24→48 fps frame interpolation | MIT | **CLEAR** | Cheap presentation anti-strobing pass. |
| **BasicVSR++ / RealBasicVSR** | Temporal video super-resolution | Apache-2.0 | **CLEAR** | Permissive alternative to CodeFormer/FLAIR. |
| **GFPGAN v1.4** | Face restoration | Apache-2.0 | **CLEAR (TAMED ONLY)** | Full-strength per-frame **banned** on quality; alpha 0.45 + mask + EMA only. |
| **CosyVoice 2 / 3** | Streaming neural TTS + clone | Apache-2.0 | **CLEAR** | CosyVoice2-0.5B too slow (3–5.8 s TTFC) for live; test CosyVoice3. |
| **Chatterbox / Turbo** | Streaming neural TTS | MIT | **CLEAR (UNVERIFIED)** | Measure latency & quality ourselves. |
| **LiveTalking** | Avatar frame scheduler / WebRTC pattern | Apache-2.0 | **CLEAR** | Architecture reference. |
| **Pipecat** (`smart-turn-v3.2`, VAD/STT) | Turn detection & barge-in patterns | BSD-2-Clause | **CLEAR** | Extract patterns only; do not adopt framework. |
| **Parakeet TDT 0.6B v3** | Local GPU/CPU STT | CC-BY-4.0 | **CLEAR (IN REPO)** | Already built natively in `psionic/` on Metal & CUDA. |
| **CMUdict + `speak.coax`** | Phoneme → 14-viseme alignment | BSD / Owned | **CLEAR (IN REPO)** | Runs locally in Coax. |
| **Hallo2 `video_sr.py` (`*-sr.mp4`)** | ×4 texture recovery | S-Lab 1.0 (CodeFormer) | **BLOCKED** | Non-commercial; must stay unrepresentable in serving catalogs. |
| **FLAIR (`wustl-cig/FLAIR`)** | Temporal face restoration | Repo MIT, **loads S-Lab `codeformer.pth`** | **BLOCKED / REJECTED** | License-encumbered checkpoint + 512² cap + 33 min/s runtime. |
| **CodeFormer / KEEP / PGTFormer / BFVR-STC** | Face restoration / deflickering | S-Lab / Non-commercial / unclear | **BLOCKED** | Research reading only; never ship. |
| **F5-TTS weights / XTTS / Fish S1-mini** | Voice cloning TTS | CC-BY-NC / CPML / NC | **BLOCKED** | Disqualified for commercial voice cloning. |
| **Sonic / FLOAT / Hallo3 weights / LivePortrait InsightFace** | Portrait animation | NC / CC-BY-NC-ND / CogVideoX conditional / research-only | **BLOCKED / REVIEW** | Do not ship without legal clearance (or MediaPipe swap for LivePortrait). |
| **Unreal MetaHuman** | 3D character rig for `speak.coax` | Epic MetaHuman terms | **CONDITIONAL** | Strict no-git, `NO-TRAINING.txt`, custom-engine & revenue-threshold rules apply ([`metahuman-support.md`](../metaverse/metahuman-support.md)). |

### 4.6 Operational & QA Laws Worth Carrying Forward

Two operational laws from `docs/sarah/` (`SQ-1` and `SQ-8`) saved real money and caught false passes — carry them unchanged into any new Sarah implementation:
1. **Playback-First Scoreboard Law (`SQ-1`, `sarah-take-scoreboard.v1`):** No media take advances on stills, frame-diff metrics, or word-level STT alone. Advancement requires **all** of: owner in-motion playback PASS, human/LLM prosody PASS, per-segment verbatim STT PASS, loudness within `-16` to `-18 LUFS` / `≤ -3 dBTP`, and zero license-blocked components.
2. **GPU Media-Run Closeout Law (`SQ-8`, `openagents.sarah.gpu_media_run_closeout.v1`):** Never monitor GPU jobs via stdout log markers — verify `object_exists` on concrete output blobs; record explicit host disposition (`stopped` | `deleted` | `left_running` with written reason) to prevent repeat idle GPU burns; record wall time + USD estimate; attest zero secrets or PII in uploaded artifacts.

---

## 5. Current Repository State Audit (`origin/main`, 2026-11-23)

Rebuilding Sarah does **not** mean resurrecting the deleted Node/TypeScript/Effect or LiveKit stacks (which `AGENTS.md` and [`docs/voice/coax-native-voice.md`](../voice/coax-native-voice.md) explicitly prohibit). Most of the hardest voice, Google, audio-judging, and viseme plumbing is **already built natively in Coax, C, and Psionic**:

| Capability Needed | What Already Exists on `origin/main` | Exact Files | Current Gaps for Sarah |
| --- | --- | --- | --- |
| **Google Vertex / Gemini LLM Gateway** | Production Coax gateway for `openagentsgemini` (`us` multi-region: `gemini-4-hydrogen`, `gemini-4-carbon`, `gemini-4-argon`, `gemini-2.5-flash`) with SA token minting & thought-part filtering. | [`coax/lib/gateway/vertex.coax`](../../coax/lib/gateway/vertex.coax), [`coax/lib/gateway/gemini.coax`](../../coax/lib/gateway/gemini.coax) | Wire a dedicated Sarah persona/route with strict relay & claim-honesty rules. |
| **Google Cloud STT (`chirp_3`) & TTS (`Chirp 3 HD`)** | Working C voice host + Coax speak tool calling `us-speech.googleapis.com/v2` (`chirp_3`) and `texttospeech.googleapis.com/v1/text:synthesize` (Chirp 3 HD), SHA-256 WAV cache, SA token refresh. Already uses Sarah-era `en-US-Chirp3-HD-Sulafat` for Foreman. | [`metaverse/voice-host/speech.c`](../../metaverse/voice-host/speech.c), [`metaverse/voice-host/host.c`](../../metaverse/voice-host/host.c), [`coax/web/speak.coax`](../../coax/web/speak.coax) | Add `sarah` profile (`en-US-Chirp3-HD-Leda`); upgrade `speech.c` from per-sentence REST (~703 ms TTFA) to **streaming** Chirp 3 HD (163–198 ms) and streaming STT; deploy behind HTTPS LB with `Hello.token` auth. |
| **Local GPU Speech-to-Text** | Resident Psionic **Parakeet TDT 0.6B v3** (Metal on macOS, CUDA on Linux) and Psionic Whisper linked directly into `voice-host` and `openagents-cli`. | `psionic/lib/parakeet/`, [`metaverse/voice-host/speech.c`](../../metaverse/voice-host/speech.c) | Already operational for zero-cloud-cost STT; can also serve as offline verbatim STT gate. |
| **Voice Engine, VAD, Turn-Taking, Barge-In, Floor & Metering** | `#authority` Coax libraries with zero surviving mutants + protobuf wire contract (`openagents.voice.v1alpha1`), echo-cut resume, and WebSocket framing. | [`metaverse/voice.coax`](../../metaverse/voice.coax), [`metaverse/voice-floor.coax`](../../metaverse/voice-floor.coax), [`metaverse/voice-asr.coax`](../../metaverse/voice-asr.coax), [`pay/voice-meter.coax`](../../pay/voice-meter.coax), [`metaverse-engine/net/voice-wire.coax`](../../metaverse-engine/net/voice-wire.coax), [`proto/openagents/voice/v1alpha1/voice.proto`](../../proto/openagents/voice/v1alpha1/voice.proto) | Ready to use as-is; replaces Pipecat, LiveKit, and `packages/audio-contract`. |
| **Sovereign Local TTS Framing** | Streaming block-wise TTS pipeline with exact sample/word barge-in accounting (`psionic-speak-v1`). | [`psionic/lib/speak.coax`](../../psionic/lib/speak.coax), [`psionic/book/ch155-speak-the-answer.md`](../../psionic/book/ch155-speak-the-answer.md) | Currently rule-based formant + Griffin-Lim (8 kHz robotic); needs learned acoustic/vocoder weights (e.g., CosyVoice3 / Chatterbox / Kokoro). |
| **Audio Judging, Loudness & ElevenLabs Tooling** | Coax tool calling ElevenLabs API + **Vertex Gemini (`gemini-2.5-flash`) audio judge** + exact ITU-R BS.1770-4 / EBU R128 LUFS & peak measurement (`measure.coax`) + WAV writer (`wav.coax`). | [`metaverse-engine/audio/sfx-tool.coax`](../../metaverse-engine/audio/sfx-tool.coax), [`metaverse-engine/audio/measure.coax`](../../metaverse-engine/audio/measure.coax), [`metaverse-engine/audio/wav.coax`](../../metaverse-engine/audio/wav.coax) | Directly reusable pattern for Sarah TTS bake-off judging, `-16 LUFS` verification, and ElevenLabs `music_v2` bed generation in Coax. |
| **3D Phoneme → Viseme Lip-Sync Pipeline** | Working end-to-end `coax/web/tool.sh speak` pipeline: Chirp 3 HD TTS → Google STT word timing → CMUdict/ARPAbet → 14 ARKit visemes with coarticulation & loudness jaw scaling → 60 fps WebGPU MetaHuman Lab render → `ffmpeg` composite + subtitles. | [`coax/web/speak.coax`](../../coax/web/speak.coax), [`metaverse-engine/audio/speak.coax`](../../metaverse-engine/audio/speak.coax), [`metaverse/cast/cast.txt`](../../metaverse/cast/cast.txt), [`metaverse/tools/humans/ph_face.coax`](../../metaverse/tools/humans/ph_face.coax), [`apps/openagents-web/labs/metahumanlab.coax`](../../apps/openagents-web/labs/metahumanlab.coax) | Missing a `sarah` cast entry & 3D character pack; missing eye saccades, head nods, breathing, `talk` body clip, and live streaming viseme mode. |
| **Web Hosting & API Surface** | Static Coax site on GCS + Cloud CDN (`openagents.com`), C API on Cloud Run (`oa-api`), content-addressed asset registry (`assets-sync`). | [`docs/audits/2026-11-20-openagents-com-live-surface-audit.md`](../audits/2026-11-20-openagents-com-live-surface-audit.md) | No `/sarah` route exists today; no 2D talking-avatar runner or GPU render sidecar exists in the repo. |

---

## 6. What Would Be Needed to Complete This Now

Because "completing Sarah" spans three very different engineering scopes — **produced talking-head videos**, **live voice + realtime 2D neural avatar**, and **3D Coax/Metaverse speaking character** — the work is best structured as a shared foundation (Phases 0–1) followed by independent visual tracks so we do not spin up standing GPU infrastructure unless the owner chooses realtime 2D video.

### Phase 0 — Asset Verification & Canonical Direction Restore (0.5 day, prerequisite for all tracks)

1. **Verify GCS asset survival (read-only check):** Confirm whether `gs://openagentsgemini-oa-artifacts/sarah-avatar/` still contains:
   - `src/` portrait (`v2_1783572530279-62463022.png`) and calm square crop (`source_square.jpg`),
   - `footage/` (the 10 clips `clip0..clip9`, especially `clip4`, `clip6`, and trimmed `clip8`),
   - `voice-ref/sarah_voice_ref_v1.wav` and the 30–60 s concatenated long reference, and
   - `openers-v2/` (the 5 MIT Hallo2 `512²` clips + `r2` takes + `manifest.json`).
   - If intact, record content hashes in an owner-local manifest (never commit media binaries or secrets to git). If pruned, re-upload the portrait/clips from owner local archives or generate a fresh character pack.
2. **Restore Sarah's canonical direction & persona contract in `docs/sarah/`:**
   - Check in an updated `sarah-direction.json` (`openagents.sarah.avatar_direction.v2`) preserving `voice: "Leda (Female)"`, `voice_anchor`, neutral/composed `video_prompt` & `negative_prompt`, and the 60 s continuous-take rule.
   - Write a current `ACTING_AS_SARAH_RUNBOOK.md` updated to the Nov 2026 product surface (OpenAgents Desktop, Inky, Psionic, Coax, Metaverse — retiring stale Omega/Khala-era language while keeping the 2026-08-10 plain-professional voice register and claim-honesty law).
3. **Confirm credentials & secrets availability:** Verify `~/work/.secrets/gcloud-sa-config` + `gcp-mvp-automation.json` (`openagentsgemini` billing/quotas for Speech V2, TTS Chirp 3 HD, Vertex Gemini, GCS), plus `~/work/.secrets/segmind.env` and `~/work/.secrets/elevenlabs.env` if executing Track A.

### Phase 1 — Live Sarah Voice on the Existing Coax Voice Host (1–2 days, zero GPU rent)

Gets Sarah speaking live with her locked Google voice across web, iOS, and desktop using infrastructure that already compiles and runs in this repo:

1. **Register Sarah in [`metaverse/voice-host/speech.c`](../../metaverse/voice-host/speech.c):**
   - Add `{"sarah", 1, "en-US-Chirp3-HD-Leda", "en-US"}` to `PROFILES[]`.
   - Add a `"sarah"` route in `ROUTES[]` bound to her live persona prompt (mirroring the retired-military plain-professional register) and backed by Vertex Gemini (`coax/lib/gateway/vertex.coax` → `gemini-4-hydrogen` / `gemini-4-carbon`) or local Inkling 2 Mini. Preserve the `#12059` relay invariant: the voice model never claims actions it did not execute through a real agent.
2. **Close the latency gap in `metaverse/voice-host` (`docs/voice/coax-native-voice.md` V3–V4):**
   - Replace per-sentence REST `text:synthesize` (currently ~703 ms to first speech) with **Google Chirp 3 HD streaming synthesis** to recover the proven **163–198 ms** time-to-first-audio.
   - Pair with either **streaming Google Chirp 3 STT** or **resident Psionic Parakeet TDT v3** on GPU (already wired in `speech.c`) so end-of-turn to first audio meets the `≤ 800 ms` interactive target.
3. **Harden & deploy `voice-host`:** Wire `Hello.token` authentication (explicitly noted missing on the local dev host in [`docs/voice/dev.md`](../voice/dev.md)), deploy behind the existing global HTTPS load balancer (`voice.openagents.com` or `/api/voice`), and record a real-device latency benchmark under `bench/voice/`.

### Track A — Produced Talking-Avatar Videos (2–4 days, Coax-native, fastest visual path)

Replaces the deleted Node scripts (`scripts/sarah-avatar/*.mjs`) cleanly in Coax without reintroducing Node/pnpm:

1. **Build a Coax produced-video tool (`coax/web/sarah-video.coax`):**
   - **Audio-first synthesis (key improvement over July 2026):** Instead of relying on Segmind's built-in TTS (which created pronunciation traps like uppercase `Zed` and tied Sarah's voice to Pruna), synthesize Sarah's spoken master track **first** with **Google Chirp 3 HD (`en-US-Chirp3-HD-Leda`)** (reusing [`coax/web/speak.coax`](../../coax/web/speak.coax)'s Google TTS + STT verbatim gate + [`metaverse-engine/audio/measure.coax`](../../metaverse-engine/audio/measure.coax) `-16 LUFS` normalization + [`sfx-tool.coax`](../../metaverse-engine/audio/sfx-tool.coax)'s Vertex Gemini audio judge).
   - **GCS V4 URL signing:** Upload the portrait PNG and locked WAV to `gs://openagentsgemini-oa-artifacts/sarah-avatar/tmp/` and mint short-lived V4 read URLs.
   - **Segmind Async V2 client (`coax/lib/net/http.coax`):** Submit (`POST /v2/<model>`), poll `status_url` with bounded timeout, download MP4, supporting `--audio <url>` across `p-video-avatar`, `kling-v2-standard-avatar` / `kling-v2-pro-avatar`, and `veed-fabric-1.0` (plus fallback `--script` mode with `voice_anchor` enforcement and paste lint).
   - **Episode RC assembly & music bed:** Port `assemble-rc.mjs` (continuous audio `A/B/C` picture cuts, silence-derived `T1`, `T2` cut-point Finder/Desktop pixel QC, transcript lock) and `elevenlabs-music-bed.mjs` (`POST /v1/music` `music_v2` + `-3 dB` mix preserving `*-rc-no-music.mp4`).
2. **Run the unfinished 3-way produced-clip A/B scoreboard:** Render the exact same portrait + locked `Leda` WAV on **VEED Fabric 1.0**, **Kling Avatar V2 Pro**, and **Pruna `p-video-avatar`**, score them under `sarah-take-scoreboard.v1` (owner playback + per-segment STT + loudness), and standardize on the winner.

### Track B — Owned Realtime 2D Lip-Sync Avatar (1–2 weeks, GPU sidecar)

Only needed if Sarah must appear as a **live interactive 2D Midjourney talking head** (e.g., on `openagents.com/sarah` or inside OpenAgents Desktop/Mobile) with zero per-minute SaaS credits:

1. **Stand up a dedicated GPU render sidecar (isolated from the CPU `voice-host`):**
   - Host on a scale-to-zero Cloud Run GPU / GCE L4 (`g2-standard-8`) or RTX 4080/4090 Pylon node — never co-located with audio/turn-taking event loops.
   - Preserve the proven `owned-renderer.ts` HTTP+media contract (`POST /sessions`, `POST /sessions/{id}/control` carrying `pcm_s16le` 24 kHz mono chunks + `speak_end` + `interrupt` + `listening`, `DELETE /sessions/{id}`) fed directly by `metaverse/voice-host`'s Chirp 3 HD audio stream.
   - Deliver video over WebRTC or low-latency H.264/WebCodecs over WebSocket (note: [`docs/voice/coax-native-voice.md`](../voice/coax-native-voice.md) bans reintroducing GKE LiveKit).
2. **Close the quality ceiling left open in July 2026 (`research.md` Matrix B1/E1/F2 + v2 ladder):**
   - **Realtime tier:** Benchmark **Ditto** (Apache-2.0, TensorRT, 385 ms TTFF, audio-driven head motion) and **SoulX-FlashHead-Lite** (Apache-2.0, 96 FPS @ 4090, multi-stream) head-to-head against **MuseTalk 1.5 + tamed GFPGAN (`v3`) + RIFE 48 fps** (with eyes-open frame pinning and placeholder-bbox fail-closed).
   - **Pre-rendered opener / cold-start tier:** Serve verified MIT opener clips (`greeting: "client_clip"`) over cold start from `openagents-web` / `oa-api`. To lift quality past raw Hallo2 `512²` legally, either (a) patch **LatentSync 1.6** (Apache-2.0) with overlap-add chunk-seam blending to eliminate its 16-frame periodic hitch, or (b) run **BasicVSR++ / RealBasicVSR** (Apache-2.0) on Hallo2 `512²` outputs (strictly excluding S-Lab CodeFormer / `video_sr.py` / FLAIR).
3. **Wire `SQ-1` scoreboards & `SQ-8` closeout checks:** Port the scoreboard & GPU closeout validator into Coax (or a small checked script) and enforce auto-stop idle timers on every experimental GPU VM.

### Track C — 3D Coax / Metaverse Speaking Character (3–5 days produced; 1–2 weeks live; zero GPU rent)

Leverages the **already-shipped** Nov 4, 2026 speaking-character pipeline ([`docs/metaverse/2026-11-04-speaking-characters.md`](../metaverse/2026-11-04-speaking-characters.md)) to give Sarah a deterministic 60 fps WebGPU/Metal face with zero cloud GPU cost:

1. **Author a Sarah 3D character pack:** Prefer a custom stylized sci-fi Blender/CC0 rig (matching her Midjourney aesthetic while completely avoiding Epic MetaHuman's custom-engine/revenue license restrictions) mapped to the 14 ARKit viseme controls in [`metaverse/tools/humans/ph_face.coax`](../../metaverse/tools/humans/ph_face.coax) — or use a MetaHuman pack if Epic license terms are accepted.
2. **Add Sarah to [`metaverse/cast/cast.txt`](../../metaverse/cast/cast.txt):** Bind `voice google en-US-Chirp3-HD-Leda`, custom `camera`, sci-fi `backdrop`, and ARPAbet `say` overrides (`OpenAgents`, `Coax`, `Psionic`, `Inky`, `Nostr`).
3. **Finish the documented MVP gaps in [`metaverse-engine/audio/speak.coax`](../../metaverse-engine/audio/speak.coax) & [`apps/openagents-web/labs/metahumanlab.coax`](../../apps/openagents-web/labs/metahumanlab.coax):**
   - Produced video: add eye saccades, stressed-word head nods, breathing, `talk` body clip, and `--revoice N` pickups.
   - Live interactive mode: stream real-time phoneme/viseme + RMS loudness frames alongside Chirp 3 HD PCM from `metaverse/voice-host` (`openagents.voice.v1alpha1`) straight into the client's 60 fps WebGPU/Metal face rig.

### Track D — Sovereign Owned Neural TTS (Psionic Ch. 155 / GPU Pylon, 1–2 weeks)

Addresses the Episode 270 lock-in lesson (OpenAI refusing Sarah's custom voice) and the CosyVoice2-0.5B short-clip plateau (`7/10`, 3–5.8 s TTFC):

1. Benchmark permissive streaming TTS successors (**CosyVoice 3** Apache-2.0, **Chatterbox Turbo** MIT, or newer Apache-2.0/MIT neural TTS checkpoints) on RTX 4080 / L4 / Apple Silicon against Google Chirp 3 HD `Leda`, gated by `< 250 ms` TTFA, verbatim Parakeet/Google STT, and `≥ 8.5/10` Gemini + owner prosody ratings.
2. Wire the winning checkpoint behind [`psionic/lib/speak.coax`](../../psionic/lib/speak.coax) (`ch155`) or a GPU Pylon `speak` service using the exact same PCM16 + barge-in accounting seam, keeping Google Chirp 3 HD as the managed baseline behind the same voice-host interface.

---

## 7. Open Owner Decisions Needed Before Implementation

| # | Decision | Options | Recommendation |
| --- | --- | --- | --- |
| **1** | **Immediate visual goal** | Produced episode videos (Track A) / Live 2D neural talking head (Track B) / 3D Coax speaking stage (Track C) / Live voice first (Phase 1) | Start with **Phase 0 + Phase 1** (live `Leda` voice on `metaverse/voice-host`) plus **Track A** (audio-first produced episodes in Coax); gate Track B GPU spend until live video demand is proven. |
| **2** | **Visual form factor for live Sarah** | 2D Midjourney portrait (requires GPU sidecar per stream) vs. 3D stylized Coax rig (60 fps local WebGPU/Metal, `$0` GPU rent) | Use the **2D Midjourney portrait** for produced videos (Track A), and evaluate a **stylized 3D Coax rig** (Track C) for live interactive surfaces so live calls scale without `$0.85/h` per concurrent stream. |
| **3** | **Canonical voice engine** | Google Chirp 3 HD `en-US-Chirp3-HD-Leda` (fast, zero GPU, stock voice) vs. owned cloned voice (CosyVoice3 / Psionic Ch. 155) | Ship **Google Chirp 3 HD `Leda`** immediately across live + produced pipelines (aligns with the 2026-10-28 Google-speech owner decision), while pursuing Track D for long-term voice sovereignty. |
| **4** | **Produced-video lip-sync vendor** | Segmind `p-video-avatar` (proven) vs. `kling-v2-pro-avatar` vs. `veed-fabric-1.0` (all fed locked `Leda` WAV) | Run a bounded 3-way A/B scoreboard (`sarah-take-scoreboard.v1`) on one 15–30 s script before locking the default. |
| **5** | **Target product surface** | Episodes & social only / OpenAgents Desktop & Mobile voice / Public `openagents.com/sarah` | Wire `metaverse/voice-host` + Desktop/Mobile first; only reopen public `/sarah` on `openagents.com` once auth, rate caps, and claim-honesty contracts are re-established on `oa-api`. |
| **6** | **Historical GCS archive disposition** | Reuse existing `gs://openagentsgemini-oa-artifacts/sarah-avatar/` assets vs. fresh portrait/clip generation | Verify bucket contents in Phase 0 first; reuse existing Midjourney portrait + `clip0..9` + MIT `openers-v2` if intact. |
