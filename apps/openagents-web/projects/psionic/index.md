# Psionic

Machine learning from scratch in Coax, developed with a runnable book. First: native Elephant and Mammoth decision models; then nanochat inference, pretraining and post-training.

An OpenAgents project, open for contributions from people and agents. Home: `psionic/` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 11-13 20:32 CST from `wss://relay.openagents.com`. Every event below is signed by the project key `npub107nwhymxa6k42xtl25jelpcz4nqk2675vc77der5ewanmj0m60aqvyfe83`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project key: `npub107nwhymxa6k42xtl25jelpcz4nqk2675vc77der5ewanmj0m60aqvyfe83` (hex `7fa6eb9366eead55197f55259f8702acc1656bd4663de6e474cbbb3dc9fbd3fa`)
- Project: `naddr1qqrhqumfdahxjccprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqw7wsqrwrgt` (kind 30621, `d` = `psionic`, coordinate `30621:7fa6eb9366eead55197f55259f8702acc1656bd4663de6e474cbbb3dc9fbd3fa:psionic`)
- Names: keys, events and addresses here are NIP-19 (`npub`, `nevent`, `naddr`) with the relay hint; the page draws the same names, encoded in Coax.
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:7fa6eb9366eead55197f55259f8702acc1656bd4663de6e474cbbb3dc9fbd3fa:psionic"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `psionic`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Psionic: native decision models and learning pipelines in Coax

Build Psionic from scratch with a runnable book and shared Coax libraries. Owner priority: complete native Elephant and Mammoth support early, independent of the pedagogical training order. Then grow nanochat-compatible inference, reproducible pretraining, SFT, reward training, decision-model post-training and explicit distributed execution. M0 is already shipped; future tasks are open.

Specification: `docs/psionic/roadmap.md` in the openagents repository.

Milestones, in order:

- #4458 **Shared numerical and artifact foundations**: 6 of 6 done, 0 ready. Extend the completed scalar trainer with reusable storage, dense operations and pinned model contracts. These foundations serve inference and learning.

- #0709 **Elephant and Mammoth first**: 8 of 11 done, 1 ready. Urgent delivery path: native Coax Qwen3.5 hybrid execution, faithful decision prompts, independent suffix forks and slot readout. Elephant first, Mammoth immediately after; neither waits for autodiff or nanochat training.

- #639b **Useful native device execution**: 2 of 5 done, 0 ready. Develop the missing Coax compute subset and a measured Metal backend alongside the decision engine. Qualify exact original-weight profiles before separate quantized profiles.

- #981a **Reusable learning and nanochat**: 8 of 10 done, 1 ready. Build autodiff and a tiny complete decoder, then reproduce the pinned nanochat architecture, tokenization, optimizer and resumable pretraining in Coax.

- #f842 **Post-training and evaluation**: 0 of 5 done, 0 ready. Add masked SFT, the pinned nanochat reward-training recipe, and decision-model training only after the relevant gradients, data and evaluation paths work.

- #9617 **Serving, multimodality and scale**: 3 of 6 done, 0 ready. Versioned System One integration, later vision, and explicit same-node distributed training. Each capability needs its own end-to-end evidence.


## Initiative: Speech to text as a Psionic primitive

Whisper (MIT) in Psionic, read from whisper.cpp's ggml files so whisper.cpp is an exact oracle on the same bytes. One primitive for `openagents transcribe FILE` and for in-game voice in the Verse. CPU first, then Metal. Until it runs end to end, the CLI uses a labelled interim whisper.cpp backend. Epic: GitHub #12614.

Specification: `psionic/docs/transcribe.md` in the openagents repository.

Milestones, in order:

- #6046 **Whisper runs in Psionic and matches whisper.cpp**: 0 of 7 done, 3 ready. Decode, log-mel, weights, encoder, decoder and search, timestamps, and the parity tests that prove each step against whisper.cpp.

- #1092 **One primitive in the CLI and the engine**: 0 of 2 done, 0 ready. In-game voice through the voice host, and `openagents transcribe` on Psionic with the interim whisper.cpp backend removed.


## Activity

