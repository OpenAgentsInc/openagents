Below is the implementation-focused paper list I’d hand to agents. I’m treating “implement” as “safe to evaluate in the owned/commercial OAV pipeline unless otherwise noted.” License notes are engineering triage, not legal advice.

Priority order

1. LatentSync 1.6 — primary offline-quality replacement for MuseTalk

Download: paper from arXiv; code from the official ByteDance GitHub; weights from Hugging Face / repo links. LatentSync 1.6 is trained at 512×512 to address blurry lip/teeth detail, and the repo states 18 GB VRAM minimum for 1.6 inference, which fits the 24 GB L4. The repo is Apache-2.0.

Why it matters for OAV: this is the most direct answer to the “MuseTalk 256² ceiling.” Unlike GFPGAN, it does not sharpen a weak mouth after the fact; it regenerates the lip-synced region at a higher quality tier with diffusion and temporal representation alignment.

Implement/test:

* Run the same 24s script through LatentSync 1.6 at 512².
* Preprocess the source to the repo’s preferred format: video at 25 fps, audio at 16 kHz, 5–10s scene chunks, and aligned faces; the repo’s training/data docs explicitly use that style of preprocessing.
* Compare against MuseTalk HQ and MuseTalk + tamed enhancement using the existing QA crop.
* If it wins on naturalness, use it for the non-realtime recording lane and opener library first, not realtime.

Verdict: implement now for OAV-1/OAV-E. This is probably the best “quality ceiling” paper in the stack.

⸻

2. FLAIR — best new candidate for temporally consistent face-video restoration

Download: paper from arXiv / WACV project page; official code and pretrained models from the GitHub repo. The repo advertises MIT licensing and provides pretrained model download links.

Why it matters for OAV: FLAIR directly targets the defect GFPGAN exposed: image restoration methods can look good per frame but fail temporal coherence. FLAIR converts an image diffusion restoration model into a video diffusion model with recurrent refinement and temporal self-attention.

Implement/test:

* Run FLAIR on a stabilized face crop, not the whole 720p frame initially.
* Test three inputs: raw MuseTalk, MuseTalk + light unsharp, MuseTalk + GFPGAN alpha blend.
* Paste back with the existing feathered mask.
* Measure temporal boil on consecutive frames and watch the crop in motion.

Risk: slower and heavier than GFPGAN; likely not realtime. But for prerecorded openers, that is acceptable.

Verdict: add to the enhancement lane immediately. This is the strongest license-friendly alternative to per-frame GFPGAN that I found.

⸻

3. RIFE — fastest way to attack 24fps mouth strobing

Download: ECCV 2022 RIFE paper / official PyTorch repo; pretrained HD models are linked from the repo. The repo is MIT-licensed and advertises 2× interpolation for 720p video at 30+ FPS on a 2080 Ti-class GPU.

Why it matters for OAV: the owner’s “choppy” perception may be crisp viseme poses at 24 fps, not dropped frames. RIFE attacks that directly by creating 48 fps presentation frames after lip-sync.

Implement/test:

* Final-pass interpolate 24 → 48 fps after mux timing is correct.
* Do not retime audio.
* Test on raw MuseTalk HQ, tamed-GFPGAN MuseTalk, and LatentSync.
* Watch for interpolation artifacts around teeth, tongue, and lip closures.

Verdict: implement now. It is cheap, reversible, permissive, and tests the strobing hypothesis cleanly.

⸻

4. CosyVoice2 / CosyVoice3 — fix mechanical audio before blaming video

Download: papers, code, demos, and model references are linked from the official CosyVoice GitHub. CosyVoice is Apache-2.0, and the official repo lists pronunciation inpainting, text normalization, streaming, and instruct controls for language/dialect/emotion/speed/volume.

CosyVoice3 is worth evaluating if the model path is usable in the environment: the paper reports improvements over CosyVoice2 in content consistency, speaker similarity, and prosody naturalness, plus richer instructed speech generation with emotion, accent, emphasis, breath, and other tags.

Why it matters for OAV: clipped TTS causes clipped visemes. If “A I” and “A P I” are produced as hard isolated phonemes, MuseTalk or LatentSync will turn that into mechanical mouth motion.

Implement/test:

* Keep STT round-trip as a hard gate.
* Test these TTS variants:
    * “AI” as written.
    * “A.I.” with punctuation.
    * “A I” letter-spoken but with inserted pauses.
    * Script rewrite avoiding initialisms.
    * CosyVoice instruct/speed/emotion variants.
