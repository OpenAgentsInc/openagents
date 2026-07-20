import { createHash } from "node:crypto"

import { canonicalJson } from "@openagentsinc/khala-sync"
import {
  type PortableSessionExecutionBinding,
  validateIdePortableDestinationActivationReceipt,
} from "@openagentsinc/portable-session-contract"
import { Effect } from "effect"

import type { PylonPortableCheckpointBundle } from "./portable-session-operation-ledger.js"
import { PylonPortableSessionOperationLedger } from "./portable-session-operation-ledger.js"
import type {
  PylonPortableDestinationLifecycle,
  PylonPortableTargetActivationReceipt,
  PylonPortableThreadCursor,
} from "./portable-session-target.js"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u

export type PylonPortableAuthorityAttachment = Readonly<{
  sessionRef: string
  targetRef: string
  attachmentRef: string
  generation: number
  state: "active" | "quiesced" | "reclaimed"
  checkpointRef?: string
  authorityEvidenceRef: string
}>

export type PylonPortableDestinationAuthority = Readonly<{
  readCurrentAttachment: (sessionRef: string) => Promise<PylonPortableAuthorityAttachment>
}>

export type PylonPortableLocalStage = Readonly<{
  operationRef: string
  sessionRef: string
  checkpointRef: string
  checkpointDigest: string
  sourceAttachmentRef: string
  sourceGeneration: number
  destinationAttachmentRef: string
  destinationGeneration: number
  repositoryPostImageDigest: string
  diffDigest: string
  graphDigest: string
  stagedAgentRefs: ReadonlyArray<string>
  threadCursors: ReadonlyArray<PylonPortableThreadCursor>
  capabilityLeaseRefs: ReadonlyArray<string>
  acceptingWork: false
  evidenceRefs: ReadonlyArray<string>
}>

/**
 * Private owner-local mechanics. Implementations may read repository/checkpoint
 * bytes and local paths, but none of those values may be returned through this
 * refs-only port or written into the portable operation ledger.
 */
export type PylonPortableLocalRehydrator = Readonly<{
  stage: (input: Readonly<{
    operationRef: string
    bundle: PylonPortableCheckpointBundle
    destinationAttachmentRef: string
    destinationGeneration: number
    capabilityLeaseRefs: ReadonlyArray<string>
  }>) => Promise<PylonPortableLocalStage>
  readStage: (operationRef: string) => Promise<PylonPortableLocalStage>
  activate: (input: Readonly<{
    operationRef: string
    stage: PylonPortableLocalStage
    authorityEvidenceRef: string
    executionBinding: PortableSessionExecutionBinding
  }>) => Promise<PylonPortableTargetActivationReceipt>
  abort: (input: Readonly<{
    operationRef: string
    stage: PylonPortableLocalStage
  }>) => Promise<Readonly<{
    cleanedAgentRefs: ReadonlyArray<string>
    releasedCapabilityLeaseRefs: ReadonlyArray<string>
    processes: "released"
    scratch: "released"
    ports: "released"
    evidenceRefs: ReadonlyArray<string>
  }>>
}>

export class PylonPortableDestinationError extends Error {
  readonly _tag = "PylonPortableDestinationError"
  override readonly name = "PylonPortableDestinationError"

  constructor(
    readonly reason:
      | "authority_mismatch"
      | "conflicting_replay"
      | "invalid_checkpoint"
      | "rehydration_failed"
      | "stale_generation",
    message: string,
  ) {
    super(message)
  }
}

const digest = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const checkpointDigest = (bundle: PylonPortableCheckpointBundle): `sha256:${string}` => {
  const { digest: _digest, ...payload } = bundle.checkpoint
  return digest(canonicalJson(payload))
}

const graphDigest = (bundle: PylonPortableCheckpointBundle): `sha256:${string}` => digest(canonicalJson({
  rootAgentRef: bundle.graph.rootAgentRef,
  nodes: [...bundle.graph.nodes].map(node => ({ ...node })).sort((left, right) =>
    left.agentRef.localeCompare(right.agentRef)),
}))

const canonicalCursors = (rows: ReadonlyArray<PylonPortableThreadCursor>): string => canonicalJson(
  [...rows].map(row => ({ ...row })).sort((left, right) => left.threadRef.localeCompare(right.threadRef)),
)

const sameRefs = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean => {
  const a = [...left].sort()
  const b = [...right].sort()
  return a.length === b.length && a.every((value, index) => value === b[index])
}