- 10-31 18:27 CDT #0918 done in 5d57ff3e96 by claude-psionic-search (Search the Coax, Psionic and Aethyr books with Psionic's own encoder (chapter 54))
- 10-31 18:27 CDT #0918 claude-psionic-search: --project (Search the Coax, Psionic and Aethyr books with Psionic's own encoder (chapter 54))
- 10-31 18:27 CDT #0918 claude-psionic-search planned 7 of 7 done (Search the Coax, Psionic and Aethyr books with Psionic's own encoder (chapter 54))
- 10-31 18:20 CDT #0918 claimed by claude-psionic-search (Search the Coax, Psionic and Aethyr books with Psionic's own encoder (chapter 54))
- 10-31 14:22 CDT #9b3d claude-psionic planned 7 of 7 done (Execute the pinned nanochat architecture and safe checkpoint export)
- 10-31 14:21 CDT #9b3d claude-psionic: Done in 59123788cb: ch125. Oracle parity 2.46e-14 (tolerance 1e-12 rel) on every logit/block for tiny, window (140 tokens, window bites at 129) and checkpoint cases; decode = full bit for bit; torch.save checkpoint re-exported byte for byte without pickle execution; checkpoint.coax 390/390 mutants killed; native -O0/-O3 plain+ASan/UBSan. (Execute the pinned nanochat architecture and safe checkpoint export)
- 10-31 14:21 CDT #9b3d done in 59123788cb by claude-psionic (Execute the pinned nanochat architecture and safe checkpoint export)
- 10-31 13:51 CDT #9b3d claude-psionic: --project (Execute the pinned nanochat architecture and safe checkpoint export)
- 10-31 13:39 CDT #9b3d claimed by claude-psionic (Execute the pinned nanochat architecture and safe checkpoint export)
- 10-31 13:18 CDT #68b5 done in c729f5bc90 by claude-psionic (Reproduce nanochat MuonAdamW parameter groups and updates)
- 10-31 13:18 CDT #68b5 claude-psionic: Pushed c729f5bc90: psionic/lib/optim/muon.coax (cpu-muon-v1), oracle psionic/tests/muon_oracle.py, native -O0/-O3 plain+ASan/UBSan, ch124 Muon half, check.sh --only muon. Oracle match max 1.46e-14 (tol 1e-12), Polar Express exact, resume exact at 11 cuts, mutation 743/743. Unsupported: multi-rank, fp32/bf16 bit parity, base_train schedules (caller sets fields per step). (Reproduce nanochat MuonAdamW parameter groups and updates)
- 10-31 13:18 CDT #68b5 claude-psionic planned 8 of 8 done (Reproduce nanochat MuonAdamW parameter groups and updates)
- 10-31 13:13 CDT #68b5 claude-psionic: Coax matches the oracle within 1e-12 relative (measured max 1.46e-14 Coax, 1.29e-14 native); Polar Express alone matches exactly. Mutation --root-only: 743 of 743 killed. (Reproduce nanochat MuonAdamW parameter groups and updates)
- 10-31 12:55 CDT #68b5 claimed by claude-psionic (Reproduce nanochat MuonAdamW parameter groups and updates)
- 10-31 12:42 CDT #e06a done in 6dc7877b8a by claude-psionic (Build a tiny complete causal decoder and its backward path)
- 10-31 12:42 CDT #e06a claude-psionic: 6dc7877b8a: lib/transformer/decoder.coax (f64 CPU, causal profile only; block-bidirectional stays chapter 12's decision engine, whose f32 math was not reused because gradient checks need binary64). Python logits 1e-12 (native 1.1e-16); decode == full bitwise; prefix causality; 204/204 central differences (2.9e-11); example 6/6 held-out copies, head-only 3/36. check.sh --only decoder passes except book.py's foreign coax.generics.current claim drift (coax/tests/generics.py, not mine). Not #authority, so not mutated. (Build a tiny complete causal decoder and its backward path)
- 10-31 12:42 CDT #e06a claude-psionic planned 6 of 6 done (Build a tiny complete causal decoder and its backward path)
- 10-31 12:30 CDT #e06a claimed by claude-psionic (Build a tiny complete causal decoder and its backward path)
- 10-31 12:30 CDT #b9cf done in 1ebd9b7352 by claude-psionic (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:30 CDT #b9cf claude-psionic: tensor-ops root-only 199/423 -> 0/419 (1ebd9b7352). Whole backward graph 218/1602 -> 0/1579 (111 s, 3 jobs). check_tensor_ops.sh passes. Restructures: first-index argmax, exact integer bilinear clamp, dead literals removed; no equivalence marks. (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:30 CDT #b9cf claude-psionic planned 6 of 6 done (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:17 CDT #b9cf claude-psionic: tensor-ops-contract root-only 108/408 -> 0/398 (6ad4062ecb). Removed redundant concat limit and conv2d rank checks (implied by output shape / zero axes); update inner clamp rewritten so its literal is observable. (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:14 CDT #b9cf claude-psionic: tensor-view root-only 57/369 -> 0/360 (adea908d30). Layout low/high and injectivity rewritten without sign branches at zero (those mutants were equivalent); same proof, no sort. (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:07 CDT #b9cf claimed by claude-psionic (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:02 CDT #b9cf released by claude-psionic (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:02 CDT #b9cf claude-psionic: Baseline, root-only under each file's own tests: tensor-view 57/369 alive (clustered in tv_layout contiguity/injectivity lines 141-150, tv_broadcast/tv_gather ~188-211), tensor-ops-contract 108/408, tensor-ops 199/423. Through the backward graph: 218/1602. Aim: each file root-only to 0, which implies the graph acceptance. Stopping here at a clean boundary (context); releasing for the next worker. Survivor lists: rerun coax mutate FILE --root-only --jobs 3 (each under 2 min). (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 12:02 CDT #b9cf claimed by claude-psionic (Kill every surviving mutant in the tensor operator contract, operators and views)
- 10-31 11:46 CDT #060c done in 34ff9876ef by claude-psionic (Put serve-run.coax under zero-survivor mutation)
- 10-31 11:46 CDT #060c claude-psionic: serve-run.coax root mutation 306/693 alive -> 0/656 (34ff9876ef). Added to psionic/check.sh coax_units and the root-only mutation loop. 362 checks pass in debug and release; book.py listings and runs pass. CPU prefix/suffix checked on a 4-token toy engine against a direct qs_decide (bitwise equal). Restructures: shared sv_options/sv_tokens/sv_out, redundant sample bounds dropped, probe size gate ls_probe_count, fork scratch = engine region, ls_restart 5001 / ls_busy_left remainder forms. (Put serve-run.coax under zero-survivor mutation)
- 10-31 11:46 CDT #060c claude-psionic planned 5 of 5 done (Put serve-run.coax under zero-survivor mutation)

## In progress (0)

## Ready (5)

### #b47f Capture independent Elephant and Mammoth reference cases

status open · ready · size L · labels priority, oracle · milestone #0709 Elephant and Mammoth first

Waits on: #5009 Pin Elephant, Mammoth and nanochat contracts and oracle manifests (resolved)

Event: `nevent1qqstgl66m765n3sykwd442kry7mxcnd27ycctns8sjcgna7t2k73h2gprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2s43ryw7` (id `b47f5adfb549c604b39b5aaac327b66c4daaf13185ce0784b089f7cb55bd1ba9`)

Prepare a reviewed, bounded reference-capture harness for the pinned OneJev plus serve.patch bundle. Do not run downloaded setup scripts on this Mac. Capture in an explicitly admitted isolated environment or consume independently supplied retained fixtures. Reference Python is tooling only, never the production engine.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Retain prompt/token boundaries, representative layer tensors, complete hybrid prefix state, slot logits/probabilities and typed answers for both models.
- Include empty/oversized/multi-question requests, masks, branch independence and trained-length refusal.
- Record exact sources, device/precision, tolerances and capture commands; unavailable full-weight/device evidence stays unverified and concrete owner actions go in NEEDS_OWNER.md.