* Pick the audio that sounds most human before rendering video.
* For prerecorded openers, prefer scripts with no initialisms.

Verdict: implement in parallel with video work. It may be the highest-leverage naturalness fix.

⸻

5. MuseTalk 1.5 — tune the current generator before over-enhancing it

Download: paper from arXiv; code and weights from the official repo / Hugging Face links. The repo says MuseTalk uses a 256×256 face region, has v1.5 updates with perceptual/GAN/sync losses and spatio-temporal sampling, and is MIT/commercially usable per the repo’s license notes.

Why it matters for OAV: this is already the production proof lane. The current output was acceptable on identity/sync before GFPGAN, so a controlled tuning sweep may beat wholesale model replacement.

Implement/test:

* Explicitly lock fps end-to-end: source 24 fps vs MuseTalk-preferred 25 fps should be tested deliberately.
* Sweep bbox / center point / bbox_shift; the repo notes that the center point materially affects results.
* Sweep audio context padding for more coarticulation.
* Render raw, unsharp-only, and tamed-GFPGAN variants.
* Do not judge stills; judge motion crop playback.

Verdict: keep as realtime/offline baseline. Do not keep full-strength GFPGAN as the default enhancer.

⸻

6. BasicVSR++ / RealBasicVSR — permissive temporal video restoration

Download: official BasicVSR++ and RealBasicVSR repos. Both official repos show Apache-2.0 licensing. RealBasicVSR provides code, Colab, and video demos; BasicVSR++ is the stronger core temporal VSR model lineage.

Why it matters for OAV: these are not face-specific hallucination restorers. They are video restoration / super-resolution systems with temporal propagation, so they are less likely than GFPGAN to create frame-to-frame “new teeth every frame” artifacts.

Implement/test:

* First test on the mouth/face crop only.
* Compare:
    * raw MuseTalk → BasicVSR++ / RealBasicVSR
    * tamed GFPGAN → BasicVSR++ / RealBasicVSR
    * LatentSync → BasicVSR++ / RealBasicVSR
* Watch for over-smoothing and temporal lag around fast mouth closures.

Verdict: implement as a conservative enhancer. It is less semantically face-aware than FLAIR, but likely cheaper and safer than diffusion.

⸻

7. GFPGAN — keep only as a tamed prior, not as full-frame truth

Download: paper/code from the official GFPGAN repo; pretrained models are linked in the repo. GFPGAN is Apache-2.0. The repo notes that v1.3 is more natural but less sharp, while older sharper variants can look less natural.

Why it matters for OAV: GFPGAN explains the current failure. It can make stills look better while making motion less human. The right use is not “replace the crop with GFPGAN output”; it is “borrow a little detail from GFPGAN.”

Implement/test recipe:

* Try GFPGAN v1.3 and v1.4.
* Use alpha blend around 0.35–0.55 against raw MuseTalk crop.
* Apply enhancement only inside a feathered mouth/lower-face mask.
* Temporal EMA the delta:
    final = raw + EMA(alpha * (gfpgan - raw))
* Keep bbox coordinates fixed or smoothed.
* Compare with sharpen-only and FLAIR.

Verdict: do not ship full-strength GFPGAN. Ship only if alpha-blended, masked, and temporally smoothed beats raw MuseTalk in playback.

⸻

High-end / opener-library candidates

8. Hallo2 / Hallo3 — consider for prerecorded “perfect opener” clips

Download: official GitHub repos and Hugging Face weights. Hallo2 provides released code/weights, is MIT-licensed, and the repo says it was tested on A100-class GPUs.   Hallo3 also has an MIT code repo, but its Hugging Face page says the model is derivative of CogVideo-5B and is governed by that upstream license, so the model-weight license chain needs review before shipping.

Why it matters for OAV: these are image/audio-driven portrait animation systems, not just lip inpainting. They may create more globally human motion for prerecorded standard phrases.

Implement/test:

* Use only for non-realtime opener library.
* Test “Hello! I’m Sarah. What’s on your mind today?”
* Compare against LatentSync 1.6 on the same phrase.
* Check identity drift carefully; full portrait animation models can change expression/head motion more than desired.

Verdict: evaluate after LatentSync. Good for the “perfect prerecorded clips” lane, not first choice for owned renderer realtime.

⸻

Research-only or blocked for shipping