const assertRefs = (values: ReadonlyArray<string>, field: string): void => {
  if (values.some(value => !SAFE_REF.test(value)) || new Set(values).size !== values.length) {
    throw new PylonPortableDestinationError("invalid_checkpoint", `${field} is not unique refs-only data`)
  }
}

const assertBundle = (
  bundle: PylonPortableCheckpointBundle,
  destinationGeneration: number,
): void => {
  const checkpoint = bundle.checkpoint
  if (checkpoint.digest !== checkpointDigest(bundle) ||
      checkpoint.graphDigest !== graphDigest(bundle) ||
      checkpoint.sessionRef !== bundle.executionBinding.sessionRef ||
      checkpoint.repositoryRef !== bundle.executionBinding.repositoryRef ||
      destinationGeneration !== checkpoint.sourceGeneration + 1) {
    throw new PylonPortableDestinationError("invalid_checkpoint", "checkpoint binding or digest is invalid")
  }
  const nodes = bundle.graph.nodes
  const nodeRefs = nodes.map(node => node.agentRef)
  const threadRefs = nodes.map(node => node.threadRef)
  const transcriptRefs = nodes.map(node => node.transcriptRef)
  assertRefs(nodeRefs, "graph agents")
  assertRefs(threadRefs, "graph threads")
  assertRefs(transcriptRefs, "graph transcripts")
  if (!nodeRefs.includes(bundle.graph.rootAgentRef) ||
      nodes.some(node => node.attachmentGeneration !== checkpoint.sourceGeneration ||
        node.lifecycle === "created" || node.lifecycle === "running" || node.lifecycle === "waiting" ||
        node.lifecycle === "quiescing")) {
    throw new PylonPortableDestinationError("invalid_checkpoint", "checkpoint graph is not fully quiesced")
  }
  const cursors = new Map(bundle.threadCursors.map(row => [row.threadRef, row]))
  if (cursors.size !== nodes.length || nodes.some(node => {
    const cursor = cursors.get(node.threadRef)
    return cursor === undefined || cursor.transcriptRef !== node.transcriptRef ||
      cursor.activityCursor !== node.activityCursor
  })) {
    throw new PylonPortableDestinationError("invalid_checkpoint", "checkpoint thread cursors do not cover the graph")
  }
}

const stageOperationRefFor = (operationRef: string): string => {
  if (!operationRef.endsWith(".destination.activate") && !operationRef.endsWith(".destination.abort")) {
    throw new PylonPortableDestinationError("conflicting_replay", "destination operation does not bind a stage")
  }
  return `${operationRef.slice(0, operationRef.lastIndexOf(".destination."))}.destination.stage`
}

const runLedger = async <A>(effect: Effect.Effect<A, unknown>): Promise<A> => {
  try {
    return await Effect.runPromise(effect)
  } catch (error) {
    const reason = error instanceof Error && "reason" in error && typeof error.reason === "string"
      ? error.reason
      : "unknown"
    throw new PylonPortableDestinationError(
      reason === "conflicting_replay" ? "conflicting_replay" : "stale_generation",
      `portable destination ledger failed closed: ${reason}`,
    )
  }
}

const assertStage = (
  stage: PylonPortableLocalStage,
  input: Readonly<{
    operationRef: string
    bundle: PylonPortableCheckpointBundle
    destinationAttachmentRef: string
    destinationGeneration: number
    capabilityLeaseRefs: ReadonlyArray<string>
  }>,
): void => {
  const checkpoint = input.bundle.checkpoint
  if (stage.operationRef !== input.operationRef ||
      stage.sessionRef !== checkpoint.sessionRef ||
      stage.checkpointRef !== checkpoint.checkpointRef ||
      stage.checkpointDigest !== checkpoint.digest ||
      stage.sourceAttachmentRef !== checkpoint.sourceAttachmentRef ||
      stage.sourceGeneration !== checkpoint.sourceGeneration ||
      stage.destinationAttachmentRef !== input.destinationAttachmentRef ||
      stage.destinationGeneration !== input.destinationGeneration ||
      stage.repositoryPostImageDigest !== checkpoint.repositoryPostImageDigest ||
      stage.diffDigest !== checkpoint.diffDigest ||
      stage.graphDigest !== checkpoint.graphDigest ||
      stage.acceptingWork !== false ||
      !sameRefs(stage.stagedAgentRefs, input.bundle.graph.nodes.map(node => node.agentRef)) ||
      !sameRefs(stage.capabilityLeaseRefs, input.capabilityLeaseRefs) ||
      canonicalCursors(stage.threadCursors) !== canonicalCursors(input.bundle.threadCursors)) {
    throw new PylonPortableDestinationError("rehydration_failed", "local stage did not restore the exact checkpoint")
  }
  assertRefs(stage.evidenceRefs, "stage evidence")
}

