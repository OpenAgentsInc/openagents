# Khala Sync — Mutator Authoring Guide (KS-3.3, #8293)

How to write, register, and test a server-authoritative mutator for the
Khala Sync push engine. Normative background: [`SPEC.md`](./SPEC.md) §2.4
(mutations), §4 (Postgres substrate), §7 (invariants). Engine source:
`packages/khala-sync-server/src/push-engine.ts`; the landed Worker surface
is `POST /api/sync/push`
(`apps/openagents.com/workers/api/src/khala-sync-push-routes.ts`) with its
registry in `apps/openagents.com/workers/api/src/khala-sync-mutators.ts`.

A mutator is a **named, server-authoritative write**: the client applies
its own optimistic implementation to an in-memory overlay and pushes the
named mutation; the server executes ITS implementation — the only one that
counts — and the client rebases onto whatever the server decided. You are
writing the server side. The shape:

```ts
import { Schema as S } from "effect"
import { EntityId, EntityType, MutationResult, MutatorName, personalScope } from "@openagentsinc/khala-sync"
import { defineMutator } from "@openagentsinc/khala-sync-server"

const ThingSetArgs = S.Struct({
  id: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  value: S.String.check(S.isMaxLength(4096)),
})
type ThingSetArgs = typeof ThingSetArgs.Type

export const thingSetMutator = defineMutator<ThingSetArgs>({
  name: MutatorName.make("thing.set"),
  // Throwing here ⇒ the engine records an in-band `invalid_args` rejection.
  decodeArgs: (argsJson) => S.decodeUnknownSync(ThingSetArgs)(JSON.parse(argsJson)),
  execute: async (args, ctx) => {
    // 1. AUTHORIZE + VALIDATE FIRST (before any write — see below).
    const scope = personalScope(ctx.userId)

    // 2. Business writes through the transaction handle.
    await ctx.writer.sql`
      INSERT INTO things (scope, id, value) VALUES (${scope}, ${args.id}, ${args.value})
      ON CONFLICT (scope, id) DO UPDATE SET value = EXCLUDED.value
    `

    // 3. Changelog append(s) — one per changed entity, mutationRef always.
    await ctx.writer.appendChange({
      scope,
      entityType: EntityType.make("thing"),
      entityId: EntityId.make(args.id),
      op: "upsert",
      postImage: { id: args.id, value: args.value },
      mutationRef: ctx.mutationRef,
    })

    // 4. Result for THIS envelope.
    return new MutationResult({ mutationId: ctx.mutationId, status: "applied" })
  },
})
```

The engine wraps `execute` so that client-state binding → idempotency gate
→ your code → mutation-ledger recording all commit atomically. The rules
below are what your code must uphold inside that wrapper.

## 1. The single-transaction rule (SPEC §7 invariant 5)

Everything a mutator does happens inside ONE Postgres transaction that the
engine opens per envelope: permission check, validation, business writes,
changelog appends, and the ledger recording. Concretely:

- Use `ctx.writer.sql` for every business write and `ctx.writer.appendChange`
  for every changelog entry. **Never** open your own transaction, use a
  different connection/handle, call `BEGIN`/`COMMIT`/savepoints, or reach a
  non-Postgres store (KV, DO, R2, fetch) for state the mutation depends on.
- No side effects that cannot roll back. If the transaction aborts, the
  world must look exactly as if the mutator never ran (the engine's
  atomicity test proves business row + changelog + ledger + scope counter
  all roll back together). Queue sends, emails, webhooks, and cache writes
  do not belong in a mutator — model them as state written here and acted
  on by a separate consumer of the changelog.
- Version allocation is automatic: `appendChange` allocates (or reuses)
  this transaction's per-scope version under the scope-counter row lock.
  Do not touch `khala_sync_scopes` yourself; that lock discipline is what
  keeps versions dense and monotonic (invariant 1).
- Appending the same `(scope, entityType, entityId)` twice in one
  transaction collapses to one row, last write wins — one changelog row per
  changed entity per transaction (SPEC §2.3).

## 2. Replay-safety (SPEC §2.4)

Clients apply your mutator optimistically and **re-apply** unconfirmed
mutations on every rebase; the server may also see the same envelope again
after a crash between execute and respond. Two consequences:

- **Server side:** you do NOT need application-level idempotency keys — the
  mutation ledger (§5 below) guarantees your `execute` runs at most once
  per `(clientGroupId, clientId, mutationId)`. What you must guarantee is
  that the outcome is a pure function of (args, current database state,
  ctx): no wall-clock branching, no randomness that changes the decision,
  no hidden inputs. If you need generated values (ids, timestamps), either
  take them from args (client-supplied, validated) or derive them
  deterministically inside the transaction; remember the recorded result is
  what a replayed envelope gets back.
- **Client side (for the paired client mutator):** the optimistic
  implementation must tolerate being rewound and re-applied any number of
  times against fresher confirmed state. Server outcome always wins.

