# Frozen C/D validation transport and execution

This component evaluates the already fixed common scheduled-PD law on the two
prospectively sealed C/D poses. It runs eight contexts: pose-major, then hold,
nominal tracking, tracking with pulse, and brake. C uses a positive pulse and
initial brake rate; D uses their negative counterparts. No gain, threshold,
horizon, objective or tolerance is adjusted from these results. Old A/B stay
closed. Part A remains nine valid candidates with none feasible.

The five new Coax files own the pose factories, reference/force signs, admitted
interfaces and metrics. They have 16 focused groups and 263 actual emitted
mutants killed, with no survivors or equivalence waivers. Existing controller,
law, force, passive and implicit-damping plant code remains unchanged. The
new case cards use profile 0/1 solely as pose labels, not two gain families.
All nine initial speeds, previous rates and initial-plus-post margins come
from the actual pose. Every pose retains scenario IDs 0..3.

## Effects performed once

Preparation makes no SDK load or plant update. It emits two source contexts,
builds the collector and three native contexts, runs 169 new sanitized ABI
assertions, and exports all eight fixtures/initials and 6,008 reference records.
The fixture/reference phase has 144,958 assertions. Initial/factory bits and
reference tick/N agree exactly; the largest independent angle and derivative
errors are 1.11e-16 and2.22e-16. The old 62 scalar table and original four-case
metric proofs are not rerun.

The SDK phase has one model load, eight compiled-default resets/initial
forwards and exactly 6,000 `mj_step` calls. Before the FIRST forward, its actual
model metadata must equal the prior qualified model byte for byte. Every
model/SDK/header/XML/85 asset/86 declaration identity is bound. The collector
checks callbacks/support, flags 17, Euler with implicit physical damping,
h=.002, zero warnings, no post-step forward, and independent zero off-scene
mass blocks. Raw current q/v go to the Coax controller before each step.
Post-step actuator-cache fields are explicitly labelled PREintegration.
The 54,000 sparse unit-moment checks establish scalar actuator-force equals
its named generalized-force component. There is no NN, render or shadow.
The first capture has 3,021,199 assertions and 18,018 lifecycle events.

The OWN phase then executes exactly 6,000 fresh-feedback steps once in release
mode, with 1,644,482 assertions and 12,008 lifecycle events. No SDK or shadow is
called. Both recorded datasets are subsequently compared and reduced in
three host profiles without any plant calls. Each cached context has 2,220,017
parity assertions and 756,521 assertions for EACH dataset's score reduction.
Complete cached outputs agree across debug, release and fail-fast ASan/UBSan.

Both datasets meet all eight frozen goals. The independent audit checks all
12,000 runtime schedules exactly, fresh SDK 210,000 PD fields and 54,000 actuator
fields, and both eight-case direct-fsum metric sets. It is a separate original
receipt. Meeting these finite tests is not a nonlinear stability, collision,
contact, model-import, real-time, hardware or bounded-execution certificate.
Reference rate/acceleration limits do not bound actual plant motion.

## Protocol

All headers have six little-endian U32 words:
magic,2 poses,4 scenarios,750 ticks,rowbytes,version1.
CDI1 input uses magic 0x31494443 and144-byte PntInitial rows.
CDS1 SDK uses 0x31534443; CDO1 OWN uses 0x314f4443, both with 1,072-byte rows.
The payload is unchanged fresh PntRecord 1,040 plus PsdSchedule 32:
signed error7, absolute error7, damping7 F64, then I64 regime.
Each context's old record scenario field remains 0..3. Pose identity is bound
by the header and eight index rows, each six I64 words:
pose,scenario,offset,count,failure_stage,failure_code.

Indexes are updated after every committed record. Pending interrupted entries
retain−1 admission markers; completed captures require all eight 750-row
ranges/status 0/0. An unexpected refusal or warning stops acquisition and
preserves actual trace/index/progress extents. A failed step has no fabricated
state. One-shot markers prevent a retry of either collection.

PsvReference is 184 bytes (21F64 plus tick/N); PsvVerdict is 16 bytes (valid,
feasible). Existing 144/744/976/256/360/72-byte data layouts are retained.
Native guards exercise only new interfaces/refusals and command overlap;
no successful extra native step probe is used.

Comparison admits every serialized float before maxima, checks all packet
status/reference/pulse/clip metadata exactly, and uses the frozen physical
per-field tolerances. Cross-stream schedule regimes are diagnostics because
their input states differ; each stream's law is independently bit-exact.
Per dataset outputs are eight360-byte case cards, two72-byte pose cards and
one16-byte AND verdict. Each pose must independently meet every goal.

## Offline gate

`check-panda-scheduled-pd-validation.sh` calls only the additive cached adapter.
It reads committed traces, indexes and original qualification records. It
never opens the SDK, resolves external assets or invokes collection/plant
subcommands. Original source, compiler and capture/cache identities are
checked before and after scoring. This wrapper is syntax/plan/stub-routing
checked in this publication; its numerical path is deliberately not rerun.
The explicit one-shot acquisition driver remains separate and immutable.
