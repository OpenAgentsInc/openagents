# Take scoreboard — Openers v2 — opener-01 hello (Hallo2 quality tier, judge-winner audio)

- Schema: `sarah-take-scoreboard.v1`
- Take: `openers-v2-opener-01-hello` (2026-07-10)
- Refs: #8610, #8618
- **Advance: NO** — Hallo2 quality-tier render of the bake-off winner audio (judge 7/10): STT verbatim PASS per segment, no boil, stable identity, generated head motion. Awaiting owner playback; SR variant is research-only pending the S-Lab license question.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | single source still source_square.jpg (square face crop per Hallo2 spec, derived from the calm Sarah source set) — Hallo2 still-animation tier, no source-motion conflict by construction |
| Script | Hello! I'm Sarah. What's on your mind today? (2.6 s) |
| TTS reference | CosyVoice2 clone, openers-v2 audio bake-off winner lr-s31337 (30-60 s concatenated long voice reference, seed 31337); wav at gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-01-hello.wav |
| Model: hallo2 | Hallo2 (MIT) inference_long.py, 512^2 audio-driven portrait animation, 16-frame chunks x 40 DDIM steps, net.pth + wav2vec2-base-960h |
| Model: video_sr | Hallo2 scripts/video_sr.py x4 (CodeFormer architecture + Hallo2 net_g.pth VFHQ weights + RealESRGAN_x2plus bg) — upstream header requires S-Lab License 1.0 compliance: RESEARCH-ONLY, non-shippable |
| Model: cosyvoice | CosyVoice2-0.5B long-reference clone (bake-off candidate pool) |
| Recipe | Hallo2 still-animation quality tier: judged winner audio over a single calm source still; re-mux with full-band judged wav, measured-gain normalization +9.7 dB to -17.4 LUFS (alimiter ceiling -3.8 dB), libx264 CRF16 slow; SR variant is evaluation-only (license) |
| Render command | python scripts/inference_long.py --config configs/inference/v2_opener01.yaml (A100 40GB); python scripts/video_sr.py -i <merge> -o <out> --bg_upsampler realesrgan --face_upsample -w 1 -s 4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-01-hello-hallo2.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-01-hello-hallo2-sr.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/qa/opener-01-hello-hallo2-consec6.jpg |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, whisper small.en on the final mux) | PASS | 2 segments, verbatim: 'Hello, I'm Sarah.' / 'What's on your mind today?' — no rising-intonation 'Hello?' flag (the v1 defect) |
| Loudness / true peak | -17.4 LUFS / -3.7 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | judge flagged 'slightly clipped phrasing on Hello! I'm Sarah' and slightly rushed pacing |
| Prosody (human verdict) | PENDING | best available candidate for this script from the bake-off pool; sub-8 overall — flagged for a future audio re-roll if owner playback confirms the judge |
| Prosody (LLM judge) | 7/10 | gemini-3.5-flash (naturalness 7 / warmth 7 / confidence 8 / overall 7) |
| Initialism risk | — | script avoids initialisms by design |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | awaiting owner playback of ~/Desktop/sarah-openers-v2/ copies — stills never advance a take |
| A/V sync (start / middle / end) | WATCH / not run / not run | speech onset articulation visible in first consecutive-frame strip; full 3-point sync check deferred to playback review |
| Crop sharpness | WATCH | 512^2 native output is soft vs 720p source footage; SR x4 variant recovers texture but is license-encumbered (S-Lab) — serving upscale strategy open |
| Temporal boil/flicker | PASS | 6 consecutive frames at t=1.3 s: smooth articulation progression, zero boil, stable identity |
| Chunk-boundary jerk | WATCH | numbers NOT comparable to MuseTalk-lane takes (full-face 512^2 portrait crop with real generated head/face motion vs 720p paste-back mouth crop) |
| Identity drift | PASS | identity faithful to source still across sampled frames |
| Paste-back seam | PASS | no paste-back — Hallo2 generates the full frame |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 65 | 25 | 2.03 / 3.77 | 1.06 / 2.86 |

Periodic hitch: jerk by frame-index mod 16: phase 14 at 2.2 vs 1.06 mean — possible 16-frame chunk seam like LatentSync's; needs playback confirmation

Hallo2 generates head/eye/face motion matched to speech — eliminates the source-motion conflict that sank opener library v1

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 326 s |
| GPU | GCE a2-highgpu-1g, 1x NVIDIA A100-SXM4-40GB (us-central1-f, SPOT) — sarah-hallo2-exp-1 |
| Cost estimate | $0.12 (~180 s inference + 146 s SR at ~$1.30/h spot; idle-host waste booked in the run closeout receipt) |
| Artifact existence (object_exists) | PASS — gsutil stat on final + SR + qa crops in gs://…/openers-v2/ |
| Host disposition | stopped |
| GCS index updated | yes |
| No secrets in artifacts | attested |
| Closeout receipt | docs/sarah/receipts/2026-07-10-openers-v2-hallo2-a100.json |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
