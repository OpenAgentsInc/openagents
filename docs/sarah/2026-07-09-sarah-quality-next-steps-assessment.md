# Sarah Quality Assessment + Next Steps

Date: 2026-07-09
Status: assessment / recommended quality program
Scope: `docs/sarah/` current docs, Sarah monorepo consolidation state, OAV
research, and web-verified primary sources linked below.

This assessment does **not** supersede `SARAH_CONTRACTS.md`,
`SARAH_KNOWLEDGE_BASE.md`, or the OAV experiment authority in `research.md`.
It is the connective tissue: what quality means for Sarah now, what is most
broken, and which next steps should improve it fastest without weakening the
contracts that already make Sarah safe to ship.

## Executive Assessment

Sarah's behavioral and authority quality is ahead of her perceptual quality.
The monorepo cutover, typed contracts, Blueprint provenance, pricing guard,
prospect isolation, owner-gated learning, account-linking seam, and Khala
gateway plan are the right foundation. The highest visible quality gap is now
the avatar experience: the latest enhanced OAV take became sharper but less
human in motion, which means the pipeline is optimizing the wrong proxy when it
trusts stills or crop sharpness by itself.

The next quality push should therefore be a measured, receipt-backed media
program first, with conversation/runtime polish running in parallel. The
winning pattern is not "pick a better model" in isolation. It is:

1. Standardize the quality harness and owner playback scoreboard.
2. Fix audio prosody before over-reading video defects.
3. Ship a prerecorded opener library using the offline quality tier.
4. Harden the realtime owned renderer only after the offline winners are known.
5. Expand Sarah's sales/behavior evals without weakening enforced contracts.

## Current Quality Baseline

### Strengths

- **Authority is explicit.** Sarah has enforced contracts for prospect
  isolation, owner-gated collective learning, query-layer scoping, no
  improvised pricing, in-chat account linking, and Blueprint provenance.
- **Knowledge quality has a source of truth.** The checked-in KB is generated
  from a typed Blueprint with per-fact provenance, not an ad hoc prompt paste.
- **Runtime migration is disciplined.** Sarah now serves from
  `https://openagents.com/sarah`; the private Vercel path is historical; money,
  CRM, credits, checkout, and receipts stay owned by openagents.com APIs.
- **The owned avatar pipeline is plausible.** OAV-1 proved MuseTalk +
  CosyVoice2 over owned footage end-to-end, with identity exact outside the
  mouth crop and acceptable baseline sync.
- **The research triage is pointed at the real defect.** The docs correctly
  identify per-frame restoration and 24fps strobing as likely causes of the
  "higher def but choppy / less humanlike" owner verdict.

### Gaps

- **Perceptual QA is not yet a first-class artifact.** There are metrics,
  stills, crops, and owner verdicts, but Sarah needs one canonical scoreboard
  per take so agents stop optimizing isolated artifacts.
- **Audio naturalness is underweighted.** Mechanical letter-speech and clipped
  pacing can create mechanical visemes even when video generation is stable.
- **The opener experience is still generated live.** The owner explicitly wants
  perfect standard phrases. The fastest path to beauty is prerecorded,
  QA-passed clips for first contact and common transitions.
- **Realtime and offline quality are being discussed together.** They should
  share seams and measurements, but not the same model choices. LatentSync /
  FLAIR-class work belongs to offline or prerecorded tiers until proven fast.
- **Conversation evals are fixture-green, not yet sales-quality rich.** S-12
  covers critical safety/honesty fixtures; it does not yet score pain hunting,
  mirroring, momentum, concise voice turns, and non-pushy account/funding moves.
- **EN UI gaps still force local shell workarounds.** Media host, streaming
  transcript, mic/level state, and card primitives need Effect Native catalog
  support so Sarah's surface quality can rise without inventing a parallel UI.

## Quality Bar

Sarah quality should be judged on five axes, in this order:

1. **Trust correctness:** no cross-prospect leakage, no improvised pricing, no
   unreceipted tool authority, no ungrounded product claims.
2. **Human perceptual quality:** avatar motion, speech prosody, A/V sync, and
   identity feel beautiful in playback, not just in stills.
3. **Conversation quality:** Sarah leads, asks one sharp question at a time,
   mirrors pain, maps to one product, and ends every turn with momentum.
