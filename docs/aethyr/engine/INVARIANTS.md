# Design invariants

Status: required contracts for the proposed engine. No production implementation
or full-system proof exists. Bounded design checks are identified explicitly;
the remaining tests become release obligations in [verification](verification.md).

| ID | Contract | Owner | Required evidence |
| --- | --- | --- | --- |
| INV-01 | Only authenticated, currently authorized typed transactions mutate authored state; model output conveys no authority | [Contracts](contracts.md), [security](security.md) | Forged principal/grant, revoked grant, model injection, same-command tests |
| INV-02 | A branch commit applies every operation or none, with durable command result and monotonic revision | [Collaboration](collaboration.md) | Concurrent commits, crash points, idempotency and lost-response tests |
| INV-03 | Active rendering, collision, navigation, and selection reference one complete compatible generation | [Asset compilation](asset-compilation.md), [runtime](runtime.md) | Abstract activation model; engine cross-output fault tests |
| INV-04 | A late or failed candidate cannot replace the desired newer result or destroy the working generation | [Rendering](rendering.md), [contracts](contracts.md) | Abstract activation model; reordered compiler/GPU completion and resource tests |
| INV-05 | Authored document, runtime state, editor presence, and release identity remain distinct | [Architecture](architecture.md), [world](world-model.md) | Runtime capture authorization, presence isolation, save/export tests |
| INV-06 | A channel update names an admitted complete release and advances sequence with expected-previous compare-and-swap | [Publishing](publishing.md) | Abstract channel model; signature, stale reference, missing blob, crash tests |
| INV-07 | A public play session pins a release until its explicit compatible migration policy runs | [Publishing](publishing.md), [collaboration](collaboration.md) | In-flight play and rollback/migration tests |
| INV-08 | A walkable building derives visual openings, support, collision, and traversal from validated semantic topology | [World](world-model.md) | Doorway/controller and changed-wall/room conformance suites |
| INV-09 | Untrusted content cannot receive editor authority; all host imports and external effects are capability-checked | [Security](security.md), [scripting](scripting.md) | Cross-origin isolation, bridge fuzzing, guest escape and revocation tests |
| INV-10 | Compatible hot changes preserve approved state; unsafe pose/state migration requires hold or explicit policy | [Runtime](runtime.md), [contracts](contracts.md) | Player-support, state migration, failed shader and device-loss tests |
| INV-11 | Asset identities, source rights, compiler keys, and export restrictions stay attributable and checked | [Asset compilation](asset-compilation.md), [integration](integration.md) | Reproducibility, tamper, missing source, license/export tests |
| INV-12 | Limits, cancellation, and resource accounting include candidate and failed work; failures never imply false save or publish success | [Performance](performance.md), [operations](operations.md) | Endurance, quota, storage-full, cancel/stale-output, restore drills |
| INV-13 | Engine ticks and GPU submission are independent of React renders, hooks, atom notifications, and UI subscriber lifetimes | [Frontend](frontend.md), [runtime](runtime.md) | No-subscriber/slow-consumer runs, bounded queues, Strict Mode/remount tests, separate UI/frame timing |

Changing an invariant requires updating its owning documents, contract fixtures,
verification obligations, and decision record together. An abstract model is not
permission to weaken runtime checks. A failed test must not be hidden by lowering
quality, dropping invalid samples, or relabeling an unsupported browser as supported.

Keep formal inputs synthetic. Do not store production secrets, account data,
private prompts, licensed geometry, or private sibling source in traces.
