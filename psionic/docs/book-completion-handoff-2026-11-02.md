# Psionic book: completion run handoff (2026-11-01 to 2026-11-02)

One session, with at most two subagents at a time, worked through the
Psionic book's remaining chapters. Each subagent worked in its own git
worktree and pushed one chapter to `main`. This note records where the book
stands, how the work was done, and what is still open.

## Where the book stands

From `psionic/book/chapters.json` at `1b2b68b65c`:

| Status | Chapters |
| --- | --- |
| implemented | 130 |
| partly implemented | 106 |
| moved to the Coder book | 17 (200–214, 251, 252) |
| design draft | 2 (254, 255) |

At the start there were 132 design drafts and 2 partly implemented chapters.
Every one of those drafts now runs code. Chapters 254 and 255, and the
implemented 253, were added by another agent during the run
(`dbe713baba`). They were not part of this work.

Each partly implemented chapter keeps a status marker on line 3 and a "What
this does not do yet" section listing what is missing. The README's status
cell, `chapters.json`, `SUMMARY.md` and `PLAN.md` all agree with that marker,
and `python3 psionic/tests/book.py` checks that they do.

## How each chapter was done

Every subagent followed the same brief:

- **Code:** Coax first. Python is tooling and oracles only; C is used only for
  host effects such as threads, timing, file mapping and CUDA launches.
- **Before promotion:** a hand-worked calculation or trace in the prose; a
  runnable example in `psionic/examples/` with a transcript that is identical
  in debug and release; a deliberately broken input with its exact refusal;
  and an independent check against an oracle, a reference, or brute force.
- **Mutation:** every changed unit reaches zero mutation survivors under
  `coax mutate --root-only`, with no mutant marked equivalent. Long runs are
  sharded.
- **Honesty:** acceptance thresholds are frozen before the measurement. A
  negative or unverifiable result is reported as found (for example chapters
  56, 102, 108, 136, 137, 141 and 175). Nothing a chapter cannot run is
  claimed.
- **Weights:** only checkpoints of 5 GB or less were downloaded, each recorded
  in a MANIFEST. Larger models were checked from their safetensors headers by
  HTTP range request. Pickles were never unpickled, and downloaded code was
  never run.

## Infrastructure notes for the next agent

- **Compiler:** every check used the cached compiler at
  `~/.cache/coax-release/8925679b…/coax`, linked into each worktree as
  `coax/bin/coax`. A fresh `coax/build.sh` per worktree takes about an hour
  and was never run. Results should be re-confirmed once on a fresh build
  before release, as the CI-free equivalent.