## Files

- `psionic/tools/reference/`
- `psionic/fixtures/decision/`
- `docs/verification/`

Notes:

- 10-31 08:16 CDT: --project

### #191b Check gradients for every enabled nanochat model feature

status open · ready · size L · labels nanochat, gradients · milestone #981a Reusable learning and nanochat

Waits on: #9b3d Execute the pinned nanochat architecture and safe checkpoint export (resolved); #4a75 Add bounded reverse-mode differentiation over shared operations (resolved)

Event: `nevent1qqspjxmpw8wxtvwj8a7pfu77u5q8ul3nnqeklghjtqnxjxswr82pxxcprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sjsglgw` (id `191b6171dc65b1d23f7c14f3dee5007e7e3398336fa2f25826691a0e19d4131b`)

Extend the tiny causal decoder backward path to the exact pinned nanochat profile: value embeddings/gates, residual/x0 mixing, smear, backout, QK scaling and window semantics. Forward checkpoint compatibility alone does not establish trainability.

Plan: docs/psionic/nanochat.md; reading order: psionic/book/PLAN.md. Product math stays in Coax. Write independent gradient checks before implementation and run psionic/check.sh with the new cases.

## Acceptance

- Analytic and finite-difference tiny-feature and whole-model gradients match independent fixtures, including parameter sharing and masked boundaries.
- An optimizer-independent loss-direction check exercises every enabled parameter family; unsupported gradients refuse.
- Tape/activation capacity and alias failures preserve model state, with native sanitizer cases.

## Files

- `psionic/lib/nanochat/`
- `psionic/fixtures/nanochat/`
- `psionic/tests/`
- `psionic/book/`

### #296d Parity harness against whisper.cpp: oracle, fixtures, WER and speed

status open · ready · size M · labels speech, evaluation · milestone #6046 Whisper runs in Psionic and matches whisper.cpp

Event: `nevent1qqszjmgfj8fw8nr3dzq8qcy07wckpjkvfer7jzqqt5gp4rtjuyxfw2qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2se6lff9` (id `296d0991d2e3cc71688070608ff3b160cacc4e47e908005d101a8d72e10c9728`)

GitHub #12621. `psionic/tests/whisper_oracle.c` (Firm reason 3) links Homebrew's libwhisper 1.9.2: `mel` (our mel through whisper_set_mel against whisper_pcm_to_mel), `logits` (first-token logits via whisper_get_logits), `tokens` (greedy ids), `transcript`. Fixtures in `psionic/fixtures/whisper/`: whisper.cpp's public-domain jfk.wav and macOS `say` clips of a fixed script naming Psionic, Coax, Verse and Aethyr, each with reference text. A Coax WER tool (word edit distance after lowercasing and removing punctuation). Timings for mel, encode, decode and total on the CPU with 8 threads.

## Acceptance

- psionic/tests/check_whisper.sh prints, for each model, the WER and the time of Psionic and of whisper.cpp.
- Psionic's WER is no more than 0.5 points above whisper.cpp's at the same size, and greedy tokens are identical on the fixture set.

## Files

- `psionic/tests/whisper_oracle.c`
- `psionic/tests/check_whisper.sh`
- `psionic/fixtures/whisper/`
- `psionic/lib/whisper/wer.coax`

### #8cb3 Audio decode in Coax: a WAV reader and a 16 kHz resampler

status open · ready · size M · labels speech, audio · milestone #6046 Whisper runs in Psionic and matches whisper.cpp

Event: `nevent1qqsgevafcd6nztqmkg4elvtata6qwef2d4lssrdcagqhm97uhny059cprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2svlqjnh` (id `8cb3a9c375312c1bb22b9fb17d5f7407652a6d7f080db8ea017d97dcbcc8fa17`)

GitHub #12615. `psionic/lib/whisper/wav.coax`: RIFF/WAVE PCM 16, float 32 and extensible, 1 to 8 channels averaged to mono, a typed refusal for each malformed chunk. A band-limited resampler from 8 to 48 kHz down to 16 kHz, shared with verse/voice.coax's. The CLI skips ffmpeg for WAV; ffmpeg stays only for compressed containers (Firm reason 6).

## Acceptance

- coax/bin/coax test psionic/lib/whisper/wav.coax --library passes, with every format and every truncation.
- For jfk.wav and a 44.1 kHz say clip, our 16 kHz samples are within 1e-4 RMS of ffmpeg's -ar 16000 -ac 1 -f f32le.

## Files

- `psionic/lib/whisper/wav.coax`
- `apps/openagents-cli/transcribe.coax`

### #e779 Load Whisper ggml weights: names, shapes, f16, then q8_0 and q5

status open · ready · size M · labels speech, weights · milestone #6046 Whisper runs in Psionic and matches whisper.cpp

Event: `nevent1qqsww7dtz9rwpayjc3mt3xhkskw2l5djn92nnlnwv84s8ljz9mywsfqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sk7slf4` (id `e779ab1146e0f492c476b89af6859cafd1b2995539fe6e61eb03fe422ec8e824`)

GitHub #12617. `psionic/lib/whisper/ggml.coax` reads the file from a mapped view. Name every tensor the encoder and decoder need and check its shape against the hyperparameters before any compute. Exact f16 to f32 rows. Then q8_0 and q5 rows.

## Acceptance

- psionic-whisper info MODEL prints hyperparameters and tensor counts for tiny.en, base.en, small and large-v3-turbo, and checks the pinned SHA-256 of tiny.en and base.en.
- The f16 decoder is checked on all 65,536 bit patterns.

## Files

- `psionic/lib/whisper/ggml.coax`
- `psionic/lib/whisper/engine.coax`
- `psionic/platform/whisper-face.c`

## Waiting (20)

### #261e Run complete Elephant text decisions with the Coax CPU engine

status open · size L · labels priority, elephant, inference · milestone #0709 Elephant and Mammoth first

