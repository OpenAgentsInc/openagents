import { createHash, randomUUID } from "node:crypto"
import type { LegacySqliteDatabase as Database } from "@openagentsinc/sqlite-runtime"
import { Effect, Schema } from "effect"
import { PylonPortableCheckpointBundleSchema } from "@openagentsinc/portable-session-contract"
import type { PylonPortableCheckpointBundle } from "@openagentsinc/portable-session-contract"

// The bundle wire shape lives in the runtime-neutral contract package so
// non-Bun consumers (the Khala Sync server provisioner, the Worker typecheck
// graph) never import this Bun-typed module for its types. Re-exported here
// so existing Pylon-side importers keep their import paths.
export { PylonPortableCheckpointBundleSchema } from "@openagentsinc/portable-session-contract"
export type { PylonPortableCheckpointBundle } from "@openagentsinc/portable-session-contract"

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
  destinationRunnerSessionReservationRef?: string
}>

const PylonPortableOperationOutcomeSchema = Schema.Struct({
  evidenceRefs: Schema.Array(Schema.String),
  checkpointRef: Schema.optionalKey(Schema.String),
  repositoryPostImageDigest: Schema.optionalKey(Schema.String),
  diffDigest: Schema.optionalKey(Schema.String),
  graphDigest: Schema.optionalKey(Schema.String),
  cleanupReceiptRef: Schema.optionalKey(Schema.String),
  destinationRunnerSessionReservationRef: Schema.optionalKey(Schema.String),
})

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

export type PylonPortableControlBindingAgent = Readonly<{
  agentRef: string
  parentAgentRef?: string
  controlSessionRef: string
  workspaceRef: string
  processLifecycle: "active" | "settled" | "absent_after_restart"
  workspaceLifecycle: "retained" | "released"
}>

export type PylonPortableControlBinding = Readonly<{
  schema: typeof PYLON_PORTABLE_OPERATION_LEDGER_VERSION
  sessionRef: string
  attachmentRef: string
  generation: number
  runtimeInstanceRef: string
  state: "accepting" | "quiesced" | "cleaned"
  revision: number
  agents: ReadonlyArray<PylonPortableControlBindingAgent>
}>

