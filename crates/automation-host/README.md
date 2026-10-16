# Finite automation

`automation-host` implements the initial once/UTC interval runtime for
[NIP-AUTO](../../nips/openagents/NIP-AUTO.md). `standing-jobs` supplies shared
scheduling and budget primitives; `coordination-host` owns actual task admission,
worker intake, original worker results, independent checks, and ancestor budgets.

Run `automation-host --help` for local commands. Select every state directory
and private key file explicitly. The AUTO controller and coordinator must have
different keys. The plan owner is the coordinator's local scope owner, and the
worker and independent checker are separately admitted scope participants.

1. Register the coordinator's `automation_operation` and container CAP cards,
   then admit grants and a task frame. Reserve a finite container for the AUTO
   controller; its task, reservation, and expiry bind the plan.
2. `init` stores a native owner policy (`owners`, `coordinator`, `worker`,
   `checker`, `max_plans`, `until`). `policy` returns its exact reference.
3. Sign the immutable plan to the AUTO controller. `admit` accepts
   `{ "event": EVENT, "artifacts": { "sha256:…": JSON_BODY } }`.
   Both bound lists explicitly name `calls`, `wall_ms`, `input_bytes`, and
   `output_bytes` with their native mechanisms. Each occurrence reserves at
   least three calls. Requirements are exactly
   `{ "v": "openagents.automation-native-requirements.v1", "requires": [], "spend_microunits": 0 }`.
4. Pass `{ "plan": ARTIFACT_REF }` to `tick`, then `advance`. The latter stores
   exact cross-host inputs before preparing one encrypted CJ request. It never
   invokes an arbitrary model, shell, or engine.
5. `run` takes `{ "event": EVENT }` plus explicit worker state/key options.
   It returns the typed input through the actual CJ worker. Submit its event
   and artifact map to `result`, with the plan reference. `check` uses the
   independently selected checker key and releases the known child reservation.
6. Owner-signed `control` supports status, pause, resume, and cancel. Status
   does not start timers or work. Pause/cancel prevent queued intake under the
   same store lock. Interrupted work keeps its active identity and budget holds;
   unsafe resume refuses. `inspect` is local owner inspection only.

Fixed intervals coalesce downtime into exact skipped ranges. One active or
unknown occurrence prevents another. Native accounting conservatively debits
reserved ceilings after a checked result; actual coordinator call records remain
separate evidence. Plan completion means finite scheduling completed, with no
WORK, LAB, payment, or goal-acceptance effect.

Source triggers, continuation, controller CAP/CJ exchange, broader recovery, and
session/environment/effect adapters remain in #11735. Unsupported bindings refuse
before activation. The released Coder commands remain unchanged.
