# Take scoreboard — Opener library v1 — 5 scripts x v3/LatentSync (10 clips)

- Schema: `sarah-take-scoreboard.v1`
- Take: `sarah-openers-v1` (2026-07-09)
- Refs: #8610, #8618
- **Advance: NO** — OWNER PLAYBACK FAIL on ALL 10 clips despite passing stills, jerk metrics, and word-level STT. Hardened rules: playback + prosody judgment gate every take; short clips are the HARDER problem. Successors: openers-v2 audio bake-off (LLM judge), calm-clip re-render, Hallo2 still-animation tier.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | clips 4, 5, 0, 3, 8-trim per script (01-hello, 02-welcome-back, 03-good-question, 04-got-it, 05-show-you); see openers/manifest.json |
| Script | 5 opener scripts, 3.6-5.8 s each: hello / welcome-back / good-question / got-it / show-you |
| TTS reference | gs://openagentsgemini-oa-artifacts/sarah-avatar/voice-ref/sarah_voice_ref_v1.wav (clip 4); CosyVoice2 seed 1986 first-pass on all five |
| Model: musetalk | 1.5 + tamed-GFPGAN v3 recipe (per-clip -v3 variants) |
| Model: latentsync | 1.6 (per-clip -ls variants, full-band 24 kHz re-mux + CRF16 slow) |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot, seed 1986, loudnorm |
| Recipe | per script: one gated CosyVoice2 audio (seed 1986, loudnorm, per-segment whisper STT gate) rendered on BOTH the v3 recipe and LatentSync 1.6 |
| Commits | hydralisk 40b2783 (spoken-form TTS normalizer) |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers/ (10 MP4s + gated wavs + manifest.json) |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, whisper (batch gate) + Gemini (spot checks)) | PASS | word-level pass on all 10 clips — and still insufficient: the opener-01 'Hello?' rising-intonation defect passes word-level STT |
| Loudness / true peak | -16 LUFS / -3 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | FAIL | robotic pacing on short scripts; zero-shot prosody instability dominates 4-6 s clips |
| Prosody (human verdict) | FAIL | opener-01 rises ('Hello?') on the single most important word in the library; wrong intonation / robotic pacing / flat affect across the set — an opener IS its prosody. LLM audio judge (Gemini 1-10) adopted for the openers-v2 bake-off. |
| Prosody (LLM judge) | not run | — |
| Initialism risk | — | opener scripts avoid initialisms by design |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | FAIL | OWNER VERDICT: ALL 10 clips fail playback — despite passing stills, jerk metrics, and word-level STT |
| A/V sync (start / middle / end) | not run / not run / not run | per-clip sync not separately measured; frame inspection surfaced no offset |
| Crop sharpness | WATCH | 02/03/04 clean on inspection; opener-05 v3 motion (1.19) and jerk (0.14) suspiciously low for 4 s of continuous speech — possible under-articulation |
| Temporal boil/flicker | PASS | no boil flagged in per-clip frame inspection |
| Chunk-boundary jerk | WATCH | per-clip mouth-crop jerk: v3 variants 0.14-0.80 vs LatentSync 0.34-1.27 (v3 smoother on every script); LS variants inherit the 16-frame chunk hitch; openers 01/02 jerk inflated by real source motion, not artifacts |
| Identity drift | PASS | clean identity on inspected clips |
| Paste-back seam | PASS | — |

Bad-frame exclusions:

- openers 01/02 used high-motion big-smile source clips (mouth-region motion 3.7-4.2 vs 1.2 on the long take) — source-motion conflict; calm-clip rule (<1.5 mouth motion) adopted for openers-v2

Ranked failure hypotheses: zero-shot TTS prosody instability on short texts; source-motion conflict (synthesized mouth vs smiling cheeks/eyes); model-tier ceiling on expressiveness (neither MuseTalk nor LatentSync generates greeting-appropriate head/face motion).

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 1324 s |
| GPU | GCE g2-standard-8, 1x NVIDIA L4 24 GB (us-central1-b) |
| Cost estimate | $0.35 (sum of per-clip renders: v3 443 s + LatentSync 881 s at ~$0.85/h, plus TTS) |
| Artifact existence (object_exists) | PASS — 10 MP4s + wavs + manifest.json verified in gs://…/sarah-avatar/openers/ |
| Host disposition | left_running (prod_render_node — sarah-avatar-gpu-1 is the permanent OAV render node after the 2026-07-09 /sarah flip (~$0.85/h)) |
| GCS index updated | yes |
| No secrets in artifacts | attested |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