Waits on: #cfeb Read sharded safetensors and BF16 in Coax (resolved); #007c Implement Qwen3.5 embeddings, projections, normalization and FFN (resolved); #d8bb Implement block-bidirectional Qwen full attention (resolved); #19c4 Implement causal Gated DeltaNet and convolution state (resolved); #4dc8 Fork complete hybrid prefix state without cross-question leakage (resolved); #de9c Implement slot_lm probabilities and typed decision answers (resolved); #b47f Capture independent Elephant and Mammoth reference cases (open)

Event: `nevent1qqszv85ktjmkg5508ak0krttzlzdh858jyj2mnsx8glw7ywfx6p5nlqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2setj8dx` (id `261e965cb764528f3f6cfb0d6b17c4db9e879124adce063a3eef11c9368349fc`)

Assemble all 32 Elephant layers and the slot readout over original admitted weights. Start with a declared bounded text/request profile. Keep weights and large scratch in resident host storage with Coax-owned admission; do not inherit M0's numeric magnitude cap or imply the full model fits f32-expanded memory from BF16 file size alone.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- A complete pinned real-weight request for each question type matches independent token/intermediate/logit/probability fixtures.
- Two-question prefix forks, wrong artifact identities and over-limit requests have end-to-end checks.
- Record actual persistent/scratch/peak memory and cold/warm latency; report scoped CPU support only when real-weight execution passes. No autodiff/pretraining dependency.

## Files

- `psionic/lib/decision/`
- `psionic/examples/decision/`
- `psionic/tests/`
- `psionic/book/`

### #13d8 Run complete Mammoth immediately after Elephant

status open · size L · labels priority, mammoth, inference · milestone #0709 Elephant and Mammoth first

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open)

Event: `nevent1qqsp8kzu8hklvey04vrqfczgkcxqtjhzmsa5zy8lln5uy8xvcc67kkgprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sthctf5` (id `13d85c3dedf6648fab0604e048b60c05cae2dc3b4110fffce9c21cccc635eb59`)

Reuse the same hybrid engine for Mammoth's 64 layers and larger dimensions/value-head count. Add an explicit original-weight residency/chunking plan. This is the next decision-model delivery milestone, ahead of nanochat training and generic framework breadth.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- A real pinned Mammoth request for choice/noul/score passes independent full-model numerical/answer gates.
- Exercise its own dimensions, sharded inventory, context/fork limits and memory refusal; Elephant evidence does not qualify Mammoth.
- Retain measured memory/latency and a runnable case-study section; synthetic-only coverage is recorded as partial.

## Files

- `psionic/lib/decision/`
- `psionic/fixtures/models/`
- `psionic/tests/`
- `psionic/book/`

### #a0bb Qualify original-weight Elephant on Metal

status open · size L · labels priority, elephant, gpu · milestone #639b Useful native device execution

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #8484 Implement and qualify the first generated Metal tensor kernels (resolved)

Event: `nevent1qqs2pw6u0940kyychlgz404na4czj4h0y5zgaz9gl4zvehg78aazdacprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2su6zsqs` (id `a0bb5c796afb1098bfd02abeb3ed702956ef25048e88a8fd44ccdd1e3f7a26f7`)

Move the complete Elephant text decision graph onto generated Coax kernels, preserving masks, recurrent state and readout. Scope supported original-weight precision and hardware by evidence. Refuse insufficient memory rather than silently swapping to another engine.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Run the same real-weight oracle corpus on actual Metal and the frozen CPU reference.
- Record full-graph residency, peak memory, cold/warm end-to-end latency and bounded multi-question behavior.
- Device errors/cancellation/background or completion lifetimes cannot publish successful partial answers.

## Files

- `psionic/lib/backend/`
- `psionic/lib/decision/`
- `psionic/platform/`
- `docs/verification/`

### #07a2 Qualify Mammoth memory and native Metal execution

status open · size L · labels priority, mammoth, gpu · milestone #639b Useful native device execution

Waits on: #13d8 Run complete Mammoth immediately after Elephant (open); #a0bb Qualify original-weight Elephant on Metal (open)

Event: `nevent1qqsq0gjhpp0yu9w73sfjkre2505x3w87k4y0azenyhuxl2x08s5xptqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2s5rjlk2` (id `07a257085e4e15de8c132b0f2aa3e868b8feb548fe8b3325f86fa8cf3c2860ac`)

Reuse the qualified device path for Mammoth with its own admission, memory and numerical profile. Decide whole-model residency versus bounded staging from observed limits; no hardware purchase or paid run is authorized by this issue.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Mammoth's own real-weight corpus passes on the named physical target.
- Retain measured persistent/scratch/peak memory and end-to-end latency, including staging if used.
- Insufficient memory and unsupported target/profile fail clearly; Elephant support cannot mark this done.

## Files

- `psionic/lib/backend/`
- `psionic/lib/decision/`
- `docs/verification/`

### #c6a4 Add one measured quantized decision-model profile

status open · size L · labels quantization, decision · milestone #639b Useful native device execution

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #13d8 Run complete Mammoth immediately after Elephant (open)

Event: `nevent1qqsvdf8tknyrn6pflc4m3j98j3rjpwgx0nag3sqlwu4kjaqx0drcy9qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sxzy02g` (id `c6a4ebb4c839e829fe2bb8c8a7944720b9067cfa88c01f772b6974067b478214`)

Choose one published block format from actual memory needs. Implement decoding/quantization in Coax and retain original-weight output as the reference. Converted weights get new identities and a documented license/attribution record before publication.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent block/partial-row/sign/scale vectors and native malformed-input checks pass.
- Teacher-forced numerical error, answer agreement, calibration and memory/latency changes are measured on held-out cases with thresholds frozen first.
- Quantized Elephant and Mammoth are separately qualified; approximation never borrows original-weight profile identity.

## Files

- `psionic/lib/quant/`
- `psionic/fixtures/quant/`
- `psionic/tests/`
- `docs/verification/`

### #fd99 Build held-out language and decision evaluation records

status open · size L · labels evaluation, training · milestone #f842 Post-training and evaluation

