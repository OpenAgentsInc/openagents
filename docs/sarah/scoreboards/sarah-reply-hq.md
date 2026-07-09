# Take scoreboard — sarah_reply_hq (v2) — normalized-text audio + CRF16 encode

- Schema: `sarah-take-scoreboard.v1`
- Take: `sarah-reply-hq` (2026-07-09)
- Refs: #8610, #8618
- **Advance: NO** — Audio defects from v1 all fixed (STT-verified); encode softness gone; residual MuseTalk 256^2 teeth softness. Baseline for the enhancement variants; no standalone owner playback verdict.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | footage clip 8 (v2_traced-df7d3b47…, 720p24), trimmed to frames 0-184 (7.71 s), cycled forward+reverse |
| Script | OAV-1 Sarah reply (opener + C.1 product answer), re-synthesized with spoken-form normalization, 23.96 s |
| TTS reference | gs://openagentsgemini-oa-artifacts/sarah-avatar/voice-ref/sarah_voice_ref_v1.wav (clip 4, 5.40 s, STT confidence 1.00) |
| Model: musetalk | 1.5 (256^2 mouth inpaint, fp16, batch 20) |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot + spoken-form normalizer |
| Model: encode | libx264 CRF16 preset slow (2.6 Mbps) + loudnorm |
| Recipe | raw MuseTalk paste-back, no enhancement; normalized-text audio re-synthesis; CRF16 encode |
| Commits | hydralisk 40b2783 (spoken-form TTS normalizer: initialism lexicon + all-caps heuristic) |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/sarah_reply_hq.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/qa2/ |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (whole-clip, Gemini (independent round-trip on the final mux)) | PASS | 'A I' spoken as letters 3/3; 'coding agents' correct; all v1 defects fixed |
| Loudness / true peak | -18.4 LUFS / -2.9 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | staccato letter-speech pacing from hard 'A I' letter-spacing |
| Prosody (human verdict) | WATCH | letter-spaced 'A I … A P I' reads staccato; punctuation-prosody variant ('A.I.') adopted in v3 |
| Prosody (LLM judge) | not run | — |
| Initialism risk | — | normalizer letter-spacing 'A I' — correct words, robotic rhythm |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | no owner in-motion verdict recorded for hq alone; owner watched the enhanced derivative |
| A/V sync (start / middle / end) | not run / not run / not run | not separately re-measured; QA2 frame inspection surfaced no offset |
| Crop sharpness | WATCH | encode softness gone at CRF16; residual MuseTalk 256^2 teeth softness remains |
| Temporal boil/flicker | PASS | no flicker in QA2 sampled windows |
| Chunk-boundary jerk | PASS | whole-timeline mouth-region mean \|diff\| 8.88 vs original 8.77 (statistically identical); native-fps jerk metrics not computed for this take |
| Identity drift | PASS | — |
| Paste-back seam | PASS | blend seam invisible in every sampled crop |

Bad-frame exclusions:

- clip 8 trimmed to frames 0-184 (no-face tail)

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 108 s |
| GPU | GCE g2-standard-8, 1x NVIDIA L4 24 GB (us-central1-b) |
| Cost estimate | $0.03 (108 s render at ~$0.85/h) |
| Artifact existence (object_exists) | PASS |
| Host disposition | left_running (prod_render_node — sarah-avatar-gpu-1 is the permanent OAV render node after the 2026-07-09 /sarah flip (~$0.85/h)) |
| GCS index updated | yes |
| No secrets in artifacts | attested |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
