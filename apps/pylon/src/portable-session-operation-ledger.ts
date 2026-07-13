import { createHash } from "node:crypto"
import type { Database } from "bun:sqlite"
import { Effect, Schema } from "effect"
import {
  PortableAgentGraphSchema,
  PortableCheckpointSchema,
  PortableSessionExecutionBindingSchema,
} from "@openagentsinc/portable-session-contract"

export const PYLON_PORTABLE_OPERATION_LEDGER_VERSION =
  "openagents.pylon.portable_operation_ledger.v1" as const

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|secret|localPath|hostname|processId|authHome|socket|port)"\s*:/iu

export type PylonPortableOperationKind =
  | "quiesce"
  | "checkpoint"
  | "cleanup"
  | "stage"
  | "activate"
  | "abort"

export type PylonPortableOperationOutcome = Readonly<{
  evidenceRefs: ReadonlyArray<string>
  checkpointRef?: string
  repositoryPostImageDigest?: string
  diffDigest?: string
  graphDigest?: string
  cleanupReceiptRef?: string
}>

const PylonPortableOperationOutcomeSchema = Schema.Struct({
  evidenceRefs: Schema.Array(Schema.String),
  checkpointRef: Schema.optionalKey(Schema.String),
  repositoryPostImageDigest: Schema.optionalKey(Schema.String),
  diffDigest: Schema.optionalKey(Schema.String),
  graphDigest: Schema.optionalKey(Schema.String),
  cleanupReceiptRef: Schema.optionalKey(Schema.String),
})

const PylonPortableThreadCursorSchema = Schema.Struct({
  threadRef: Schema.String,
  transcriptRef: Schema.String,
  activityCursor: Schema.Number,
  eventCursor: Schema.Number,
})

export const PylonPortableCheckpointBundleSchema = Schema.Struct({
  checkpoint: PortableCheckpointSchema,
  executionBinding: PortableSessionExecutionBindingSchema,
  graph: PortableAgentGraphSchema,
  threadCursors: Schema.Array(PylonPortableThreadCursorSchema),
})
export type PylonPortableCheckpointBundle = typeof PylonPortableCheckpointBundleSchema.Type

export type PylonPortableSessionFence = Readonly<{
  schema: typeof PYLON_PORTABLE_OPERATION_LEDGER_VERSION
  sessionRef: string
  attachmentRef: string
  generation: number
  acceptingWork: boolean
  revision: number
}>

export type PylonPortableOperationRecord = Readonly<{
  schema: typeof PYLON_PORTABLE_OPERATION_LEDGER_VERSION
  operationRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  kind: PylonPortableOperationKind
  state: "admitted" | "completed"
  outcome?: PylonPortableOperationOutcome
}>

export class PylonPortableOperationLedgerError extends Error {
  readonly _tag = "PylonPortableOperationLedgerError"
  override readonly name = "PylonPortableOperationLedgerError"

  constructor(
    readonly reason:
      | "conflicting_replay"
      | "invalid_scope"
      | "not_found"
      | "stale_generation"
      | "unsafe_result",
    message: string,
  ) {
    super(message)
  }
}

type SessionRow = {
  session_ref: string
  attachment_ref: string
  generation: number
  accepting_work: number
  revision: number
}

