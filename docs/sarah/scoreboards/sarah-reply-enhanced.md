# Take scoreboard — sarah_reply_enhanced — hq + full-strength per-frame GFPGAN

- Schema: `sarah-take-scoreboard.v1`
- Take: `sarah-reply-enhanced` (2026-07-09)
- Refs: #8610, #8618
- **Advance: NO** — OWNER PLAYBACK FAIL: choppier, plastic, less humanlike in motion — after passing stills QA. Full-strength per-frame GFPGAN is banned as a default; tamed recipe (v3) replaces it.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | footage clip 8 (v2_traced-df7d3b47…, 720p24), trimmed to frames 0-184 (7.71 s), cycled forward+reverse |
| Script | OAV-1 Sarah reply (opener + C.1 product answer), same normalized-text audio as hq, 23.96 s |
| TTS reference | gs://openagentsgemini-oa-artifacts/sarah-avatar/voice-ref/sarah_voice_ref_v1.wav (clip 4, 5.40 s, STT confidence 1.00) |
| Model: musetalk | 1.5 (256^2 mouth inpaint, fp16, batch 20) |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot + spoken-form normalizer |
| Model: gfpgan | v1.4 full-strength per-frame on the 256^2 crop pre-paste-back |
| Model: encode | libx264 CRF16 preset slow + loudnorm |
| Recipe | hq + GFPGAN v1.4 full-strength per-frame on the synthesized 256^2 crop before paste-back |
| Commits | hydralisk 40b2783 (spoken-form TTS normalizer) |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/sarah_reply_enhanced.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/qa2/ |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (whole-clip, Gemini (same audio as hq)) | PASS | 'A I' letters 3/3; 'coding agents' correct |
| Loudness / true peak | -18.4 LUFS / -2.9 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | staccato letter-speech pacing (same audio as hq) |
| Prosody (human verdict) | WATCH | same staccato letter-spaced audio as hq; clipped audio likely amplified the clipped-viseme percept |
| Prosody (LLM judge) | not run | — |
| Initialism risk | — | normalizer letter-spacing 'A I' — correct words, robotic rhythm |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | FAIL | OWNER VERDICT: choppier, plastic, way less humanlike in motion — after passing stills QA (crisp teeth, no measured flicker, invisible seam) |
| A/V sync (start / middle / end) | not run / not run / not run | not separately re-measured; frame inspection surfaced no offset |
| Crop sharpness | PASS | individual tooth definition, crisp lip edges — in stills; the sharpness is the problem in motion |
| Temporal boil/flicker | WATCH | no measured boil at sampled 24-frame windows (2.92 vs hq 2.82 mean diff at t=8 s) — but playback percept is per-frame GAN mode-snapping, invisible to frame-diff statistics |
| Chunk-boundary jerk | PASS | metrics statistically identical to the original — the owner-perceived choppiness is per-frame GAN restoration, not gross frame-level jitter |
| Identity drift | WATCH | no drift in stills; over-restored skin/teeth texture reads uncanny/plastic in motion |
| Paste-back seam | PASS | seam invisible in every sampled crop |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 575 | 24 | 1.43 / 2.44 | 0.21 / 0.56 |

Bad-frame exclusions:

- clip 8 trimmed to frames 0-184 (no-face tail)

The stills-vs-motion split that created the playback-first law: per-frame GFPGAN has no temporal model, so every frame is pulled toward its own restored mode.

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 254 s |
| GPU | GCE g2-standard-8, 1x NVIDIA L4 24 GB (us-central1-b) |
| Cost estimate | $0.06 (254 s render at ~$0.85/h) |
| Artifact existence (object_exists) | PASS |
| Host disposition | left_running (prod_render_node — sarah-avatar-gpu-1 is the permanent OAV render node after the 2026-07-09 /sarah flip (~$0.85/h)) |
| GCS index updated | yes |
| No secrets in artifacts | attested |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
