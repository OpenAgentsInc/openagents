# `openagents.cloud_computer_command.v1`

Status: Implemented for `openagents.cloud_computer.v1` runtimes.

Owning package: `packages/khala-sync-server`

## Safety boundary

A command has one stable execution ref and one canonical request digest. The
request binds the owner, tenant, logical workspace, runtime lease, runtime
identity, computer generation, working directory, capability snapshot, budget
snapshot, and timeout. Reusing an idempotency ref with different bytes fails
before a runtime effect.

The durable journal, not a WebSocket or LiveView process, owns command state.
Live projections can disconnect or drop messages without changing terminal
evidence.

## Dispatch and acknowledgement

The control plane reserves `may_have_started` before it writes a dispatch frame
to the runtime. A runtime acknowledgement for the exact execution ref, runtime
identity, lease, and generation advances the command to `dispatched`. Transport
loss before acknowledgement leaves `may_have_started`; the controller never
starts a replacement command automatically.

A controller restart can resume an `admitted` or `not_dispatched` command
because no runtime effect crossed the dispatch boundary. Recovery treats
`may_have_started`, `dispatched`, and `running` as observation or reattachment
work, never replay work.

## Runtime sessions and reattachment

The runtime reverse-dials with an authenticated session that binds its runtime
identity, lease, workspace, and computer generation. Reattachment accepts only
that same identity tuple. A replacement runtime or later generation cannot
claim an older execution ref.

The reattachment cursor names the last durable event sequence observed by the
caller. The journal returns the next retained sequence without duplicating or
omitting an event. A cursor before the retained window returns a typed reset
with the earliest available sequence and durable terminal evidence.

## Events and output retention

Each execution uses a dense, increasing sequence across these event kinds:

- `stdout` and `stderr` output.
- Tool and lifecycle events.
- Checkpoint evidence.
- One terminal result.

An exact sequence replay returns the existing event when its canonical digest
matches. Different bytes at an existing sequence, a gap, or an event after the
terminal result fail. The journal retains bounded inline output and replaces
larger or expired payloads with content-addressed artifact refs. Artifact refs
bind the digest and byte count; they do not expose storage locations.

## Cancellation, timeout, and loss

Cancellation reserves one exact command, runtime identity, lease, and
generation before sending a cancel frame. Exact retries return the stored
result. A cancel or timeout cannot target a reused execution ref or a later
runtime generation. The first terminal compare-and-swap wins; later terminal
events return the existing result or a conflict when their bytes differ.

Recovery can settle `lost` only with durable evidence that the bound runtime or
host cannot reattach. Checkpoint and cleanup failures remain distinct evidence
and never authorize command replay.

## Recovery matrix

| Failure boundary                             | Durable state                                              | Recovery action                                                                  |
| -------------------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Controller restart before transport exposure | `admitted` or `not_dispatched`                             | Reuse the prepared dispatch attempt, or create one, and dispatch once.           |
| Transport loss after exposure                | `may_have_started`                                         | Observe or reattach to the same runtime. Never dispatch a replacement.           |
| Runtime acknowledgement committed            | `dispatched`                                               | Resume the durable event cursor and wait for runtime start or terminal evidence. |
| Runtime crash                                | Active command plus `runtime_lost` evidence                | Settle `lost`; never redispatch the execution ref.                               |
| Host loss or replacement                     | Active command plus generation-fenced `host_lost` evidence | Settle `lost` and reject late events from the old generation.                    |
| Checkpoint failure                           | `checkpoint_failed` evidence                               | Record the checkpoint failure separately. Do not infer command replay safety.    |
| Cleanup failure                              | `cleanup_failed` evidence                                  | Record the cleanup failure separately and retain terminal command evidence.      |

## Phoenix streaming interface

Phoenix consumers request pages after a durable cursor and subscribe only for
wakeup hints. After every wakeup or reconnect, the consumer reads the journal
again. This pattern preserves order across process restart and treats PubSub or
WebSocket delivery as lossy by design.

The public projection contains command state, event sequence, bounded inline
text, opaque artifact refs, timestamps, and terminal evidence. It excludes
runtime addresses, session credentials, provider handles, private paths, and
raw capability or budget documents.

## Implementation and verification

- Command, event, cursor, and reverse-dial protocol:
  `packages/khala-sync-server/src/cloud-computer-command.ts`
- Durable journal and retention authority:
  `packages/khala-sync-server/src/cloud-computer-command-store.ts`
- Crash-safe dispatch coordinator and Postgres adapter:
  `packages/khala-sync-server/src/cloud-computer-command-dispatch.ts` and
  `packages/khala-sync-server/src/cloud-computer-command-dispatch-store.ts`
- Phoenix-compatible stream and Postgres journal adapter:
  `packages/khala-sync-server/src/cloud-computer-command-stream.ts` and
  `packages/khala-sync-server/src/cloud-computer-command-postgres-journal.ts`
- Cursor conversion, output artifacts, and recovery policy:
  `packages/khala-sync-server/src/cloud-computer-command-cursor.ts`,
  `packages/khala-sync-server/src/cloud-computer-command-artifact.ts`, and
  `packages/khala-sync-server/src/cloud-computer-command-recovery.ts`
- Schema:
  `packages/khala-sync-server/migrations/0139_cloud_computer_commands.sql`

Run:

```sh
pnpm --filter @openagentsinc/khala-sync-server test
pnpm --filter @openagentsinc/khala-sync-server typecheck
```
