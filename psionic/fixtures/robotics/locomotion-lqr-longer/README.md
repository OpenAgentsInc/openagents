# Scalar-LQR longer continuation

This is a new fixed-budget continuation of the [original128-batch study](../locomotion-lqr/README.md). Every original source, fixture, failed outcome and dated record is preserved.

All 81 prescribed actions completed and passed independent audits. Each seed reached 512 total batches and 2,048 AdamW updates. **All three meet both unchanged quality goals**, including the independently evaluated real-arithmetic margins. The original 128-batch misses remain recorded above.

| Seed | Original gain at 128 batches | Final gain at 512 batches | Final absolute gain error | Worst final nominal cost gap |
| --- | ---: | ---: | ---: | ---: |
| 0 | 0.5184256094 | 0.6045622741 | 0.0008301397 | 0.0001218323% |
| 1 | 0.5227558679 | 0.6219745781 | 0.0182424437 | 0.0579627245% |
| 7 | 0.5214162090 | 0.5997993114 | 0.0039328230 | 0.0027460548% |

The limits remain gain error at most .05 and nominal cost gap at most 1%; the optimum is K*=.6037321344. No optimizer setting, learned state, quality threshold or seed was changed. All three imports preserve every non-counter byte. All three declared next-batch checkpoint replays are byte-exact. The new continuation has zero projected updates and zero PPO-clipped samples; clipping branch coverage comes from the mathematical fixtures.

The fixed longer budget is sufficient for these three measured continuations. This supports the diagnosis that the original schedule ended before sufficient optimization progress, without establishing a general convergence theorem. Finite-grid RNG statistics and broader training recipes retain their stated scope.

New native guard checks pass in debug, release and ASan/UBSan: 1,409 assertions, 249 refusals and three by-value storage-overlap observations per mode, with identical synthetic import bytes. They execute zero full training batches. The unchanged old arithmetic is carried through source and generated-C closure binding; new captures are independently audited. Summed new action time is about 506 seconds, including about 501 seconds of independent audits and 2.8 seconds in native children/file effects. This tiny scalar workload is not a GPU training throughput benchmark.

See [the chapter](../../../book/ch171-learn-a-gait-in-many-worlds.md) and [experiment notes](../../../docs/robotics/lqr-longer-experiment.md) for the prospective design and limits. `experiment.json`/`contract.md` were frozen before imports; `source-qualification.json`, `native-publication.json`, lifecycle/audit guard receipts and `independent-learning/` retain actual evidence. Large complete iteration/coefficient streams remain private with hashes.

New work:589824 canonical steps/4608updates/36864tape pairs/2359296sample gradients;1536 replay steps/12updates;2304 deterministic final-evaluation steps. Original baseline and196608-step prefix are carried, never rerun. Imports perform no plant/RNG/optimizer/tape work. SourceVM synthetic/mutant checks and native guard observations remain separately scoped.

This is normalized CPU F64 scalar learning with fixed variance and analytic nominal critic, not quadruped or hardware qualification. No full-curve replay or compiler R14 claim. New LXS/LXI/LXC/LXE/LXG magics bind the larger counter reservation; original LQS profiles retain their meaning. File fsync/checkpoint rename excludes directory fsync and crash-durability qualification.
