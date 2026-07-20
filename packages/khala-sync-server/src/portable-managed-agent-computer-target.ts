import { createHash } from "node:crypto"

import { canonicalJson } from "@openagentsinc/khala-sync"
import type {
  PortableAgentGraph,
  PortableSessionExecutionBinding,
} from "@openagentsinc/portable-session-contract"
import { validateIdePortableDestinationActivationReceipt } from "@openagentsinc/portable-session-contract"

import type {
  PortableCheckpointBundle,
  PortableSessionExecutionTarget,
  PortableSourceCleanupReceipt,
  PortableTargetActivationReceipt,
  PortableTargetStageReceipt,
  PortableThreadCursor,
} from "./portable-session-move.js"
import type { SyncSql, SyncTransactionSql } from "./sql.js"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu

type OperationKind = "stage" | "activate" | "abort" | "quiesce" | "checkpoint" | "cleanup"

type OperationRow = Readonly<{
  session_ref: string
  kind: string
  fingerprint: string
  status: string
  result_json: unknown
}>

type TargetRow = Readonly<{
  attachment_ref: string
  generation: string | number
  checkpoint_ref: string
  resource_ref: string
  state: "staged" | "active" | "quiesced" | "reclaimed"
  accepting_work: boolean
  bundle_json: unknown
  stage_receipt_json: unknown
  authority_evidence_ref: string | null
}>

export class PortableManagedAgentComputerTargetError extends Error {
  readonly _tag = "PortableManagedAgentComputerTargetError"
  override readonly name = "PortableManagedAgentComputerTargetError"

  constructor(
    readonly code:
      | "invalid"
      | "conflict"
      | "stale_generation"
      | "not_staged"
      | "authority_not_committed"
      | "provisioner_rejected"
      | "unsafe_result",
    message: string,
  ) {
    super(message)
  }
}

export type ManagedAgentComputerStageReceipt = PortableTargetStageReceipt & Readonly<{
  resourceRef: string
}>

/**
 * Stateful managed provisioner seam.
 *
 * Every method MUST be byte-idempotent by operationRef. The Postgres adapter
 * records a pending operation before invoking it, so a process crash may retry
 * the same operation but can never lawfully change its input bytes.
 */
export type ManagedAgentComputerPortableProvisioner = Readonly<{
  stage: (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    bundle: PortableCheckpointBundle
    attachmentRef: string
    generation: number
    capabilityLeaseRefs: ReadonlyArray<string>
  }>) => Promise<ManagedAgentComputerStageReceipt>
  activate: (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    resourceRef: string
    checkpointRef: string
    sessionRef: string
    executionBinding: PortableSessionExecutionBinding
    attachmentRef: string
    generation: number
    capabilityLeaseRefs: ReadonlyArray<string>
    authorityEvidenceRef: string
  }>) => Promise<PortableTargetActivationReceipt>
  abort: (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    resourceRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
  }>) => Promise<Readonly<{ evidenceRefs: ReadonlyArray<string> }>>
  quiesce: (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    resourceRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    graph: PortableAgentGraph
    threadCursors: ReadonlyArray<PortableThreadCursor>
  }>) => Promise<Readonly<{
    quiescedAgentRefs: ReadonlyArray<string>
    evidenceRefs: ReadonlyArray<string>
  }>>
  checkpoint: (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    resourceRef: string
    checkpointRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    eventLogCursor: number
    executionBinding: PortableSessionExecutionBinding
    graph: PortableAgentGraph
    threadCursors: ReadonlyArray<PortableThreadCursor>
  }>) => Promise<PortableCheckpointBundle>
  reclaim: (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    resourceRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    agentRefs: ReadonlyArray<string>
  }>) => Promise<PortableSourceCleanupReceipt>
}>

export type PostgresManagedAgentComputerTargetConfig = Readonly<{
  sql: SyncSql
  ownerRef: string
  targetRef: string
  provisioner: ManagedAgentComputerPortableProvisioner
  now?: () => Date
}>