export type PylonPortableControlBindingRecovery = Readonly<{
  schema: typeof PYLON_PORTABLE_OPERATION_LEDGER_VERSION
  recoveryRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  runtimeInstanceRef: string
  outcome: "recovered_quiesced" | "already_cleaned"
  acceptingWork: false
  agentRefs: ReadonlyArray<string>
  controlSessionRefs: ReadonlyArray<string>
  workspaceRefs: ReadonlyArray<string>
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

type ControlBindingRow = {
  session_ref: string
  attachment_ref: string
  generation: number
  runtime_instance_ref: string
  state: "accepting" | "quiesced" | "cleaned"
  revision: number
}

type ControlBindingAgentRow = {
  session_ref: string
  agent_ref: string
  parent_agent_ref: string | null
  control_session_ref: string
  workspace_ref: string
  process_lifecycle: PylonPortableControlBindingAgent["processLifecycle"]
  workspace_lifecycle: PylonPortableControlBindingAgent["workspaceLifecycle"]
}

type ControlBindingRecoveryRow = {
  recovery_ref: string
  fingerprint: string
  outcome_json: string
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
    outcome.destinationRunnerSessionReservationRef,
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
      CREATE TABLE IF NOT EXISTS pylon_portable_control_bindings (
        session_ref TEXT PRIMARY KEY,
        attachment_ref TEXT NOT NULL,
        generation INTEGER NOT NULL CHECK (generation >= 0),
        runtime_instance_ref TEXT NOT NULL,
        state TEXT NOT NULL CHECK (state IN ('accepting', 'quiesced', 'cleaned')),
        revision INTEGER NOT NULL CHECK (revision >= 0),
        FOREIGN KEY (session_ref) REFERENCES pylon_portable_session_fences(session_ref) ON DELETE CASCADE
      );
      CREATE TABLE IF NOT EXISTS pylon_portable_control_binding_agents (
        session_ref TEXT NOT NULL,
        agent_ref TEXT NOT NULL,
        parent_agent_ref TEXT,
        control_session_ref TEXT NOT NULL,
        workspace_ref TEXT NOT NULL,
        process_lifecycle TEXT NOT NULL CHECK (process_lifecycle IN ('active', 'settled', 'absent_after_restart')),
        workspace_lifecycle TEXT NOT NULL CHECK (workspace_lifecycle IN ('retained', 'released')),
        PRIMARY KEY (session_ref, agent_ref),
        UNIQUE (session_ref, control_session_ref),
        FOREIGN KEY (session_ref) REFERENCES pylon_portable_control_bindings(session_ref) ON DELETE CASCADE
      );
      CREATE TABLE IF NOT EXISTS pylon_portable_control_binding_recoveries (
        recovery_ref TEXT PRIMARY KEY,
        fingerprint TEXT NOT NULL,
        outcome_json TEXT NOT NULL
      );
    `)
  }

  persistControlBinding(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
    agents: ReadonlyArray<Readonly<{
      agentRef: string
      parentAgentRef?: string
      controlSessionRef: string
      workspaceRef: string
    }>>
  }>): Effect.Effect<Readonly<{ status: "stored" | "replayed"; binding: PylonPortableControlBinding }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() => {
      this.assertControlBindingInput(input)
      const existing = this.readControlBindingRow(input.sessionRef)
      if (existing !== null) {
        const binding = this.controlBinding(existing)
        const expected = this.controlBindingComparable({
          ...input,
          state: "accepting",
          agents: input.agents.map(agent => ({
            ...agent,
            processLifecycle: "active" as const,
            workspaceLifecycle: "retained" as const,
          })),
        })
        if (canonical(this.controlBindingComparable(binding)) !== canonical(expected)) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "portable control binding conflicts")
        }
        return { status: "replayed" as const, binding }
      }
      const fence = this.requireExactFence(input)
      if (Number(fence.accepting_work) !== 1) {
        throw new PylonPortableOperationLedgerError("stale_generation", "portable control binding is not accepting")
      }
      this.database.query(`
        INSERT INTO pylon_portable_control_bindings
          (session_ref, attachment_ref, generation, runtime_instance_ref, state, revision)
        VALUES (?, ?, ?, ?, 'accepting', 0)
      `).run(input.sessionRef, input.attachmentRef, input.generation, input.runtimeInstanceRef)
      const insertAgent = this.database.query(`
        INSERT INTO pylon_portable_control_binding_agents
          (session_ref, agent_ref, parent_agent_ref, control_session_ref, workspace_ref,
           process_lifecycle, workspace_lifecycle)
        VALUES (?, ?, ?, ?, ?, 'active', 'retained')
      `)
      for (const agent of input.agents) {
        insertAgent.run(
          input.sessionRef,
          agent.agentRef,
          agent.parentAgentRef ?? null,
          agent.controlSessionRef,
          agent.workspaceRef,
        )
      }
      return {
        status: "stored" as const,
        binding: this.controlBinding(this.requireControlBindingRow(input.sessionRef)),
      }
    }).immediate())
  }

  readControlBinding(sessionRef: string): Effect.Effect<PylonPortableControlBinding, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      assertRef(sessionRef, "sessionRef")
      return this.controlBinding(this.requireControlBindingRow(sessionRef))
    })
  }

  listControlBindings(): Effect.Effect<ReadonlyArray<PylonPortableControlBinding>, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      const rows = this.database.query(`
        SELECT session_ref, attachment_ref, generation, runtime_instance_ref, state, revision
        FROM pylon_portable_control_bindings ORDER BY session_ref ASC
      `).all() as ControlBindingRow[]
      return rows.map(row => this.controlBinding(row))
    })
  }

  assertControlBindingAccepting(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
  }>): Effect.Effect<PylonPortableControlBinding, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      this.assertScope(input)
      assertRef(input.runtimeInstanceRef, "runtimeInstanceRef")
      const binding = this.controlBinding(this.requireControlBindingRow(input.sessionRef))
      if (binding.attachmentRef !== input.attachmentRef || binding.generation !== input.generation ||
          binding.runtimeInstanceRef !== input.runtimeInstanceRef || binding.state !== "accepting") {
        throw new PylonPortableOperationLedgerError("stale_generation", "portable control binding is not accepting")
      }
      return binding
    })
  }

  markControlBindingQuiesced(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
  }>): Effect.Effect<Readonly<{ status: "completed" | "replayed"; binding: PylonPortableControlBinding }>, PylonPortableOperationLedgerError> {
    return this.transitionControlBinding(input, "quiesced")
  }

  markControlBindingCleaned(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
  }>): Effect.Effect<Readonly<{ status: "completed" | "replayed"; binding: PylonPortableControlBinding }>, PylonPortableOperationLedgerError> {
    return this.transitionControlBinding(input, "cleaned")
  }

  recoverControlBinding(input: Readonly<{
    recoveryRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
  }>): Effect.Effect<Readonly<{
    status: "recovered" | "replayed"
    binding: PylonPortableControlBinding
    recovery: PylonPortableControlBindingRecovery
  }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() => {
      this.assertScope(input)
      assertRef(input.recoveryRef, "recoveryRef")
      assertRef(input.runtimeInstanceRef, "runtimeInstanceRef")
      const exactFingerprint = fingerprint(input)
      const prior = this.database.query(`
        SELECT recovery_ref, fingerprint, outcome_json
        FROM pylon_portable_control_binding_recoveries WHERE recovery_ref = ?
      `).get(input.recoveryRef) as ControlBindingRecoveryRow | null
      if (prior !== null) {
        if (prior.fingerprint !== exactFingerprint) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "portable control recovery conflicts")
        }
        const recovery = this.decodeControlBindingRecovery(prior.outcome_json)
        return {
          status: "replayed" as const,
          binding: this.controlBinding(this.requireControlBindingRow(input.sessionRef)),
          recovery,
        }
      }
      const row = this.requireControlBindingRow(input.sessionRef)
      if (row.attachment_ref !== input.attachmentRef || Number(row.generation) !== input.generation) {
        throw new PylonPortableOperationLedgerError("stale_generation", "portable control recovery generation is stale")
      }
      const alreadyCleaned = row.state === "cleaned"
      const alreadyRecoveredByRuntime = row.state === "quiesced" &&
        row.runtime_instance_ref === input.runtimeInstanceRef
      if (!alreadyCleaned && !alreadyRecoveredByRuntime) {
        this.database.query(`
          UPDATE pylon_portable_control_bindings
          SET runtime_instance_ref = ?, state = 'quiesced', revision = revision + 1
          WHERE session_ref = ? AND attachment_ref = ? AND generation = ? AND state != 'cleaned'
        `).run(input.runtimeInstanceRef, input.sessionRef, input.attachmentRef, input.generation)
        this.database.query(`
          UPDATE pylon_portable_control_binding_agents
          SET process_lifecycle = 'absent_after_restart'
          WHERE session_ref = ? AND workspace_lifecycle = 'retained'
        `).run(input.sessionRef)
      }
      const binding = this.controlBinding(this.requireControlBindingRow(input.sessionRef))
      const recovery = this.validateControlBindingRecovery({
        schema: PYLON_PORTABLE_OPERATION_LEDGER_VERSION,
        recoveryRef: input.recoveryRef,
        sessionRef: input.sessionRef,
        attachmentRef: input.attachmentRef,
        generation: input.generation,
        runtimeInstanceRef: alreadyCleaned ? row.runtime_instance_ref : input.runtimeInstanceRef,
        outcome: alreadyCleaned ? "already_cleaned" : "recovered_quiesced",
        acceptingWork: false,
        agentRefs: binding.agents.map(agent => agent.agentRef),
        controlSessionRefs: binding.agents.map(agent => agent.controlSessionRef),
        workspaceRefs: [...new Set(binding.agents.map(agent => agent.workspaceRef))].sort(),
      })
      this.database.query(`
        INSERT INTO pylon_portable_control_binding_recoveries
          (recovery_ref, fingerprint, outcome_json) VALUES (?, ?, ?)
      `).run(input.recoveryRef, exactFingerprint, JSON.stringify(recovery))
      return { status: "recovered" as const, binding, recovery }
    }).immediate())
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

  readOperation(operationRef: string): Effect.Effect<PylonPortableOperationRecord | null, PylonPortableOperationLedgerError> {
    return this.effect(() => {
      assertRef(operationRef, "operationRef")
      const row = this.readOperationRow(operationRef)
      return row === null ? null : operationRecord(row)
    })
  }

  reserveDestinationRunnerSession(operationRef: string): Effect.Effect<string, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() => {
      assertRef(operationRef, "operationRef")
      const row = this.requireOperation(operationRef)
      if (row.kind !== "stage") {
        throw new PylonPortableOperationLedgerError(
          "invalid_scope",
          "runner-session reservation requires a destination stage",
        )
      }
      const existing = row.outcome_json === null
        ? undefined
        : decodeOutcome(row.outcome_json).destinationRunnerSessionReservationRef
      if (existing !== undefined) return existing
      if (row.state !== "admitted") {
        throw new PylonPortableOperationLedgerError(
          "conflicting_replay",
          "completed destination stage has no runner-session reservation",
        )
      }
      const reservationRef = `runner-session-reservation.${randomUUID()}`
      const outcome = decodeOutcome(JSON.stringify({
        evidenceRefs: [],
        destinationRunnerSessionReservationRef: reservationRef,
      }))
      this.database.query(`
        UPDATE pylon_portable_session_operations
        SET outcome_json = ?
        WHERE operation_ref = ? AND state = 'admitted'
      `).run(JSON.stringify(outcome), operationRef)
      return reservationRef
    }).immediate())
  }

  /**
   * Admits work for a future owner-local destination without pretending the
   * old local fence owns the currently-active remote generation. PORT-01 is
   * still the authority for that source fact; the local ledger only requires
   * its prior generation to be quiesced and prevents two local stages for the
   * same destination generation.
   */
  admitDestinationOperation(input: Readonly<{
    operationRef: string
    sessionRef: string
    sourceAttachmentRef: string
    sourceGeneration: number
    destinationAttachmentRef: string
    destinationGeneration: number
    kind: Extract<PylonPortableOperationKind, "stage" | "activate" | "abort">
    exactInput: unknown
  }>): Effect.Effect<Readonly<{ status: "admitted" | "replayed"; record: PylonPortableOperationRecord }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() =>
      this.admitDestinationOperationSync(input)).immediate())
  }

  private admitDestinationOperationSync(input: Readonly<{
    operationRef: string
    sessionRef: string
    sourceAttachmentRef: string
    sourceGeneration: number
    destinationAttachmentRef: string
    destinationGeneration: number
    kind: Extract<PylonPortableOperationKind, "stage" | "activate" | "abort">
    exactInput: unknown
  }>): Readonly<{ status: "admitted" | "replayed"; record: PylonPortableOperationRecord }> {
    assertRef(input.operationRef, "operationRef")
    assertRef(input.sessionRef, "sessionRef")
    assertRef(input.sourceAttachmentRef, "sourceAttachmentRef")
    assertRef(input.destinationAttachmentRef, "destinationAttachmentRef")
    assertGeneration(input.sourceGeneration)
    assertGeneration(input.destinationGeneration)
    if (input.destinationGeneration !== input.sourceGeneration + 1) {
      throw new PylonPortableOperationLedgerError("stale_generation", "destination generation must advance exactly once")
    }
    const operationFingerprint = fingerprint(input.exactInput)
    const existing = this.readOperationRow(input.operationRef)
    if (existing !== null) {
      if (existing.fingerprint !== operationFingerprint ||
          existing.session_ref !== input.sessionRef ||
          existing.attachment_ref !== input.destinationAttachmentRef ||
          Number(existing.generation) !== input.destinationGeneration ||
          existing.kind !== input.kind) {
        throw new PylonPortableOperationLedgerError("conflicting_replay", "destination operation conflicts")
      }
      return { status: "replayed" as const, record: operationRecord(existing) }
    }
    const fence = this.requireSession(input.sessionRef)
    if (Number(fence.accepting_work) !== 0 || Number(fence.generation) > input.sourceGeneration) {
      throw new PylonPortableOperationLedgerError("stale_generation", "prior local generation is not durably quiesced")
    }
    const candidates = this.database.query(`
      SELECT operation_ref, session_ref, attachment_ref, generation, kind,
             fingerprint, state, outcome_json
      FROM pylon_portable_session_operations
      WHERE session_ref = ? AND generation = ? AND kind = ?
    `).all(input.sessionRef, input.destinationGeneration, input.kind) as OperationRow[]
    const competing = candidates.some(candidate => {
      if (input.kind !== "stage") return candidate.attachment_ref === input.destinationAttachmentRef
      const abortRef = candidate.operation_ref.endsWith(".destination.stage")
        ? `${candidate.operation_ref.slice(0, -".destination.stage".length)}.destination.abort`
        : ""
      const abort = abortRef.length === 0 ? null : this.readOperationRow(abortRef)
      return abort?.kind !== "abort" || abort.state !== "completed"
    })
    if (competing) {
      throw new PylonPortableOperationLedgerError("conflicting_replay", "destination generation already has an operation")
    }
    this.database.query(`
      INSERT INTO pylon_portable_session_operations
        (operation_ref, session_ref, attachment_ref, generation, kind, fingerprint, state, outcome_json)
      VALUES (?, ?, ?, ?, ?, ?, 'admitted', NULL)
    `).run(
      input.operationRef,
      input.sessionRef,
      input.destinationAttachmentRef,
      input.destinationGeneration,
      input.kind,
      operationFingerprint,
    )
    return {
      status: "admitted" as const,
      record: operationRecord(this.requireOperation(input.operationRef)),
    }
  }

  commitDestinationGeneration(input: Readonly<{
    operationRef: string
    sessionRef: string
    sourceAttachmentRef: string
    sourceGeneration: number
    destinationAttachmentRef: string
    destinationGeneration: number
    stageOperationRef: string
    authorityEvidenceRef: string
    evidenceRefs: ReadonlyArray<string>
    exactInput: unknown
  }>): Effect.Effect<Readonly<{ status: "completed" | "replayed"; fence: PylonPortableSessionFence; record: PylonPortableOperationRecord }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() => {
      const admitted = this.admitDestinationOperationSync({
        ...input,
        kind: "activate",
        exactInput: input.exactInput,
      })
      const outcome = decodeOutcome(JSON.stringify({
        evidenceRefs: [input.authorityEvidenceRef, ...input.evidenceRefs],
      }))
      if (admitted.record.state === "completed") {
        if (canonical(admitted.record.outcome) !== canonical(outcome)) {
          throw new PylonPortableOperationLedgerError("conflicting_replay", "destination activation outcome conflicts")
        }
        return {
          status: "replayed" as const,
          fence: sessionFence(this.requireSession(input.sessionRef)),
          record: admitted.record,
        }
      }
      const stage = this.requireOperation(input.stageOperationRef)
      if (stage.kind !== "stage" || stage.state !== "completed" ||
          stage.session_ref !== input.sessionRef ||
          stage.attachment_ref !== input.destinationAttachmentRef ||
          Number(stage.generation) !== input.destinationGeneration) {
        throw new PylonPortableOperationLedgerError("invalid_scope", "destination activation requires the exact completed stage")
      }
      const fence = this.requireSession(input.sessionRef)
      if (Number(fence.accepting_work) !== 0 || Number(fence.generation) > input.sourceGeneration) {
        throw new PylonPortableOperationLedgerError("stale_generation", "prior local generation is not fenced")
      }
      const advanced = this.database.query(`
        UPDATE pylon_portable_session_fences
        SET attachment_ref = ?, generation = ?, accepting_work = 1, revision = revision + 1
        WHERE session_ref = ? AND accepting_work = 0 AND generation <= ?
      `).run(
        input.destinationAttachmentRef,
        input.destinationGeneration,
        input.sessionRef,
        input.sourceGeneration,
      )
      if (advanced.changes !== 1) {
        throw new PylonPortableOperationLedgerError("stale_generation", "destination fence advance lost its authority race")
      }
      this.database.query(`
        UPDATE pylon_portable_session_operations
        SET state = 'completed', outcome_json = ?
        WHERE operation_ref = ? AND state = 'admitted'
      `).run(JSON.stringify(outcome), input.operationRef)
      return {
        status: "completed" as const,
        fence: sessionFence(this.requireSession(input.sessionRef)),
        record: operationRecord(this.requireOperation(input.operationRef)),
      }
    }).immediate())
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

  private transitionControlBinding(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
  }>, destination: "quiesced" | "cleaned"): Effect.Effect<Readonly<{
    status: "completed" | "replayed"
    binding: PylonPortableControlBinding
  }>, PylonPortableOperationLedgerError> {
    return this.effect(() => this.database.transaction(() => {
      this.assertScope(input)
      assertRef(input.runtimeInstanceRef, "runtimeInstanceRef")
      const row = this.requireControlBindingRow(input.sessionRef)
      if (row.attachment_ref !== input.attachmentRef || Number(row.generation) !== input.generation ||
          row.runtime_instance_ref !== input.runtimeInstanceRef) {
        throw new PylonPortableOperationLedgerError("stale_generation", "portable control binding owner is stale")
      }
      if (row.state === destination) {
        return { status: "replayed" as const, binding: this.controlBinding(row) }
      }
      if (destination === "quiesced" && row.state !== "accepting") {
        throw new PylonPortableOperationLedgerError("invalid_scope", "portable control binding cannot quiesce")
      }
      if (destination === "cleaned" && row.state !== "quiesced") {
        throw new PylonPortableOperationLedgerError("invalid_scope", "portable control binding must quiesce before cleanup")
      }
      this.database.query(`
        UPDATE pylon_portable_control_bindings
        SET state = ?, revision = revision + 1
        WHERE session_ref = ? AND runtime_instance_ref = ? AND state = ?
      `).run(destination, input.sessionRef, input.runtimeInstanceRef, row.state)
      this.database.query(`
        UPDATE pylon_portable_control_binding_agents
        SET process_lifecycle = 'settled', workspace_lifecycle = ?
        WHERE session_ref = ?
      `).run(destination === "cleaned" ? "released" : "retained", input.sessionRef)
      return {
        status: "completed" as const,
        binding: this.controlBinding(this.requireControlBindingRow(input.sessionRef)),
      }
    }).immediate())
  }

  private assertControlBindingInput(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
    agents: ReadonlyArray<Readonly<{
      agentRef: string
      parentAgentRef?: string
      controlSessionRef: string
      workspaceRef: string
    }>>
  }>): void {
    this.assertScope(input)
    assertRef(input.runtimeInstanceRef, "runtimeInstanceRef")
    if (input.agents.length === 0) {
      throw new PylonPortableOperationLedgerError("invalid_scope", "portable control binding requires agents")
    }
    const agentRefs = new Set<string>()
    const controlSessionRefs = new Set<string>()
    for (const agent of input.agents) {
      assertRef(agent.agentRef, "agentRef")
      if (agent.parentAgentRef !== undefined) assertRef(agent.parentAgentRef, "parentAgentRef")
      assertRef(agent.controlSessionRef, "controlSessionRef")
      assertRef(agent.workspaceRef, "workspaceRef")
      if (agentRefs.has(agent.agentRef) || controlSessionRefs.has(agent.controlSessionRef)) {
        throw new PylonPortableOperationLedgerError("invalid_scope", "portable control binding agents are not unique")
      }
      agentRefs.add(agent.agentRef)
      controlSessionRefs.add(agent.controlSessionRef)
    }
    const roots = input.agents.filter(agent => agent.parentAgentRef === undefined)
    if (roots.length !== 1 || input.agents.some(agent =>
      agent.parentAgentRef !== undefined && !agentRefs.has(agent.parentAgentRef))) {
      throw new PylonPortableOperationLedgerError("invalid_scope", "portable control binding graph is invalid")
    }
  }

  private controlBindingComparable(input: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    runtimeInstanceRef: string
    state: "accepting" | "quiesced" | "cleaned"
    agents: ReadonlyArray<PylonPortableControlBindingAgent>
  }>): unknown {
    return {
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      generation: input.generation,
      runtimeInstanceRef: input.runtimeInstanceRef,
      state: input.state,
      agents: [...input.agents].sort((left, right) => left.agentRef.localeCompare(right.agentRef)),
    }
  }

  private controlBinding(row: ControlBindingRow): PylonPortableControlBinding {
    const agents = this.database.query(`
      SELECT session_ref, agent_ref, parent_agent_ref, control_session_ref, workspace_ref,
             process_lifecycle, workspace_lifecycle
      FROM pylon_portable_control_binding_agents
      WHERE session_ref = ? ORDER BY agent_ref ASC
    `).all(row.session_ref) as ControlBindingAgentRow[]
    return {
      schema: PYLON_PORTABLE_OPERATION_LEDGER_VERSION,
      sessionRef: row.session_ref,
      attachmentRef: row.attachment_ref,
      generation: Number(row.generation),
      runtimeInstanceRef: row.runtime_instance_ref,
      state: row.state,
      revision: Number(row.revision),
      agents: agents.map(agent => ({
        agentRef: agent.agent_ref,
        ...(agent.parent_agent_ref === null ? {} : { parentAgentRef: agent.parent_agent_ref }),
        controlSessionRef: agent.control_session_ref,
        workspaceRef: agent.workspace_ref,
        processLifecycle: agent.process_lifecycle,
        workspaceLifecycle: agent.workspace_lifecycle,
      })),
    }
  }

  private validateControlBindingRecovery(value: PylonPortableControlBindingRecovery): PylonPortableControlBindingRecovery {
    const refs = [
      value.recoveryRef,
      value.sessionRef,
      value.attachmentRef,
      value.runtimeInstanceRef,
      ...value.agentRefs,
      ...value.controlSessionRefs,
      ...value.workspaceRefs,
    ]
    if (value.schema !== PYLON_PORTABLE_OPERATION_LEDGER_VERSION || value.acceptingWork !== false ||
        !Number.isSafeInteger(value.generation) || value.generation < 0 ||
        (value.outcome !== "recovered_quiesced" && value.outcome !== "already_cleaned") ||
        refs.some(ref => !SAFE_REF.test(ref)) || FORBIDDEN_PRIVATE_MATERIAL.test(canonical(value)) ||
        new Set(value.agentRefs).size !== value.agentRefs.length ||
        new Set(value.controlSessionRefs).size !== value.controlSessionRefs.length ||
        new Set(value.workspaceRefs).size !== value.workspaceRefs.length) {
      throw new PylonPortableOperationLedgerError("unsafe_result", "portable control recovery outcome is invalid")
    }
    return value
  }

  private decodeControlBindingRecovery(serialized: string): PylonPortableControlBindingRecovery {
    let value: unknown
    try {
      value = JSON.parse(serialized)
    } catch {
      throw new PylonPortableOperationLedgerError("unsafe_result", "portable control recovery outcome is invalid")
    }
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      throw new PylonPortableOperationLedgerError("unsafe_result", "portable control recovery outcome is invalid")
    }
    const candidate = value as Record<string, unknown>
    if (!Array.isArray(candidate.agentRefs) || !Array.isArray(candidate.controlSessionRefs) ||
        !Array.isArray(candidate.workspaceRefs) ||
        !candidate.agentRefs.every(item => typeof item === "string") ||
        !candidate.controlSessionRefs.every(item => typeof item === "string") ||
        !candidate.workspaceRefs.every(item => typeof item === "string") ||
        typeof candidate.recoveryRef !== "string" || typeof candidate.sessionRef !== "string" ||
        typeof candidate.attachmentRef !== "string" || typeof candidate.runtimeInstanceRef !== "string" ||
        typeof candidate.generation !== "number") {
      throw new PylonPortableOperationLedgerError("unsafe_result", "portable control recovery outcome is invalid")
    }
    return this.validateControlBindingRecovery(candidate as unknown as PylonPortableControlBindingRecovery)
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

  private readControlBindingRow(sessionRef: string): ControlBindingRow | null {
    return this.database.query(`
      SELECT session_ref, attachment_ref, generation, runtime_instance_ref, state, revision
      FROM pylon_portable_control_bindings WHERE session_ref = ?
    `).get(sessionRef) as ControlBindingRow | null
  }

  private requireControlBindingRow(sessionRef: string): ControlBindingRow {
    const row = this.readControlBindingRow(sessionRef)
    if (row === null) throw new PylonPortableOperationLedgerError("not_found", "portable control binding is absent")
    return row
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