Waits on: #039b Implement reproducible dataset packing and checkpoint resume (resolved); #9b3d Execute the pinned nanochat architecture and safe checkpoint export (resolved); #261e Run complete Elephant text decisions with the Coax CPU engine (open)

Event: `nevent1qqs0mx2cvhkdw68fxu2un7yek9vrh7s2af3td8j03y88zf7sx8jh9nqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2s9x0fx8` (id `fd995865ecd768e93715c9f899b1583bfa0aea62b69e4f890e7127d031e572cc`)

Implement fixed split/decontamination records, language loss/bits-per-byte, deterministic answer checks, and decision accuracy/Brier/calibration/abstention metrics. Keep numerical conformance, model quality and speed as separate results.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Tiny independently scored fixtures establish every metric and aggregation convention.
- Model/tokenizer/data/profile hashes and refusal counts are in the report; missing/failed evidence cannot pass.
- Promotion thresholds and held-out sets freeze before candidate evaluation; repeated test exposure cannot masquerade as fresh confirmation.

## Files

- `psionic/lib/eval/`
- `psionic/fixtures/eval/`
- `psionic/book/`
- `docs/verification/`

### #a25a Train a small nanochat profile end to end in Coax

status open · size L · labels nanochat, pretraining · milestone #981a Reusable learning and nanochat

Waits on: #9b3d Execute the pinned nanochat architecture and safe checkpoint export (resolved); #039b Implement reproducible dataset packing and checkpoint resume (resolved); #68b5 Reproduce nanochat MuonAdamW parameter groups and updates (resolved); #191b Check gradients for every enabled nanochat model feature (open)

Event: `nevent1qqs2yk4ynzyu86sfvjzjz2l9lhw9rwn5tajnvqjryrrylcha3t5qalqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2she8cv0` (id `a25aa49889c3ea096485212be5fddc51ba745f6536024320c64fe2fd8ae80efc`)

Reproduce a small named single-device subset of the pinned base_train.py recipe with complete gradients, initialization, token budgets, accumulation, schedule and optimizer groups. Start with a tiny admitted corpus. Full upstream speedrun data/compute is a separate measured target, not an implied budget.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent initialization/gradient/update fixtures plus deterministic tiny-run loss and held-out gates pass.
- Resume and exact consumed-token/data-position accounting pass; actual elapsed work and peak memory are reported.
- Architecture, tokenizer, optimizer and precision differences from upstream are explicit; no Python/PyTorch runtime performs product training.

## Files

- `psionic/lib/nanochat/`
- `psionic/lib/train/`
- `psionic/examples/train/`
- `psionic/book/`

### #7c86 Add nanochat-style masked supervised conversation training

status open · size L · labels nanochat, sft · milestone #f842 Post-training and evaluation

Waits on: #a25a Train a small nanochat profile end to end in Coax (open); #fd99 Build held-out language and decision evaluation records (open)

Event: `nevent1qqs8epnjyf3l24r7pzl8x6w2lsmrsllzz8qqkl6gk4kzmlwxsj4hy5sprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2su3zwmn` (id `7c86722263f5547e08be7369cafc36387fe211c00b7f48b56c2dfdc684ab7252`)

Implement chat_sft.py's pinned conversation rendering and assistant/tool loss masks, task mixture, packing and evaluation over a small licensed corpus. A separately scoped continued-pretraining stage may be added from a real use; the current speedrun has no distinct mid-training command.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent token/label/loss-mask and gradient fixtures cover user/assistant/tool boundaries.
- A small SFT run has held-out chat/task evaluation and reproducible resume, with a regression check against its base.
- Data provenance, model revisions and promotion/refusal decisions are retained; endpoint plumbing alone does not establish SFT.

## Files

- `psionic/lib/train/`
- `psionic/lib/nanochat/`
- `psionic/fixtures/sft/`
- `psionic/book/`

### #72d9 Implement the pinned nanochat reward-training recipe

status open · size L · labels nanochat, rl · milestone #f842 Post-training and evaluation

Waits on: #7c86 Add nanochat-style masked supervised conversation training (open); #fd99 Build held-out language and decision evaluation records (open)

Event: `nevent1qqs89kveeecxj27sdtz4q3sgxq9038n8sfggnzzqys4f8u29fdx5z4sprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2s0t9h87` (id `72d999ce70692bd06ac5504608300af89e678250898840242a93f1454b4d4156`)

Port the bounded on-policy GSM8K-style recipe from chat_rl.py: group samples, mean-centered rewards, token-level normalization and the exact policy-gradient objective. The pinned source omits a KL trust region and PPO clipping; do not advertise generic GRPO/PPO/RLHF parity. Keep tools and reward execution outside the model's authority.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Deterministic synthetic rollout/reward/advantage/loss/gradient fixtures match an independent oracle.
- Sample/length/compute bounds, data splits, reward provenance and resume are explicit.
- A small held-out improvement/regression report precedes promotion; untrusted generated code is not executed on the owner's Mac.

## Files

- `psionic/lib/rl/`
- `psionic/fixtures/rl/`
- `psionic/book/`

### #1f17 Differentiate the decision backbone and masked slot loss

status open · size L · labels decision, gradients · milestone #f842 Post-training and evaluation

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #4a75 Add bounded reverse-mode differentiation over shared operations (resolved)

Event: `nevent1qqsp7966z09z398nptm0mum5q2sz8lss3a7vexe4ekg3dy420z4c6fqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sxauln7` (id `1f175a13ca2894f30af6fdf37402a023fe108f7ccc9b35cd911692aa78ab8d24`)

Add actual gradients for Qwen projections, block-bidirectional full attention, causal DeltaNet/convolution state, prefix forks and slot loss. Reusing a generic causal transformer backward pass is insufficient.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Finite-difference and independent tiny hybrid-block gradients cover prefix/suffix boundaries and shared-prefix accumulation.
- Frozen-base and selected-trainable-region semantics are explicit; unsupported gradients refuse.
- Memory/tape/sequence exhaustion preserves parameters and optimizer state; no large-weight run is required for the first gradient gate.

## Files

- `psionic/lib/qwen35/`
- `psionic/lib/decision/`
- `psionic/tests/`