Paper/model	Where to read/download	Why it is relevant	Shipping status
KEEP	Paper/project/repo	Kalman-inspired temporal feature propagation for stable face-video restoration; conceptually very aligned with the GFPGAN flicker problem.  	Do not ship code/weights without permission; repo uses S-Lab non-commercial licensing.
PGTFormer	Paper/project/repo; pretrained models linked from repo.  	Parsing-guided temporal-coherent transformer for face restoration; directly targets alignment/jitter issues.	Do not ship without permission; license is non-commercial.
BFVR-STC / Efficient Video Face Enhancement with Enhanced Spatial-Temporal Consistency	Paper/repo; repo provides pretrained models and explicitly mentions pixel deflickering for synthesized talking-head videos.  	Very relevant: the README names talking-head deflickering as a use case.	Research-only until license is cleared. The repo says it is modified from CodeFormer, and I did not find a clean permissive license signal on the surfaced repo page.
CodeFormer	Paper/repo	Useful baseline for face restoration quality.	Blocked for commercial use under S-Lab non-commercial terms.
StableVSR	Paper/repo	Diffusion video super-resolution with temporal consistency; potentially useful for full-frame or face-crop enhancement. The repo page shows MIT.  	Needs dependency/model license review before shipping, especially if it depends on StableSR / Stable Diffusion weight chains.

⸻

Concrete experiment matrix

Run these in this order. Each take should use the same script, same source clip, same crop inspection, same loudnorm, and the same owner playback protocol.

Test	Recipe	Hypothesis tested	Pass signal
A0	MuseTalk raw HQ, no GFPGAN	Baseline naturalness	Owner sees smoothest motion even if soft.
A1	MuseTalk + sharpen-only crop	Detail can improve without GAN hallucination	Sharper crop, no plastic motion.
A2	MuseTalk + GFPGAN v1.3/v1.4 alpha 0.35/0.45/0.55	GFPGAN is usable only when tamed	More definition than raw, no plastic/choppy feel.
A3	A2 + temporal EMA on enhancement delta	Choppiness is per-frame high-frequency mode snapping	Motion feels calmer than A2.
B1	Best A-test + RIFE 48 fps	Choppiness is 24 fps strobing	Mouth motion feels smoother without tooth warping.
C1	New CosyVoice prosody variants + MuseTalk raw	Audio pacing is driving mechanical visemes	More human mouth motion without video model change.
D1	LatentSync 1.6	MuseTalk 256² is the ceiling	Better teeth/lip detail and more natural articulation than best MuseTalk.
E1	LatentSync 1.6 + RIFE 48 fps	Diffusion quality plus smoother presentation	Best offline candidate for prerecorded clips.
F1	Raw MuseTalk or LatentSync + FLAIR	Temporally consistent restoration beats GFPGAN	Sharpness improves without boil/plasticity.
F2	Raw MuseTalk or LatentSync + BasicVSR++/RealBasicVSR	Conservative temporal VSR beats face GAN	More detail, no identity hallucination.

Recommended repo/paper manifest

Create or update projects/papers/manifest.md with this structure:

P0_IMPLEMENT_NOW
- LatentSync 1.6 — arXiv + official GitHub + HF weights — Apache-2.0
- FLAIR — arXiv/WACV + official GitHub pretrained models — MIT
- RIFE — ECCV 2022 + official PyTorch repo pretrained HD models — MIT
- CosyVoice2/CosyVoice3 — official repo + papers + HF models — Apache-2.0
- MuseTalk 1.5 — arXiv + official repo/HF — MIT/commercially usable
- BasicVSR++ / RealBasicVSR — official repos — Apache-2.0
- GFPGAN — official repo pretrained models — Apache-2.0; use only alpha-blended/masked/EMA
P1_EVALUATE_FOR_PRERECORDED_OPENERS
- Hallo2 — official repo + weights — MIT
- Hallo3 — official repo/HF — code MIT, model license chain requires review
RESEARCH_ONLY_OR_BLOCKED
- KEEP — non-commercial
- PGTFormer — non-commercial
- BFVR-STC — promising, but license unclear / CodeFormer-derived
- CodeFormer — non-commercial
- StableVSR — repo MIT, dependency/model license chain needs review

My implementation recommendation

The fastest path to a better take is:

1. MuseTalk raw → GFPGAN alpha blend 0.45 → temporal EMA on delta → RIFE 48 fps.
2. In parallel, generate CosyVoice prosody variants and rerender the best audio.
3. Run LatentSync 1.6 as the first true offline-quality replacement.
4. Add FLAIR as the most promising permissive temporally consistent face-restoration paper.
5. Keep BasicVSR++/RealBasicVSR as the conservative temporal enhancement fallback.
6. Treat KEEP, PGTFormer, BFVR-STC, and CodeFormer as papers to learn from, not code to ship.
