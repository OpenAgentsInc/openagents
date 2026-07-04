import {
  canonicalJson,
  type ClientGroupId,
  type ClientId,
  encodeMutationResult,
  KHALA_SYNC_PROTOCOL_VERSION,
  type MutationEnvelope,
  type MutationId,
  MutationResult,
  type MutatorName,
  type PushRequest,
  PushResponse,
} from "@openagentsinc/khala-sync"
import { KhalaSyncStorageError } from "./errors.js"
import {
  checkAndReserve,
  lastMutationId,
  recordMutation,
  upsertClientState,
} from "./mutation-ledger.js"
import type { SyncTransactionWriter } from "./outbox-writer.js"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"

/**
 * Push engine (KS-3.1; SPEC §2.4/§3, invariants 3 and 5).
 *
 * `executePush` runs one `PushRequest` batch for an authenticated user:
 * every mutation envelope executes in its OWN single Postgres transaction —
 * client-state upsert (row lock + user binding) → idempotency/ordering gate
 * (`checkAndReserve`) → mutator execution with a transaction-scoped
 * {@link MutatorContext} → mutation-ledger recording — all atomic. Results
 * come back in request order; rejections are IN-BAND `MutationResult`
 * values that ack the mutation (except `out_of_order`, which acks nothing
 * per the ledger contract) and never block the client queue.
 *
 * Batch-aborting failures are typed and thrown: a client-group/user
 * mismatch throws `KhalaSyncClientStateMismatchError` (403-class — the
 * route maps it to a whole-request `SyncError`), and storage failures
 * throw `KhalaSyncStorageError` (retryable — the already-committed prefix
 * of the batch stays committed and replays as duplicates).
 */

// ---------------------------------------------------------------------------
// Mutator registry
// ---------------------------------------------------------------------------

/**
 * A named, server-authoritative mutator. `execute` runs inside ONE Postgres
 * transaction and must perform: permission check, validation, business
 * writes (via `ctx.writer.sql`), and changelog appends (via
 * `ctx.writer.appendChange`), returning the per-mutation result. Rejections
 * are VALUES (`status: "rejected"`, never thrown queue poison); throwing is
 * reserved for storage failures that abort the batch.
 *
 * Rejection discipline: the engine commits the transaction even for a
 * `rejected` result (the ledger row that acks the rejection must commit),
 * so mutators MUST validate and permission-check BEFORE issuing any
 * business write or changelog append. A mutator that writes and then
 * rejects would commit those writes.
 *
 * Execution is Promise-based at this substrate seam (driver transactions
 * are Promise-scoped); Effect wrapping happens above the transaction
 * boundary.
 */
export interface MutatorContext {
  readonly userId: string
  readonly clientGroupId: ClientGroupId
  readonly clientId: ClientId
  /** The executing envelope's mutation id (echo it in the result). */
  readonly mutationId: MutationId
  /**
   * Canonical mutation ref for this envelope — pass it as `mutationRef` on
   * every `appendChange` so changelog entries stay attributable (SPEC §7
   * invariant 3).
   */
  readonly mutationRef: string
  /** Transaction-scoped changelog writer + business-write SQL handle. */
  readonly writer: SyncTransactionWriter
}

export interface MutatorDefinition<Args = unknown> {
  readonly name: MutatorName
  /** Decode/validate `argsJson`. Throwing ⇒ in-band `invalid_args` rejection. */
  readonly decodeArgs: (argsJson: string) => Args
  readonly execute: (args: Args, ctx: MutatorContext) => Promise<MutationResult>
}

/**
 * Type-erase a mutator definition for registry storage. Safe by
 * construction: `decodeArgs` is the only producer of the `Args` value
 * `execute` consumes.
 */
export const defineMutator = <Args>(
  definition: MutatorDefinition<Args>,
): MutatorDefinition => definition as MutatorDefinition

export interface MutatorRegistry {
  readonly get: (name: MutatorName) => MutatorDefinition | undefined
  readonly names: () => ReadonlyArray<MutatorName>
}

export const makeMutatorRegistry = (
  mutators: ReadonlyArray<MutatorDefinition>,
): MutatorRegistry => {
  const byName = new Map<string, MutatorDefinition>(
    mutators.map((m) => [String(m.name), m]),
  )
  if (byName.size !== mutators.length) {
    throw new Error("duplicate mutator name in registry")
  }
  return {
    get: (name) => byName.get(String(name)),
    names: () => mutators.map((m) => m.name),
  }
}

// ---------------------------------------------------------------------------
// In-band rejection codes minted by the engine itself
// ---------------------------------------------------------------------------

export const UNKNOWN_MUTATOR_ERROR_CODE = "unknown_mutator"
export const INVALID_ARGS_ERROR_CODE = "invalid_args"

/** Canonical mutation ref recorded on changelog entries and ledger rows. */
export const mutationRefFor = (
  clientGroupId: ClientGroupId,
  clientId: ClientId,
  mutationId: MutationId,
): string => `mutation:${clientGroupId}:${clientId}:${mutationId}`

