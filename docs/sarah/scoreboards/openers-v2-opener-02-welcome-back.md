# Take scoreboard — Openers v2 — opener-02 welcome-back (Hallo2 quality tier, judge-winner audio)

- Schema: `sarah-take-scoreboard.v1`
- Take: `openers-v2-opener-02-welcome-back` (2026-07-10)
- Refs: #8610, #8618
- **Advance: NO** — Strongest audio in the library (judge 9/10). Video gates clean in temporal inspection; STT 'did'->'do' word deviation inherited from the winner audio is flagged for owner adjudication. Awaiting owner playback.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | single source still src_opener02.jpg (square face crop per Hallo2 spec) — Hallo2 still-animation tier |
| Script | Hey, welcome back! Where did we leave off? (2.4 s) |
| TTS reference | CosyVoice2 clone, openers-v2 audio bake-off winner lr-s1986 (30-60 s concatenated long voice reference, seed 1986); wav at gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-02-welcome-back.wav |
| Model: hallo2 | Hallo2 (MIT) inference_long.py, 512^2 audio-driven portrait animation, 16-frame chunks x 40 DDIM steps, net.pth + wav2vec2-base-960h |
| Model: video_sr | Hallo2 scripts/video_sr.py x4 (CodeFormer architecture + Hallo2 net_g.pth VFHQ weights + RealESRGAN_x2plus bg) — upstream header requires S-Lab License 1.0 compliance: RESEARCH-ONLY, non-shippable |
| Model: cosyvoice | CosyVoice2-0.5B long-reference clone (bake-off candidate pool) |
| Recipe | Hallo2 still-animation quality tier: judged winner audio over a single calm source still; re-mux with full-band judged wav, measured-gain normalization +12.5 dB to -17.1 LUFS (alimiter ceiling -3.8 dB), libx264 CRF16 slow; SR variant is evaluation-only (license) |
| Render command | python scripts/inference_long.py --config configs/inference/v2_opener02.yaml (A100 40GB); python scripts/video_sr.py -i <merge> -o <out> --bg_upsampler realesrgan --face_upsample -w 1 -s 4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-02-welcome-back-hallo2.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-02-welcome-back-hallo2-sr.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/qa/opener-02-welcome-back-hallo2-consec6.jpg |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, whisper small.en on the final mux) | WATCH | hears 'Where do we leave off?' vs script 'Where did we leave off?' — the deviation is in the winner audio itself (bake-off manifest records the same transcript and still scored it 9/10); flagged for owner adjudication, not silently passed |
| Loudness / true peak | -17.1 LUFS / -3.7 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | judge flagged slightly abrupt cutoff at the very end |
| Prosody (human verdict) | PENDING | highest-scored candidate in the bake-off: 'easily passes for a professional human voice actor' |
| Prosody (LLM judge) | 9/10 | gemini-3.5-flash (naturalness 9 / warmth 8 / confidence 9 / overall 9) |
| Initialism risk | — | script avoids initialisms by design |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | awaiting owner playback of ~/Desktop/sarah-openers-v2/ copies — stills never advance a take |
| A/V sync (start / middle / end) | WATCH / not run / not run | speech onset articulation visible in first consecutive-frame strip; full 3-point sync check deferred to playback review |
| Crop sharpness | WATCH | 512^2 native output is soft vs 720p source footage; SR x4 variant recovers texture but is license-encumbered (S-Lab) — serving upscale strategy open |
| Temporal boil/flicker | PASS | 6 consecutive frames at t=1.2 s: smooth progression, zero boil, stable identity |
| Chunk-boundary jerk | WATCH | numbers NOT comparable to MuseTalk-lane takes (full-face 512^2 portrait crop with real generated head/face motion vs 720p paste-back mouth crop) |
| Identity drift | PASS | identity faithful to source still across sampled frames |
| Paste-back seam | PASS | no paste-back — Hallo2 generates the full frame |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 61 | 25 | 2.29 / 4.64 | 1.28 / 3.19 |

Periodic hitch: jerk by frame-index mod 16: phase 0 at 2.7 vs 1.28 mean — possible chunk seam; needs playback confirmation

Hallo2 generates head/eye/face motion matched to speech — eliminates the source-motion conflict that sank opener library v1

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 278 s |
| GPU | GCE a2-highgpu-1g, 1x NVIDIA A100-SXM4-40GB (us-central1-f, SPOT) — sarah-hallo2-exp-1 |
| Cost estimate | $0.1 (~140 s inference + 138 s SR at ~$1.30/h spot; idle-host waste booked in the run closeout receipt) |
| Artifact existence (object_exists) | PASS — gsutil stat on final + SR + qa crops in gs://…/openers-v2/ |
| Host disposition | stopped |
| GCS index updated | yes |
| No secrets in artifacts | attested |
| Closeout receipt | docs/sarah/receipts/2026-07-10-openers-v2-hallo2-a100.json |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