## 3. In-band rejection discipline — never 4xx business validation

This is the load-bearing acceptance rule (SPEC §2.4, invariant 2 of the
push engine; behavior contract
`khala_sync.push.validation_never_blocks_queue.v1`):

> acceptance is synchronous with the transaction; **validation failures ack
> the mutation and report the error in-band** — they never 4xx/block the
> queue.

The client's push queue is FIFO with sequential `mutationId`s. An HTTP
error for one bad mutation would wedge every mutation behind it forever —
the PowerSync scar tissue this design imports. So:

- A failed permission check, failed validation, missing target row,
  conflict, quota, or any other **business** refusal is a **returned
  value**: `new MutationResult({ mutationId: ctx.mutationId, status:
  "rejected", errorCode, errorMessageSafe })`. It ACKs the mutation — the
  ledger records it, `lastMutationId` advances, the client dequeues it and
  surfaces the typed error.
- **Throwing is reserved for storage failures** (`KhalaSyncStorageError`
  and driver errors mapped to it) that abort the batch and tell the client
  to retry. A thrown error means "this push attempt failed", never "this
  mutation is invalid".
- **Reject BEFORE you write.** The engine commits the transaction even for
  a `rejected` result — the ledger row that acks the rejection must commit.
  A mutator that writes and then rejects would commit those writes. Order
  inside `execute` is therefore fixed: authorize → validate → only then
  write.
- `errorMessageSafe` must be safe to show and to log client-side: no raw
  argument echoes, no internal identifiers, no secrets. (The engine already
  refuses to echo decode errors for the same reason.) Error CODES are the
  contract; keep them stable, lower_snake_case, and typed on the client.
- The engine mints two rejection codes itself: `unknown_mutator` and
  `invalid_args` (a throwing `decodeArgs`). Both ack and are recorded —
  retrying identical bytes can never succeed, so blocking would be pure
  poison. `out_of_order` is the one rejection that acks NOTHING (no ledger
  row, watermark unchanged) so the client can re-push the missing prefix.
- HTTP status codes belong to the ROUTE for whole-request failures only:
  401 unauthenticated, 400 undecodable body / unsupported protocol or
  schema version, 403 client group bound to another user, 503 storage
  unavailable, 500 internal. If you find yourself wanting a per-mutation
  4xx, you are on the wrong side of the seam.

## 4. No session state through Hyperdrive (SPEC §4)

Worker request paths reach Cloud SQL through Hyperdrive in
**transaction-mode pooling**: consecutive statements from "your"
connection may run on different backend sessions, and anything
session-scoped silently breaks. The route's driver is configured
`prepare: false`, `max: 1` for exactly this reason. Inside a mutator this
means:

- No `LISTEN`/`NOTIFY`, no session `PREPARE`/named statements, no
  `SET`/`SET LOCAL` you depend on later, no temp tables, no cursors held
  across statements, no session advisory locks (`pg_advisory_lock`).
  Transaction-scoped advisory locks (`pg_advisory_xact_lock`) technically
  work but are banned by convention — serialize on **ordinary row locks**
  (`SELECT … FOR UPDATE`, or the implicit lock of an `UPDATE`), the same
  discipline the scope counter and client-state rows use.
- Everything you need must live inside the single transaction the engine
  opened. If a design seems to need cross-statement session state, redesign
  it as rows.
- Capture and migrations run on a DIRECT connection and may use
  LISTEN/NOTIFY — that is their seam, never the mutator path.

## 5. Idempotency via the ledger — what the engine already gives you

Per envelope, before your code runs, the engine (inside the same
transaction):

1. `upsertClientState` — takes the client group's row lock (serializing
   concurrent pushes for the group) and enforces the group⇄user binding.
2. `checkAndReserve` — the `khala_sync_mutations` ledger gate:
   - **duplicate** (`mutationId ≤ lastMutationId` with a recorded row):
     answers from the recording; your `execute` is NOT called; no changelog
     side effects. This covers crash-between-execute-and-respond replays
     (invariant 3).
   - **out_of_order** (`mutationId > lastMutationId + 1`): in-band
     rejection that acks nothing; the ledger stays dense.
   - **execute** (exactly `lastMutationId + 1`): your code runs, then
     `recordMutation` commits the result atomically with your writes.

Trust it. Do not build a second idempotency table, do not check "have I
seen this mutation" yourself, and do not catch storage errors to fabricate
a result — let them propagate so the batch aborts and the committed prefix
replays as duplicates.

## 6. Scope authorization inside the mutator

