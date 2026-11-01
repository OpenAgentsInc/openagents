# Extending the frozen scalar-LQR experiment

The original chapter 171 experiment completed 128 batches and 512 AdamW
updates per seed. All three seeds improved from K=.25 to approximately .52,
but missed both the .05 gain-error and 1% nominal cost-gap goals. Those
[original results](../../fixtures/robotics/locomotion-lqr/README.md), their
source, raw receipts, curves and dated assessment remain unchanged.

The recorded trajectories were still improving at the fixed cutoff. Every
recorded update increased K; the last 16 batches added .022807, .022985 and
.012635 for seeds 0, 1 and 7. Recent gradients still favored increasing K.
No PPO clipping occurred, and gain projections were confined to the early
part of training. This suggested a finite optimization-budget limitation;
it did not prove that additional training would pass.

The owner authorized a longer experiment with the same seeds and optimizer
settings, followed by a chapter update and push, with no subsequent chapter.

## Prospective design

Continue each exact saved batch-128 checkpoint to **512 total batches and
2,048 total AdamW updates**. The additional 384 batches preserve the actual
learned gain, both optimizer moments, optimizer step, skipped count and all
16 world states. Nothing is restarted and the original prefix is not rerun.

The learner continues calling the original rollout and epoch functions.
Learning rate .001, betas (.9,.999), epsilon 1e-8, zero decay, four full-batch
epochs, fixed variance, analytic nominal critic, GAE and gain projection are
unchanged. The nominal optimum remains K*=.603732134399152. Acceptance still
requires every prescribed seed to meet both original quality goals.

The original counter allocation reserved exactly 8,192 draws per world and
was exhausted at batch 128. Extending those counters directly would reuse
other worlds' past draws. The new profile explicitly reserves fresh ranges:

| Item | Allocation |
| --- | --- |
| Original consumed counters, per seed | [0, 131072) |
| Extension world w, consumed counters | [131072+w*24576, 131072+(w+1)*24576) |
| Next counter at total batch b | 131072+w*24576+(b-128)*64 |
| Final endpoint | 524288; never consumed |

Seed keys remain 0, 1 and 7. Import changes only the sixteen counter words
in each state; every other byte must match the original independently
audited checkpoint. This is a declared continuation profile with a larger
stream reservation, rather than an in-place change to the original format.
New wire magics and a new descriptor identity keep the two profiles distinct.

The additive Coax authority root owns the import, state bounds and extended
batch admission. The four-epoch arithmetic segment is byte-identical to the
original function and calls the same learner procedures. The original
learner and scoring sources are retained unchanged. The scorer remains
isolated from training and cannot select an update, seed or stopping point.

## Work and verification

There are exactly 81 prescribed actions: three imports, 72 fixed chunks of
16 new batches, three one-batch checkpoint replays and three final evaluations.
An independent audit must pass before each next action. The next-batch
replay begins after 256 total batches. The original baseline evaluation is
carried, and each final mean policy executes six 128-step held-out trajectories.

| Work | New execution | Carried history |
| --- | ---: | ---: |
| Canonical plant steps | 589,824 | 196,608 |
| Canonical AdamW updates | 4,608 | 1,536 |
| Canonical tape forward/backward pairs | 36,864 | 12,288 |
| Canonical sample-gradient evaluations | 2,359,296 | 786,432 |
| Declared checkpoint-replay steps | 1,536 | Original replay remains recorded |
| Final deterministic evaluation steps | 2,304 | Original baseline and finals remain recorded |

Imports have zero plant, RNG, optimizer or tape effects. Source-VM synthetic
wiring and native guard observations are separate from these workload counts.
The coefficient sidecar is a recomputed source-bound observation, with one
probe per sample-gradient evaluation; it is not internal tape instrumentation.

New checks cover only the continuation code and metadata. The new authority
root closes 86 actual emitted sites with zero survivors. The first run's
inclusive-boundary survivor is retained; an exact ±16 state check closes it
without changing production arithmetic. Existing source, optimizer, RNG,
tape, GAE/Gaussian and mathematical native proofs are bound and carried.
Independent numerical audits apply those frozen oracles to the newly
captured samples. Old passing gates and expected-output generation are not
repeated.

## Measured outcome

All 81 prescribed actions completed and passed independent audits. Each seed reached 512 total batches and 2,048 AdamW updates. **All three meet both unchanged quality goals**, including the independently evaluated real-arithmetic margins. The original 128-batch misses remain retained in the original fixture.

| Seed | Original gain at 128 batches | Final gain at 512 batches | Final absolute gain error | Worst final nominal cost gap |
| --- | ---: | ---: | ---: | ---: |
| 0 | 0.5184256094 | 0.6045622741 | 0.0008301397 | 0.0001218323% |
| 1 | 0.5227558679 | 0.6219745781 | 0.0182424437 | 0.0579627245% |
| 7 | 0.5214162090 | 0.5997993114 | 0.0039328230 | 0.0027460548% |

The limits remain gain error at most .05 and nominal cost gap at most 1%; the optimum is K*=.6037321344. No optimizer setting, learned state, quality threshold or seed was changed. All three imports preserve every non-counter byte. All three declared next-batch checkpoint replays are byte-exact. The new continuation has zero projected updates and zero PPO-clipped samples; clipping branch coverage comes from the mathematical fixtures.

The fixed longer budget is sufficient for these three measured continuations. This supports the diagnosis that the original schedule ended before sufficient optimization progress, without establishing a general convergence theorem. Finite-grid RNG statistics and broader training recipes retain their stated scope.

New native guard checks pass in debug, release and ASan/UBSan: 1,409 assertions, 249 refusals and three by-value storage-overlap observations per mode, with identical synthetic import bytes. They execute zero full training batches. The unchanged old arithmetic is carried through source and generated-C closure binding; new captures are independently audited. Summed new action time is about 506 seconds, including about 501 seconds of independent audits and 2.8 seconds in native children/file effects. This tiny scalar workload is not a GPU training throughput benchmark.

The [continuation fixture](../../fixtures/robotics/locomotion-lqr-longer/README.md)
retains the prospective contract, source/native checks, import lineage,
complete outcome table and compact curves. No early stopping, setting changes,
best-seed selection or extra training is permitted within this profile.

This remains a normalized scalar CPU F64 experiment with one learned gain,
fixed variance and an analytic nominal continuous-Gaussian critic. Its
finite-top53 sampler is not an exact iid continuous-Gaussian law. This work
does not implement a learned critic, neural multidimensional actor,
quadruped, gait, GPU training, hardware, compiler R14 or full learning-curve
reproducibility. One next-batch replay has its stated narrow scope.

This follow-up changes only the LQR convergence status in the earlier dated
robotics assessment. Broader robotics gaps remain. Work stops after the
requested chapter update and push; no subsequent chapter is started.
