# docs/sarah

Historical forensics, media/voice/lip-sync architecture audit, and completion plan for **Sarah** on the current Coax / Metaverse / Psionic repository.

## Documents

- [`2026-11-23-sarah-media-google-lipsync-audit.md`](2026-11-23-sarah-media-google-lipsync-audit.md) — full audit of the deleted `docs/sarah/` corpus and surrounding Sarah pipelines:
  - visual/portrait asset lineage and Google Cloud Storage V4 signed-URL hosting,
  - every Google service and model used across Sarah's brain, audio, STT/TTS, judging, storage, and GPU compute,
  - the lip-syncing to audio research program (MuseTalk 1.5, tamed GFPGAN, LatentSync 1.6, Hallo2, FLAIR rejection, RIFE, BasicVSR++, HeyGen LiveAvatar, Segmind Pruna/Kling/VEED Fabric, and Coax's Nov 2026 14-viseme speaking stage),
  - measured scoreboard metrics, license verdicts, and owner playback laws, and
  - what is already built on `origin/main` today plus a phased plan to complete Sarah now.

## Recovering historical Sarah material from git

The original `docs/sarah/` folder (59 files) and legacy Sarah code were removed across four cleanup/reset commits. Nothing is lost; inspect them read-only via `git show`:

```sh
# 1. Original docs/sarah/ folder (59 files) + sarah-direction.json right before the Sep 18, 2026 Nuke:
git ls-tree -r --name-only dabc08102f~1 docs/sarah
git show dabc08102f~1:docs/sarah/README.md
git show dabc08102f~1:docs/sarah/2026-07-09-owned-avatar-video-pipeline-spec.md
git show dabc08102f~1:docs/sarah/2026-07-09-oav-quality-strategy.md
git show dabc08102f~1:docs/sarah/research.md
git show dabc08102f~1:docs/sarah/2026-07-22-segmind-talking-avatar-pipeline.md
git show dabc08102f~1:docs/sarah/ACTING_AS_SARAH_RUNBOOK.md
git show dabc08102f~1:scripts/sarah-avatar/sarah-direction.json

# 2. Produced-video Node runners (Segmind, Episode RC assembly, ElevenLabs music bed, paste lint)
#    right before Wave 4 Node deletion (Aug 28, 2026):
git ls-tree -r --name-only a098b3e29f~1 scripts/sarah-avatar
git show a098b3e29f~1:scripts/sarah-avatar/segmind-talking-avatar.mjs

# 3. Live /sarah Bun+Effect surface (owned renderer client, opener clips allowlist, Google inference)
#    right before owner-directed removal (Jul 10, 2026):
git ls-tree -r --name-only 13bc1e7443~1 apps/sarah
git show 13bc1e7443~1:apps/sarah/src/services/owned-renderer.ts
git show 13bc1e7443~1:apps/sarah/src/services/opener-clips.ts
git show 13bc1e7443~1:apps/sarah/src/services/google-inference.ts

# 4. Cross-repo voice infrastructure audit (covering Aug–Oct 2026 Gemini Live / Rust LiveKit / Elixir eras)
#    right before the Oct 27, 2026 Nuke:
git show 0b401561a0^:docs/audits/2026-10-21-voice-infrastructure.md
```