- **Compiler regressions:**
  - Commit `629a66626e` (#12611) made `psionic/examples/suite-digests.coax`
    fail with E0381, and that stopped `check.sh`'s run of every book example.
    The newest cached build (`4b6b2e1d…`) runs it again, so this looks fixed
    upstream.
  - E0400 on files that load `coax/lib/crypto/schnorr_sign.coax` was seen
    with the 8925679b build. Chapter 188 used a MAC instead of Schnorr
    invitations because of it. Recheck it on a current build.
- **Compile-time cost:** a program that loads an `#authority` unit runs all
  of that unit's checks while it compiles. Some examples are slow because of
  this and carry `"timeout"` in `examples.json`:
  - about 180 s: 118, 128, 136, 137, 139;
  - 600 s: 197 and 204.

  Splitting heavy checks into `psionic/tests/*-checks.coax` files helped where
  it was done.
- **Disk:** the disk filled once, at 116 MB free, because finished worktrees
  had piled up. The fix was to remove each worktree after verifying its HEAD
  is in `origin/main` and it has no tracked changes. About 43 GB is free now.
  Psionic model files live in `~/models/`:
  - Parakeet;
  - Ministral-3-3B (b6d637be);
  - Minitron-4B (5205ef7d);
  - Nemotron-3-Nano-4B (dfaf35de);
  - C-RADIOv3-B.
- **The chapter 250 drift check:** chapter 250's capability matrix is
  generated. If you change any chapter's status, profile or targets, or a
  transcript line cited in `psionic/fixtures/pytorch/workflows.json`, run
  `python3 psionic/tests/pytorch_matrix.py --write`, then
  `python3 psionic/tests/book.py`.
- **The tools rule:** every new `.py`, `.sh` or `.c` tool file needs a
  `Firm reason: N (...)` line in its first 60 lines. Check with
  `coax/web/tool.sh tools-check`.
- **Capability claims:** when an evidence file changes, its sha256 in
  `coax/book/capability-claims.json` must be refreshed after reviewing the
  claim. This run refreshed it twice for other agents' commits: `1c455ab6e1`
  (WGSL) and `e0b78eeac5` (native-gpu.md).
- **Watcher loops:** `until ! pgrep -f "PATTERN"` matches its own command
  line and never exits. Several such loops had to be killed. Use a sentinel
  file or wait on the PID instead.

## Fixes made to shared files outside chapter work

- **`0916ce730d`:** broke dependency cycles in `chapters.json` (48↔50,
  86↔199, 99↔102, 177↔178, 200↔203, 245↔246). The 115↔119 cycle was broken
  in `c2f6473b92`.
- **`ec76562303` and `fe2b4f566a`:** Coder chapter 16 used `sh` fences, and a
  stdio tool sat under `psionic/examples/`. The tool moved to
  `psionic/tools/protocols-stdio.coax`.
- **`8762b35a74`:** added the missing `Firm reason` line to chapter 101's
  check script.

## Open items, by kind

### Owner actions (also in `NEEDS_OWNER.md`)

- **34:** approve a packaged CUDA build of llama.cpp for the same-file
  comparison.
- **59:** put the browser search on openagents.com.
- **74:** run gpt-oss-120b on the 128 GiB Mac, and decide whether the 120b
  weights should live on this host. The disk can't hold them now.
- **177, 179, 181, 188–197:** hosts, budgets and real peers for live
  multi-replica and mesh runs.
- **196:** joining a Mesh-LLM mesh, and paying or accepting payment there.
- **238:** payout caps; executing intents through Breez Spark.
- **100, 101, 197:** DeepSeek V4.1 Flash. The real model needs about 501 GB of
  disk, and redistributing stage packages is a licence question.
- **198, 199:** Mistral Large 4 serving needs Mistral's commercial licence.

### Needs a compiler change in `coax cuda`

- **48:** the `dp4a` and int8 tensor-core (IMMA) instructions. Until then the
  int8 device kernels are exact but slower than f32 and BF16.
- **68 and 69:** shuffle instructions, FP16 matrix primitives, and tensor
  cores inside group kernels (the compiler refuses these with E0700
  reason 34). This also caps chapter 42's chunk size at 16, because of the
  32 KiB shared-array limit.

### Needs weights or hardware that this host lacks

Big checkpoints:

- **85:** Mistral Small 4, 121 GB.
- **86, 87, 198, 199:** Mistral Large 4, 1 TB.
- **164:** Nemotron Lightning, 66 GB.
- **166:** the 28-layer transformer of Cosmos3-Edge.

Other missing pieces:

- **160:** DoMINO ships only as pickles.
- **161:** the FourCastNet3 checkpoint.
- **149:** MotionBricks' root predictor, and the real weights through the
  decoder.
- **83:** Devstral, 26 GB, which can't run usefully here.
- **47, 72, 74:** Metal parity.

### Engineering this host could do next

These were measured or left open in the chapters:

- **41:** wire `td_attend` into gpt-oss serving. It is 8–23× faster than the
  existing attention kernel at 512–4096 tokens.
- **42:** let chapter 44's decision program use the chunked DeltaNet kernel
  for long prompts, chosen by prompt length.
- **175:** batched decode. It beats concurrency 1 by 1.5×, but is still slower
  than serving requests one after another on chapter 70's path. It needs a
  multi-row decode kernel and a recorded step without the host wait.
- **76 and 102:** speculation stays off. The n-gram drafter's acceptance is
  too low, so a better drafter is needed.
- **129:** split a row's work across threads; the GPU training step is slower
  than the CPU's.
- **153:** NeMo-recorded tensor parity, and multilingual test sets. Real
  English transcription works (0 errors on 4 LibriSpeech clips).
- **81, 58, 163:** independent references exist now. Remaining: positions past
  64 on Ministral, wall-clock speed, and Nemotron's non-ASCII tokenizer.
- **53:** masks over the full 131,072-token Tekken vocabulary.
- **51:** llama.cpp's own quantizers, and cards on a named model's original
  weights.
- **142:** chapter 5's operator table and manifest entries for convolution and
  patches.
- **55 and 56:** more labelled held-out queries. 36 queries can't decide the
  reranker's gain.
- **57:** a better way to pick the answer sentence (citation precision is 5 of
  19), checked on new held-out questions.
- Every remaining "real trained model" item: 140, 141, 145, 148, 155, 159,
  165, 222, 223, 245, 246.

## Commits

Each chapter's commit message starts "Psionic book:". Use
`git log --oneline --grep "Psionic book" --since 2026-11-01` to list them.
The subagent for each chapter recorded its checks, mutation counts and
reusable API in that commit's chapter text, and the chapter's "How it is
checked" section lists the exact commands.
