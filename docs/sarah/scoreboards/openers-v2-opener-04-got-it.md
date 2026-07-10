# Take scoreboard — Openers v2 — opener-04 got-it (Hallo2 quality tier, judge-winner audio)

- Schema: `sarah-take-scoreboard.v1`
- Take: `openers-v2-opener-04-got-it` (2026-07-10)
- Refs: #8610, #8618
- **Advance: NO** — Top-tier audio (judge 9/10) with the most expressive generated delivery in the batch (big smile, highest motion). STT verbatim PASS; clean temporal inspection. OWNER PLAYBACK PASS 2026-07-10 (v2 direction approved); advance still gated on license-clean sharpness tier (audio already 9/10 verbatim).

## Input refs

| Field | Value |
| --- | --- |
| Source clip | single source still src_opener04.jpg (square face crop per Hallo2 spec) — Hallo2 still-animation tier |
| Script | Got it. And how much time does that eat up each week? (3.4 s) |
| TTS reference | CosyVoice2 clone, openers-v2 audio bake-off winner lr-s555 (30-60 s concatenated long voice reference, seed 555); wav at gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-04-got-it.wav |
| Model: hallo2 | Hallo2 (MIT) inference_long.py, 512^2 audio-driven portrait animation, 16-frame chunks x 40 DDIM steps, net.pth + wav2vec2-base-960h |
| Model: video_sr | Hallo2 scripts/video_sr.py x4 (CodeFormer architecture + Hallo2 net_g.pth VFHQ weights + RealESRGAN_x2plus bg) — upstream header requires S-Lab License 1.0 compliance: RESEARCH-ONLY, non-shippable |
| Model: cosyvoice | CosyVoice2-0.5B long-reference clone (bake-off candidate pool) |
| Recipe | Hallo2 still-animation quality tier: judged winner audio over a single calm source still; re-mux with full-band judged wav, measured-gain normalization +2.0 dB to -17.3 LUFS (alimiter ceiling -3.8 dB), libx264 CRF16 slow; SR variant is evaluation-only (license) |
| Render command | python scripts/inference_long.py --config configs/inference/v2_opener04.yaml (A100 40GB); python scripts/video_sr.py -i <merge> -o <out> --bg_upsampler realesrgan --face_upsample -w 1 -s 4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-04-got-it-hallo2.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-04-got-it-hallo2-sr.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/qa/opener-04-got-it-hallo2-consec6.jpg |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, whisper small.en on the final mux) | PASS | verbatim: 'Got it. And how much time does that eat up each week?' |
| Loudness / true peak | -17.3 LUFS / -2.9 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | judge flagged an abrupt end cutoff |
| Prosody (human verdict) | PENDING | 'exceptionally natural and conversational delivery' — top-scored alongside opener-02 |
| Prosody (LLM judge) | 9/10 | gemini-3.5-flash (naturalness 9 / warmth 8 / confidence 9 / overall 9) |
| Initialism risk | — | script avoids initialisms by design |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PASS | OWNER PLAYBACK 2026-07-10 (batch verdict, his words verbatim): "those v2s are much better - opener-05-show-you-hallo2.mp4 is for example close to shippable so proceed in that direction" — openers-v2 Hallo2 direction APPROVED. Advance stays gated on: license-clean sharpness tier (audio already 9/10 verbatim). |
| A/V sync (start / middle / end) | WATCH / not run / not run | speech onset articulation visible in first consecutive-frame strip; full 3-point sync check deferred to playback review |
| Crop sharpness | WATCH | 512^2 native output is soft vs 720p source footage; SR x4 variant recovers texture but is license-encumbered (S-Lab) — serving upscale strategy open |
| Temporal boil/flicker | PASS | 6 consecutive frames at t=1.7 s: bright open smile with defined teeth, smooth progression, zero boil |
| Chunk-boundary jerk | WATCH | numbers NOT comparable to MuseTalk-lane takes (full-face 512^2 portrait crop with real generated head/face motion vs 720p paste-back mouth crop); high motion here is expressiveness, not necessarily artifact |
| Identity drift | PASS | identity faithful to source still across sampled frames |
| Paste-back seam | PASS | no paste-back — Hallo2 generates the full frame |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 84 | 25 | 3.14 / 7.39 | 1.6 / 5.34 |

Periodic hitch: highest motion/jerk in the batch — Hallo2 generated an expressive big-smile delivery; no single dominant mod-16 phase

Hallo2 generates head/eye/face motion matched to speech — eliminates the source-motion conflict that sank opener library v1

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 390 s |
| GPU | GCE a2-highgpu-1g, 1x NVIDIA A100-SXM4-40GB (us-central1-f, SPOT) — sarah-hallo2-exp-1 |
| Cost estimate | $0.14 (~230 s inference + ~160 s SR at ~$1.30/h spot; idle-host waste booked in the run closeout receipt) |
| Artifact existence (object_exists) | PASS — gsutil stat on final + SR + qa crops in gs://…/openers-v2/ |
| Host disposition | stopped |
| GCS index updated | yes |
| No secrets in artifacts | attested |
| Closeout receipt | docs/sarah/receipts/2026-07-10-openers-v2-hallo2-a100.json |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