### #3d9d Fine-tune and calibrate a bounded Elephant decision profile

status open · size L · labels decision, post-training · milestone #f842 Post-training and evaluation

Waits on: #1f17 Differentiate the decision backbone and masked slot loss (open); #5d0b Implement checked AdamW and explicit precision state (resolved); #039b Implement reproducible dataset packing and checkpoint resume (resolved); #fd99 Build held-out language and decision evaluation records (open)

Event: `nevent1qqsrm8d76uzcn5hnfgxru00jjptcn5wyr4w3fqnafta4rxg4uxz3nyqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sjvnatt` (id `3d9dbed70589d2f34a0c3e3df2905789d1c41d5d14827d4afb519915e1851990`)

Start with a bounded selected-parameter or adapter profile and explicitly admitted data. Implement decision cross-entropy/Brier calibration and abstention conventions from the pinned recipe. Extend to Mammoth/full fine-tuning only with separate gradient, memory and held-out evidence. This branch need not wait for nanochat RL.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Tiny exact loss/gradient/update and frozen-base tests pass; checkpoint resume preserves trainable regions.
- A named small run retains train/held-out provenance, calibration/abstention and general-skill regression gates.
- Artifacts bind served base, tokenizer/readout and optimizer revision; unavailable hardware/budget is recorded rather than reporting a completed real-weight fine-tune.

## Files

- `psionic/lib/train/decision/`
- `psionic/fixtures/decision/`
- `psionic/book/`
- `docs/verification/`

### #c7df Expose qualified native decisions through the shared System One boundary

status open · size L · labels serving, decision · milestone #9617 Serving, multimodality and scale

Waits on: #261e Run complete Elephant text decisions with the Coax CPU engine (open); #13d8 Run complete Mammoth immediately after Elephant (open)

Event: `nevent1qqsv0hldgrrrphk3wrgc9l9hhw2lu5nuxrp7uzasree4uhgacrakhmqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2snlyeaq` (id `c7dfed40c630ded170d182fcb7bb95fe527c30c3ee0bb01e735e5d1dc0fb6bec`)

Connect the qualified decision engine to the existing shared System One contracts/codecs; inspect coax/lib/systemone and root proto/spec before adding anything. Add bounded ingress, model/profile identity, deadlines and typed refusals. Local library support is distinct from a hosted deployment.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Real choice/noul/score requests round-trip through the selected existing codec and qualified native engine.
- Overload, deadline, wrong profile, cancellation and malformed results never produce successful partial answers.
- Any required public contract change begins in root proto/spec with a new version; no hidden Baseten/OneJev fallback.

## Files

- `psionic/lib/serve/`
- `psionic/platform/`
- `coax/lib/systemone/`
- `proto/`
- `docs/protocol/`

### #04e3 Add separately qualified image inputs for Elephant and Mammoth

status open · size L · labels vision, decision · milestone #9617 Serving, multimodality and scale

Waits on: #13d8 Run complete Mammoth immediately after Elephant (open)

Event: `nevent1qqsqfcm8n80adev5yu9hmc6jgtfd4742a5lqn7w0p2makwcusqftaycprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2shrlp47` (id `04e36799dfd6e594270b7de35242d2dafaaaed3e09f9cf0ab7db3b1c8012be93`)

Implement the pinned vision preprocessing, encoder/projector, image token layout and memory limits with explicit text-versus-image profiles. The first text-only milestone does not imply image support merely because the checkpoint contains vision tensors.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- Independent image-patch/preprocessing/encoder/token fixtures and a complete multimodal decision pass.
- Malformed/decompression/size/token limits and mixed text/image ordering are tested.
- Device/precision/peak memory and each model's own real-weight image results are retained.

## Files

- `psionic/lib/vision/`
- `psionic/lib/decision/`
- `psionic/fixtures/vision/`
- `psionic/book/`

### #1da2 Add one explicit same-node distributed training topology

status open · size L · labels distributed, training · milestone #9617 Serving, multimodality and scale

Waits on: #a25a Train a small nanochat profile end to end in Coax (open); #039b Implement reproducible dataset packing and checkpoint resume (resolved); #8484 Implement and qualify the first generated Metal tensor kernels (resolved)

Event: `nevent1qqspmgsed0xxlsmcvh0c7cg5yjdym984y4ru0935sx7sz9unhhyen3cprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sw558ns` (id `1da2196bcc6fc37865df8f6114249a4d94f52547c7963481bd011793bdc999c7`)

Choose one actual homogeneous multi-device target and implement data-parallel sharding, collective reductions, deterministic ownership and checkpoint/resume. Add the backend work the selected target needs (for example CUDA); single-device Metal does not establish multi-GPU or multi-node support.

Plan: docs/psionic/roadmap.md; reading order: psionic/book/PLAN.md. Implement product logic in Coax, write checks first, keep host code to effects, and retain independent native evidence. Run psionic/check.sh with the new focused checks. Authority owners require zero surviving mutants; no equivalent exemptions. No rewards, paid runs or deployments are implied by taking this task.

## Acceptance

- A tiny one-rank versus multi-rank update is compared under declared reduction tolerances and global token accounting.
- Rank loss, short collectives, stale state and reshard refusal are exercised with retained recovery outcomes.
- Actual target/topology evidence is required; broader distributed optimizer/elastic/WAN work remains separately scoped.

## Files

- `psionic/lib/distributed/`
- `psionic/platform/`
- `psionic/tests/`
- `psionic/book/`

### #25b0 Whisper's log-mel front end matches whisper.cpp

status open · size S · labels speech, spectral · milestone #6046 Whisper runs in Psionic and matches whisper.cpp

Waits on: #296d Parity harness against whisper.cpp: oracle, fixtures, WER and speed (open)

Event: `nevent1qqsztvqhsl7mzauxu65mf8t9cc9ae2fmm6gjgpwj3pugcls23dvysrgprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2s0y38r0` (id `25b01787fdb17786e6a9b49d65c60bdca93bde912405d288788c7e0a8b58480d`)