export const createPylonOwnerLocalDestinationLifecycle = (input: Readonly<{
  targetRef: string
  ledger: PylonPortableSessionOperationLedger
  authority: PylonPortableDestinationAuthority
  rehydrator: PylonPortableLocalRehydrator
}>): PylonPortableDestinationLifecycle => ({
  stageCheckpoint: async operation => {
    assertBundle(operation.bundle, operation.destinationGeneration)
    assertRefs(operation.capabilityLeaseRefs, "capability leases")
    const checkpoint = operation.bundle.checkpoint
    const authority = await input.authority.readCurrentAttachment(checkpoint.sessionRef)
    assertRefs([authority.authorityEvidenceRef], "source authority evidence")
    if (authority.sessionRef !== checkpoint.sessionRef || authority.state !== "active" ||
        authority.attachmentRef !== checkpoint.sourceAttachmentRef ||
        authority.generation !== checkpoint.sourceGeneration ||
        authority.targetRef === input.targetRef) {
      throw new PylonPortableDestinationError("authority_mismatch", "durable source authority does not match checkpoint")
    }
    const exactInput = { ...operation, sourceAuthorityEvidenceRef: authority.authorityEvidenceRef }
    const admitted = await runLedger(input.ledger.admitDestinationOperation({
      operationRef: operation.operationRef,
      sessionRef: checkpoint.sessionRef,
      sourceAttachmentRef: checkpoint.sourceAttachmentRef,
      sourceGeneration: checkpoint.sourceGeneration,
      destinationAttachmentRef: operation.destinationAttachmentRef,
      destinationGeneration: operation.destinationGeneration,
      kind: "stage",
      exactInput,
    }))
    const stage = await input.rehydrator.stage(operation)
    assertStage(stage, operation)
    await runLedger(input.ledger.completeOperation({
      operationRef: operation.operationRef,
      outcome: {
        evidenceRefs: stage.evidenceRefs,
        checkpointRef: checkpoint.checkpointRef,
        repositoryPostImageDigest: checkpoint.repositoryPostImageDigest,
        diffDigest: checkpoint.diffDigest,
        graphDigest: checkpoint.graphDigest,
      },
    }))
    return {
      checkpointDigest: checkpoint.digest,
      repositoryPostImageDigest: checkpoint.repositoryPostImageDigest,
      diffDigest: checkpoint.diffDigest,
      graphDigest: checkpoint.graphDigest,
      threadCursors: stage.threadCursors,
      acceptingWork: false,
      evidenceRefs: admitted.record.state === "completed"
        ? admitted.record.outcome?.evidenceRefs ?? stage.evidenceRefs
        : stage.evidenceRefs,
    }
  },
  activate: async operation => {
    const stageOperationRef = stageOperationRefFor(operation.operationRef)
    const stage = await input.rehydrator.readStage(stageOperationRef)
    if (stage.sessionRef !== operation.sessionRef ||
        stage.checkpointRef !== operation.checkpointRef ||
        stage.destinationAttachmentRef !== operation.destinationAttachmentRef ||
        stage.destinationGeneration !== operation.destinationGeneration ||
        !sameRefs(stage.capabilityLeaseRefs, operation.capabilityLeaseRefs) ||
        operation.executionBinding.sessionRef !== operation.sessionRef) {
      throw new PylonPortableDestinationError("conflicting_replay", "activation does not match the staged checkpoint")
    }
    const authority = await input.authority.readCurrentAttachment(operation.sessionRef)
    assertRefs([authority.authorityEvidenceRef], "destination authority evidence")
    if (authority.sessionRef !== operation.sessionRef || authority.state !== "active" || authority.targetRef !== input.targetRef ||
        authority.attachmentRef !== operation.destinationAttachmentRef ||
        authority.generation !== operation.destinationGeneration ||
        authority.checkpointRef !== operation.checkpointRef) {
      throw new PylonPortableDestinationError("authority_mismatch", "local destination is not the durable active authority")
    }
    const activation = validateIdePortableDestinationActivationReceipt(await input.rehydrator.activate({
      operationRef: operation.operationRef,
      stage,
      authorityEvidenceRef: authority.authorityEvidenceRef,
      executionBinding: operation.executionBinding,
    }), {
      operationRef: operation.operationRef,
      sessionRef: operation.sessionRef,
      checkpointRef: operation.checkpointRef,
      destinationTargetRef: input.targetRef,
      destinationAttachmentRef: operation.destinationAttachmentRef,
      destinationGeneration: operation.destinationGeneration,
      authenticationPolicyRef: "policy.portable.destination.owner_local.v1",
    })
    if (!sameRefs(activation.activatedAgentRefs, stage.stagedAgentRefs) ||
        activation.acceptedWorkRefs.some(row => !stage.stagedAgentRefs.includes(row.agentRef)) ||
        new Set(activation.acceptedWorkRefs.map(row => `${row.agentRef}:${row.turnRef}`)).size !==
          activation.acceptedWorkRefs.length) {
      throw new PylonPortableDestinationError("rehydration_failed", "activation did not preserve the staged graph")
    }
    assertRefs(activation.activatedAgentRefs, "activated agents")
    if (activation.acceptedWorkRefs.some(row => !SAFE_REF.test(row.agentRef) || !SAFE_REF.test(row.turnRef))) {
      throw new PylonPortableDestinationError("rehydration_failed", "accepted work is not refs-only")
    }
    assertRefs(activation.evidenceRefs, "activation evidence")
    const committed = await runLedger(input.ledger.commitDestinationGeneration({
      operationRef: operation.operationRef,
      sessionRef: operation.sessionRef,
      sourceAttachmentRef: stage.sourceAttachmentRef,
      sourceGeneration: stage.sourceGeneration,
      destinationAttachmentRef: operation.destinationAttachmentRef,
      destinationGeneration: operation.destinationGeneration,
      stageOperationRef,
      authorityEvidenceRef: authority.authorityEvidenceRef,
      evidenceRefs: activation.evidenceRefs,
      exactInput: { ...operation, authorityEvidenceRef: authority.authorityEvidenceRef },
    }))
    return {
      ...activation,
      evidenceRefs: committed.record.outcome?.evidenceRefs ?? activation.evidenceRefs,
    }
  },
  abortStaged: async operation => {
    const stageOperationRef = stageOperationRefFor(operation.operationRef)
    const stage = await input.rehydrator.readStage(stageOperationRef)
    if (stage.sessionRef !== operation.sessionRef ||
        stage.destinationAttachmentRef !== operation.destinationAttachmentRef ||
        stage.destinationGeneration !== operation.destinationGeneration) {
      throw new PylonPortableDestinationError("conflicting_replay", "abort does not match the staged checkpoint")
    }
    const authority = await input.authority.readCurrentAttachment(operation.sessionRef)
    assertRefs([authority.authorityEvidenceRef], "abort authority evidence")
    if (authority.sessionRef !== operation.sessionRef) {
      throw new PylonPortableDestinationError("authority_mismatch", "abort authority belongs to another session")
    }
    if (authority.state === "active" && authority.targetRef === input.targetRef &&
        authority.attachmentRef === operation.destinationAttachmentRef &&
        authority.generation === operation.destinationGeneration) {
      throw new PylonPortableDestinationError("authority_mismatch", "an authoritative destination cannot be aborted")
    }
    await runLedger(input.ledger.admitDestinationOperation({
      operationRef: operation.operationRef,
      sessionRef: operation.sessionRef,
      sourceAttachmentRef: stage.sourceAttachmentRef,
      sourceGeneration: stage.sourceGeneration,
      destinationAttachmentRef: operation.destinationAttachmentRef,
      destinationGeneration: operation.destinationGeneration,
      kind: "abort",
      exactInput: operation,
    }))
    const cleanup = await input.rehydrator.abort({ operationRef: operation.operationRef, stage })
    if (!sameRefs(cleanup.cleanedAgentRefs, stage.stagedAgentRefs) ||
        !sameRefs(cleanup.releasedCapabilityLeaseRefs, stage.capabilityLeaseRefs) ||
        cleanup.processes !== "released" || cleanup.scratch !== "released" || cleanup.ports !== "released") {
      throw new PylonPortableDestinationError("rehydration_failed", "staged abort left destination residue")
    }
    assertRefs(cleanup.evidenceRefs, "abort evidence")
    const completed = await runLedger(input.ledger.completeOperation({
      operationRef: operation.operationRef,
      outcome: { evidenceRefs: cleanup.evidenceRefs },
    }))
    return { evidenceRefs: completed.record.outcome?.evidenceRefs ?? cleanup.evidenceRefs }
  },
})
