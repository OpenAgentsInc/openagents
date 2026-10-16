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
session-host submit --state DIR --key-file KEY --input BUNDLE \
  --env-state ENV --env-key ENVKEY --env-root RESOURCES \
  --attachment ID --fingerprint DIGEST --grant GRANT.json \
  --coordinator-state COORD --coordinator-key-file COORDKEY
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

Adapter and feature operations pin complete CAP definitions, including input
and output schemas, the engine and protocol identities, and the binding's
actual effects. Each adapter has distinct operation IDs. Opening schemas pin
the source kind; command schemas restrict the action. Unsupported features pin
nothing. These definitions do not advertise a CJ endpoint, enforce an omitted
bound, or promise transport retries; those need their own runtime admission.

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
  (`Bindings::with_current`; `env_host::allocator::Allocator` implements it). A native
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
