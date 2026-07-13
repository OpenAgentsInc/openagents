import { createHash } from "node:crypto"

import { canonicalJson } from "@openagentsinc/khala-sync"
import {
  PortableCapabilityBroker,
  PortableAgentGraphSchema,
  PortableCheckpointSchema,
  PortableSessionExecutionBindingSchema,
  type PortableAgentGraph,
  type PortableAttachment,
  type PortableCheckpoint,
  type PortableSessionCommand,
  type PortableSessionCommandOutcome,
  type PortableSessionExecutionBinding,
  type PortableTargetClass,
} from "@openagentsinc/portable-session-contract"
import { Effect, Schema as S } from "effect"

import type { SyncTransactionWriter } from "./outbox-writer.js"
import {
  completePortableSessionMove,
  computePortableAgentGraphDigest,
  quiescePortableSessionGraph,
  readPortableSessionAuthoritySnapshot,
  recordPortableSessionMoveFailure,
  requestPortableSessionCommand,
  type PortableSessionAuthoritySnapshot,
} from "./portable-session-authority.js"
import type { SqlTag, SyncSql } from "./sql.js"

export const PORTABLE_SESSION_MOVE_VERSION =
  "openagents.portable_session_move.v1" as const

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|secret|localPath|hostname|processId|authHome)"\s*:/iu

export type PortableThreadCursor = Readonly<{
  threadRef: string
  transcriptRef: string
  activityCursor: number
  eventCursor: number
}>

const PortableThreadCursorSchema = S.Struct({
  threadRef: S.String,
  transcriptRef: S.String,
  activityCursor: S.Number,
  eventCursor: S.Number,
})

const PortableCheckpointBundleSchema = S.Struct({
  checkpoint: PortableCheckpointSchema,
  executionBinding: PortableSessionExecutionBindingSchema,
  graph: PortableAgentGraphSchema,
  threadCursors: S.Array(PortableThreadCursorSchema),
})
const decodeCheckpointBundle = S.decodeUnknownSync(PortableCheckpointBundleSchema)

export type PortableCheckpointBundle = Readonly<{
  checkpoint: PortableCheckpoint
  executionBinding: PortableSessionExecutionBinding
  graph: PortableAgentGraph
  threadCursors: ReadonlyArray<PortableThreadCursor>
}>

export type PortableTargetStageReceipt = Readonly<{
  checkpointDigest: string
  repositoryPostImageDigest: string
  diffDigest: string
  graphDigest: string
  threadCursors: ReadonlyArray<PortableThreadCursor>
  acceptingWork: false
  evidenceRefs: ReadonlyArray<string>
}>

export type PortableTargetActivationReceipt = Readonly<{
  activatedAgentRefs: ReadonlyArray<string>
  acceptedWorkRefs: ReadonlyArray<Readonly<{
    agentRef: string
    turnRef: string
  }>>
  evidenceRefs: ReadonlyArray<string>
}>

export type PortableSourceCleanupReceipt = Readonly<{
  cleanedAgentRefs: ReadonlyArray<string>
  processes: "released"
  scratch: "released"
  ports: "released"
  evidenceRefs: ReadonlyArray<string>
}>

/**
 * Concrete owner-local Pylons and Agent Computers implement this same port.
 * Every operationRef is byte-idempotent. `stageCheckpoint` MUST NOT start or
 * accept work; `activate` runs only after PORT-01 commits the next generation.
 */
export type PortableSessionExecutionTarget = Readonly<{
  targetRef: string
  targetClass: PortableTargetClass
  quiesceGraph: (input: Readonly<{
    operationRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    graph: PortableAgentGraph
    threadCursors: ReadonlyArray<PortableThreadCursor>
  }>) => Promise<Readonly<{
    quiescedAgentRefs: ReadonlyArray<string>
    evidenceRefs: ReadonlyArray<string>
  }>>
  createCheckpoint: (input: Readonly<{
    operationRef: string
    checkpointRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    eventLogCursor: number
    executionBinding: PortableSessionExecutionBinding
    graph: PortableAgentGraph
    threadCursors: ReadonlyArray<PortableThreadCursor>
  }>) => Promise<PortableCheckpointBundle>
  cleanupSource: (input: Readonly<{
    operationRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    agentRefs: ReadonlyArray<string>
  }>) => Promise<PortableSourceCleanupReceipt>
  stageCheckpoint: (input: Readonly<{
    operationRef: string
    bundle: PortableCheckpointBundle
    destinationAttachmentRef: string
    destinationGeneration: number
    capabilityLeaseRefs: ReadonlyArray<string>
  }>) => Promise<PortableTargetStageReceipt>
  activate: (input: Readonly<{
    operationRef: string
    checkpointRef: string
    sessionRef: string
    executionBinding: PortableSessionExecutionBinding
    destinationAttachmentRef: string
    destinationGeneration: number
    capabilityLeaseRefs: ReadonlyArray<string>
  }>) => Promise<PortableTargetActivationReceipt>
  abortStaged: (input: Readonly<{
    operationRef: string
    sessionRef: string
    destinationAttachmentRef: string
    destinationGeneration: number
  }>) => Promise<Readonly<{ evidenceRefs: ReadonlyArray<string> }>>
}>