4. **Operational quality:** sessions start/stop cleanly, GPU cost is bounded,
   token usage is exact, and every consequential act has an artifact/receipt.
5. **Surface quality:** the `/sarah` page feels intentional: clear AI
   disclosure, calm mic state, transcript/caption clarity, and useful cards at
   the exact moment Sarah references them.

No quality improvement should trade off axis 1. Better media cannot weaken
pricing, memory, learning approval, or registry-bound claims.

## Recommended Next Steps

### P0 - Build the Sarah Quality Scoreboard

Create a small, boring, hard-to-game scoreboard for every media take:

- Input refs: source clip, script, TTS prompt/reference, model versions,
  render command, enhancement recipe, commit refs, and artifact URIs.
- Audio gates: STT round-trip, LUFS / true peak, pause timing, human prosody
  verdict, and "initialism risk" notes.
- Video gates: owner playback verdict, A/V sync at start/middle/end,
  crop sharpness, temporal boil/flicker, chunk-boundary jerk, identity drift,
  paste-back seam, and bad-frame exclusions.
- Operational gates: render wall time, GPU type/cost, artifact-existence
  monitor result, and whether the host was stopped or intentionally left up.

This should live as a machine-readable JSON/NDJSON artifact plus a short
Markdown summary beside each take. The key cultural change: **no take advances
because it looks good in stills**. Playback verdict plus temporal evidence wins.

### P0 - Run the Next Experiment Matrix to Completion

Use the existing `research.md` matrix, but tighten the exit conditions:

- **Audio first:** render CosyVoice variants before video rerenders:
  `AI`, `A.I.`, paused letter-speech, no-initialism rewrite, speed/emotion /
  instruct controls. Keep STT as a hard correctness gate, but choose by ear for
  naturalness.
- **Best MuseTalk realtime candidate:** raw MuseTalk, sharpen-only, tamed
  GFPGAN, tamed GFPGAN + EMA, then RIFE 48fps. Full-strength GFPGAN stays
  banned as a default.
- **Best offline candidate:** LatentSync 1.6, then fix its 16-frame
  chunk-boundary hitch before declaring it the winner.
- **Temporal restoration lane:** run FLAIR on stabilized face/mouth crops; run
  BasicVSR++ / RealBasicVSR as the conservative temporal VSR fallback.
- **Decision rule:** pick one realtime recipe and one offline/prerecorded
  recipe. Do not let the offline winner expand the realtime scope unless it
  fits the session frame budget.

### P0 - Produce the Perfect Opener Library

The first live seconds matter disproportionately. Build a tiny prerecorded
library before chasing full realtime perfection:

- "Hello! I'm Sarah. What's on your mind today?"
- One no-initialism OpenAgents positioning line.
- One pain-discovery follow-up.
- One acknowledgement / "tell me more" line.
- One pricing-deflection-to-human line.
- One close / next-step line.

Use the offline winner, not the realtime renderer. The scripts should avoid
initialisms entirely unless the selected TTS prosody makes them genuinely
natural. Integrate the library through the owned renderer and KHS-6 semantic
cache path so standard answers can play a QA-passed clip with zero live render.

### P1 - Harden the Owned Realtime Renderer

Keep the realtime target modest and explicit:

- Patch or wrap the MuseTalk placeholder-bbox crash so undetected-face frames
  fail closed or are excluded before render.
- Pin speaking-state frame ranges to eyes-open, detectable-face windows.
- Close the L4 20fps to 24fps gap through the optimized loop, TensorRT /
  compile work, or an honest 20fps output mode if that is the stable choice.
- Stream sentence/phrase TTS chunks instead of one long paragraph.
- Keep the LiveAvatar seam as a fallback until OAV-4 can prove owned renderer
  sessions with teardown, caps, receipts, and acceptable first-frame latency.

### P1 - Expand Behavioral Quality Evals

Add a Sarah "sales quality" eval pack alongside safety fixtures:

- Pain-hunting: first two Sarah turns ask concrete, single questions.
- Mirroring: Sarah restates the user's pain before pitching.
- One-product strike: she maps to one relevant product, not a catalog tour.
- Momentum: every answer ends with a useful question or call to action.
- Voice length: spoken replies stay short enough for a live avatar.
- Account/funding move: she can suggest linking/funding without being pushy.
- Human handoff: enterprise, legal, custom discount, and delivery commitments
  escalate with a concise brief.

