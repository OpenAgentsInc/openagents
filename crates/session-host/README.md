# Managed sessions

`session-host` implements NIP-SESS managed sessions: admitted opening, durable
command and queue rules, turn control, pending interactions, history
export/import, and child linkage. The read-only retained-history observer
stays in `coder-connect`; it is a separate profile and grants no control here.
A session, an ENV execution attachment, a RUN controller, and a commercial
order keep separate identities.

```sh
session-host init --state DIR --key-file KEY --policy POLICY.json
session-host admit --state DIR --key-file KEY --engine loop-mid-turn
session-host observe|status|export --state DIR --key-file KEY --session ID
```

POLICY names the independently authorized `owners` and `controllers`. BUNDLE
carries one signed private `3188` event and the exact artifacts it references.
The `openagents session` commands for managed sessions wait for the next Coder
release; the existing observer commands are unchanged.

## Adapters

An adapter is admitted from its engine's exact profile. That profile states
each of the 14 features as `native`, `emulated`, or `unsupported`, along with
the steer feature's native mode, emulation, and acknowledgment. Supported
entries pin an operation, semantics, and evidence; unsupported entries pin
nothing. The host never upgrades a claim. Two in-process Rust engines exercise
the paths:

| Adapter | Steering | Queue/reorder | Questions | Fork |
| --- | --- | --- | --- | --- |
| `loop-mid-turn` | native `mid_turn`, acknowledged by `engine_replay` | emulated (host FIFO) | native | unsupported |
| `loop-boundary` | `turn_boundary`; emulated `cancel_and_continue`, acknowledged by `next_turn_start` | queue emulated, reorder unsupported | unsupported | unsupported |

Mapping Claude Code, Codex, and the ACP delegates to this host is still to
come. Their steering rows stay in their own adapter crates.

Configured opening passes the admitted settings and model, recipient, and
policy content to the adapter. The adapter validates them without effects and
must acknowledge the exact values it applied. Both loop adapters use the fixed
model descriptor `{"model":"loop"}`. Their `effort` setting limits a turn to
32 steps for `low` or 64 for `high`; native output records the setting used.
Changing settings while idle calls the actual engine under its own charged
execution. Model, recipient, and policy pins stay fixed for that native binding.
Unsupported configuration bounds refuse.

Configuration saves a complete RUN intent and unknown command reply before
calling the engine. Only a matching acknowledgment changes the effective
configuration. A lost or different acknowledgment keeps the original effective
record, blocks new input and configuration changes, and cannot repeat on retry
or restart. New intents must name the current requested or effective
configuration; the opening's old settings cannot authorize a later turn.

The native `replace_runtime` command uses the full adapter CAP and a fresh
charged ENV binding. It requires an idle session, known quiescence, no queued
or pending input, and quiescent managed children. The old native session must
close before the replacement opens. The portable session ID and lineage stay
fixed. A new generation is saved before opening; old controls then become
stale. A matching acknowledgment records the new runtime pins in
`openagents.session-runtime-binding.v1`, which later turns and continuations
use. Its reference appears in state and history under `meta.runtime`.

A replacement saves its complete intent and unknown reply before closing the
old session. An uncertain close or opening cannot repeat, release its lineage,
or allow another replacement. Its original retry can read the outcome in the
generation it established. Resume requires the owner's latest compatible
host-signed history. An explicit acknowledged replacement can clear a current
protocol fault; earlier faults stay in native history and turn results.

Native bindings also identify the engine instance that created them. Reusing a
native name in another host instance cannot receive earlier input, cleanup,
observation, or resume reads. Losing that instance leaves the old session
unknown, including an idle session. Pending interactions become unknown when
observation is lost, and cancelled on known terminal completion; their current
signed records reflect those changes.

Known admission refusals after charged intake return bounded, saved CJ failures.
Each request keeps its own signed control reply, including native duplicates.
A refusal does not claim another session's command ID or disclose its state;
its cache still requires the caller's current host membership. Uncertain
effects keep their original holds. Recovery can use a native pending reply only
when its full input fingerprint matches the original request.

