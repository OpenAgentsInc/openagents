# Finite automation

`automation-host` implements the finite schedule, native source-watch, and checked continuation runtime for
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

`transport` distributes the exact controller CAP and pins. `invoke` signs a
plan/control request with `--participant-key-file`; `serve` retains its CJ
acceptance, RUN root, intent, result, and output artifacts. Exact retries return
the original response. `recover` returns an interrupted call's saved result or
fences a proven absence; it never dispatches the operation again. A remote hash
lookup or a relay claim cannot replace these admitted pins.

For a source watch, register the coordinator's `automation-source` reader for
one native resource. Admit the source key as a read-only scope participant and
include it in the immutable recipient set. `source-init` creates its separate
store and key binding (`controller`, `coordinator`, `scope`, `resource`, `until`);
`source-binding` exports the exact CAP and pin bundle. Use that bundle as the
AUTO policy's optional `source`. Mount the same workspace map with `--sources`
and the independent source store/key with `--source-state` and
`--source-key-file`. Keep one CJ identity book per source key.

The source trigger uses the bundle's target, lock, context, requirements, and
config reference. Its bounds declare one call, at most 30 seconds, and exactly
1 MiB each for input/output. Source occurrences need at least four calls and
1 MiB byte ceilings; the aggregate allowance also covers every initial poll,
including unavailable or unchanged results. The reader delivers at most
512 KiB of content. Current scoped read grants, original source declarations,
opaque provider revisions, and actual intake revalidation govern dispatch.
Worker and source CJ results carry their complete RUN intent-to-terminal chain.
Occurrence `settled` references the authenticated worker terminal record;
conservative accounting receipts stay in `usage`.
Queued pause/cancel blocks source intake before its read is charged.

For checked continuation, admit a native completion rule into the protected
frame alongside the exact-output acceptance rule before reserving the parent.
The closed rule has `v: openagents.automation-native-completion-rule.v1`,
`requires: []`, `check: verified-occurrence-count`, `minimum_successes` (1–16),
and the exact output `acceptance` reference. `evaluator-init` stores its separate
checker policy (`controller`, `coordinator`, `scope`, `producer`, `rule`, `until`);
`evaluator-binding` exports the pins. Set the AUTO policy's optional `evaluator`
to that bundle and mount it with `--evaluation-state` and `--checker-key-file`.
Keep one CJ identity book per evaluator key.

A continuation plan requires that exact `completion` binding, one checker call,
at most 30 seconds, and 1 MiB input/output bounds. Each occurrence reserves at
least four calls, or five with source revalidation. After `check`, `evaluate`
executes the independent pinned checker through CJ. It reads authenticated
current coordinator facts, counts independently checked occurrences, and
requires every managed child to have settled. Only a policy-valid `continue`
permits another occurrence after the minimum interval. Model text cannot
satisfy the rule. Exact saved replies survive restart without a second call.
Exhausted budgets or occurrence limits stop scheduling without claiming the
objective succeeded.

Broader occurrence cleanup recovery remains in #11735. This native profile
supports fixed data execution and read-only Git observations. It refuses
session/environment/effect bindings before activation; those adapters must
await their own terminal work and cleanup. The released Coder commands remain
unchanged.