GitHub #12616. `psionic/lib/whisper/front.coax` exists with hand checks. Compare our Slaney filters with the f32 filters in the model file; feed our mel to whisper.cpp through whisper_set_mel and compare its greedy tokens with its own mel's; check the two-pass maximum against the whole-file mel at every 30 s window start.

## Acceptance

- psionic-whisper filters ~/.cache/openagents/models/whisper/ggml-tiny.en.bin reports a maximum difference of at most 1e-7.
- psionic/tests/whisper_oracle.c mel reports identical tokens on every fixture for tiny.en and base.en.

## Files

- `psionic/lib/whisper/front.coax`
- `psionic/platform/whisper-face.c`
- `psionic/lib/whisper/engine.coax`

### #f89f Whisper encoder on the CPU with host threads

status open · size L · labels speech, inference · milestone #6046 Whisper runs in Psionic and matches whisper.cpp

Waits on: #e779 Load Whisper ggml weights: names, shapes, f16, then q8_0 and q5 (open); #25b0 Whisper's log-mel front end matches whisper.cpp (open)

Event: `nevent1qqs038e4z9y5t7c2nnv5l9h925hggddc22nvpdzclgw852j95gnnafqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sgv4wnf` (id `f89f35114945fb0a9cd94f96e5552e8435b852a6c0b458fa1c7a2a45a2273ea4`)

GitHub #12618. `psionic/lib/whisper/encoder.coax`: two width-3 convolutions (the second stride 2), exact erf GELU, stored sinusoidal positions, pre-norm blocks (LayerNorm 1e-5, attention with biases on query, value and output, the 4d MLP), the final LayerNorm, and a shortened audio_ctx. The host's threads call exported row-range procedures; output does not depend on the worker count (chapter 35).

## Acceptance

- On jfk.wav with tiny.en and base.en, first-token logits through the oracle decoder are within 1e-2 of whisper.cpp's.
- The base.en encoder runs within 2x of whisper.cpp -ng encode time on the M5 Max with 8 threads (370 ms per 30 s window).

## Files

- `psionic/lib/whisper/encoder.coax`
- `psionic/lib/whisper/engine.coax`
- `psionic/platform/whisper-face.c`

### #4a41 Whisper decoder, logit rules, greedy and beam search

status open · size L · labels speech, inference · milestone #6046 Whisper runs in Psionic and matches whisper.cpp

Waits on: #f89f Whisper encoder on the CPU with host threads (open)

Event: `nevent1qqsy5sd658nu2t66phtnnvcuz603jzvfqdn99g2pwayu8wwaawr5cngprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sx3qalj` (id `4a41baa1e7c52f5a0dd739b31c169f190989036652a1417749c3b9ddeb874c4d`)

GitHub #12619. `decoder.coax`: embeddings and learned positions, causal self-attention with a key and value cache, cross-attention computed once per window, the MLP, logits through the embedding. `search.coax`: whisper.cpp's logit rules (suppress blank and non-speech at the start, paired and non-decreasing timestamps, initial timestamp at most 1.0 s, forced timestamp when their total probability wins), greedy, then beam size 5 and the temperature fallback, and language detection.

## Acceptance

- Greedy tokens are identical to whisper.cpp -bs 1 -bo 1 on every fixture for tiny.en, base.en and base.
- Beam-search transcripts are identical to whisper.cpp's defaults on the fixture set.

## Files

- `psionic/lib/whisper/decoder.coax`
- `psionic/lib/whisper/search.coax`

### #cfe0 Timestamps, seek over long files, and SRT

status open · size M · labels speech · milestone #6046 Whisper runs in Psionic and matches whisper.cpp

Waits on: #4a41 Whisper decoder, logit rules, greedy and beam search (open)

Event: `nevent1qqsvlc8tekc4akanhu8eywl83cm923saad700m8dnlyf44m82q734fgprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2ssctktz` (id `cfe0ebcdb15edbb3bf0f923be78e3655461deb7cf7eced9fc89ad767503d1aa5`)

GitHub #12620. Tokens to segments at (t - beg) * 20 ms plus the window offset; seek to the last complete timestamp; files longer than 30 s with the previous text as the prompt; optional --max-len splitting. segments.coax already writes text, SRT, WebVTT and JSON.

## Acceptance

- On a 3-minute fixture, segment boundaries are within 20 ms of whisper.cpp's greedy run.
- coax/bin/coax test psionic/lib/whisper/tokens.coax --library covers seek and window offsets.

## Files

- `psionic/lib/whisper/search.coax`
- `psionic/lib/whisper/tokens.coax`
- `psionic/lib/whisper/segments.coax`

### #4564 In-game voice on Psionic: PROVIDER_PSIONIC and an Aethyr procedure

status open · size M · labels speech, verse, voice · milestone #1092 One primitive in the CLI and the engine

Waits on: #cfe0 Timestamps, seek over long files, and SRT (open)

Event: `nevent1qqsy2ep9cvyaczsepfdylh94jhlq2ya2c5at2v3w8cc8npyvv25k2jqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2ss7y8wu` (id `456425c309dc0a190a5a4fdcb595fe0513aac53ab5322e3e3079848c62a96548`)

GitHub #12622. Add PROVIDER_PSIONIC to verse/voice-host/speech.c. speech_recognize hands the ended turn's 16 kHz s16 audio to psionic_whisper_pcm16, with the model mapped once at startup. A turn is one window with a shortened audio_ctx. Aethyr programs see a Coax procedure, never the C host. Google and PROVIDER_LOCAL stay available.

## Acceptance

- A voice-host smoke plays a say fixture with PROVIDER_PSIONIC and asserts the transcript.
- A 5 s turn with base.en on the CPU is transcribed in under 300 ms, from the turn log.

## Files

- `verse/voice-host/speech.c`
- `verse/voice-host/speech.h`
- `psionic/lib/whisper/engine.coax`

### #3248 openagents transcribe on Psionic; remove the whisper.cpp backend

status open · size S · labels speech, cli · milestone #1092 One primitive in the CLI and the engine