These should reuse the existing deterministic guard/oracle style where
possible. LLM-judged rubric can be useful for sales quality, but it must never
replace the hard contract tests.

### P1 - Improve Learning Quality Without Diluting Isolation

The owner-approved learning queue is the right mechanism. Improve its quality
by adding review ergonomics:

- Candidate taxonomy: objection, winning answer, pain phrase, product mapping,
  bad-fit signal, follow-up phrasing.
- A "why this should generalize" field generated during distillation.
- Example-count and source-recency metadata after PII redaction.
- A regression fixture created whenever an approved learning changes Sarah's
  answer style materially.

Nothing from this path should reach shared serving without the existing approval
receipt. This keeps learning useful without turning memory into prompt soup.

### P1 - Close the EN Surface Gaps That Affect Sarah

Prioritize the Effect Native components that directly improve perceived
quality:

- `media-video` host kind for WebRTC / owned renderer attach targets.
- Streaming transcript primitive with partial-utterance updates.
- Mic state + audio level indicator.
- Handoff / checkout / receipt card primitives.
- First-contact AI disclosure banner.

Until these exist, Sarah can keep the zero-React DOM shell, but it should not
grow a separate design system.

### P2 - Operationalize Cost, Teardown, and Receipts

Media quality work is GPU-shaped and can quietly leak money. Add a standard
run closeout checklist:

- Artifact existence monitor, not log-marker monitor.
- Host state: stopped, deleted, or explicitly left running with a reason.
- GCS artifact index updated.
- Token/GPU cost estimate recorded.
- No raw prompts, secrets, private prospect data, or live credentials in
  artifacts.

This is not glamorous, but it is the difference between a research lane and a
production-quality lane.

## Research Basis

The local `research.md` triage remains the working experiment authority. The
external primary-source checks I would keep attached to this quality program:

- [MuseTalk](https://github.com/TMElyralab/MuseTalk): current realtime
  lip-sync baseline; efficient, MIT, but a 256x256 mouth-region ceiling and
  single-frame artifacts remain the practical risk.
- [LatentSync](https://github.com/bytedance/LatentSync): offline diffusion
  lip-sync tier; repo notes 18GB minimum VRAM for 1.6, which fits the L4.
- [FLAIR paper](https://arxiv.org/abs/2311.15445): directly targets
  temporally coherent face-video restoration, the failure mode exposed by
  per-frame GFPGAN.
- [CosyVoice](https://github.com/FunAudioLLM/CosyVoice) and
  [CosyVoice2 model card](https://huggingface.co/FunAudioLLM/CosyVoice2-0.5B):
  streaming TTS, text normalization, pronunciation control, and low-latency
  synthesis are central to natural visemes.
- [RIFE](https://github.com/hzwer/ECCV2022-RIFE): permissive, fast 2x frame
  interpolation candidate for the 24fps strobing hypothesis.
- [GFPGAN](https://github.com/TencentARC/GFPGAN): useful only as a tamed prior;
  its own model notes distinguish sharper identity/detail from more natural
  restoration, matching the owner-observed stills-vs-motion split.
- [BasicVSR++](https://github.com/ckkelvinchan/BasicVSR_PlusPlus) and
  [RealBasicVSR](https://github.com/ckkelvinchan/RealBasicVSR): permissive
  temporal video restoration fallbacks that are less face-hallucination-heavy
  than per-frame GAN restoration.

## My Read

The fastest route to "Sarah feels high quality" is not a heroic realtime
renderer rewrite. It is a careful front-of-house strategy:

1. Make the first five seconds beautiful with prerecorded, QA-passed openers.
2. Make every generated take compete on a scoreboard that privileges playback.
3. Let audio prosody and offline LatentSync/FLAIR work set the perceptual bar.
4. Keep MuseTalk/tamed enhancement as the realtime bridge until a better model
   proves it can meet the frame budget.
5. Keep expanding behavioral evals so the better face is attached to a better
   closer, not just a prettier renderer.

That gives Sarah a quality ladder instead of a single fragile bet: perfect
standard clips now, honest realtime improvement next, and stronger sales
behavior continuously, all without loosening the safety and authority contracts
that already make the system credible.