The route authenticates the CALLER (`ctx.userId` is trustworthy — it comes
from the Worker's actor auth, never the request body). It does not — and
cannot — know which scopes a given mutator may touch. **Authorization is
the mutator's job, before any write:**

- Derive or verify the target scope from `ctx.userId` and args. A mutator
  that writes the caller's own data must pin the scope
  (`personalScope(ctx.userId)`) or reject when `args.scope` differs — see
  `sync.debugEcho`'s guard for the canonical example. A team/fleet mutator
  must check membership with a query inside the transaction.
- An unauthorized scope is an in-band rejection (the Worker convention is
  `errorCode: "unauthorized_scope"`), not a throw.
- Remember invariant 9: raw private material (prompts, tokens, wallet
  data, local paths) never enters post-images for scopes broader than the
  owner. Scope choice is also a data-classification decision.

## 7. Post-images are canonical JSON

`appendChange` serializes `postImage` with `canonicalJson` from
`@openagentsinc/khala-sync` — sorted keys, deterministic bytes — so
byte-equality means value-equality across bootstrap/log/delta paths and in
tests. Rules:

- Pass post-images as plain **values** (objects), never pre-stringified
  JSON; never hand-`JSON.stringify`.
- The post-image is the entity's FULL state after the write (state-based
  replication, not diffs), containing only what every reader of that scope
  may see.
- `op: "delete"` entries are tombstones and must not carry a post-image —
  enforced at the type level, at runtime, and by a schema CHECK.
- Keep post-images bounded; large blobs ride elsewhere (R2 etc.) with refs
  in the post-image.

## 8. Registering in the Worker registry

The `openagents.com` Worker owns the production registry
(`apps/openagents.com/workers/api/src/khala-sync-mutators.ts`):

1. Define the mutator there (or in a sibling module) with
   `defineMutator<Args>({ name, decodeArgs, execute })`. Use Effect Schema
   for `decodeArgs` with explicit bounds (`isMinLength`/`isMaxLength`) on
   every string — decode failures become in-band `invalid_args` rejections,
   so tight schemas are free validation.
2. Add it to the array passed to `makeMutatorRegistry([...])`
   (`makeKhalaSyncMutatorRegistry` there). Duplicate names throw at
   registry construction, so collisions fail at boot, not at runtime.
3. Mutator names are `domain.verbCamel` (`sync.debugEcho`, `thing.set`)
   and are part of the wire contract — never rename or repurpose a shipped
   name; add `thing.setV2` instead. The registry has no versioning beyond
   names; removing a name turns in-flight client queues into recorded
   `unknown_mutator` rejections (acked, non-blocking — but user-visible).
4. If args change compatibly, widen the schema; if incompatibly, that is a
   new name plus a client `schemaVersion` bump gate in the route
   (`KHALA_SYNC_PUSH_SUPPORTED_SCHEMA_VERSIONS`).
5. Ship the paired client-side optimistic implementation under the same
   name in the client engine's mutator map (KS-5 lanes).

## 9. Testing checklist

Integration tests run `executePush` against **real local Postgres**
(`src/test/local-postgres.ts` spins up a throwaway instance; tests skip
cleanly when `postgresql@16` is absent). Follow
`packages/khala-sync-server/src/push-engine.test.ts` as the template. For
every new mutator, cover at least:

- [ ] **Applied flow** — business rows, changelog entries (right scope,
      entity type/id, canonical post-image bytes, `mutationRef ===
      ctx.mutationRef`), and ledger row all present after one push; dense
      per-scope versions.
- [ ] **Validation rejection** — the in-band `rejected` result with your
      typed `errorCode`; ledger row recorded; **no** business write, **no**
      changelog entry; `lastMutationId` advanced (the ack).
- [ ] **Queue never blocks** — a `[valid, invalid, valid]` batch yields
      `[applied, rejected, applied]` with `lastMutationId` past all three,
      and a subsequent push applies normally (this is the behavior-contract
      oracle shape).
- [ ] **Authorization** — a foreign/unauthorized scope rejects in-band
      before any write.
- [ ] **Duplicate replay** — re-pushing an executed envelope returns
      `duplicate` with the recorded outcome and does not re-execute
      (count executions in the test mutator).
- [ ] **Reject-before-write discipline** — if the mutator has multi-step
      writes, prove a rejection path leaves zero rows behind.
- [ ] **Bad args** — malformed `argsJson` yields `invalid_args` without
      echoing raw values in `errorMessageSafe`.
- [ ] **Route-level wiring** (Worker repo) — registry test asserting the
      name resolves and route tests with an injected fake SQL client if the
      route contract changed.

Run: `bun test packages/khala-sync-server` and
`bun run --cwd packages/khala-sync-server typecheck`.

## Behavior contract

The acceptance rule in §3 is registered as an enforced behavior contract:
`khala_sync.push.validation_never_blocks_queue.v1` in
`packages/behavior-contracts/src/khala-sync.ts`, with its bun-test oracle
in `packages/khala-sync-server/src/push-engine.test.ts` (the
`[valid, invalid, valid]` batch test). Do not weaken the engine's rejection
semantics without updating that contract in the same change.
