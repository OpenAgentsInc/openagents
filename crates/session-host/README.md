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
  --attachment ID --fingerprint DIGEST --grant GRANT.json
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

## Rules the host enforces

- **Opening.** The owner must be authorized by this host and the adapter
  admitted. The source kind must be supported. Lease and materialization must
  be exact retained ENV records, and every requested setting must validate
  against the adapter's closed settings schema; an unsupported setting
  refuses. Requested and effective configuration are both retained, and the
  effective one carries the enforcement record. Every opening and every turn
  start needs a current ENV attachment for that exact execution
  (`Bindings::check`; `env_host::allocator::Allocator` implements it). A native
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

The store retains at most 64 sessions and 256 queued intents per session.
Limits stop new admissions and never discard history. All fixtures use
throwaway keys and local state; no money moves.
