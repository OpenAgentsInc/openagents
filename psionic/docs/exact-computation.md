# Programs compiled into weights: what the pre-reset work teaches

Planning note, 2026-10-29, for Part XXIII of the [book plan](../book/PLAN.md).
Before the reset this idea was called Tassadar, after Percepta's published
construction. Sources, readable with `git show 0b401561a0^:<path>` unless
noted:

- `docs/history/2026-09-28-tassadar-percepta.md`: the full history, about
  2,000 lines.
- `docs/roadmap/2026-09-28-tassadar-revival.md`.
- `docs/tassadar/RESEARCH_PLAN.md`, read with
  `git show e1fbd1c185^:docs/tassadar/RESEARCH_PLAN.md`.
- `docs/audits/2026-10-10-training-system-audit/`.
- Transcripts 216, 220, 236–238, 243 and 274–275 in `docs/transcripts/`.
- About 366 commits whose messages mention it.

None of that code survives the reset. The numbers below are historical, not
Psionic results.

## The idea

Percepta, a research lab, published "Can LLMs Be Computers?" and
"Constructing an LLM-Computer" (March 2026), with an Apache-2.0 repository,
`Percepta-Core/transformer-vm`. A deliberately ordinary transformer (width 36,
18 heads of 2 dimensions, 7 layers) executes programs exactly because its
weights are constructed, not trained. The program is in the prompt; each
generated token is a step of the machine's state, so the output is the trace.

- **Exact memory.** A key k becomes the point (2k, −k²) and a query q the
  direction (q, 1); the score is uniquely largest at k = q, so a hard-max
  attention read returns exactly the stored value.
- **Arithmetic and branches.** Products from ReGLU
  (a·b = a·ReLU(b) − a·ReLU(−b)), conditionals from ReLU steps and sums from
  residual wiring.
- **Fast decode.** A hard-max query is a convex-hull query, so a step costs
  O(log t) rather than O(t).
- **Compilation.** Gates are scheduled into layers and residual slots reused,
  either for a universal interpreter or for one program baked into the weights.

The trace is the receipt: a computation that is exact can be verified by
replaying it.

## What the pre-reset work found

**Compilation worked; learning did not.**
- Compiled programs reproduced the reference interpreter's trace digest bit
  for bit: 4/4 exact on 9×9 Sudoku and on 10×10 Hungarian assignment.
- Learned executors never did. The best reached 0/2 exact over its first 32
  tokens and plateaued. A sweep over 103.6M verified tokens gave pass@1 of 0.0
  for next-token, auxiliary-state and lookup-initialized students.
- A frozen compiled core with a learned interface scored 1.0. That became the
  design rule: train the routing and judging interface, and keep exactness in
  deterministic machinery.

**It was slow, and said so.**
- The compiled 9×9 Sudoku ran at 9,321 steps/s, against 12.46M on a CPU
  reference interpreter: a ratio of about 0.00075.
- Faster figures quoted at the time were a Rust interpreter over fixture tables,
  not the model.

**It paid verified contributors.**
- The public run's Pylon closeout matched the reference digest and rejected a
  tampered one.
- It settled 1,000 sats over Spark, then 5 sats each to worker and validator per
  verified pair.
- It verified only one trivial program, so it built machinery rather than
  capability. The worker's self-test, separate validator devices, owner caps
  and receipt-first settlement are kept in chapter 177.

**Status.**
- Retired in July 2026 until cash-flow positive.
- Its Rust code was reimported in October as a serving dependency.
- Removed again by the reset.

## Lessons the chapters keep

1. **Name the path that ran.** A fast figure from an interpreter is not the
   model's speed, and a gate that tokenized a CPU trace did not run the model.
2. **No green status on a trivial artifact.** A production contract was marked
   green after training 303 scalars.
3. **State the CPU's speed beside the model's,** before anyone asks.
4. **Partial exactness is not exactness.** The first 32 tokens being right is
   not a pass, and the benchmark corpus is not held-out data.
5. **Benchmarks must be what their names say.** A "Sudoku class" that filled
   two cells of a 4×4 grid was not one.
6. **Curricula can erase what worked.** Full-trace supervision destroyed
   first-token accuracy.
7. **Differential harnesses pay off.** The first one found two compiler bugs on
   its first run; chapter 169 keeps them as planted mutants.
8. **Keep fixtures small and regenerable.** The import carried 737 MB of
   fixtures, and most of their paths were missing.
9. **Count only real work and real payments.** One program verified forever,
   and a simulated payment counted in a total, are not progress.
10. **Ship the falsifier with the claim.**

## What the owner wants from it

The transcripts describe Psion growing an executor model, CPU computation
inside model weights, a long-running distributed run that pays contributors in
Bitcoin for verified work, and modules of verified function that are built once
and reused instead of regenerated: "an agentic npm". The book represents all of
these: the construction and its verification (165–173), the learning
experiments (174–175), composable modules (176) and the paid run (177).
