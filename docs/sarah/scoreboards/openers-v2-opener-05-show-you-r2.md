# Take scoreboard — Openers v2 r2 — opener-05 show-you (Hallo2 tier, wave-2 audio: 8/10)

- Schema: `sarah-take-scoreboard.v1`
- Take: `openers-v2-opener-05-show-you-r2` (2026-07-10)
- Refs: #8610, #8618
- **Advance: NO** — Audio re-roll target achieved: 8/10 (w2-pb-s777, verbatim, up from 7/10 zs-s7). Same approved still/recipe as the near-shippable original, which stays unchanged for A/B. Zero boil, stable identity, no dominant chunk seam. Awaiting owner playback. Sharpness tier stays raw 512^2 (FLAIR rejected on runtime + license).

## Input refs

| Field | Value |
| --- | --- |
| Source clip | single source still src_opener05.jpg (same still as the owner-praised v2 take) — Hallo2 still-animation tier |
| Script | Let me show you what that would look like with an agent doing it. (3.2 s) |
| TTS reference | CosyVoice2 clone, audio re-roll wave-2 winner w2-pb-s777 (long28 concatenated ref, comma prosody variant — same words verbatim, seed 777) — Gemini judge 8/10 (runner-up w2-lr-s3 also 8). Upgrades the 7/10 zs-s7 of the owner-praised take. Brisker 3.2 s read vs the original 5.0 s — both kept for A/B. Wav at gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-05-show-you-r2.wav |
| Model: hallo2 | Hallo2 (MIT) inference_long.py, 512^2 audio-driven portrait animation, 16-frame chunks x 40 DDIM steps, net.pth + wav2vec2-base-960h |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot clone (long-reference), TTS run on the A100 experiment host — prod L4 untouched |
| Recipe | Hallo2 still-animation quality tier: wave-2 judge-winner audio over the same calm source still. Re-mux with full-band judged wav, measured-gain -0.2 dB to -16.9 LUFS (alimiter -3.0 dB), libx264 CRF16 slow. No SR pass — the S-Lab-encumbered video_sr.py lane is retired pending a license-clean upscaler |
| Render command | python scripts/inference_long.py --config configs/inference/v2r2_opener05.yaml (A100 40GB, clean save_path v2r2_opener05) |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-05-show-you-hallo2-r2.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-05-show-you-r2.wav |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/qa/opener-05-show-you-hallo2-r2-consec6.jpg |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/bakeoff-r2/bakeoff_r2_results.json |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, whisper small.en on the final mux) | PASS | verbatim: 'Let me show you what that would look like with an agent doing it.' |
| Loudness / true peak | -16.9 LUFS / -3.2 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | judge flagged a slight artificial pause after 'look like'. The wave-1 low-frequency rumble and flat-ending defects are gone |
| Prosody (human verdict) | PENDING | judge: 'A highly polished and professional delivery that almost perfectly mimics a human presenter.' Same rubric/method as the 07-09 bake-off |
| Prosody (LLM judge) | 8/10 | gemini-3.5-flash (naturalness 8 / warmth 8 / confidence 9 / overall 8) |
| Initialism risk | — | script avoids initialisms by design |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | NEW take — awaiting owner playback of ~/Desktop/sarah-openers-v2/opener-05-show-you-hallo2-r2.mp4. The ORIGINAL opener-05 take was called 'close to shippable' by the owner and is kept unchanged beside it. This r2 only upgrades the audio (8/10 vs 7/10) with a brisker read — owner picks on A/B |
| A/V sync (start / middle / end) | WATCH / not run / not run | speech onset articulation visible in consecutive-frame reads. Full 3-point sync check deferred to playback review |
| Crop sharpness | WATCH | 512^2 native output soft vs 720p source. FLAIR evaluated as the permissive upscaler and REJECTED (runtime ~33 min per second of video on A100, S-Lab CodeFormer aux inside its sampling loop, hard 512^2 output cap) — raw 512^2 stays the ship tier |
| Temporal boil/flicker | PASS | 6 consecutive frames at t=1.4 s (corrected mouth crop): broad smile with defined teeth, smooth articulation, zero boil, stable identity |
| Chunk-boundary jerk | WATCH | healthy articulation. Motion profile matches the take the owner called close to shippable |
| Identity drift | PASS | identity faithful to source still across sampled frames and full-frame stills |
| Paste-back seam | PASS | no paste-back — Hallo2 generates the full frame |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 80 | 25 | 2.31 / 4.6 | 1.26 / 3.18 |

Periodic hitch: mod-16 phase max 2.29 vs 1.26 mean — no dominant seam. In family with the owner-praised original (1.87 vs 1.14)

same proven still + recipe as the owner-praised v2 take. Only the audio changed (8/10 vs 7/10, faster read)

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 175 s |
| GPU | GCE a2-highgpu-1g, 1x NVIDIA A100-SXM4-40GB (us-central1-f, SPOT) — sarah-hallo2-exp-1 |
| Cost estimate | $0.07 (~175 s inference + remux/QA share at ~$1.30/h spot, session total in the run closeout receipt) |
| Artifact existence (object_exists) | PASS — gsutil stat on final mp4 + winner wav + qa crops/strips + provenance JSONs (12/12 OK) |
| Host disposition | stopped |
| GCS index updated | yes |
| No secrets in artifacts | attested |
| Closeout receipt | docs/sarah/receipts/2026-07-10-openers-v2-r2-a100.json |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