// ---------------------------------------------------------------------------
// executePush
// ---------------------------------------------------------------------------

export interface ExecutePushInput {
  /** Root SQL handle (Bun `SQL` or postgres.js via Hyperdrive — see ./sql.ts). */
  readonly sql: SyncSql
  readonly registry: MutatorRegistry
  /** The AUTHENTICATED user — resolved by the route, never from the request body. */
  readonly userId: string
  /** Already decoded + version-gated by the route. */
  readonly request: PushRequest
}

export const executePush = async (
  input: ExecutePushInput,
): Promise<PushResponse> => {
  const { registry, request, sql, userId } = input
  const { clientGroupId, clientId, schemaVersion } = request

  if (request.mutations.length === 0) {
    // Still bind/refresh the client group (user-binding mismatch throws the
    // typed 403-class error even for an empty push).
    await upsertClientState(sql, { clientGroupId, schemaVersion, userId })
  }

  const results: Array<MutationResult> = []
  for (const envelope of request.mutations) {
    // ONE transaction per envelope: upsertClientState takes the client
    // group's row lock FIRST (per-group serialization + user binding), then
    // checkAndReserve gates, then the mutator + ledger recording commit
    // atomically with the changelog appends. A storage failure aborts the
    // batch here; committed prefix results stay committed.
    const result = await withSyncTransaction(sql, async (writer) => {
      await upsertClientState(writer.sql, {
        clientGroupId,
        schemaVersion,
        userId,
      })
      const outcome = await checkAndReserve(writer.sql, {
        clientGroupId,
        clientId,
        envelope,
      })
      if (outcome.kind !== "execute") {
        // duplicate: recorded result, nothing executes.
        // out_of_order: in-band rejection, NO ledger write (acks nothing —
        // the client re-pushes the missing prefix and the gap heals).
        return outcome.result
      }
      return executeEnvelope(writer, registry, userId, {
        clientGroupId,
        clientId,
        envelope,
      })
    })
    results.push(result)
  }

  const last = await lastMutationId(sql, { clientGroupId, clientId })
  return new PushResponse({
    lastMutationId: last,
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    results,
  })
}

// ---------------------------------------------------------------------------
// Envelope execution (inside the transaction, after checkAndReserve)
// ---------------------------------------------------------------------------

const executeEnvelope = async (
  writer: SyncTransactionWriter,
  registry: MutatorRegistry,
  userId: string,
  input: {
    readonly clientGroupId: ClientGroupId
    readonly clientId: ClientId
    readonly envelope: MutationEnvelope
  },
): Promise<MutationResult> => {
  const { clientGroupId, clientId, envelope } = input

  const record = async (result: MutationResult): Promise<MutationResult> => {
    await recordMutation(writer.sql, {
      clientGroupId,
      clientId,
      mutationId: envelope.mutationId,
      name: envelope.name,
      status: result.status,
      ...(result.errorCode === undefined ? {} : { errorCode: result.errorCode }),
      resultJson: canonicalJson(encodeMutationResult(result)),
    })
    return result
  }

  const definition = registry.get(envelope.name)
  if (definition === undefined) {
    // Unknown mutator: in-band rejection, recorded in the ledger (it ACKS
    // the mutation — retrying an unknown name can never succeed).
    return record(
      new MutationResult({
        errorCode: UNKNOWN_MUTATOR_ERROR_CODE,
        errorMessageSafe: `unknown mutator: ${envelope.name}`,
        mutationId: envelope.mutationId,
        status: "rejected",
      }),
    )
  }

  let args: unknown
  try {
    args = definition.decodeArgs(envelope.argsJson)
  } catch {
    // Bad args: in-band rejection, recorded (acked — a byte-identical retry
    // can never decode). The decode error itself is NOT echoed: it can
    // embed raw argument values, which must not leave the server.
    return record(
      new MutationResult({
        errorCode: INVALID_ARGS_ERROR_CODE,
        errorMessageSafe:
          `args failed to decode for mutator ${envelope.name}`,
        mutationId: envelope.mutationId,
        status: "rejected",
      }),
    )
  }

  const result = await definition.execute(args, {
    clientGroupId,
    clientId,
    mutationId: envelope.mutationId,
    mutationRef: mutationRefFor(clientGroupId, clientId, envelope.mutationId),
    userId,
    writer,
  })

  if (
    Number(result.mutationId) !== Number(envelope.mutationId) ||
    result.status === "duplicate"
  ) {
    // A mutator can only report applied/rejected for ITS OWN envelope;
    // anything else is an engine-contract bug — abort the batch (rollback)
    // rather than record a lie.
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `mutator ${envelope.name} returned an invalid result ` +
        `(status ${result.status}, mutationId ${result.mutationId} ` +
        `for envelope ${envelope.mutationId})`,
    )
  }

  return record(result)
}
