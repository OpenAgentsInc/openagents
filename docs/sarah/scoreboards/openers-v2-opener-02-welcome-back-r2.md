# Take scoreboard — Openers v2 r2 — opener-02 welcome-back (Hallo2 tier, wave-2 audio: 9/10 VERBATIM)

- Schema: `sarah-take-scoreboard.v1`
- Take: `openers-v2-opener-02-welcome-back-r2` (2026-07-10)
- Refs: #8610, #8618
- **Advance: NO** — Audio re-roll target achieved: 9/10 VERBATIM 'did' (w2-slow92-s31415; wave-1 slur fixed at equal judge score). STT verbatim PASS, zero boil, stable identity, same approved still/recipe. Awaiting owner playback of the new render; sharpness tier stays raw 512^2 (FLAIR rejected on runtime + license).

## Input refs

| Field | Value |
| --- | --- |
| Source clip | single source still src_opener02.jpg (same still as the owner-approved v2 take) — Hallo2 still-animation tier |
| Script | Hey, welcome back! Where did we leave off? (3.6 s) |
| TTS reference | CosyVoice2 clone, audio re-roll wave-2 winner w2-slow92-s31415 (long28 concatenated ref, speed=0.92, seed 31415) — Gemini judge 9/10 with VERBATIM 'did' (fixes the wave-1 lr-s1986 'did'->'do' slur at equal score); wav at gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-02-welcome-back-r2.wav; full wave provenance in openers-v2/bakeoff-r2/bakeoff_r2_results.json |
| Model: hallo2 | Hallo2 (MIT) inference_long.py, 512^2 audio-driven portrait animation, 16-frame chunks x 40 DDIM steps, net.pth + wav2vec2-base-960h |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot clone (long-reference + speed control), TTS run on the A100 experiment host — prod L4 untouched |
| Recipe | Hallo2 still-animation quality tier: wave-2 judge-winner audio over the same calm source still; re-mux with full-band judged wav, measured-gain -0.9 dB to -16.5 LUFS (alimiter -3.0 dB), libx264 CRF16 slow. No SR pass — the S-Lab-encumbered video_sr.py lane is retired pending a license-clean upscaler |
| Render command | python scripts/inference_long.py --config configs/inference/v2r2_opener02.yaml (A100 40GB, clean save_path v2r2_opener02) |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-02-welcome-back-hallo2-r2.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-02-welcome-back-r2.wav |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/qa/opener-02-welcome-back-hallo2-r2-consec6.jpg |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/bakeoff-r2/bakeoff_r2_results.json |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, whisper small.en on the final mux) | PASS | VERBATIM: 'Hey, welcome back. Where did we leave off?' — the wave-1 'did'->'do' deviation is fixed in the winner audio itself |
| Loudness / true peak | -16.5 LUFS / -3.8 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | judge flagged a slightly abrupt audio cutoff at the very end; otherwise natural pacing at speed 0.92 |
| Prosody (human verdict) | PENDING | judge: 'An exceptionally natural and warm delivery that easily passes for a professional human voice actor.' Same rubric/method as the 07-09 bake-off |
| Prosody (LLM judge) | 9/10 | gemini-3.5-flash (naturalness 9 / warmth 9 / confidence 9 / overall 9) |
| Initialism risk | — | script avoids initialisms by design |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | NEW take — awaiting owner playback of ~/Desktop/sarah-openers-v2/opener-02-welcome-back-hallo2-r2.mp4 (original v2 take kept beside it for A/B). The v2 DIRECTION is owner-approved 2026-07-10; this specific render is not yet watched |
| A/V sync (start / middle / end) | WATCH / not run / not run | speech onset articulation visible in consecutive-frame reads; full 3-point sync check deferred to playback review |
| Crop sharpness | WATCH | 512^2 native output soft vs 720p source; FLAIR evaluated as the permissive upscaler and REJECTED (runtime ~33 min per second of video on A100, S-Lab CodeFormer aux inside its sampling loop, hard 512^2 output cap) — raw 512^2 stays the ship tier |
| Temporal boil/flicker | PASS | 6 consecutive frames at t=1.5 s (corrected mouth crop): smooth articulation, visible teeth, zero boil, stable identity |
| Chunk-boundary jerk | WATCH | in family with the owner-approved v2 batch (2.29/1.28 on the original opener-02 take) |
| Identity drift | PASS | identity faithful to source still across sampled frames and full-frame stills |
| Paste-back seam | PASS | no paste-back — Hallo2 generates the full frame |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 91 | 25 | 2.23 / 5.14 | 1.11 / 3.21 |

Periodic hitch: mod-16 phase max 2.32 vs 1.11 mean (phases 12-14 elevated) — possible mild chunk seam; playback will tell

same proven still + recipe as the owner-approved v2 take; only the audio changed (9/10 verbatim vs 9/10 with slur)

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 199 s |
| GPU | GCE a2-highgpu-1g, 1x NVIDIA A100-SXM4-40GB (us-central1-f, SPOT) — sarah-hallo2-exp-1 |
| Cost estimate | $0.08 (~199 s inference + remux/QA share at ~$1.30/h spot; session total in the run closeout receipt) |
| Artifact existence (object_exists) | PASS — gsutil stat on final mp4 + winner wav + qa crops/strips + provenance JSONs (12/12 OK) |
| Host disposition | stopped |
| GCS index updated | yes |
| No secrets in artifacts | attested |
| Closeout receipt | docs/sarah/receipts/2026-07-10-openers-v2-r2-a100.json |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