export type PortableCapabilityTransfer = Readonly<{
  sourceLeaseRef: string
  destinationLeaseRef: string
  destinationSourceGrantRef: string
  expiresAt: string
}>

export type PortableSessionMoveInput = Readonly<{
  command: PortableSessionCommand
  destinationAttachmentRef: string
  capabilityTransfers: ReadonlyArray<PortableCapabilityTransfer>
  source: PortableSessionExecutionTarget
  destination: PortableSessionExecutionTarget
}>

export type PortableSessionMoveResult = Readonly<{
  schema: typeof PORTABLE_SESSION_MOVE_VERSION
  status: "completed" | "replayed" | "authority_pending_reconcile" | "activation_pending_reconcile" | "failed"
  commandRef: string
  sessionRef: string
  runRef: string
  repositoryRef: string
  pinnedBaseRef: string
  sourceAttachmentRef: string
  sourceGeneration: number
  destinationAttachmentRef?: string
  destinationGeneration?: number
  checkpointRef?: string
  capabilityLeaseRefs: ReadonlyArray<string>
  acceptedWorkRefs: ReadonlyArray<Readonly<{ agentRef: string; turnRef: string }>>
  evidenceRefs: ReadonlyArray<string>
  reasonRef?: string
}>

export type PortableSessionMoveCoordinatorConfig = Readonly<{
  sql: SyncSql
  transaction: <A>(run: (writer: SyncTransactionWriter) => Promise<A>) => Promise<A>
  broker: PortableCapabilityBroker
}>

export class PortableSessionMoveError extends Error {
  readonly _tag = "PortableSessionMoveError"
  override readonly name = "PortableSessionMoveError"

  constructor(
    readonly reason:
      | "authority_rejected"
      | "broker_failed"
      | "checkpoint_invalid"
      | "destination_rejected"
      | "source_cleanup_failed"
      | "target_mismatch",
    message: string,
  ) {
    super(message)
  }
}