const digest = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`

const parseJson = <A>(value: unknown): A =>
  (typeof value === "string" ? JSON.parse(value) : value) as A

const same = (left: unknown, right: unknown): boolean =>
  canonicalJson(left) === canonicalJson(right)

const publicSafe = <A>(value: A): A => {
  const encoded = canonicalJson(value)
  if (FORBIDDEN_PRIVATE_MATERIAL.test(encoded)) {
    throw new PortableManagedAgentComputerTargetError(
      "unsafe_result",
      "managed target value contains forbidden private material",
    )
  }
  return value
}

const refs = (values: ReadonlyArray<string>, field: string): ReadonlyArray<string> => {
  if (values.length !== new Set(values).size || values.some(value => !SAFE_REF.test(value))) {
    throw new PortableManagedAgentComputerTargetError("invalid", `${field} must contain unique public-safe refs`)
  }
  return values
}

const generation = (value: string | number): number => {
  const parsed = Number(value)
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new PortableManagedAgentComputerTargetError("invalid", "managed target generation is invalid")
  }
  return parsed
}

export class PostgresManagedAgentComputerTarget implements PortableSessionExecutionTarget {
  readonly targetClass = "openagents_managed" as const
  readonly targetRef: string

  constructor(private readonly config: PostgresManagedAgentComputerTargetConfig) {
    this.targetRef = config.targetRef
    if (![config.ownerRef, config.targetRef].every(value => SAFE_REF.test(value))) {
      throw new PortableManagedAgentComputerTargetError("invalid", "managed target scope is invalid")
    }
  }

  async stageCheckpoint(
    input: Parameters<PortableSessionExecutionTarget["stageCheckpoint"]>[0],
  ): Promise<PortableTargetStageReceipt> {
    this.assertBase(input.operationRef, input.bundle.checkpoint.sessionRef)
    if (input.bundle.executionBinding.ownerRef !== this.config.ownerRef ||
        input.bundle.executionBinding.sessionRef !== input.bundle.checkpoint.sessionRef ||
        input.destinationGeneration !== input.bundle.checkpoint.sourceGeneration + 1 ||
        !SAFE_REF.test(input.destinationAttachmentRef)) {
      throw new PortableManagedAgentComputerTargetError("invalid", "managed stage binding or generation is invalid")
    }
    if (!same(Object.keys(input.bundle).sort(), ["checkpoint", "executionBinding", "graph", "threadCursors"])) {
      throw new PortableManagedAgentComputerTargetError("invalid", "managed checkpoint bundle contains unknown fields")
    }
    publicSafe(input.bundle)
    await this.assertAuthorizedSessionTarget(input.bundle.checkpoint.sessionRef)
    const operation = {
      kind: "stage" as const,
      operationRef: input.operationRef,
      sessionRef: input.bundle.checkpoint.sessionRef,
      attachmentRef: input.destinationAttachmentRef,
      generation: input.destinationGeneration,
      checkpointRef: input.bundle.checkpoint.checkpointRef,
      bundle: input.bundle,
      capabilityLeaseRefs: refs(input.capabilityLeaseRefs, "capability leases"),
    }
    return this.runOperation(operation.kind, operation, async () => {
      const existing = await this.targetRow(operation.sessionRef)
      if (existing !== undefined) this.assertSameTarget(existing, operation)
      const receipt = publicSafe(await this.config.provisioner.stage({
        operationRef: operation.operationRef,
        ownerRef: this.config.ownerRef,
        targetRef: this.targetRef,
        bundle: input.bundle,
        attachmentRef: operation.attachmentRef,
        generation: operation.generation,
        capabilityLeaseRefs: operation.capabilityLeaseRefs,
      }))
      this.validateStage(input.bundle, receipt)
      await this.config.sql.begin(async tx => {
        const locked = await this.targetRow(operation.sessionRef, tx, true)
        if (locked !== undefined) this.assertSameTarget(locked, operation)
        await tx`
          INSERT INTO khala_sync_portable_managed_targets
            (owner_user_id, session_ref, target_ref, attachment_ref, generation,
             checkpoint_ref, resource_ref, state, accepting_work, bundle_json,
             stage_receipt_json)
          VALUES
            (${this.config.ownerRef}, ${operation.sessionRef}, ${this.targetRef},
             ${operation.attachmentRef}, ${operation.generation}, ${operation.checkpointRef},
             ${receipt.resourceRef}, 'staged', FALSE, ${JSON.stringify(input.bundle)}::jsonb,
             ${JSON.stringify(receipt)}::jsonb)
          ON CONFLICT (owner_user_id, session_ref, target_ref) DO NOTHING
        `
        const after = await this.targetRow(operation.sessionRef, tx, true)
        if (after === undefined) {
          throw new PortableManagedAgentComputerTargetError("conflict", "managed stage was not retained")
        }
        this.assertSameTarget(after, operation)
        if (after.resource_ref !== receipt.resourceRef || !same(parseJson(after.stage_receipt_json), receipt)) {
          throw new PortableManagedAgentComputerTargetError("conflict", "managed stage receipt conflicts")
        }
      })
      const { resourceRef: _resourceRef, ...portableReceipt } = receipt
      return portableReceipt
    })
  }

  async activate(
    input: Parameters<PortableSessionExecutionTarget["activate"]>[0],
  ): Promise<PortableTargetActivationReceipt> {
    this.assertBase(input.operationRef, input.sessionRef)
    const operation = {
      kind: "activate" as const,
      ...input,
      capabilityLeaseRefs: refs(input.capabilityLeaseRefs, "capability leases"),
    }
    return this.runOperation(operation.kind, operation, async () => {
      const row = await this.requireTarget(input.sessionRef, input.destinationAttachmentRef, input.destinationGeneration)
      if (row.checkpoint_ref !== input.checkpointRef || row.state === "reclaimed") {
        throw new PortableManagedAgentComputerTargetError("not_staged", "managed target is not the exact retained stage")
      }
      const stagedBundle = parseJson<PortableCheckpointBundle>(row.bundle_json)
      if (!same(stagedBundle.executionBinding, input.executionBinding)) {
        throw new PortableManagedAgentComputerTargetError("conflict", "activation execution binding differs from the retained stage")
      }
      const authorityEvidenceRef = await this.authorityEvidence(
        input.sessionRef,
        input.destinationAttachmentRef,
        input.destinationGeneration,
      )
      const stageReceipt = parseJson<ManagedAgentComputerStageReceipt>(row.stage_receipt_json)
      const receipt = validateIdePortableDestinationActivationReceipt(publicSafe(await this.config.provisioner.activate({
        operationRef: input.operationRef,
        ownerRef: this.config.ownerRef,
        targetRef: this.targetRef,
        resourceRef: row.resource_ref,
        checkpointRef: input.checkpointRef,
        sessionRef: input.sessionRef,
        executionBinding: input.executionBinding,
        attachmentRef: input.destinationAttachmentRef,
        generation: input.destinationGeneration,
        capabilityLeaseRefs: operation.capabilityLeaseRefs,
        authorityEvidenceRef,
      })), {
        operationRef: input.operationRef,
        sessionRef: input.sessionRef,
        checkpointRef: input.checkpointRef,
        destinationTargetRef: this.targetRef,
        destinationAttachmentRef: input.destinationAttachmentRef,
        destinationRunnerSessionReservationRef:
          stageReceipt.destinationRunnerSessionReservationRef,
        destinationGeneration: input.destinationGeneration,
        authenticationPolicyRef: `policy.portable.destination.${this.targetClass}.v1`,
        ...(this.config.now === undefined ? {} : { now: this.config.now() }),
      })
      const bundle = parseJson<PortableCheckpointBundle>(row.bundle_json)
      const expectedAgents = bundle.graph.nodes.map(node => node.agentRef).sort()
      if (!same([...receipt.activatedAgentRefs].sort(), expectedAgents)) {
        throw new PortableManagedAgentComputerTargetError("provisioner_rejected", "activation omitted canonical agents")
      }
      refs(receipt.evidenceRefs, "activation evidence")
      refs(receipt.acceptedWorkRefs.map(item => item.agentRef), "accepted agent refs")
      refs(receipt.acceptedWorkRefs.map(item => item.turnRef), "accepted turn refs")
      await this.config.sql`
        UPDATE khala_sync_portable_managed_targets
        SET state = 'active', accepting_work = TRUE,
            authority_evidence_ref = ${authorityEvidenceRef}, updated_at = NOW()
        WHERE owner_user_id = ${this.config.ownerRef}
          AND session_ref = ${input.sessionRef}
          AND target_ref = ${this.targetRef}
          AND state = 'staged'
          AND accepting_work = FALSE
      `
      const active = await this.requireTarget(input.sessionRef, input.destinationAttachmentRef, input.destinationGeneration)
      if (active.state !== "active" || !active.accepting_work || active.authority_evidence_ref !== authorityEvidenceRef) {
        throw new PortableManagedAgentComputerTargetError("conflict", "managed activation was not retained")
      }
      return receipt
    })
  }

  async abortStaged(
    input: Parameters<PortableSessionExecutionTarget["abortStaged"]>[0],
  ): Promise<Readonly<{ evidenceRefs: ReadonlyArray<string> }>> {
    this.assertBase(input.operationRef, input.sessionRef)
    const operation = { kind: "abort" as const, ...input }
    return this.runOperation(operation.kind, operation, async () => {
      const row = await this.requireTarget(input.sessionRef, input.destinationAttachmentRef, input.destinationGeneration)
      if (row.state === "active" || row.state === "quiesced") {
        throw new PortableManagedAgentComputerTargetError("conflict", "an activated managed target cannot be aborted")
      }
      const receipt = publicSafe(await this.config.provisioner.abort({
        operationRef: input.operationRef,
        ownerRef: this.config.ownerRef,
        targetRef: this.targetRef,
        resourceRef: row.resource_ref,
        sessionRef: input.sessionRef,
        attachmentRef: input.destinationAttachmentRef,
        generation: input.destinationGeneration,
      }))
      refs(receipt.evidenceRefs, "abort evidence")
      if (row.state === "staged") await this.reclaimRow(input.sessionRef, "staged")
      return receipt
    })
  }

  async quiesceGraph(
    input: Parameters<PortableSessionExecutionTarget["quiesceGraph"]>[0],
  ): Promise<Readonly<{ quiescedAgentRefs: ReadonlyArray<string>; evidenceRefs: ReadonlyArray<string> }>> {
    this.assertBase(input.operationRef, input.sessionRef)
    const operation = { kind: "quiesce" as const, ...input }
    return this.runOperation(operation.kind, operation, async () => {
      const row = await this.requireTarget(input.sessionRef, input.attachmentRef, input.generation)
      if ((row.state !== "active" && row.state !== "quiesced") ||
          (row.state === "active") !== row.accepting_work) {
        throw new PortableManagedAgentComputerTargetError("conflict", "only the active managed generation may quiesce")
      }
      const receipt = publicSafe(await this.config.provisioner.quiesce({
        ...input,
        ownerRef: this.config.ownerRef,
        targetRef: this.targetRef,
        resourceRef: row.resource_ref,
      }))
      const expected = input.graph.nodes.map(node => node.agentRef).sort()
      if (!same([...receipt.quiescedAgentRefs].sort(), expected)) {
        throw new PortableManagedAgentComputerTargetError("provisioner_rejected", "quiescence omitted canonical agents")
      }
      refs(receipt.evidenceRefs, "quiesce evidence")
      if (row.state === "active") {
        await this.config.sql`
          UPDATE khala_sync_portable_managed_targets
          SET state = 'quiesced', accepting_work = FALSE, updated_at = NOW()
          WHERE owner_user_id = ${this.config.ownerRef} AND session_ref = ${input.sessionRef}
            AND target_ref = ${this.targetRef} AND state = 'active' AND accepting_work = TRUE
        `
      }
      const quiesced = await this.requireTarget(input.sessionRef, input.attachmentRef, input.generation)
      if (quiesced.state !== "quiesced" || quiesced.accepting_work) {
        throw new PortableManagedAgentComputerTargetError("conflict", "managed quiescence was not retained")
      }
      return receipt
    })
  }

  async createCheckpoint(
    input: Parameters<PortableSessionExecutionTarget["createCheckpoint"]>[0],
  ): Promise<PortableCheckpointBundle> {
    this.assertBase(input.operationRef, input.sessionRef)
    const operation = { kind: "checkpoint" as const, ...input }
    return this.runOperation(operation.kind, operation, async () => {
      const row = await this.requireTarget(input.sessionRef, input.attachmentRef, input.generation)
      if (row.state !== "quiesced" || row.accepting_work) {
        throw new PortableManagedAgentComputerTargetError("conflict", "managed checkpoint requires durable quiescence")
      }
      const bundle = publicSafe(await this.config.provisioner.checkpoint({
        ...input,
        ownerRef: this.config.ownerRef,
        targetRef: this.targetRef,
        resourceRef: row.resource_ref,
      }))
      if (bundle.checkpoint.checkpointRef !== input.checkpointRef ||
          bundle.checkpoint.sessionRef !== input.sessionRef ||
          bundle.checkpoint.sourceAttachmentRef !== input.attachmentRef ||
          bundle.checkpoint.sourceGeneration !== input.generation ||
          !same(bundle.executionBinding, input.executionBinding) ||
          !same(bundle.graph, input.graph) ||
          !same(bundle.threadCursors, input.threadCursors)) {
        throw new PortableManagedAgentComputerTargetError("provisioner_rejected", "managed checkpoint does not match source state")
      }
      return bundle
    })
  }

  async cleanupSource(
    input: Parameters<PortableSessionExecutionTarget["cleanupSource"]>[0],
  ): Promise<PortableSourceCleanupReceipt> {
    this.assertBase(input.operationRef, input.sessionRef)
    const operation = { kind: "cleanup" as const, ...input }
    return this.runOperation(operation.kind, operation, async () => {
      const row = await this.requireTarget(input.sessionRef, input.attachmentRef, input.generation)
      if ((row.state !== "quiesced" && row.state !== "reclaimed") || row.accepting_work) {
        throw new PortableManagedAgentComputerTargetError("conflict", "managed cleanup requires durable quiescence")
      }
      const receipt = publicSafe(await this.config.provisioner.reclaim({
        ...input,
        ownerRef: this.config.ownerRef,
        targetRef: this.targetRef,
        resourceRef: row.resource_ref,
      }))
      if (!same([...receipt.cleanedAgentRefs].sort(), [...input.agentRefs].sort()) ||
          receipt.processes !== "released" || receipt.scratch !== "released" || receipt.ports !== "released") {
        throw new PortableManagedAgentComputerTargetError("provisioner_rejected", "managed cleanup is incomplete")
      }
      refs(receipt.evidenceRefs, "cleanup evidence")
      if (row.state === "quiesced") await this.reclaimRow(input.sessionRef, "quiesced")
      return receipt
    })
  }

  private async runOperation<A>(kind: OperationKind, input: object & { operationRef: string; sessionRef: string }, effect: () => Promise<A>): Promise<A> {
    const sessionRef = input.sessionRef
    const fingerprint = digest(input)
    const replay = await this.config.sql.begin(async tx => {
      await tx`
        INSERT INTO khala_sync_portable_managed_target_operations
          (owner_user_id, session_ref, target_ref, operation_ref, kind, fingerprint, status)
        VALUES
          (${this.config.ownerRef}, ${sessionRef}, ${this.targetRef}, ${input.operationRef},
           ${kind}, ${fingerprint}, 'pending')
        ON CONFLICT (owner_user_id, target_ref, operation_ref) DO NOTHING
      `
      const rows: OperationRow[] = await tx`
        SELECT session_ref, kind, fingerprint, status, result_json
        FROM khala_sync_portable_managed_target_operations
        WHERE owner_user_id = ${this.config.ownerRef} AND target_ref = ${this.targetRef}
          AND operation_ref = ${input.operationRef}
        FOR UPDATE
      `
      const row = rows[0]
      if (row === undefined || row.session_ref !== sessionRef || row.kind !== kind || row.fingerprint !== fingerprint) {
        throw new PortableManagedAgentComputerTargetError("conflict", "managed operation ref was reused with different bytes")
      }
      return row.status === "completed" ? parseJson<A>(row.result_json) : undefined
    })
    if (replay !== undefined) return replay

    const result = publicSafe(await effect())
    await this.config.sql.begin(async tx => {
      const rows: OperationRow[] = await tx`
        SELECT session_ref, kind, fingerprint, status, result_json
        FROM khala_sync_portable_managed_target_operations
        WHERE owner_user_id = ${this.config.ownerRef} AND target_ref = ${this.targetRef}
          AND operation_ref = ${input.operationRef}
        FOR UPDATE
      `
      const row = rows[0]
      if (row === undefined || row.session_ref !== sessionRef || row.kind !== kind || row.fingerprint !== fingerprint) {
        throw new PortableManagedAgentComputerTargetError("conflict", "managed operation changed before completion")
      }
      if (row.status === "completed") {
        if (!same(parseJson(row.result_json), result)) {
          throw new PortableManagedAgentComputerTargetError("conflict", "managed operation completed with a different result")
        }
        return
      }
      await tx`
        UPDATE khala_sync_portable_managed_target_operations
        SET status = 'completed', result_json = ${JSON.stringify(result)}::jsonb, updated_at = NOW()
        WHERE owner_user_id = ${this.config.ownerRef} AND target_ref = ${this.targetRef}
          AND operation_ref = ${input.operationRef} AND status = 'pending'
      `
    })
    return result
  }

  private assertBase(operationRef: string, sessionRef: string): void {
    if (![operationRef, sessionRef].every(value => SAFE_REF.test(value))) {
      throw new PortableManagedAgentComputerTargetError("invalid", "managed operation scope is invalid")
    }
  }

  private async targetRow(sessionRef: string, sql: SyncSql | SyncTransactionSql = this.config.sql, lock = false): Promise<TargetRow | undefined> {
    const rows: TargetRow[] = lock
      ? await sql`
          SELECT attachment_ref, generation, checkpoint_ref, resource_ref, state,
                 accepting_work, bundle_json, stage_receipt_json, authority_evidence_ref
          FROM khala_sync_portable_managed_targets
          WHERE owner_user_id = ${this.config.ownerRef} AND session_ref = ${sessionRef}
            AND target_ref = ${this.targetRef}
          FOR UPDATE
        `
      : await sql`
          SELECT attachment_ref, generation, checkpoint_ref, resource_ref, state,
                 accepting_work, bundle_json, stage_receipt_json, authority_evidence_ref
          FROM khala_sync_portable_managed_targets
          WHERE owner_user_id = ${this.config.ownerRef} AND session_ref = ${sessionRef}
            AND target_ref = ${this.targetRef}
        `
    return rows[0]
  }

  private async requireTarget(sessionRef: string, attachmentRef: string, expectedGeneration: number): Promise<TargetRow> {
    const row = await this.targetRow(sessionRef)
    if (row === undefined) {
      throw new PortableManagedAgentComputerTargetError("not_staged", "managed target has no retained resource")
    }
    if (row.attachment_ref !== attachmentRef || generation(row.generation) !== expectedGeneration) {
      throw new PortableManagedAgentComputerTargetError("stale_generation", "managed attachment generation is stale")
    }
    return row
  }

  private assertSameTarget(row: TargetRow, input: { attachmentRef: string; generation: number; checkpointRef: string; bundle: PortableCheckpointBundle }): void {
    if (row.attachment_ref !== input.attachmentRef || generation(row.generation) !== input.generation ||
        row.checkpoint_ref !== input.checkpointRef || !same(parseJson(row.bundle_json), input.bundle)) {
      throw new PortableManagedAgentComputerTargetError("conflict", "a different managed stage already exists")
    }
  }

  private validateStage(bundle: PortableCheckpointBundle, receipt: ManagedAgentComputerStageReceipt): void {
    if (!SAFE_REF.test(receipt.resourceRef) || receipt.acceptingWork !== false ||
        receipt.checkpointDigest !== bundle.checkpoint.digest ||
        receipt.repositoryPostImageDigest !== bundle.checkpoint.repositoryPostImageDigest ||
        receipt.diffDigest !== bundle.checkpoint.diffDigest ||
        receipt.graphDigest !== bundle.checkpoint.graphDigest ||
        !same(receipt.threadCursors, bundle.threadCursors)) {
      throw new PortableManagedAgentComputerTargetError("provisioner_rejected", "managed stage did not verify the exact checkpoint")
    }
    refs(receipt.evidenceRefs, "stage evidence")
  }

  private async authorityEvidence(sessionRef: string, attachmentRef: string, expectedGeneration: number): Promise<string> {
    const rows: Array<{ current_attachment_ref: string | null; current_attachment_generation: string | number; state: string; target_ref: string; evidence_refs_json: unknown }> = await this.config.sql`
      SELECT s.current_attachment_ref, s.current_attachment_generation,
             a.state, a.target_ref, a.evidence_refs_json
      FROM khala_sync_portable_sessions s
      JOIN khala_sync_portable_attachments a ON a.attachment_ref = s.current_attachment_ref
      WHERE s.session_ref = ${sessionRef} AND s.owner_user_id = ${this.config.ownerRef}
    `
    const row = rows[0]
    const evidence = row === undefined ? [] : parseJson<ReadonlyArray<string>>(row.evidence_refs_json)
    if (row === undefined || row.current_attachment_ref !== attachmentRef ||
        generation(row.current_attachment_generation) !== expectedGeneration || row.state !== "active" ||
        row.target_ref !== this.targetRef || evidence.length === 0) {
      throw new PortableManagedAgentComputerTargetError("authority_not_committed", "destination authority is not durably active")
    }
    return refs(evidence, "authority evidence")[0]!
  }

  private async assertAuthorizedSessionTarget(sessionRef: string): Promise<void> {
    const rows: Array<{ authorized: boolean }> = await this.config.sql`
      SELECT TRUE AS authorized
      FROM khala_sync_portable_sessions s
      JOIN khala_sync_portable_session_targets st ON st.session_ref = s.session_ref
      JOIN khala_sync_portable_targets t ON t.target_ref = st.target_ref
      WHERE s.session_ref = ${sessionRef}
        AND s.owner_user_id = ${this.config.ownerRef}
        AND t.owner_user_id = ${this.config.ownerRef}
        AND t.target_ref = ${this.targetRef}
        AND t.target_class = 'openagents_managed'
        AND t.health = 'ready'
    `
    if (rows[0]?.authorized !== true) {
      throw new PortableManagedAgentComputerTargetError(
        "invalid",
        "managed target is not authorized and ready for this owner session",
      )
    }
  }

  private async reclaimRow(sessionRef: string, expectedState: "staged" | "quiesced"): Promise<void> {
    await this.config.sql`
      UPDATE khala_sync_portable_managed_targets
      SET state = 'reclaimed', accepting_work = FALSE, updated_at = NOW()
      WHERE owner_user_id = ${this.config.ownerRef} AND session_ref = ${sessionRef}
        AND target_ref = ${this.targetRef} AND state = ${expectedState} AND accepting_work = FALSE
    `
    const row = await this.targetRow(sessionRef)
    if (row?.state !== "reclaimed" || row.accepting_work) {
      throw new PortableManagedAgentComputerTargetError("conflict", "managed resource reclaim was not retained")
    }
  }
}
