# Take scoreboard — Openers v2 — opener-05 show-you (Hallo2 quality tier, judge-winner audio)

- Schema: `sarah-take-scoreboard.v1`
- Take: `openers-v2-opener-05-show-you` (2026-07-10)
- Refs: #8610, #8618
- **Advance: NO** — Longest clip in the batch, only zero-shot audio winner (judge 7/10). STT verbatim PASS, healthy articulation (fixes v1's under-articulated opener-05), flattest chunk-phase profile. OWNER PLAYBACK PASS 2026-07-10 (v2 direction approved, named close to shippable); advance still gated on audio judge 7/10 (re-roll to >=8 in flight) and license-clean sharpness tier.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | single source still src_opener05.jpg (square face crop per Hallo2 spec) — Hallo2 still-animation tier |
| Script | Let me show you what that would look like with an agent doing it. (5.0 s) |
| TTS reference | CosyVoice2 clone, openers-v2 audio bake-off winner zs-s7 (zero-shot on sarah_voice_ref_v1.wav, seed 7 — the only zero-shot winner; long-reference candidates won 01-04); wav at gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-05-show-you.wav |
| Model: hallo2 | Hallo2 (MIT) inference_long.py, 512^2 audio-driven portrait animation, 16-frame chunks x 40 DDIM steps, net.pth + wav2vec2-base-960h |
| Model: video_sr | Hallo2 scripts/video_sr.py x4 (CodeFormer architecture + Hallo2 net_g.pth VFHQ weights + RealESRGAN_x2plus bg) — upstream header requires S-Lab License 1.0 compliance: RESEARCH-ONLY, non-shippable |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot clone |
| Recipe | Hallo2 still-animation quality tier: judged winner audio over a single calm source still; re-mux with full-band judged wav, measured-gain normalization +1.3 dB to -16.9 LUFS (alimiter ceiling -3.8 dB), libx264 CRF16 slow; SR variant is evaluation-only (license) |
| Render command | python scripts/inference_long.py --config configs/inference/v2_opener05.yaml (A100 40GB); python scripts/video_sr.py -i <merge> -o <out> --bg_upsampler realesrgan --face_upsample -w 1 -s 4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-05-show-you-hallo2.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/opener-05-show-you-hallo2-sr.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/openers-v2/qa/opener-05-show-you-hallo2-consec6.jpg |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (per-segment, whisper small.en on the final mux) | PASS | verbatim: 'Let me show you what that would look like with an agent doing it.' |
| Loudness / true peak | -16.9 LUFS / -3.2 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | judge flagged slightly robotic pacing on 'what that would look like' and flat intonation on the final words |
| Prosody (human verdict) | PENDING | judge also flagged low-frequency background rumble; sub-8 overall — flagged for a future audio re-roll if owner playback confirms the judge |
| Prosody (LLM judge) | 7/10 | gemini-3.5-flash (naturalness 7 / warmth 8 / confidence 8 / overall 7) |
| Initialism risk | — | script avoids initialisms by design |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PASS | OWNER PLAYBACK 2026-07-10 (batch verdict, his words verbatim): "those v2s are much better - opener-05-show-you-hallo2.mp4 is for example close to shippable so proceed in that direction" — openers-v2 Hallo2 direction APPROVED — this clip called out by name as close to shippable. Advance stays gated on: audio judge 7/10 (re-roll to >=8 in flight) and license-clean sharpness tier. |
| A/V sync (start / middle / end) | WATCH / not run / not run | speech onset articulation visible in first consecutive-frame strip; full 3-point sync check deferred to playback review |
| Crop sharpness | WATCH | 512^2 native output is soft vs 720p source footage; SR x4 variant recovers texture but is license-encumbered (S-Lab) — serving upscale strategy open |
| Temporal boil/flicker | PASS | 6 consecutive frames at t=2.5 s: smooth articulation with visible teeth, zero boil, stable identity |
| Chunk-boundary jerk | WATCH | numbers NOT comparable to MuseTalk-lane takes (full-face 512^2 portrait crop with real generated head/face motion vs 720p paste-back mouth crop); healthy articulation vs the under-articulated v1 opener-05 |
| Identity drift | PASS | identity faithful to source still across sampled frames |
| Paste-back seam | PASS | no paste-back — Hallo2 generates the full frame |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 124 | 25 | 2.32 / 5.39 | 1.14 / 3.73 |

Periodic hitch: no dominant mod-16 phase (max 1.87 vs 1.14 mean) — longest clip in the batch (8 chunks) shows the flattest phase profile

Hallo2 generates head/eye/face motion matched to speech — eliminates the source-motion conflict that sank opener library v1

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 500 s |
| GPU | GCE a2-highgpu-1g, 1x NVIDIA A100-SXM4-40GB (us-central1-f, SPOT) — sarah-hallo2-exp-1 |
| Cost estimate | $0.18 (~250 s inference + ~250 s SR at ~$1.30/h spot; idle-host waste booked in the run closeout receipt) |
| Artifact existence (object_exists) | PASS — gsutil stat on final + SR + qa crops in gs://…/openers-v2/ |
| Host disposition | stopped |
| GCS index updated | yes |
| No secrets in artifacts | attested |
| Closeout receipt | docs/sarah/receipts/2026-07-10-openers-v2-hallo2-a100.json |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