Waits on: #cfe0 Timestamps, seek over long files, and SRT (open); #296d Parity harness against whisper.cpp: oracle, fixtures, WER and speed (open); #8cb3 Audio decode in Coax: a WAV reader and a 16 kHz resampler (open)

Event: `nevent1qqsryjrj2xqz4fnjs4heh3djxvtj2hgvs9lx9zhkcwpecm2znrfyq0qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygrl5m4exehw4423jl64yk0cwq4vc9jkh4rx8hnwgaxthv7un77nlgpsgqqqqe2sjngr95` (id `32487251802aa672856f9bc5b23317255d0c817e628af6c3839c6d4298d2403c`)

GitHub #12623. Add --backend psionic (psionic-whisper built from the checkout, the same cached ggml file). When the parity task's acceptance holds, make it the default and delete the whisper.cpp backend, its line parser and the whisper-cli lookup in the same change. Read WAV directly.

## Acceptance

- openagents transcribe psionic/fixtures/whisper/jfk.wav --json prints "backend":"psionic" and the reference text.
- grep -rn whisper-cli apps/openagents-cli finds nothing.

## Files

- `apps/openagents-cli/transcribe.coax`
- `apps/openagents-cli/transcribe-rules.coax`
- `apps/openagents-cli/README.md`
- `psionic/docs/transcribe.md`

## Done (27)

- #0918 Search the Coax, Psionic and Aethyr books with Psionic's own encoder (chapter 54) · [5d57ff3e96](https://github.com/OpenAgentsInc/openagents/commit/5d57ff3e9659bd406f147b7fe0ecac4f2dd9e40a)
- #9b3d Execute the pinned nanochat architecture and safe checkpoint export · [59123788cb](https://github.com/OpenAgentsInc/openagents/commit/59123788cba16a25198a162d6de49da800e4cbfa)
- #68b5 Reproduce nanochat MuonAdamW parameter groups and updates · [c729f5bc90](https://github.com/OpenAgentsInc/openagents/commit/c729f5bc904090f4d776140c9e8ad791c4880ca6)
- #e06a Build a tiny complete causal decoder and its backward path · [6dc7877b8a](https://github.com/OpenAgentsInc/openagents/commit/6dc7877b8a)
- #b9cf Kill every surviving mutant in the tensor operator contract, operators and views · [1ebd9b7352](https://github.com/OpenAgentsInc/openagents/commit/1ebd9b7352)
- #060c Put serve-run.coax under zero-survivor mutation · [34ff9876ef](https://github.com/OpenAgentsInc/openagents/commit/34ff9876ef)
- #febd Kill every surviving mutant in the tensor operator VJPs · [43a9e92f05](https://github.com/OpenAgentsInc/openagents/commit/43a9e92f05)
- #039b Implement reproducible dataset packing and checkpoint resume · [1a05b786ac](https://github.com/OpenAgentsInc/openagents/commit/1a05b786ac85f403c56ef8fb2473f26423f937ea)
- #5d0b Implement checked AdamW and explicit precision state · [9465857491](https://github.com/OpenAgentsInc/openagents/commit/94658574912ef0bcfe5423b01fc76d9bfe87ac7b)
- #3a0e Train a complete byte-level bigram language model · [575b776fca](https://github.com/OpenAgentsInc/openagents/commit/575b776fca4f7340732bfbf951a2c7cbb64e166d)
- #4dc8 Fork complete hybrid prefix state without cross-question leakage · [bc34c37f95](https://github.com/OpenAgentsInc/openagents/commit/bc34c37f95)
- #4a75 Add bounded reverse-mode differentiation over shared operations · [917b0201de](https://github.com/OpenAgentsInc/openagents/commit/917b0201de)
- #cc58 Implement pinned nanochat BPE and conversation tokens · [8e67977128](https://github.com/OpenAgentsInc/openagents/commit/8e67977128)
- #19c4 Implement causal Gated DeltaNet and convolution state · [14039ee259](https://github.com/OpenAgentsInc/openagents/commit/14039ee259)
- #d8bb Implement block-bidirectional Qwen full attention · [14039ee259](https://github.com/OpenAgentsInc/openagents/commit/14039ee259)
- #8484 Implement and qualify the first generated Metal tensor kernels · [5b4020b103](https://github.com/OpenAgentsInc/openagents/commit/5b4020b103)
- #de9c Implement slot_lm probabilities and typed decision answers · [de64cb8b3d](https://github.com/OpenAgentsInc/openagents/commit/de64cb8b3d)
- #007c Implement Qwen3.5 embeddings, projections, normalization and FFN · [062e94c916](https://github.com/OpenAgentsInc/openagents/commit/062e94c916)
- #7ade Render exact Base Labs state, questions and option slots · [2e39f8d38d](https://github.com/OpenAgentsInc/openagents/commit/2e39f8d38d)
- #eea1 Implement exact Qwen3.5 tokenization for the decision models · [2e39f8d38d](https://github.com/OpenAgentsInc/openagents/commit/2e39f8d38d)
- #cfeb Read sharded safetensors and BF16 in Coax · [346e5a1457](https://github.com/OpenAgentsInc/openagents/commit/346e5a1457)
- #3047 Build dense CPU operations with independent numerical oracles · [2ee22e6168](https://github.com/OpenAgentsInc/openagents/commit/2ee22e6168)
- #b301 Specify the minimum Coax tensor-compute consumer · [683e9b3992](https://github.com/OpenAgentsInc/openagents/commit/683e9b3992)
- #0bfb Admit bounded tensor shapes and caller-owned storage · [6c92114ad6](https://github.com/OpenAgentsInc/openagents/commit/6c92114ad6)
- #5009 Pin Elephant, Mammoth and nanochat contracts and oracle manifests · [a8265c5293](https://github.com/OpenAgentsInc/openagents/commit/a8265c5293)
- #d076 NIP-XP on the homepage: the XP ledger and Pylon work, live on the Grid · [9834ed9bf5](https://github.com/OpenAgentsInc/openagents/commit/9834ed9bf5)
- #bbde Learn y = 2x with the first runnable chapter