type OperationRow = {
  operation_ref: string
  session_ref: string
  attachment_ref: string
  generation: number
  kind: PylonPortableOperationKind
  fingerprint: string
  state: "admitted" | "completed"
  outcome_json: string | null
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

const fingerprint = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonical(value)).digest("hex")}`

const assertRef = (value: string, field: string): void => {
  if (!SAFE_REF.test(value)) {
    throw new PylonPortableOperationLedgerError("invalid_scope", `${field} is not a public-safe ref`)
  }
}

const assertGeneration = (value: number): void => {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new PylonPortableOperationLedgerError("invalid_scope", "generation is invalid")
  }
}

const decodeOutcome = (value: string): PylonPortableOperationOutcome => {
  let outcome: PylonPortableOperationOutcome
  try {
    outcome = Schema.decodeUnknownSync(PylonPortableOperationOutcomeSchema)(JSON.parse(value))
  } catch {
    throw new PylonPortableOperationLedgerError("unsafe_result", "portable operation outcome is invalid")
  }
  const serialized = canonical(outcome)
  const refs = [
    ...outcome.evidenceRefs,
    outcome.checkpointRef,
    outcome.repositoryPostImageDigest,
    outcome.diffDigest,
    outcome.graphDigest,
    outcome.cleanupReceiptRef,
  ].filter((item): item is string => item !== undefined)
  if (FORBIDDEN_PRIVATE_MATERIAL.test(serialized) ||
      refs.some(ref => !SAFE_REF.test(ref)) ||
      new Set(outcome.evidenceRefs).size !== outcome.evidenceRefs.length) {
    throw new PylonPortableOperationLedgerError("unsafe_result", "portable operation outcome is not refs-only")
  }
  return outcome
}

const sessionFence = (row: SessionRow): PylonPortableSessionFence => ({
  schema: PYLON_PORTABLE_OPERATION_LEDGER_VERSION,
  sessionRef: row.session_ref,
  attachmentRef: row.attachment_ref,
  generation: Number(row.generation),
  acceptingWork: Number(row.accepting_work) === 1,
  revision: Number(row.revision),
})

const operationRecord = (row: OperationRow): PylonPortableOperationRecord => ({
  schema: PYLON_PORTABLE_OPERATION_LEDGER_VERSION,
  operationRef: row.operation_ref,
  sessionRef: row.session_ref,
  attachmentRef: row.attachment_ref,
  generation: Number(row.generation),
  kind: row.kind,
  state: row.state,
  ...(row.outcome_json === null ? {} : { outcome: decodeOutcome(row.outcome_json) }),
})

export class PylonPortableSessionOperationLedger {
  constructor(private readonly database: Database) {
    database.exec("PRAGMA foreign_keys = ON")
    database.exec("PRAGMA journal_mode = WAL")
    database.exec(`
      CREATE TABLE IF NOT EXISTS pylon_portable_session_fences (
        session_ref TEXT PRIMARY KEY,
        attachment_ref TEXT NOT NULL,
        generation INTEGER NOT NULL CHECK (generation >= 0),
        accepting_work INTEGER NOT NULL CHECK (accepting_work IN (0, 1)),
        revision INTEGER NOT NULL CHECK (revision >= 0)
      );
      CREATE TABLE IF NOT EXISTS pylon_portable_session_operations (
        operation_ref TEXT PRIMARY KEY,
        session_ref TEXT NOT NULL,
        attachment_ref TEXT NOT NULL,
        generation INTEGER NOT NULL CHECK (generation >= 0),
        kind TEXT NOT NULL CHECK (kind IN ('quiesce', 'checkpoint', 'cleanup', 'stage', 'activate', 'abort')),
        fingerprint TEXT NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('admitted', 'completed')),
        outcome_json TEXT,
        FOREIGN KEY (session_ref) REFERENCES pylon_portable_session_fences(session_ref) ON DELETE CASCADE
      );
      CREATE INDEX IF NOT EXISTS pylon_portable_session_operations_scope
        ON pylon_portable_session_operations(session_ref, generation, kind);
      CREATE TABLE IF NOT EXISTS pylon_portable_checkpoint_bundles (
        operation_ref TEXT PRIMARY KEY,
        bundle_json TEXT NOT NULL,
        FOREIGN KEY (operation_ref) REFERENCES pylon_portable_session_operations(operation_ref) ON DELETE CASCADE
      );
    `)
  }

  registerSession(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    acceptingWork: boolean
  }>): Effect.Effect<PylonPortableSessionFence, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      this.assertScope(input)
      const row = this.readSessionRow(input.sessionRef)
      if (row !== null) {
        if (row.attachment_ref !== input.attachmentRef ||
            Number(row.generation) !== input.generation ||
            (Number(row.accepting_work) === 1) !== input.acceptingWork) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "portable session registration conflicts")
        }
        return sessionFence(row)
      }
      this.database.query(`
        INSERT INTO pylon_portable_session_fences
          (session_ref, attachment_ref, generation, accepting_work, revision)
        VALUES (?, ?, ?, ?, 0)
      `).run(input.sessionRef, input.attachmentRef, input.generation, input.acceptingWork ? 1 : 0)
      return sessionFence(this.requireSession(input.sessionRef))
    })
  }

  readSession(sessionRef: string): Effect.Effect<PylonPortableSessionFence, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      assertRef(sessionRef, "sessionRef")
      return sessionFence(this.requireSession(sessionRef))
    })
  }

  admitOperation(input: Readonly<{
    operationRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    kind: PylonPortableOperationKind
  }>): Effect.Effect<Readonly<{ status: "admitted" | "replayed"; record: PylonPortableOperationRecord }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.admitOperationSync(input))
  }

  completeOperation(input: Readonly<{
    operationRef: string
    outcome: PylonPortableOperationOutcome
  }>): Effect.Effect<Readonly<{ status: "completed" | "replayed"; record: PylonPortableOperationRecord }>, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      assertRef(input.operationRef, "operationRef")
      const outcome = decodeOutcome(JSON.stringify(input.outcome))
      const row = this.requireOperation(input.operationRef)
      if (row.state === "completed") {
        if (canonical(decodeOutcome(row.outcome_json!)) !== canonical(outcome)) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "portable operation outcome conflicts")
        }
        return { status: "replayed" as const, record: operationRecord(row) }
      }
      this.database.query(`
        UPDATE pylon_portable_session_operations
        SET state = 'completed', outcome_json = ?
        WHERE operation_ref = ? AND state = 'admitted'
      `).run(JSON.stringify(outcome), input.operationRef)
      return { status: "completed" as const, record: operationRecord(this.requireOperation(input.operationRef)) }
    })
  }

  storeCheckpointBundle(input: Readonly<{
    operationRef: string
    bundle: PylonPortableCheckpointBundle
  }>): Effect.Effect<Readonly<{ status: "stored" | "replayed"; bundle: PylonPortableCheckpointBundle }>, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      assertRef(input.operationRef, "operationRef")
      const operation = this.requireOperation(input.operationRef)
      if (operation.kind !== "checkpoint") {
        throw new PylonPortableOperationLedgerError("invalid_scope", "checkpoint bundle requires a checkpoint operation")
      }
      let bundle: PylonPortableCheckpointBundle
      try {
        bundle = Schema.decodeUnknownSync(PylonPortableCheckpointBundleSchema)(input.bundle)
      } catch {
        throw new PylonPortableOperationLedgerError("unsafe_result", "portable checkpoint bundle is invalid")
      }
      const serialized = canonical(bundle)
      if (FORBIDDEN_PRIVATE_MATERIAL.test(serialized)) {
        throw new PylonPortableOperationLedgerError("unsafe_result", "portable checkpoint bundle contains private material")
      }
      const existing = this.database.query(
        "SELECT bundle_json FROM pylon_portable_checkpoint_bundles WHERE operation_ref = ?",
      ).get(input.operationRef) as { bundle_json: string } | null
      if (existing !== null) {
        const stored = Schema.decodeUnknownSync(PylonPortableCheckpointBundleSchema)(JSON.parse(existing.bundle_json))
        if (canonical(stored) !== serialized) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "portable checkpoint bundle conflicts")
        }
        return { status: "replayed" as const, bundle: stored }
      }
      this.database.query(
        "INSERT INTO pylon_portable_checkpoint_bundles (operation_ref, bundle_json) VALUES (?, ?)",
      ).run(input.operationRef, JSON.stringify(bundle))
      return { status: "stored" as const, bundle }
    })
  }

  readCheckpointBundle(operationRef: string): Effect.Effect<PylonPortableCheckpointBundle, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      assertRef(operationRef, "operationRef")
      const row = this.database.query(
        "SELECT bundle_json FROM pylon_portable_checkpoint_bundles WHERE operation_ref = ?",
      ).get(operationRef) as { bundle_json: string } | null
      if (row === null) throw new PylonPortableOperationLedgerError("not_found", "portable checkpoint bundle is absent")
      try {
        return Schema.decodeUnknownSync(PylonPortableCheckpointBundleSchema)(JSON.parse(row.bundle_json))
      } catch {
        throw new PylonPortableOperationLedgerError("unsafe_result", "stored portable checkpoint bundle is invalid")
      }
    })
  }

  quiesceGeneration(input: Readonly<{
    operationRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    evidenceRefs: ReadonlyArray<string>
  }>): Effect.Effect<Readonly<{ status: "completed" | "replayed"; fence: PylonPortableSessionFence; record: PylonPortableOperationRecord }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() => {
      const admitted = this.admitOperationSync({ ...input, kind: "quiesce" })
      const outcome = decodeOutcome(JSON.stringify({ evidenceRefs: input.evidenceRefs }))
      if (admitted.record.state === "completed") {
        if (canonical(admitted.record.outcome) !== canonical(outcome)) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "quiesce outcome conflicts")
        }
        return {
          status: "replayed" as const,
          fence: sessionFence(this.requireSession(input.sessionRef)),
          record: admitted.record,
        }
      }
      const fence = this.requireExactFence(input)
      if (Number(fence.accepting_work) === 1) {
        this.database.query(`
          UPDATE pylon_portable_session_fences
          SET accepting_work = 0, revision = revision + 1
          WHERE session_ref = ? AND attachment_ref = ? AND generation = ?
        `).run(input.sessionRef, input.attachmentRef, input.generation)
      }
      this.database.query(`
        UPDATE pylon_portable_session_operations
        SET state = 'completed', outcome_json = ?
        WHERE operation_ref = ?
      `).run(JSON.stringify(outcome), input.operationRef)
      return {
        status: "completed" as const,
        fence: sessionFence(this.requireSession(input.sessionRef)),
        record: operationRecord(this.requireOperation(input.operationRef)),
      }
    }).immediate())
  }

  activateGeneration(input: Readonly<{
    operationRef: string
    sessionRef: string
    sourceAttachmentRef: string
    sourceGeneration: number
    destinationAttachmentRef: string
    destinationGeneration: number
    authorityEvidenceRef: string
  }>): Effect.Effect<Readonly<{ status: "completed" | "replayed"; fence: PylonPortableSessionFence; record: PylonPortableOperationRecord }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() => {
      assertRef(input.destinationAttachmentRef, "destinationAttachmentRef")
      assertRef(input.authorityEvidenceRef, "authorityEvidenceRef")
      assertGeneration(input.destinationGeneration)
      if (input.destinationGeneration !== input.sourceGeneration + 1) {
        throw new PylonPortableOperationLedgerError("stale_generation", "destination generation must advance exactly once")
      }
      const admitted = this.admitOperationSync({
        operationRef: input.operationRef,
        sessionRef: input.sessionRef,
        attachmentRef: input.sourceAttachmentRef,
        generation: input.sourceGeneration,
        kind: "activate",
      }, { ...input, kind: "activate" })
      const outcome = decodeOutcome(JSON.stringify({ evidenceRefs: [input.authorityEvidenceRef] }))
      if (admitted.record.state === "completed") {
        if (canonical(admitted.record.outcome) !== canonical(outcome)) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "activation outcome conflicts")
        }
        return {
          status: "replayed" as const,
          fence: sessionFence(this.requireSession(input.sessionRef)),
          record: admitted.record,
        }
      }
      const fence = this.requireExactFence({
        sessionRef: input.sessionRef,
        attachmentRef: input.sourceAttachmentRef,
        generation: input.sourceGeneration,
      })
      if (Number(fence.accepting_work) !== 0) {
        throw new PylonPortableOperationLedgerError("invalid_scope", "source generation must be quiesced before activation")
      }
      this.database.query(`
        UPDATE pylon_portable_session_fences
        SET attachment_ref = ?, generation = ?, accepting_work = 1, revision = revision + 1
        WHERE session_ref = ? AND attachment_ref = ? AND generation = ? AND accepting_work = 0
      `).run(
        input.destinationAttachmentRef,
        input.destinationGeneration,
        input.sessionRef,
        input.sourceAttachmentRef,
        input.sourceGeneration,
      )
      this.database.query(`
        UPDATE pylon_portable_session_operations
        SET state = 'completed', outcome_json = ?
        WHERE operation_ref = ?
      `).run(JSON.stringify(outcome), input.operationRef)
      return {
        status: "completed" as const,
        fence: sessionFence(this.requireSession(input.sessionRef)),
        record: operationRecord(this.requireOperation(input.operationRef)),
      }
    }).immediate())
  }

  private admitOperationSync(input: Readonly<{
    operationRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    kind: PylonPortableOperationKind
  }>, exactInput: unknown = input): Readonly<{ status: "admitted" | "replayed"; record: PylonPortableOperationRecord }> {
    this.assertScope(input)
    const operationFingerprint = fingerprint(exactInput)
    const existing = this.readOperationRow(input.operationRef)
    if (existing !== null) {
      if (existing.fingerprint !== operationFingerprint) {
        throw new PylonPortableOperationLedgerError("conflicting_replay", "operation ref was replayed with different bytes")
      }
      return { status: "replayed", record: operationRecord(existing) }
    }
    this.requireExactFence(input)
    this.database.query(`
      INSERT INTO pylon_portable_session_operations
        (operation_ref, session_ref, attachment_ref, generation, kind, fingerprint, state, outcome_json)
      VALUES (?, ?, ?, ?, ?, ?, 'admitted', NULL)
    `).run(
      input.operationRef,
      input.sessionRef,
      input.attachmentRef,
      input.generation,
      input.kind,
      operationFingerprint,
    )
    return { status: "admitted", record: operationRecord(this.requireOperation(input.operationRef)) }
  }

  private assertScope(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    operationRef?: string
  }>): void {
    assertRef(input.sessionRef, "sessionRef")
    assertRef(input.attachmentRef, "attachmentRef")
    if (input.operationRef !== undefined) assertRef(input.operationRef, "operationRef")
    assertGeneration(input.generation)
  }

  private requireExactFence(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
  }>): SessionRow {
    const row = this.requireSession(input.sessionRef)
    if (row.attachment_ref !== input.attachmentRef || Number(row.generation) !== input.generation) {
      throw new PylonPortableOperationLedgerError("stale_generation", "portable session attachment generation is stale")
    }
    return row
  }

  private readSessionRow(sessionRef: string): SessionRow | null {
    return this.database.query(`
      SELECT session_ref, attachment_ref, generation, accepting_work, revision
      FROM pylon_portable_session_fences WHERE session_ref = ?
    `).get(sessionRef) as SessionRow | null
  }

  private requireSession(sessionRef: string): SessionRow {
    const row = this.readSessionRow(sessionRef)
    if (row === null) throw new PylonPortableOperationLedgerError("not_found", "portable session is absent")
    return row
  }

  private readOperationRow(operationRef: string): OperationRow | null {
    return this.database.query(`
      SELECT operation_ref, session_ref, attachment_ref, generation, kind,
             fingerprint, state, outcome_json
      FROM pylon_portable_session_operations WHERE operation_ref = ?
    `).get(operationRef) as OperationRow | null
  }

  private requireOperation(operationRef: string): OperationRow {
    const row = this.readOperationRow(operationRef)
    if (row === null) throw new PylonPortableOperationLedgerError("not_found", "portable operation is absent")
    return row
  }

  private effect<A>(run: () => A): Effect.Effect<A, PylonPortableOperationLedgerError> {
    return Effect.try({
      try: run,
      catch: error => error instanceof PylonPortableOperationLedgerError
        ? error
        : new PylonPortableOperationLedgerError("invalid_scope", "portable operation ledger failed closed"),
    })
  }
}