Queue promotion uses a fresh charged CJ execution. Its native `promote`
command selects the exact current head by intent ID, frozen input digest, and
original requester, under the current revision and generation. It verifies the
retained signed enqueue and current requester rights. The original requester's
read scope supplies the input; the selector does not share private context with
the caller. An expired or incompatible head gets a recorded disposition without
skipping to another item. The turn's frame must match its ENV execution, and
unsupported intent bounds refuse. Intent consumption is saved before native
input and stays consumed across changed commands, bindings, generations, and
resume sessions in the same lineage, including lost acknowledgments and restart.

Adapter and feature operations pin complete CAP definitions, including input
and output schemas, the engine and protocol identities, and the binding's
actual effects. Each adapter has distinct operation IDs. Opening schemas pin
the source kind; command schemas restrict the action. Unsupported features pin
nothing. Definitions alone do not authorize execution or enforce an omitted bound.

The library's `SessionHost::serve` accepts exact ENV-bound CJ execution for
admitted adapter operations. The operator registers the exact native CAP with
COORD for this host's distinct worker key. The allocator checks the original
prepared request and active attachment; COORD saves the call charge and unknown
hold before the host reads private input or calls the engine. A claim for a
different operation cannot authorize session input. The private `submit` path
cannot start production engines; its uncharged engine path is limited to
explicitly selected local test bindings.

Opening and metadata commands return encrypted results. A turn start returns
encrypted progress; `SessionHost::poll` observes that original turn until its
terminal evidence produces a CJ result. Retries return the identical saved
event and bounded artifact set. They do not repeat intake, charge, or native
input. Session and native RUN identities remain separate from the CJ RUN.
Cached replies still require current ownership and generation.

This receiver currently drives the two synchronous loop adapters through the
library. It requires one job, zero spend, 1–300 seconds of wall time, and
64 KiB–1 MiB of output allowance. Polling after expiry requests a stop of the
original turn before advancing it; only native terminal evidence proves the
stop. Large known outputs produce a bounded failure while their full local
history remains available to its owner. An unknown native outcome remains
unknown and retains its COORD hold after restart. The loop profiles read
artifacts and have no external resource writes; their internal bookkeeping is
not a separately claimed workspace resource.

`SessionHost::continue_turn` accepts the original CJ event and a separately
signed private question reply, steer, or interrupt command. The original
request must still own the exact active native turn. ENV and COORD hold their
current authority checks through delivery, including resource generation,
executor registration, grants, frame, expiry, and original call accounting.
This path neither starts a turn nor charges a second call. Its proof type
cannot authorize opening or a new turn. Polling also checks current authority
before advancing the loop; revoked or drained work receives a stop request.
The engine instance must still own the native session before any new input.

Steer and interrupt commands save an unknown pending reply before native
delivery. A lost acknowledgment cannot repeat delivery on retry or restart.
An uncertain steering command blocks further input for that turn. Exact cached
controls still require the current owner and generation; reading a saved reply
does not deliver anything. Replies also check their own expiry before delivery.

Resident CLI transport and real engine processes remain unfinished. This
receiver does not advertise those roles.

The production binding reads the allocator's actual saved attachment. It
checks the requesting controller, executor key, lease, materialization, and
admitted engine identity. Opening and turn start keep the ENV and COORD fences
through native admission and input disclosure. Turn starts recheck after saving
their durable intent; elapsed time cannot extend a grant. A read-only attachment
probe cannot implement this admission. Missing live authority refuses.


## Rules the host enforces

- **Opening.** The owner must be authorized by this host and the adapter
  admitted. The source kind must be supported. Lease and materialization must
  be exact retained ENV records, and every requested setting must validate
  against the adapter's closed settings schema; an unsupported setting
  refuses. Requested and effective configuration are both retained, and the
  effective one carries the enforcement record. Every opening and every turn
  start needs a current ENV attachment for that exact execution
  (`Bindings::with_execution`; `env_host::allocator::Allocator` implements it). A native
  session already bound to an open session refuses. Resume keeps the source
  lineage only while no open session holds it. Fork needs the adapter's fork
  support. Import admits foreign history read-only.
