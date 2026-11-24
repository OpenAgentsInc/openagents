# docs/sarah

Historical forensics, media/voice/lip-sync architecture audit, and completion plan for **Sarah** on the current Coax / Metaverse / Psionic repository.

## Documents & Artifacts

- [`2026-11-23-sarah-media-google-lipsync-audit.md`](2026-11-23-sarah-media-google-lipsync-audit.md) — full audit of the deleted `docs/sarah/` corpus, Google stack usage, lip-sync research, and phased completion plan.
- [`sarah-direction.json`](sarah-direction.json) — canonical Sarah voice (`gemini-2.5-pro-tts` `Sulafat` primary / `Leda` alternate; `en-US-Chirp3-HD-Leda` realtime), gender anchor, visual direction, vertical-slice geometry, and audio targets (`openagents.sarah.avatar_direction.v2`).
- [`receipts/2026-11-23-sarah-mvp-validation.md`](receipts/2026-11-23-sarah-mvp-validation.md) — initial live end-to-end MVP validation receipt (live `voice-host` `Leda` latency + Vertex portrait + Chirp 3 HD + Segmind Pruna vs. Kling A/B).
- [`receipts/2026-11-24-voice-ab-and-ep298-install-video.md`](receipts/2026-11-24-voice-ab-and-ep298-install-video.md) — owner playback verdicts (Kling rejected; Pruna + `gemini-2.5-pro-tts` `Sulafat` winner) and full 60-second Episode 298–style OpenAgents Desktop + Mobile install walkthrough with vertical-slice Sarah overlay.
- [`scoreboards/`](scoreboards/) — `sarah-take-scoreboard.v1` JSON + Markdown records (`advance: false` until owner playback PASS).

## Running the MVP

```sh
# 1. Live interactive Sarah voice (en-US-Chirp3-HD-Leda + Vertex Gemini + barge-in):
metaverse/voice-host/test.sh

# 2. Pure-Coax Sarah unit checks (paste lint, voice anchor, STT matcher, GCS V4, Segmind payloads, SQ-1 law):
./coax/bin/coax test coax/web/sarah-lib.coax --library

# 3. End-to-end produced clip (real Sarah still -> Gemini 2.5 Pro TTS Sulafat -> -16 LUFS -> STT -> Gemini judge -> Pruna 1080p + 48kHz remux):
coax/web/tool.sh sarah mvp \
  --image "/path/to/sarah-portrait-settled.png" \
  --text "Welcome to Acceleration Week. I'm Sarah." \
  --out "$HOME/.openagents/sarah-mvp/take1"

# 4. Crop a 1920x1088 Pruna clip into an Episode-298 vertical speaker overlay slice (486x688 @ 30fps):
coax/web/tool.sh sarah slice --in "$HOME/.openagents/sarah-mvp/take1/sarah-p-video-avatar.mp4" --out "$HOME/.openagents/sarah-mvp/take1/sarah-slice.mp4"
```

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
