# First genuine neural + owned-query prefix

One frozen invocation of the already-linked `owned-shadow-v1` binary couples the original GR00T policy to the owned Panda controller-sample constructor. This is a bounded integration prefix on one exact original `demo_000` scene and full 45-F64 reset. It is not a task-success benchmark or a complete owned simulator.

## Measured outcome

- Nine action ticks and two genuine resident Metal model calls.
- 225 live SDK steps and 225 mandatory recorded-control replay steps, 450 total.
- Every one of the 45 F64 state words matches exactly during native replay.
- Zero holds, refusals and torque-clipped coordinates; empty sanitizer stderr.
- Task success was not required and was not observed. The prescribed clean nine-tick prefix is complete.

The original model's wide-statistics/F32 profile and fixed noise are unchanged. The supervisor admits a decoded 16×7 action chunk and consumes eight rows before requesting a new observation. This run consumes the first row of the second genuine request, rather than extending a cached chunk.

Each control substep forwards the SDK state, reads named `q9/v9`, constructs the Coax FK/Jacobian/mass/bias sample and supplies it to the unchanged operational-space controller. Owned refusal never falls back to SDK controller math. SDK telemetry remains required; request observations and offscreen cameras retain the cached SDK stage before the next forward. The SDK still performs source actuation, contacts, integration and rendering. This is distinct from chapter 167's C/D scheduled-PD and owned-Euler/no-contact profile.

## Independent offline audit

[independent-qualification.json](independent-qualification.json) decodes 225 canonical 512-byte records and checks all 1,575 F32 action words and 12,150 F64 control/state words, canonical metadata, every simulation clock, the independently derived seed and the complete SHA chain through the footer. Maximum clock error, 3.331e−16 seconds, is below 2⁻²⁶. Request serial 1 covers ticks 0..7; serial 2 covers tick 8. The first eight consumed actions bind exactly to saved first-decoded bytes. The full second decoded array is not separately stored. New reader guards exercise malformed numeric fields, metadata, records and hashes without NN, SDK, build or physics execution.

The first decoded 448-byte hash equals the older recorded frame-0 value. This is a diagnostic, not the acceptance condition, a new vendor-framework execution or proof that every hidden model state matches. Earlier public-endpoint and conditional-head proofs retain their scope and failed global-hidden diagnostics.

The bound source implies four offscreen captures, 452 explicit forwards including two resets, 237 telemetry samples and 226 robot/owned-sample paths. These are source-derived counts, not runtime-instrumented counters. The 225 live plus 225 replay updates are integrated SDK steps, not totals of every SDK API or internal forward call.

## Timing and limits

| Observation | Seconds |
| --- | ---: |
| Whole invocation | 36.6552 |
| Model context open | 12.4638 |
| Live loop wall, excluding open/replay | 10.3489 |
| Inference total, two calls | 8.9717 |
| Maximum inference call | 4.7554 |
| GPU compute total | 2.0947 |
| Simulated time | 0.4500 |

Replay is not individually timed. Synchronous inference pauses simulation; these timings do not establish 20 Hz wall action updates or 500 Hz wall control. Rendering is outside the exact-physics-replay claim. Zero torque clipping was not an acceptance threshold; the observed count happens to be zero.

The one-shot lifecycle preserves input/source/binary/shader identities, launch marker, stdout/stderr and partial failures, imposes an 840-second timeout and permits no retry. Only a complete normal prefix or a separately identified authoritative source-success stop can qualify; partial or refused output cannot. There is no task-success threshold, held-out benchmark, generic model/import claim, global stability result, hardware qualification or compiler R13/R14 guarantee.

## Evidence

- [experiment.json](experiment.json): inputs, acceptance, effect budgets and diagnostic-only first hash, frozen before execution.
- [preparation.json](preparation.json), [execution.json](execution.json) and [launch-once.json](launch-once.json): the first invocation and unchanged execution context.
- [Native case record](episode/demo_000-fault0/record.json), [physics trace](episode/demo_000-fault0/physics.ptr) and [first decoded output](episode/demo_000-fault0/first-decoded.f32): the measured artifacts.
- [independent-qualification.json](independent-qualification.json), [reader-qualification.json](reader-qualification.json) and [observations.json](observations.json): the first offline audit and guard evidence.

The focused `check-panda-owned-query-neural.sh` path uses a distinct cached adapter. Its additive numerical path is not rerun here; syntax, plan and stub routing carry their own qualification. It does not launch the neural provider, SDK or renderer. Existing build/mock/preflight receipts remain unchanged in [the historical runtime checkpoint](../panda-query-runtime/README.md); no passing old model or arithmetic gate was repeated.
