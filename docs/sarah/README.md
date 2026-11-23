# docs/sarah

Historical forensics, media/voice/lip-sync architecture audit, and completion plan for **Sarah** on the current Coax / Metaverse / Psionic repository.

## Documents & Artifacts

- [`2026-11-23-sarah-media-google-lipsync-audit.md`](2026-11-23-sarah-media-google-lipsync-audit.md) — full audit of the deleted `docs/sarah/` corpus, Google stack usage, lip-sync research, and phased completion plan.
- [`sarah-direction.json`](sarah-direction.json) — canonical Sarah voice (`en-US-Chirp3-HD-Leda`), gender anchor, visual direction, and audio target profile (`openagents.sarah.avatar_direction.v2`).
- [`receipts/2026-11-23-sarah-mvp-validation.md`](receipts/2026-11-23-sarah-mvp-validation.md) — live end-to-end MVP validation receipt (live `voice-host` `Leda` latency + Vertex `gemini-2.5-flash-image` portrait + Google Chirp 3 HD `Leda` synth/STT/Gemini judge + Segmind Pruna vs. Kling A/B lip-sync).
- [`scoreboards/`](scoreboards/) — `sarah-take-scoreboard.v1` JSON + Markdown records (`advance: false` until owner playback PASS).

## Running the MVP

```sh
# 1. Live interactive Sarah voice (en-US-Chirp3-HD-Leda + Vertex Gemini + barge-in):
metaverse/voice-host/test.sh

# 2. Pure-Coax Sarah unit checks (paste lint, voice anchor, STT matcher, GCS V4, Segmind payloads, SQ-1 law):
./coax/bin/coax test coax/web/sarah-lib.coax --library

# 3. End-to-end produced clip MVP (Vertex portrait -> Chirp 3 HD Leda -> -16 LUFS -> STT -> Gemini judge -> Segmind A/B):
coax/web/tool.sh sarah mvp \
  --text "Hello! I'm Sarah. Our voice and lip sync pipeline is back online. Shall we begin?" \
  --out "$HOME/.openagents/sarah-mvp/2026-11-23" \
  --models "p-video-avatar,kling-v2-standard-avatar"
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
