# Take scoreboard — sarah_reply v1 — MuseTalk 1.5 baseline (OAV-1 offline proof)

- Schema: `sarah-take-scoreboard.v1`
- Take: `sarah-reply-v1` (2026-07-09)
- Refs: #8610, #8611, #8618
- **Advance: NO** — Agent frame-QA GO on identity/sync (zero jitter, invisible seam); audio FAIL ('AI' as 'eye', 'coding' slur, -0.2 dBFS peak). Superseded by hq.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | footage clip 8 (v2_traced-df7d3b47…, 720p24), trimmed to frames 0-184 (7.71 s), cycled forward+reverse |
| Script | OAV-1 Sarah reply (opener + C.1 product answer, 371 chars, public-safe KB pitch), 21.84 s |
| TTS reference | gs://openagentsgemini-oa-artifacts/sarah-avatar/voice-ref/sarah_voice_ref_v1.wav (clip 4, 5.40 s, STT confidence 1.00) |
| Model: musetalk | 1.5 (256^2 mouth inpaint, fp16, batch 20) |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot (no text normalizer yet) |
| Model: encode | libx264 ~1.0 Mbps |
| Recipe | raw MuseTalk paste-back, no enhancement; pre-normalizer audio |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/sarah_oav1_reply.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/sarah_reply.wav |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/qa/ |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (whole-clip, dual independent transcription) | FAIL | 'AI' rendered as 'eye'; 'coding agents' slurred to 'Cody's agents' (heard identically by both passes) |
| Loudness / true peak | — / -0.2 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | inter-sentence pauses slightly compressed/robotic (one continuous 371-char take); peak -0.2 dBFS near clipping, mean -18.5 dB — loudnorm added in hq |
| Prosody (human verdict) | WATCH | energetic, clearly the reference voice; pacing slightly robotic between sentences (rated 8/10); no LLM judge yet in this era |
| Prosody (LLM judge) | not run | — |
| Initialism risk | — | no spoken-form normalizer yet — 'AI' spoken as 'eye' |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | no explicit owner in-motion verdict recorded; QA1 GO was agent frame inspection; superseded by the hq/enhanced/v3 ladder |
| A/V sync (start / middle / end) | PASS / PASS / PASS | mouth state tracks speech onset/offset across sampled boundaries (QA1) |
| Crop sharpness | WATCH | mouth soft/mushy: ~1.0 Mbps encode softness on top of the MuseTalk 256^2 ceiling; teeth a soft bright band at 2x zoom |
| Temporal boil/flicker | PASS | zero single-frame jitter in consecutive-frame bursts (7.0 s burst; t=5 s and t=12 s tiles) |
| Chunk-boundary jerk | PASS | — |
| Identity drift | PASS | pixel-identical to source outside the mouth crop (paste-back architecture) |
| Paste-back seam | PASS | no seam line, color shift, or chin discontinuity at any sampled timestamp incl. tilted head at t=18 s |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 523 | 24 | 1.33 / 2.27 | 0.19 / 0.52 |

Bad-frame exclusions:

- clip 8 frames 185-191 dropped (no detectable face — DWPose placeholder bbox crashes MuseTalk prep)
- eyes-closed source pose at t≈18 s noted — speaking-state ranges must pin to eyes-open windows (OAV-2)

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 119 s |
| GPU | GCE g2-standard-8, 1x NVIDIA L4 24 GB (us-central1-b) |
| Cost estimate | $6 (full OAV-1 session ~7 h wall (~1.5 h busy) incl. env setup and two monitor-stall gaps; this render itself 119 s) |
| Artifact existence (object_exists) | PASS — monitors rewritten to artifact existence after two log-marker stall incidents (OAV-1 receipt §6.1) |
| Host disposition | left_running (staged for OAV-2/OAV-3 (host later became prod_render_node after the 2026-07-09 /sarah flip)) |
| GCS index updated | yes |
| No secrets in artifacts | attested |
| Closeout receipt | apps/sarah/fixtures/gpu-media-run-closeout.example.json (v1 receipt shaped from this run) |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
