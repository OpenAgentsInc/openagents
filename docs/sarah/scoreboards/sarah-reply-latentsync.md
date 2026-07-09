# Take scoreboard — sarah_reply_latentsync — LatentSync 1.6 diffusion (offline quality tier)

- Schema: `sarah-take-scoreboard.v1`
- Take: `sarah-reply-latentsync` (2026-07-09)
- Refs: #8610, #8618
- **Advance: NO** — Best articulation of any take; blocked by the 16-frame chunk-boundary hitch (~2x jerk, periodic every 0.64 s). Offline/recording-tier candidate only, after chunk-seam smoothing.

## Input refs

| Field | Value |
| --- | --- |
| Source clip | footage clip 8 (v2_traced-df7d3b47…, 720p24), trimmed to frames 0-184 (7.71 s); output resampled to 25 fps (LatentSync training fps) |
| Script | OAV-1 Sarah reply (opener + C.1 product answer), same v2 normalized-text audio, ~24 s |
| TTS reference | gs://openagentsgemini-oa-artifacts/sarah-avatar/voice-ref/sarah_voice_ref_v1.wav (clip 4, 5.40 s, STT confidence 1.00) |
| Model: latentsync | 1.6 (512^2 lip-region diffusion, 20 steps, deepcache; Apache-2.0 verified in-repo) |
| Model: cosyvoice | CosyVoice2-0.5B zero-shot + spoken-form normalizer |
| Recipe | ByteDance LatentSync 1.6 diffusion (D1); NOTE this take muxed the 16 kHz audio copy — final recipe must re-mux the full-band 24 kHz wav |
| Commits | hydralisk 40b2783 (spoken-form TTS normalizer) |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/sarah_reply_latentsync.mp4 |
| Artifact | gs://openagentsgemini-oa-artifacts/sarah-avatar/oav1/qa2/ |

## Audio gates

| Gate | Status | Notes |
| --- | --- | --- |
| STT round-trip (whole-clip, Gemini (same v2 audio as hq)) | PASS | same v2 audio; 16 kHz mux copy in this take — re-mux full-band wav in the final recipe |
| Loudness / true peak | -18.6 LUFS / -2.8 dBTP | target ≈ -16 LUFS / ≤ -3 dBTP |
| Pause timing | WATCH | same staccato v2 letter-spaced audio as hq |
| Prosody (human verdict) | WATCH | video looks natural DESPITE the staccato v2 audio — supporting the diagnosis that full-strength GFPGAN was the main motion problem in the enhanced take |
| Prosody (LLM judge) | not run | — |
| Initialism risk | — | same v2 letter-spaced audio |

## Video gates

| Gate | Status | Notes |
| --- | --- | --- |
| **Owner playback verdict** | PENDING | head-to-head vs sarah_reply_v3 on the owner's Desktop pending; agent frame inspection called it the naturalness winner so far |
| A/V sync (start / middle / end) | not run / not run / not run | not separately re-measured; frame inspection surfaced no offset |
| Crop sharpness | PASS | teeth naturally soft (not GAN-crisp) — reads human, not plastic; 512^2 region |
| Temporal boil/flicker | PASS | consecutive-frame check at t=8 s: smooth coarticulation, zero boil — its temporal layer works |
| Chunk-boundary jerk | FAIL | ~2x jerk vs baseline even normalized for its wider articulation, and it is periodic |
| Identity drift | PASS | identity stable across consecutive frames |
| Paste-back seam | PASS | seam invisible |

Motion metrics (mouth crop, lower jerk = smoother):

| Frames | fps | Motion mean / p95 | Jerk mean / p95 |
| --- | --- | --- | --- |
| 603 | 25 | 1.48 / 2.49 | 0.37 / 1.1 |

Periodic hitch: mean jerk by frame-index mod 16: phases 14-15 at 0.62/0.58 vs ~0.33 baseline — rhythmic hitch every 0.64 s at the 16-frame diffusion chunk boundary; upstream scripts/inference.py exposes no overlap/blending param, so mitigation needs pipeline surgery

Bad-frame exclusions:

- clip 8 trimmed to frames 0-184 (no-face tail)

Best articulation of any take: clearly wider and more natural mouth shapes than MuseTalk (which under-articulates); rounded visemes present.

## Operational gates (SQ-8 closeout)

| Item | Value |
| --- | --- |
| Render wall | 756 s |
| GPU | GCE g2-standard-8, 1x NVIDIA L4 24 GB (us-central1-b) |
| Cost estimate | $0.18 (756 s render at ~$0.85/h (~31x realtime — offline recording tier only)) |
| Artifact existence (object_exists) | PASS |
| Host disposition | left_running (prod_render_node — sarah-avatar-gpu-1 is the permanent OAV render node after the 2026-07-09 /sarah flip (~$0.85/h)) |
| GCS index updated | yes |
| No secrets in artifacts | attested |

Closeout checklist: `docs/sarah/GPU_MEDIA_RUN_CLOSEOUT.md` (#8625) — artifact-existence monitors only, host disposition with reason, GCS index updated, cost recorded, no secrets.

---

Law: **no take advances on stills.** Owner playback verdict plus temporal evidence decide (SQ-1 #8618).
