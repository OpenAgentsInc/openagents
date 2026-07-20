import { createHash } from "node:crypto"
import { chmod, lstat, mkdir } from "node:fs/promises"
import { dirname, isAbsolute } from "node:path"

import {
  PortableAgentGraphSchema,
  PortablePhaseOperationRequestSchema,
  PortableSessionExecutionBindingSchema,
  PylonPortableCheckpointBundleSchema,
  PylonPortableThreadCursorSchema,
  type PortablePhaseOperationRequest,
} from "@openagentsinc/portable-session-contract"
import {
  openLegacySqliteDatabase,
  type LegacySqliteDatabase,
} from "@openagentsinc/sqlite-runtime"
import { Schema } from "effect"

import type { PylonPortablePhaseTargetResolver } from "./portable-phase-operation-worker.js"
import type { PylonOwnerLocalExecutionTarget } from "./portable-session-target.js"

export const PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA =
  "openagents.pylon.portable_phase_context_admission.v1" as const

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const PositiveInt = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0))
const NonNegativeInt = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))
const Refs = Schema.Array(Ref).check(Schema.isMaxLength(256))

const commonSource = {
  operationRef: Ref,
  sessionRef: Ref,
  attachmentRef: Ref,
  generation: PositiveInt,
}
const commonDestination = {
  operationRef: Ref,
  sessionRef: Ref,
  destinationAttachmentRef: Ref,
  destinationGeneration: PositiveInt,
}

export const PylonPortablePhasePrivatePayloadSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("quiesce"),
    input: Schema.Struct({
      ...commonSource,
      graph: PortableAgentGraphSchema,
      threadCursors: Schema.Array(PylonPortableThreadCursorSchema),
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("checkpoint-create"),
    checkpointObjectRef: Ref,
    input: Schema.Struct({
      ...commonSource,
      checkpointRef: Ref,
      eventLogCursor: NonNegativeInt,
      executionBinding: PortableSessionExecutionBindingSchema,
      graph: PortableAgentGraphSchema,
      threadCursors: Schema.Array(PylonPortableThreadCursorSchema),
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("source-cleanup"),
    input: Schema.Struct({ ...commonSource, agentRefs: Refs }),
  }),
  Schema.Struct({
    kind: Schema.Literal("checkpoint-stage"),
    input: Schema.Struct({
      operationRef: Ref,
      bundle: PylonPortableCheckpointBundleSchema,
      destinationAttachmentRef: Ref,
      destinationGeneration: PositiveInt,
      capabilityLeaseRefs: Refs,
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("destination-activate"),
    input: Schema.Struct({
      ...commonDestination,
      checkpointRef: Ref,
      executionBinding: PortableSessionExecutionBindingSchema,
      capabilityLeaseRefs: Refs,
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("staged-abort"),
    input: Schema.Struct(commonDestination),
  }),
])
export type PylonPortablePhasePrivatePayload =
  typeof PylonPortablePhasePrivatePayloadSchema.Type

export const PylonPortablePhaseContextAdmissionInputSchema = Schema.Struct({
  schema: Schema.Literal(PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA),
  request: PortablePhaseOperationRequestSchema,
  payload: PylonPortablePhasePrivatePayloadSchema,
  recoverySemantics: Schema.optionalKey(
    Schema.Literals(["not_proven", "operation_ref_idempotent"]),
  ),
})
export type PylonPortablePhaseContextAdmissionInput =
  typeof PylonPortablePhaseContextAdmissionInputSchema.Type

type AdmissionRow = {
  operation_ref: string
  schema: string
  request_json: string
  request_fingerprint: string
  context_json: string
  context_digest: string
  recovery_semantics: "not_proven" | "operation_ref_idempotent"
  admitted_at: string
  expires_at: string
  state: "admitted" | "terminal_acknowledged"
  terminal_acknowledged_at: string | null
}

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`
  }
  return JSON.stringify(value)
}

const digest = (bytes: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`

export class PylonPortablePhaseContextAdmissionError extends Error {
  override readonly name = "PylonPortablePhaseContextAdmissionError"

  constructor(
    readonly reason:
      | "conflicting_replay"
      | "corrupt_admission"
      | "expired"
      | "invalid_admission"
      | "invalid_store",
  ) {
    super(`Pylon private phase context admission failed closed: ${reason}`)
  }
}

const decodeExact = <A>(schema: Schema.Decoder<A>, value: unknown): A => {
  try {
    return Schema.decodeUnknownSync(schema)(value, { onExcessProperty: "error" })
  } catch {
    throw new PylonPortablePhaseContextAdmissionError("invalid_admission")
  }
}

const assertBindings = (
  request: PortablePhaseOperationRequest,
  payload: PylonPortablePhasePrivatePayload,
): void => {
  const input = payload.input
  const sessionRef = "sessionRef" in input ? input.sessionRef : input.bundle.checkpoint.sessionRef
  const attachmentRef =
    "attachmentRef" in input ? input.attachmentRef : input.destinationAttachmentRef
  const generation = "generation" in input ? input.generation : input.destinationGeneration
  if (
    payload.kind !== request.kind ||
    input.operationRef !== request.operationRef ||
    sessionRef !== request.sessionRef ||
    attachmentRef !== request.attachmentRef ||
    generation !== request.attachmentGeneration
  ) {
    throw new PylonPortablePhaseContextAdmissionError("invalid_admission")
  }
  if (payload.kind === "checkpoint-create") {
    if (
      payload.input.checkpointRef !== request.checkpointRef ||
      payload.checkpointObjectRef !== request.checkpointObjectRef ||
      payload.input.executionBinding.ownerRef !== request.ownerRef
    ) throw new PylonPortablePhaseContextAdmissionError("invalid_admission")
  } else if (payload.kind === "checkpoint-stage") {
    if (
      payload.input.bundle.checkpoint.checkpointRef !== request.checkpointRef ||
      payload.input.bundle.checkpoint.digest !== request.checkpointDigest ||
      payload.input.bundle.executionBinding.ownerRef !== request.ownerRef
    ) throw new PylonPortablePhaseContextAdmissionError("invalid_admission")
  } else if (payload.kind === "destination-activate") {
    if (
      payload.input.checkpointRef !== request.checkpointRef ||
      payload.input.executionBinding.ownerRef !== request.ownerRef
    ) throw new PylonPortablePhaseContextAdmissionError("invalid_admission")
  }
}

export type PylonPortablePhaseContextAdmissionRecord = Readonly<{
  schema: typeof PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA
  operationRef: string
  requestFingerprint: `sha256:${string}`
  contextDigest: `sha256:${string}`
  admittedAt: string
  expiresAt: string
  state: "admitted" | "terminal_acknowledged"
  recoverySemantics: "not_proven" | "operation_ref_idempotent"
}>

export class PylonPortablePhaseContextAdmissionStore {
  constructor(
    private readonly database: LegacySqliteDatabase,
    private readonly now: () => Date = () => new Date(),
    private readonly scope?: Readonly<{ pylonRef: string; targetRef: string }>,
  ) {
    this.database.exec(`
      PRAGMA foreign_keys = ON;
      CREATE TABLE IF NOT EXISTS pylon_portable_phase_context_admissions (
        operation_ref TEXT PRIMARY KEY,
        schema TEXT NOT NULL,
        request_json TEXT NOT NULL,
        request_fingerprint TEXT NOT NULL,
        context_json TEXT NOT NULL,
        context_digest TEXT NOT NULL,
        recovery_semantics TEXT NOT NULL CHECK (recovery_semantics IN ('not_proven', 'operation_ref_idempotent')),
        admitted_at TEXT NOT NULL,
        expires_at TEXT NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('admitted', 'terminal_acknowledged')),
        terminal_acknowledged_at TEXT
      );
    `)
  }

  private readRow(operationRef: string): AdmissionRow | null {
    return this.database.query<AdmissionRow, [string]>(`
      SELECT operation_ref, schema, request_json, request_fingerprint, context_json,
             context_digest, recovery_semantics, admitted_at, expires_at, state,
             terminal_acknowledged_at
        FROM pylon_portable_phase_context_admissions
       WHERE operation_ref = ?
    `).get(operationRef)
  }

  admit(value: unknown): PylonPortablePhaseContextAdmissionRecord {
    return this.database.transaction((): PylonPortablePhaseContextAdmissionRecord => {
      const input = decodeExact(PylonPortablePhaseContextAdmissionInputSchema, value)
      if (
        this.scope !== undefined &&
        (input.request.pylonRef !== this.scope.pylonRef ||
          input.request.targetRef !== this.scope.targetRef)
      ) throw new PylonPortablePhaseContextAdmissionError("invalid_admission")
      assertBindings(input.request, input.payload)
      const requestJson = canonical(input.request)
      const contextJson = canonical(input.payload)
      const requestFingerprint = digest(requestJson)
      const contextDigest = digest(contextJson)
      const recoverySemantics = input.recoverySemantics ?? "not_proven"
      const existing = this.readRow(input.request.operationRef)
      if (existing !== null) {
        if (
          existing.request_json !== requestJson ||
          existing.context_json !== contextJson ||
          existing.recovery_semantics !== recoverySemantics ||
          existing.expires_at !== input.request.expiresAt
        ) throw new PylonPortablePhaseContextAdmissionError("conflicting_replay")
        return this.decodeRow(existing).record
      }
      const admittedAt = this.now().toISOString()
      this.database.query(`
        INSERT INTO pylon_portable_phase_context_admissions
          (operation_ref, schema, request_json, request_fingerprint, context_json,
           context_digest, recovery_semantics, admitted_at, expires_at, state,
           terminal_acknowledged_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'admitted', NULL)
      `).run(
        input.request.operationRef,
        PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA,
        requestJson,
        requestFingerprint,
        contextJson,
        contextDigest,
        recoverySemantics,
        admittedAt,
        input.request.expiresAt,
      )
      return {
        schema: PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA,
        operationRef: input.request.operationRef,
        requestFingerprint,
        contextDigest,
        admittedAt,
        expiresAt: input.request.expiresAt,
        state: "admitted",
        recoverySemantics,
      }
    }).immediate()
  }

  private decodeRow(row: AdmissionRow): Readonly<{
    request: PortablePhaseOperationRequest
    payload: PylonPortablePhasePrivatePayload
    record: PylonPortablePhaseContextAdmissionRecord
  }> {
    try {
      const requestUnknown: unknown = JSON.parse(row.request_json)
      const payloadUnknown: unknown = JSON.parse(row.context_json)
      const request = decodeExact(PortablePhaseOperationRequestSchema, requestUnknown)
      const payload = decodeExact(PylonPortablePhasePrivatePayloadSchema, payloadUnknown)
      assertBindings(request, payload)
      if (
        canonical(request) !== row.request_json ||
        canonical(payload) !== row.context_json ||
        digest(row.request_json) !== row.request_fingerprint ||
        digest(row.context_json) !== row.context_digest ||
        row.schema !== PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA ||
        request.operationRef !== row.operation_ref ||
        request.expiresAt !== row.expires_at
      ) throw new Error("durable bytes drift")
      return {
        request,
        payload,
        record: {
          schema: PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA,
          operationRef: row.operation_ref,
          requestFingerprint: row.request_fingerprint as `sha256:${string}`,
          contextDigest: row.context_digest as `sha256:${string}`,
          admittedAt: row.admitted_at,
          expiresAt: row.expires_at,
          state: row.state,
          recoverySemantics: row.recovery_semantics,
        },
      }
    } catch {
      throw new PylonPortablePhaseContextAdmissionError("corrupt_admission")
    }
  }

  resolve(operationRef: string): ReturnType<PylonPortablePhaseContextAdmissionStore["decodeRow"]> | undefined {
    const row = this.readRow(operationRef)
    if (row === null || row.state !== "admitted") return undefined
    const decoded = this.decodeRow(row)
    if (Date.parse(decoded.record.expiresAt) <= this.now().getTime()) return undefined
    return decoded
  }

  acknowledgeTerminal(operationRef: string): void {
    this.database.transaction(() => {
      const row = this.readRow(operationRef)
      if (row === null) return
      this.decodeRow(row)
      this.database.query(`
        UPDATE pylon_portable_phase_context_admissions
           SET state = 'terminal_acknowledged', terminal_acknowledged_at = ?
         WHERE operation_ref = ? AND state = 'admitted'
      `).run(this.now().toISOString(), operationRef)
    }).immediate()
  }

  purge(): number {
    return this.database.transaction(() => Number(this.database.query(`
      DELETE FROM pylon_portable_phase_context_admissions
       WHERE state = 'terminal_acknowledged' OR expires_at <= ?
    `).run(this.now().toISOString()).changes)).immediate()
  }
}

export type PylonPortablePhaseTargetLookup = (
  targetRef: string,
) => PylonOwnerLocalExecutionTarget | undefined

export const makeDurablePylonPortablePhaseTargetResolver = (input: Readonly<{
  store: PylonPortablePhaseContextAdmissionStore
  target: PylonPortablePhaseTargetLookup
}>): PylonPortablePhaseTargetResolver => ({
  resolve: async request => {
    const admission = input.store.resolve(request.operationRef)
    if (
      admission === undefined ||
      canonical(admission.request) !== canonical(request)
    ) return undefined
    const target = input.target(request.targetRef)
    if (target === undefined || target.targetRef !== request.targetRef) return undefined
    return {
      target,
      call: admission.payload,
      operationRefSemantics: admission.record.recoverySemantics,
    }
  },
})

export const openPylonPortablePhaseContextAdmissionStore = async (input: Readonly<{
  databasePath: string
  now?: () => Date
  pylonRef?: string
  targetRef?: string
}>): Promise<Readonly<{
  database: LegacySqliteDatabase
  store: PylonPortablePhaseContextAdmissionStore
  close: () => void
}>> => {
  if (!isAbsolute(input.databasePath)) {
    throw new PylonPortablePhaseContextAdmissionError("invalid_store")
  }
  const directory = dirname(input.databasePath)
  await mkdir(directory, { recursive: true, mode: 0o700 })
  const info = await lstat(directory)
  if (!info.isDirectory() || info.isSymbolicLink() || (info.mode & 0o077) !== 0) {
    throw new PylonPortablePhaseContextAdmissionError("invalid_store")
  }
  const database = openLegacySqliteDatabase(input.databasePath)
  try {
    await chmod(input.databasePath, 0o600)
    if ((input.pylonRef === undefined) !== (input.targetRef === undefined)) {
      throw new PylonPortablePhaseContextAdmissionError("invalid_store")
    }
    const store = new PylonPortablePhaseContextAdmissionStore(
      database,
      input.now,
      input.pylonRef === undefined || input.targetRef === undefined
        ? undefined
        : { pylonRef: input.pylonRef, targetRef: input.targetRef },
    )
    return { database, store, close: () => database.close() }
  } catch (error) {
    database.close()
    throw error
  }
}