- **Commands.** Idempotency is `(authority, session, command)` with a SHA-256
  fingerprint of the JCS command. An exact retry returns `duplicate`; changed
  bytes return `idempotency_conflict`. Generation, revision, expiry, and
  control authority are checked before every admission. A stale revision
  conflicts. Each accepted mutation advances the revision by one and retains
  a signed receipt and session state.
- **Queue.** `submit` admits only on an idle, quiescent session with nothing
  queued. `enqueue` appends to one durable FIFO. `reorder` must name the exact
  remaining queue. Promotion takes only the head, only after the prior turn's
  quiescence evidence (terminal engine evidence bound to its attachment), and
  only under a fresh binding. An expired head becomes `expired` and is not run.
- **Turn control.** Native steering needs a `mid_turn` engine; consumption is
  claimed only on engine replay. Emulated steering runs only when the signed
  correction chooses `emulated`. The host interrupts the turn, and the next
  started turn carries the correction, which is recorded as consumed at that
  start. Interrupt records the request; the stop is the later terminal
  observation.
- **Interactions.** Only the admitted recipient can answer, and only while the
  exact turn and generation are current. The answer must match its answer
  schema. The first answer is claimed durably before delivery to the engine.
  Identical retries return the record, and a conflicting answer refuses. A
  delivery whose acknowledgment was lost stays `unknown` and is never sent
  again.
- **Restart.** A turn whose engine instance is gone is observed as `unknown`
  (`lost_observation`) with no quiescence. It blocks promotion and is never
  rerun.
- **History.** Exports carry every native capture, a portable view derived
  from turn results (a recorded `derived` loss), opaque resume state when the
  adapter supports resume, and a signed WS projection cut. A foreign import is
  archival: a new lineage, no native resume, recorded losses.
- **Children.** A managed child needs the child's admission and a delegated
  authorization. An observed child needs native evidence and grants no
  control.

`session-host policy --state DIR --key-file KEY --policy POLICY.json` replaces
the operator's owner/controller sets. This administrative path needs the host
key; remote session commands cannot change policy. Cached replies still check
current ownership, control rights, and scope generation. Promotion also checks
the recorded queue requester's current rights. An old queue entry without that
identity must be cancelled and submitted again.

Engine starts now save the authoritative active turn, RUN dispatch intent,
consumed queue head, and original command's pending result before calling the
engine. A lost start reply stays unknown, including on exact retry or restart.
A successful later observation can finish that original turn, but cannot
promote its consumed queue head again.

Opening also saves the session ID, resume lineage, complete RUN intent, and
pending reply before calling the engine. A lost acknowledgment or process
restart keeps that opening unknown. Exact retries return its original reply;
another command cannot reuse the session ID or resume lineage. An unknown
opening cannot be closed to make that lineage available again. Archiving only
hides it and does not assert that the native engine stopped.

A required native message that the adapter cannot handle records a protocol
fault. The host keeps the actual native capture and a RUN record, exposes the
fault in status and portable history, and leaves the turn's outcome unknown.
New input, steering, configuration, answers, and queue promotion refuse.
Observation and stopping remain available. A later terminal result can prove
quiescence but does not erase the fault or make the session ready again.

Close needs an idle session with known turn quiescence. Stop the active turn
first. For a managed session, close saves its RUN intent and pending command
reply before calling the adapter's native close. Only an acknowledged native
close makes it closed. A lost reply stays unknown, cannot be sent again under
another command, and keeps the resume lineage reserved. Read-only imports have
no native engine to stop. Adapters without native close refuse that operation.

Private reads use the requesting principal's supplied or shared bytes, including
on queue promotion. A hash, a refusal, or an accepted queue record cannot grant
access to another owner's cached input or resume state. Adapter definitions are
explicitly public. Identical private records have separate encrypted declarations
for each recipient. Resume also needs a host-signed managed export tied to the
requester's own source session; possessing an export does not transfer control.
Foreign imports require their actual content and remain read-only.

The store retains at most 64 sessions and 256 queued intents per session.
Limits stop new admissions and never discard history. All fixtures use
throwaway keys and local state; no money moves.