const digest = (value: unknown): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`

export const computePortableCheckpointDigest = (
  checkpoint: Omit<PortableCheckpoint, "digest"> | PortableCheckpoint,
): `sha256:${string}` => {
  const { digest: _ignored, ...payload } = checkpoint as PortableCheckpoint
  return digest(payload)
}

const parseJson = <A>(value: unknown): A =>
  (typeof value === "string" ? JSON.parse(value) : value) as A

const refs = (values: ReadonlyArray<string>, field: string): ReadonlyArray<string> => {
  const unique = [...new Set(values)]
  if (unique.length !== values.length || unique.some(value => !SAFE_REF.test(value))) {
    throw new PortableSessionMoveError("checkpoint_invalid", `${field} must contain unique public-safe refs`)
  }
  return unique
}

const same = (left: unknown, right: unknown): boolean =>
  canonicalJson(left) === canonicalJson(right)

const sortGraph = (graph: PortableAgentGraph): PortableAgentGraph => ({
  rootAgentRef: graph.rootAgentRef,
  nodes: [...graph.nodes].map(node => ({ ...node })).sort((a, b) => a.agentRef.localeCompare(b.agentRef)),
})

const sortCursors = (
  cursors: ReadonlyArray<PortableThreadCursor>,
): ReadonlyArray<PortableThreadCursor> =>
  [...cursors].map(cursor => ({ ...cursor })).sort((a, b) => a.threadRef.localeCompare(b.threadRef))

type AuthorityView = Readonly<{
  ownerRef: string
  sessionRef: string
  eventLogCursor: number
  sourceAttachmentRef: string
  sourceGeneration: number
  sourceTargetRef: string
  sourceTargetClass: PortableTargetClass
  sourceLeaseRefs: ReadonlyArray<string>
  targets: ReadonlyArray<Readonly<{ targetRef: string; targetClass: PortableTargetClass }>>
  executionBinding: PortableSessionExecutionBinding
  graph: PortableAgentGraph
  threadCursors: ReadonlyArray<PortableThreadCursor>
}>

const authorityView = (snapshot: PortableSessionAuthoritySnapshot): AuthorityView => {
  const session = snapshot.session
  const sourceAttachmentRef = String(session.current_attachment_ref)
  const sourceGeneration = Number(session.current_attachment_generation)
  const attachment = snapshot.attachments.find(row => row.attachment_ref === sourceAttachmentRef)
  if (attachment === undefined || Number(attachment.generation) !== sourceGeneration) {
    throw new PortableSessionMoveError("authority_rejected", "current attachment is absent from durable authority")
  }
  const sourceTarget = snapshot.targets.find(row => row.target_ref === attachment.target_ref)
  if (sourceTarget === undefined) {
    throw new PortableSessionMoveError("authority_rejected", "source target is not authorized for this session")
  }
  if (snapshot.executionBinding === null) {
    throw new PortableSessionMoveError(
      "authority_rejected",
      "portable movement requires a durable canonical execution binding",
    )
  }
  const executionBinding: PortableSessionExecutionBinding = {
    schema: "openagents.portable_session_execution_binding.v1",
    sessionRef: String(snapshot.executionBinding.session_ref),
    ownerRef: String(snapshot.executionBinding.owner_user_id),
    runRef: String(snapshot.executionBinding.run_ref),
    repositoryRef: String(snapshot.executionBinding.repository_ref),
    pinnedBaseRef: String(snapshot.executionBinding.pinned_base_ref),
  }
  if (executionBinding.sessionRef !== String(session.session_ref) ||
      executionBinding.ownerRef !== String(session.owner_user_id) ||
      [executionBinding.runRef, executionBinding.repositoryRef, executionBinding.pinnedBaseRef]
        .some(value => !SAFE_REF.test(value))) {
    throw new PortableSessionMoveError("authority_rejected", "canonical execution binding is invalid")
  }
  const graph: PortableAgentGraph = sortGraph({
    rootAgentRef: String(session.root_agent_ref),
    nodes: snapshot.agents.map(row => ({
      agentRef: String(row.agent_ref),
      ...(row.parent_agent_ref === null ? {} : { parentAgentRef: String(row.parent_agent_ref) }),
      threadRef: String(row.thread_ref),
      transcriptRef: String(row.transcript_ref),
      activityCursor: Number(row.activity_cursor),
      lifecycle: String(row.lifecycle) as PortableAgentGraph["nodes"][number]["lifecycle"],
      attachmentGeneration: Number(row.attachment_generation),
    })),
  })
  const currentByThread = new Map(snapshot.current.map(row => [String(row.thread_ref), row]))
  const threadCursors = sortCursors(graph.nodes.map(node => {
    const current = currentByThread.get(node.threadRef)
    if (current === undefined) {
      throw new PortableSessionMoveError("checkpoint_invalid", "every canonical thread requires a durable cursor")
    }
    return {
      threadRef: node.threadRef,
      transcriptRef: node.transcriptRef,
      activityCursor: node.activityCursor,
      eventCursor: Number(current.latest_cursor),
    }
  }))
  return {
    ownerRef: String(session.owner_user_id),
    sessionRef: String(session.session_ref),
    eventLogCursor: Number(session.latest_event_cursor),
    sourceAttachmentRef,
    sourceGeneration,
    sourceTargetRef: String(attachment.target_ref),
    sourceTargetClass: String(sourceTarget.target_class) as PortableTargetClass,
    sourceLeaseRefs: refs(parseJson<ReadonlyArray<string>>(attachment.capability_lease_refs_json), "source leases"),
    targets: snapshot.targets.map(row => ({
      targetRef: String(row.target_ref),
      targetClass: String(row.target_class) as PortableTargetClass,
    })),
    executionBinding,
    graph,
    threadCursors,
  }
}

const checkpointFromRow = (row: Record<string, unknown>): PortableCheckpoint => ({
  schema: "openagents.portable_checkpoint.v1",
  checkpointRef: String(row.checkpoint_ref),
  sessionRef: String(row.session_ref ?? "session.unknown"),
  sourceAttachmentRef: String(row.source_attachment_ref),
  sourceGeneration: Number(row.source_generation),
  digest: String(row.digest) as PortableCheckpoint["digest"],
  ...(row.parent_checkpoint_ref === null || row.parent_checkpoint_ref === undefined
    ? {}
    : { parentCheckpointRef: String(row.parent_checkpoint_ref) }),
  repositoryRef: String(row.repository_ref),
  repositoryRevisionRef: String(row.repository_revision_ref),
  repositoryPostImageDigest: String(row.repository_post_image_digest) as PortableCheckpoint["repositoryPostImageDigest"],
  diffDigest: String(row.diff_digest) as PortableCheckpoint["diffDigest"],
  eventLogCursor: Number(row.event_log_cursor),
  catalogGenerationRef: String(row.catalog_generation_ref),
  graphDigest: String(row.graph_digest) as PortableCheckpoint["graphDigest"],
  approvalRefs: parseJson<ReadonlyArray<string>>(row.approval_refs_json),
  artifactRefs: parseJson<ReadonlyArray<string>>(row.artifact_refs_json),
  receiptRefs: parseJson<ReadonlyArray<string>>(row.receipt_refs_json),
  secretMaterial: "excluded",
  processState: "excluded",
})

const commandOutcome = (
  command: PortableSessionCommand,
  status: "failed" | "unknown_pending_reconcile" | "completed",
  evidenceRefs: ReadonlyArray<string>,
  reasonRef?: string,
  destinationAttachmentRef?: string,
): PortableSessionCommandOutcome => ({
  commandRef: command.commandRef,
  sessionRef: command.sessionRef,
  status,
  sourceAttachmentRef: command.expectedAttachmentRef,
  sourceGeneration: command.expectedGeneration,
  ...(destinationAttachmentRef ? {
    destinationAttachmentRef,
    destinationGeneration: command.expectedGeneration + 1,
  } : {}),
  ...(command.checkpointRef ? { checkpointRef: command.checkpointRef } : {}),
  ...(reasonRef ? { reasonRef } : {}),
  evidenceRefs: refs(evidenceRefs, "outcome evidence"),
})

const ensureTargetMatches = (
  view: AuthorityView,
  input: PortableSessionMoveInput,
): void => {
  const destinationTarget = view.targets.find(row => row.targetRef === input.command.destinationTargetRef)
  if (input.source.targetRef !== view.sourceTargetRef ||
      input.source.targetClass !== view.sourceTargetClass ||
      destinationTarget === undefined ||
      input.destination.targetRef !== destinationTarget.targetRef ||
      input.destination.targetClass !== destinationTarget.targetClass ||
      input.source.targetRef === input.destination.targetRef) {
    throw new PortableSessionMoveError("target_mismatch", "runtime targets do not match durable movement authority")
  }
}

const validateTransfers = (
  view: AuthorityView,
  input: PortableSessionMoveInput,
  broker: PortableCapabilityBroker,
): void => {
  const sourceRefs = refs(input.capabilityTransfers.map(item => item.sourceLeaseRef), "transfer source leases")
  const destinationRefs = refs(input.capabilityTransfers.map(item => item.destinationLeaseRef), "transfer destination leases")
  if (!same([...sourceRefs].sort(), [...view.sourceLeaseRefs].sort())) {
    throw new PortableSessionMoveError("broker_failed", "every source attachment lease must transfer exactly once")
  }
  if (destinationRefs.some(value => view.sourceLeaseRefs.includes(value))) {
    throw new PortableSessionMoveError("broker_failed", "destination capability refs must be fresh")
  }
  const records = new Map(broker.snapshot().leases.map(record => [record.lease.leaseRef, record.lease]))
  for (const transfer of input.capabilityTransfers) {
    const lease = records.get(transfer.sourceLeaseRef)
    const destinationLease = records.get(transfer.destinationLeaseRef)
    if (lease === undefined ||
        lease.ownerRef !== view.ownerRef ||
        lease.sessionRef !== view.sessionRef ||
        lease.attachmentRef !== view.sourceAttachmentRef ||
        lease.attachmentGeneration !== view.sourceGeneration ||
        lease.targetRef !== view.sourceTargetRef ||
        (!["issued", "redeemed"].includes(lease.state) && !(
          lease.state === "revoked" &&
          destinationLease?.ownerRef === view.ownerRef &&
          destinationLease.sessionRef === view.sessionRef &&
          destinationLease.attachmentRef === input.destinationAttachmentRef &&
          destinationLease.attachmentGeneration === view.sourceGeneration + 1 &&
          destinationLease.targetRef === input.destination.targetRef &&
          ["issued", "redeemed"].includes(destinationLease.state)
        ))) {
      throw new PortableSessionMoveError("broker_failed", "source capability lease does not match current attachment")
    }
  }
}

const validateBundle = (view: AuthorityView, command: PortableSessionCommand, bundle: PortableCheckpointBundle): void => {
  let decoded: typeof PortableCheckpointBundleSchema.Type
  try {
    decoded = decodeCheckpointBundle(bundle)
  } catch {
    throw new PortableSessionMoveError("checkpoint_invalid", "source checkpoint bundle schema is invalid")
  }
  if (!same(decoded, bundle)) {
    throw new PortableSessionMoveError("checkpoint_invalid", "source checkpoint bundle contains unknown fields")
  }
  const checkpoint = bundle.checkpoint
  if (checkpoint.sessionRef !== view.sessionRef ||
      checkpoint.checkpointRef !== command.checkpointRef ||
      checkpoint.sourceAttachmentRef !== view.sourceAttachmentRef ||
      checkpoint.sourceGeneration !== view.sourceGeneration ||
      checkpoint.eventLogCursor !== view.eventLogCursor ||
      checkpoint.secretMaterial !== "excluded" ||
      checkpoint.processState !== "excluded" ||
      !same(bundle.executionBinding, view.executionBinding) ||
      checkpoint.repositoryRef !== view.executionBinding.repositoryRef ||
      !same(sortGraph(bundle.graph), view.graph) ||
      !same(sortCursors(bundle.threadCursors), view.threadCursors) ||
      checkpoint.graphDigest !== computePortableAgentGraphDigest(view.graph) ||
      checkpoint.digest !== computePortableCheckpointDigest(checkpoint)) {
    throw new PortableSessionMoveError("checkpoint_invalid", "source checkpoint does not exactly match durable authority")
  }
  if (FORBIDDEN_PRIVATE_MATERIAL.test(canonicalJson(bundle))) {
    throw new PortableSessionMoveError("checkpoint_invalid", "checkpoint contains forbidden private material")
  }
}

const validateStage = (bundle: PortableCheckpointBundle, receipt: PortableTargetStageReceipt): void => {
  if (receipt.acceptingWork !== false ||
      receipt.checkpointDigest !== bundle.checkpoint.digest ||
      receipt.repositoryPostImageDigest !== bundle.checkpoint.repositoryPostImageDigest ||
      receipt.diffDigest !== bundle.checkpoint.diffDigest ||
      receipt.graphDigest !== bundle.checkpoint.graphDigest ||
      !same(sortCursors(receipt.threadCursors), sortCursors(bundle.threadCursors))) {
    throw new PortableSessionMoveError("destination_rejected", "destination did not verify the exact checkpoint")
  }
  refs(receipt.evidenceRefs, "stage evidence")
}

const validateCleanup = (graph: PortableAgentGraph, receipt: PortableSourceCleanupReceipt): void => {
  if (!same([...receipt.cleanedAgentRefs].sort(), graph.nodes.map(node => node.agentRef).sort()) ||
      receipt.processes !== "released" || receipt.scratch !== "released" || receipt.ports !== "released") {
    throw new PortableSessionMoveError("source_cleanup_failed", "source cleanup does not cover the complete graph")
  }
  refs(receipt.evidenceRefs, "cleanup evidence")
}

const validateActivation = (
  graph: PortableAgentGraph,
  receipt: PortableTargetActivationReceipt,
): void => {
  if (!same([...receipt.activatedAgentRefs].sort(), graph.nodes.map(node => node.agentRef).sort())) {
    throw new PortableSessionMoveError("destination_rejected", "destination activation does not cover the complete graph")
  }
  const keys = receipt.acceptedWorkRefs.map(item => `${item.agentRef}:${item.turnRef}`)
  refs(receipt.evidenceRefs, "activation evidence")
  if (new Set(keys).size !== keys.length || receipt.acceptedWorkRefs.some(item =>
    !SAFE_REF.test(item.agentRef) || !SAFE_REF.test(item.turnRef) ||
    !graph.nodes.some(node => node.agentRef === item.agentRef))) {
    throw new PortableSessionMoveError("destination_rejected", "destination accepted work for an unknown agent")
  }
}

const reasonRef = (error: unknown): string => {
  const reason = error instanceof PortableSessionMoveError ? error.reason : "authority_rejected"
  return `reason.portable_move.${reason}`
}

const resultBinding = (view: AuthorityView) => ({
  runRef: view.executionBinding.runRef,
  repositoryRef: view.executionBinding.repositoryRef,
  pinnedBaseRef: view.executionBinding.pinnedBaseRef,
})

export class PortableSessionMoveCoordinator {
  constructor(private readonly config: PortableSessionMoveCoordinatorConfig) {}

  async move(input: PortableSessionMoveInput): Promise<PortableSessionMoveResult> {
    if (!["move", "attach", "failback"].includes(input.command.kind) ||
        input.command.checkpointRef === undefined ||
        input.command.destinationTargetRef === undefined ||
        !SAFE_REF.test(input.destinationAttachmentRef)) {
      throw new PortableSessionMoveError("authority_rejected", "movement command is incomplete")
    }
    let snapshot = await readPortableSessionAuthoritySnapshot(
      this.config.sql as unknown as SqlTag,
      { sessionRef: input.command.sessionRef, ownerUserId: input.command.ownerRef },
    )
    if (snapshot === null) throw new PortableSessionMoveError("authority_rejected", "portable session is unavailable")

    const existing = snapshot.commands.find(row => row.command_ref === input.command.commandRef)
    if (existing !== undefined && !same(parseJson(existing.command_json), input.command)) {
      throw new PortableSessionMoveError("authority_rejected", "movement command ref was replayed with different bytes")
    }
    if (existing?.status === "completed") {
      return this.reconcileCompleted(input, snapshot)
    }
    if (existing !== undefined && ["failed", "unknown_pending_reconcile", "rejected", "expired"].includes(String(existing.status))) {
      const outcome = parseJson<PortableSessionCommandOutcome>(existing.outcome_json)
      const failedView = authorityView(snapshot)
      return {
        schema: PORTABLE_SESSION_MOVE_VERSION,
        status: "failed",
        commandRef: input.command.commandRef,
        sessionRef: input.command.sessionRef,
        ...resultBinding(failedView),
        sourceAttachmentRef: input.command.expectedAttachmentRef,
        sourceGeneration: input.command.expectedGeneration,
        capabilityLeaseRefs: [],
        acceptedWorkRefs: [],
        evidenceRefs: outcome.evidenceRefs,
        ...(outcome.reasonRef ? { reasonRef: outcome.reasonRef } : {}),
      }
    }

    let view = authorityView(snapshot)
    ensureTargetMatches(view, input)
    validateTransfers(view, input, this.config.broker)
    const durableEvidence: string[] = []
    const destinationLeaseRefs: string[] = []
    let staged = false
    let authorityCompleted = false
    let bundle: PortableCheckpointBundle | undefined

    try {
      await this.config.transaction(writer => requestPortableSessionCommand(
        writer,
        input.command,
        input.command.ownerRef,
        `mutation.${input.command.commandRef}.accept`,
      ))
      const quiesce = await input.source.quiesceGraph({
        operationRef: `operation.${input.command.commandRef}.source.quiesce`,
        sessionRef: view.sessionRef,
        attachmentRef: view.sourceAttachmentRef,
        generation: view.sourceGeneration,
        graph: view.graph,
        threadCursors: view.threadCursors,
      }).catch(() => {
        throw new PortableSessionMoveError("source_cleanup_failed", "source graph could not quiesce")
      })
      if (!same([...quiesce.quiescedAgentRefs].sort(), view.graph.nodes.map(node => node.agentRef).sort())) {
        throw new PortableSessionMoveError("source_cleanup_failed", "source quiescence does not cover the complete graph")
      }
      durableEvidence.push(...refs(quiesce.evidenceRefs, "quiesce evidence"))
      await this.config.transaction(writer => quiescePortableSessionGraph(writer, {
        commandRef: input.command.commandRef,
        descendantAgentRefs: quiesce.quiescedAgentRefs,
        evidenceRefs: quiesce.evidenceRefs,
      }, `mutation.${input.command.commandRef}.quiesce`))

      snapshot = await readPortableSessionAuthoritySnapshot(
        this.config.sql as unknown as SqlTag,
        { sessionRef: input.command.sessionRef, ownerUserId: input.command.ownerRef },
      )
      if (snapshot === null) throw new PortableSessionMoveError("authority_rejected", "portable session disappeared")
      view = authorityView(snapshot)
      bundle = await input.source.createCheckpoint({
        operationRef: `operation.${input.command.commandRef}.checkpoint`,
        checkpointRef: input.command.checkpointRef,
        sessionRef: view.sessionRef,
        attachmentRef: view.sourceAttachmentRef,
        generation: view.sourceGeneration,
        eventLogCursor: view.eventLogCursor,
        executionBinding: view.executionBinding,
        graph: view.graph,
        threadCursors: view.threadCursors,
      }).catch(() => {
        throw new PortableSessionMoveError("checkpoint_invalid", "source checkpoint creation failed")
      })
      validateBundle(view, input.command, bundle)
      durableEvidence.push(...bundle.checkpoint.receiptRefs)

      for (const transfer of input.capabilityTransfers) {
        destinationLeaseRefs.push(transfer.destinationLeaseRef)
        const moved = await Effect.runPromise(this.config.broker.reissue({
          operationRef: `operation.${input.command.commandRef}.capability.${transfer.sourceLeaseRef}.reissue`,
          leaseRef: transfer.sourceLeaseRef,
          newLeaseRef: transfer.destinationLeaseRef,
          destinationSourceGrantRef: transfer.destinationSourceGrantRef,
          destinationAttachmentRef: input.destinationAttachmentRef,
          destinationAttachmentGeneration: view.sourceGeneration + 1,
          destinationTargetRef: input.destination.targetRef,
          expiresAt: transfer.expiresAt,
        })).catch(() => {
          throw new PortableSessionMoveError("broker_failed", "capability reissue failed closed")
        })
        durableEvidence.push(...moved.evidenceRefs)
        const redeemed = await Effect.runPromise(this.config.broker.redeem({
          operationRef: `operation.${input.command.commandRef}.capability.${transfer.destinationLeaseRef}.redeem`,
          leaseRef: transfer.destinationLeaseRef,
        })).catch(() => {
          throw new PortableSessionMoveError("broker_failed", "destination capability redemption failed closed")
        })
        durableEvidence.push(...redeemed.evidenceRefs)
      }

      staged = true
      const stage = await input.destination.stageCheckpoint({
        operationRef: `operation.${input.command.commandRef}.destination.stage`,
        bundle,
        destinationAttachmentRef: input.destinationAttachmentRef,
        destinationGeneration: view.sourceGeneration + 1,
        capabilityLeaseRefs: destinationLeaseRefs,
      }).catch(() => {
        throw new PortableSessionMoveError("destination_rejected", "destination rejected checkpoint staging")
      })
      validateStage(bundle, stage)
      durableEvidence.push(...stage.evidenceRefs)

      const cleanup = await input.source.cleanupSource({
        operationRef: `operation.${input.command.commandRef}.source.cleanup`,
        sessionRef: view.sessionRef,
        attachmentRef: view.sourceAttachmentRef,
        generation: view.sourceGeneration,
        agentRefs: view.graph.nodes.map(node => node.agentRef),
      }).catch(() => {
        throw new PortableSessionMoveError("source_cleanup_failed", "source cleanup failed closed")
      })
      validateCleanup(view.graph, cleanup)
      durableEvidence.push(...cleanup.evidenceRefs)

      const destinationAttachment: PortableAttachment = {
        attachmentRef: input.destinationAttachmentRef,
        sessionRef: view.sessionRef,
        targetRef: input.destination.targetRef,
        generation: view.sourceGeneration + 1,
        state: "active",
        descendantAgentRefs: view.graph.nodes.map(node => node.agentRef),
        capabilityLeaseRefs: destinationLeaseRefs,
        checkpointRef: bundle.checkpoint.checkpointRef,
        evidenceRefs: refs(durableEvidence, "move evidence"),
      }
      const outcome = commandOutcome(
        input.command,
        "completed",
        destinationAttachment.evidenceRefs,
        undefined,
        input.destinationAttachmentRef,
      )
      try {
        await this.config.transaction(writer => completePortableSessionMove(writer, {
          commandRef: input.command.commandRef,
          checkpoint: bundle!.checkpoint,
          destinationAttachment,
          outcome,
        }, `mutation.${input.command.commandRef}.complete`))
      } catch {
        try {
          const afterUnknown = await readPortableSessionAuthoritySnapshot(
            this.config.sql as unknown as SqlTag,
            { sessionRef: input.command.sessionRef, ownerUserId: input.command.ownerRef },
          )
          if (afterUnknown?.commands.some(row =>
            row.command_ref === input.command.commandRef && row.status === "completed")) {
            return this.reconcileCompleted(input, afterUnknown)
          }
        } catch {
          // The completion outcome is still unknown; destructive compensation is forbidden.
        }
        return {
          schema: PORTABLE_SESSION_MOVE_VERSION,
          status: "authority_pending_reconcile",
          commandRef: input.command.commandRef,
          sessionRef: view.sessionRef,
          ...resultBinding(view),
          sourceAttachmentRef: view.sourceAttachmentRef,
          sourceGeneration: view.sourceGeneration,
          destinationAttachmentRef: input.destinationAttachmentRef,
          destinationGeneration: view.sourceGeneration + 1,
          checkpointRef: bundle.checkpoint.checkpointRef,
          capabilityLeaseRefs: destinationLeaseRefs,
          acceptedWorkRefs: [],
          evidenceRefs: refs(durableEvidence, "result evidence"),
          reasonRef: "reason.portable_move.authority_pending_reconcile",
        }
      }
      authorityCompleted = true

      try {
        const activation = await input.destination.activate({
          operationRef: `operation.${input.command.commandRef}.destination.activate`,
          checkpointRef: bundle.checkpoint.checkpointRef,
          sessionRef: view.sessionRef,
          executionBinding: view.executionBinding,
          destinationAttachmentRef: input.destinationAttachmentRef,
          destinationGeneration: view.sourceGeneration + 1,
          capabilityLeaseRefs: destinationLeaseRefs,
        })
        validateActivation(view.graph, activation)
        return {
          schema: PORTABLE_SESSION_MOVE_VERSION,
          status: "completed",
          commandRef: input.command.commandRef,
          sessionRef: view.sessionRef,
          ...resultBinding(view),
          sourceAttachmentRef: view.sourceAttachmentRef,
          sourceGeneration: view.sourceGeneration,
          destinationAttachmentRef: input.destinationAttachmentRef,
          destinationGeneration: view.sourceGeneration + 1,
          checkpointRef: bundle.checkpoint.checkpointRef,
          capabilityLeaseRefs: destinationLeaseRefs,
          acceptedWorkRefs: activation.acceptedWorkRefs,
          evidenceRefs: refs([...durableEvidence, ...activation.evidenceRefs], "result evidence"),
        }
      } catch {
        return {
          schema: PORTABLE_SESSION_MOVE_VERSION,
          status: "activation_pending_reconcile",
          commandRef: input.command.commandRef,
          sessionRef: view.sessionRef,
          ...resultBinding(view),
          sourceAttachmentRef: view.sourceAttachmentRef,
          sourceGeneration: view.sourceGeneration,
          destinationAttachmentRef: input.destinationAttachmentRef,
          destinationGeneration: view.sourceGeneration + 1,
          checkpointRef: bundle.checkpoint.checkpointRef,
          capabilityLeaseRefs: destinationLeaseRefs,
          acceptedWorkRefs: [],
          evidenceRefs: refs(durableEvidence, "result evidence"),
          reasonRef: "reason.portable_move.activation_pending_reconcile",
        }
      }
    } catch (error) {
      if (authorityCompleted) throw error
      if (staged) {
        try {
          const aborted = await input.destination.abortStaged({
            operationRef: `operation.${input.command.commandRef}.destination.abort`,
            sessionRef: input.command.sessionRef,
            destinationAttachmentRef: input.destinationAttachmentRef,
            destinationGeneration: input.command.expectedGeneration + 1,
          })
          durableEvidence.push(...refs(aborted.evidenceRefs, "abort evidence"))
        } catch {
          durableEvidence.push(`evidence.${input.command.commandRef}.destination.abort_failed`)
        }
      }
      for (const leaseRef of destinationLeaseRefs) {
        try {
          const released = await Effect.runPromise(this.config.broker.release({
            operationRef: `operation.${input.command.commandRef}.capability.${leaseRef}.release`,
            leaseRef,
          }))
          durableEvidence.push(...released.evidenceRefs)
        } catch {
          durableEvidence.push(`evidence.${input.command.commandRef}.capability.release_failed`)
        }
      }
      const failureReasonRef = reasonRef(error)
      const failureEvidence = refs(
        durableEvidence.length > 0 ? durableEvidence : [`evidence.${input.command.commandRef}.failed`],
        "failure evidence",
      )
      try {
        await this.config.transaction(writer => recordPortableSessionMoveFailure(
          writer,
          commandOutcome(input.command, "failed", failureEvidence, failureReasonRef),
          `mutation.${input.command.commandRef}.failed`,
        ))
      } catch {
        // Command admission failures have no durable command row to terminate.
      }
      return {
        schema: PORTABLE_SESSION_MOVE_VERSION,
        status: "failed",
        commandRef: input.command.commandRef,
        sessionRef: input.command.sessionRef,
        ...resultBinding(view),
        sourceAttachmentRef: input.command.expectedAttachmentRef,
        sourceGeneration: input.command.expectedGeneration,
        capabilityLeaseRefs: destinationLeaseRefs,
        acceptedWorkRefs: [],
        evidenceRefs: failureEvidence,
        reasonRef: failureReasonRef,
      }
    }
  }

  private async reconcileCompleted(
    input: PortableSessionMoveInput,
    snapshot: PortableSessionAuthoritySnapshot,
  ): Promise<PortableSessionMoveResult> {
    const commandRow = snapshot.commands.find(row => row.command_ref === input.command.commandRef)!
    const outcome = parseJson<PortableSessionCommandOutcome>(commandRow.outcome_json)
    const destination = snapshot.attachments.find(row => row.attachment_ref === outcome.destinationAttachmentRef)
    const checkpointRow = snapshot.checkpoints.find(row => row.checkpoint_ref === outcome.checkpointRef)
    if (destination === undefined || checkpointRow === undefined ||
        String(destination.target_ref) !== input.destination.targetRef ||
        Number(destination.generation) !== input.command.expectedGeneration + 1) {
      throw new PortableSessionMoveError("authority_rejected", "completed move replay does not match durable destination")
    }
    const graph = sortGraph({
      rootAgentRef: String(snapshot.session.root_agent_ref),
      nodes: snapshot.agents.map(row => ({
        agentRef: String(row.agent_ref),
        ...(row.parent_agent_ref === null ? {} : { parentAgentRef: String(row.parent_agent_ref) }),
        threadRef: String(row.thread_ref),
        transcriptRef: String(row.transcript_ref),
        activityCursor: Number(row.activity_cursor),
        lifecycle: String(row.lifecycle) as PortableAgentGraph["nodes"][number]["lifecycle"],
        attachmentGeneration: input.command.expectedGeneration,
      })),
    })
    const checkpoint = { ...checkpointFromRow({ ...checkpointRow, session_ref: input.command.sessionRef }) }
    const view = authorityView(snapshot)
    const capabilityLeaseRefs = refs(
      parseJson<ReadonlyArray<string>>(destination.capability_lease_refs_json),
      "destination leases",
    )
    try {
      const activation = await input.destination.activate({
        operationRef: `operation.${input.command.commandRef}.destination.activate`,
        checkpointRef: checkpoint.checkpointRef,
        sessionRef: input.command.sessionRef,
        executionBinding: view.executionBinding,
        destinationAttachmentRef: String(destination.attachment_ref),
        destinationGeneration: Number(destination.generation),
        capabilityLeaseRefs,
      })
      validateActivation(graph, activation)
      return {
        schema: PORTABLE_SESSION_MOVE_VERSION,
        status: "replayed",
        commandRef: input.command.commandRef,
        sessionRef: input.command.sessionRef,
        ...resultBinding(view),
        sourceAttachmentRef: input.command.expectedAttachmentRef,
        sourceGeneration: input.command.expectedGeneration,
        destinationAttachmentRef: String(destination.attachment_ref),
        destinationGeneration: Number(destination.generation),
        checkpointRef: checkpoint.checkpointRef,
        capabilityLeaseRefs,
        acceptedWorkRefs: activation.acceptedWorkRefs,
        evidenceRefs: refs([...outcome.evidenceRefs, ...activation.evidenceRefs], "replay evidence"),
      }
    } catch {
      return {
        schema: PORTABLE_SESSION_MOVE_VERSION,
        status: "activation_pending_reconcile",
        commandRef: input.command.commandRef,
        sessionRef: input.command.sessionRef,
        ...resultBinding(view),
        sourceAttachmentRef: input.command.expectedAttachmentRef,
        sourceGeneration: input.command.expectedGeneration,
        destinationAttachmentRef: String(destination.attachment_ref),
        destinationGeneration: Number(destination.generation),
        checkpointRef: checkpoint.checkpointRef,
        capabilityLeaseRefs,
        acceptedWorkRefs: [],
        evidenceRefs: outcome.evidenceRefs,
        reasonRef: "reason.portable_move.activation_pending_reconcile",
      }
    }
  }
}
