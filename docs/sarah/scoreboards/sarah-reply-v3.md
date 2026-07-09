# Take scoreboard — sarah_reply_v3 — tamed GFPGAN + punctuation-prosody audio

- Schema: `sarah-take-scoreboard.v1`
- Take: `sarah-reply-v3` (2026-07-09)
- Refs: #8610, #8618
- **Advance: NO** — Measured smoothest take of all (jerk mean 0.15, beating even the original); current best MuseTalk-lane recipe. Owner playback pending — cannot advance on metrics alone.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | footage clip 8 (v2_traced-df7d3b47…, 720p24), trimmed to frames 0-184 (7.71 s), cycled forward+reverse |
| Script | OAV-1 Sarah reply (opener + C.1 product answer), punctuation-prosody audio ('A.I.' replaces hard letter-spacing) |
| TTS reference | gs://openagentsgemini-oa-artifacts/sarah-avatar/voice-ref/sarah_voice_ref_v1.wav (clip 4, 5.40 s, STT confidence 1.00) |
| Model: musetalk | 1.5 (256^2 mouth inpaint, audio_padding 3/3) |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot + punctuation-prosody normalization |
| Model: gfpgan | v1.4 tamed: alpha-blend 0.45 + feathered mouth-only mask + temporal EMA (pixels + bbox) |
| Model: encode | libx264 CRF16 preset slow + loudnorm |
| Recipe | tamed GFPGAN (A2+A3): alpha 0.45 + feathered mouth mask + temporal EMA on the delta + audio_padding 3/3 + punctuation-prosody audio |
| Commits | hydralisk 40b2783 (spoken-form TTS normalizer) |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/sarah_reply_v3.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/qa3/ |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, per-segment STT) | PASS | all four 'A.I.' occurrences transcribe cleanly per-segment; whole-clip artifacts ('a AI', 'I.I.') were transcriber noise; 'coding agents' correct |
| Loudness / true peak | -18.4 LUFS / -3 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | improved rhythm via punctuation prosody; owner ear verdict pending |
| Prosody (human verdict) | PENDING | punctuation-prosody 'A.I.' variant; owner ear verdict pending with the playback head-to-head |
| Prosody (LLM judge) | not run | — |
| Initialism risk | — | punctuation form 'A.I.' — natural rhythm, verified per-segment |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | head-to-head vs sarah_reply_latentsync on the owner's Desktop pending |
| A/V sync (start / middle / end) | not run / not run / not run | not separately re-measured; frame inspection surfaced no offset |
| Crop sharpness | PASS | teeth defined but natural — clearly between raw MuseTalk softness and plastic full-GFPGAN |
| Temporal boil/flicker | PASS | consecutive frames at t=8 s smooth, no boil |
| Chunk-boundary jerk | PASS | measures smoother than every other take INCLUDING the original — the EMA + 0.45 blend + feathered mask recipe delivered |
| Identity drift | PASS | — |
| Paste-back seam | PASS | — |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 565 | 24 | 1.26 / 2.13 | 0.15 / 0.44 |

Bad-frame exclusions:

- clip 8 trimmed to frames 0-184 (no-face tail)

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 252 s |
| GPU | GCE g2-standard-8, 1x NVIDIA L4 24 GB (us-central1-b) |
| Cost estimate | $0.06 (252 s render at ~$0.85/h (~3x faster than LatentSync)) |
| Artifact existence (object_exists) | PASS |
| Host disposition | left_running (prod_render_node — sarah-avatar-gpu-1 is the permanent OAV render node after the 2026-07-09 /sarah flip (~$0.85/h)) |
| GCS index updated | yes |
| No secrets in artifacts | attested |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
