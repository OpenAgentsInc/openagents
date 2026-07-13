import { createHash } from "node:crypto"
import { lstat, readFile, readlink } from "node:fs/promises"
import { join } from "node:path"

import { canonicalJson } from "@openagentsinc/khala-sync"
import type {
  PortableAgentGraph,
  PortableCheckpoint,
  PortableSessionExecutionBinding,
  PortableTargetClass,
} from "@openagentsinc/portable-session-contract"
import { Effect } from "effect"

import type { PylonPortableControlSessionLifecycle } from "./node/control-sessions.js"
import type { PylonPortableCheckpointArtifactStore } from "./portable-session-checkpoint-artifact.js"
import {
  type PylonPortableCheckpointBundle,
  PylonPortableSessionOperationLedger,
} from "./portable-session-operation-ledger.js"

export type PylonPortableThreadCursor = Readonly<{
  threadRef: string
  transcriptRef: string
  activityCursor: number
  eventCursor: number
}>

export type PylonPortableSourceCleanupReceipt = Readonly<{
  cleanedAgentRefs: ReadonlyArray<string>
  processes: "released"
  scratch: "released"
  ports: "released"
  evidenceRefs: ReadonlyArray<string>
}>

export type PylonPortableTargetStageReceipt = Readonly<{
  checkpointDigest: string
  repositoryPostImageDigest: string
  diffDigest: string
  graphDigest: string
  threadCursors: ReadonlyArray<PylonPortableThreadCursor>
  acceptingWork: false
  evidenceRefs: ReadonlyArray<string>
}>

export type PylonPortableTargetActivationReceipt = Readonly<{
  activatedAgentRefs: ReadonlyArray<string>
  acceptedWorkRefs: ReadonlyArray<Readonly<{ agentRef: string; turnRef: string }>>
  evidenceRefs: ReadonlyArray<string>
}>

export type PylonPortableDestinationLifecycle = Readonly<{
  stageCheckpoint: (input: Readonly<{
    operationRef: string
    bundle: PylonPortableCheckpointBundle
    destinationAttachmentRef: string
    destinationGeneration: number
    capabilityLeaseRefs: ReadonlyArray<string>
  }>) => Promise<PylonPortableTargetStageReceipt>
  activate: (input: Readonly<{
    operationRef: string
    checkpointRef: string
    sessionRef: string
    executionBinding: PortableSessionExecutionBinding
    destinationAttachmentRef: string
    destinationGeneration: number
    capabilityLeaseRefs: ReadonlyArray<string>
  }>) => Promise<PylonPortableTargetActivationReceipt>
  abortStaged: (input: Readonly<{
    operationRef: string
    sessionRef: string
    destinationAttachmentRef: string
    destinationGeneration: number
  }>) => Promise<Readonly<{ evidenceRefs: ReadonlyArray<string> }>>
}>

export type PylonOwnerLocalExecutionTarget = Readonly<{
  targetRef: string
  targetClass: PortableTargetClass
  quiesceGraph: (input: Readonly<{
    operationRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    graph: PortableAgentGraph
    threadCursors: ReadonlyArray<PylonPortableThreadCursor>
  }>) => Promise<Readonly<{ quiescedAgentRefs: ReadonlyArray<string>; evidenceRefs: ReadonlyArray<string> }>>
  createCheckpoint: (input: Readonly<{
    operationRef: string
    checkpointRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    eventLogCursor: number
    executionBinding: PortableSessionExecutionBinding
    graph: PortableAgentGraph
    threadCursors: ReadonlyArray<PylonPortableThreadCursor>
  }>) => Promise<PylonPortableCheckpointBundle>
  cleanupSource: (input: Readonly<{
    operationRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    agentRefs: ReadonlyArray<string>
  }>) => Promise<PylonPortableSourceCleanupReceipt>
  stageCheckpoint: PylonPortableDestinationLifecycle["stageCheckpoint"]
  activate: PylonPortableDestinationLifecycle["activate"]
  abortStaged: PylonPortableDestinationLifecycle["abortStaged"]
}>

export class PylonPortableTargetError extends Error {
  readonly _tag = "PylonPortableTargetError"
  override readonly name = "PylonPortableTargetError"

  constructor(
    readonly reason:
      | "checkpoint_failed"
      | "destination_not_configured"
      | "invalid_binding"
      | "operation_failed",
    message: string,
  ) {
    super(message)
  }
}

const digest = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`

const graphDigest = (graph: PortableAgentGraph): `sha256:${string}` => digest(canonicalJson({
  rootAgentRef: graph.rootAgentRef,
  nodes: [...graph.nodes].map(node => ({ ...node })).sort((left, right) =>
    left.agentRef.localeCompare(right.agentRef)),
}))

const checkpointDigest = (checkpoint: Omit<PortableCheckpoint, "digest">): `sha256:${string}` =>
  digest(canonicalJson(checkpoint))

const runGit = async (cwd: string, args: ReadonlyArray<string>): Promise<Uint8Array> => {
  const proc = Bun.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" })
  const [stdout, _stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).bytes(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) throw new PylonPortableTargetError("checkpoint_failed", "git checkpoint command failed")
  return stdout
}

const repositoryEntryBytes = async (cwd: string, relativePath: string): Promise<Uint8Array> => {
  const path = join(cwd, relativePath)
  const info = await lstat(path)
  return info.isSymbolicLink()
    ? new TextEncoder().encode(await readlink(path))
    : readFile(path)
}

const repositorySnapshot = async (cwd: string) => {
  const revision = new TextDecoder().decode(await runGit(cwd, ["rev-parse", "HEAD"])).trim()
  if (!/^[a-f0-9]{40,64}$/u.test(revision)) {
    throw new PylonPortableTargetError("checkpoint_failed", "repository revision is not a pinned commit")
  }
  const listed = new TextDecoder().decode(await runGit(cwd, ["ls-files", "-co", "--exclude-standard", "-z"]))
    .split("\0").filter(Boolean).sort()
  const postImage = createHash("sha256")
  for (const relativePath of listed) {
    const bytes = await repositoryEntryBytes(cwd, relativePath)
    postImage.update(relativePath).update("\0").update(bytes).update("\0")
  }
  const trackedDiff = await runGit(cwd, ["diff", "--binary", "HEAD", "--"])
  const untracked = new TextDecoder().decode(await runGit(cwd, ["ls-files", "--others", "--exclude-standard", "-z"]))
    .split("\0").filter(Boolean).sort()
  const diff = createHash("sha256").update(trackedDiff)
  for (const relativePath of untracked) {
    diff.update(relativePath).update("\0").update(await repositoryEntryBytes(cwd, relativePath)).update("\0")
  }
  return {
    repositoryRevisionRef: revision,
    repositoryPostImageDigest: `sha256:${postImage.digest("hex")}` as `sha256:${string}`,
    diffDigest: `sha256:${diff.digest("hex")}` as `sha256:${string}`,
  }
}

const runLedger = <A>(effect: Effect.Effect<A, unknown>): Promise<A> =>
  Effect.runPromise(effect).catch((error) => {
    const reason = error instanceof Error && "reason" in error && typeof error.reason === "string"
      ? error.reason
      : "unknown"
    throw new PylonPortableTargetError("operation_failed", `portable operation ledger failed closed: ${reason}`)
  })

export const createPylonOwnerLocalExecutionTarget = async (input: Readonly<{
  targetRef: string
  ledger: PylonPortableSessionOperationLedger
  lifecycle: PylonPortableControlSessionLifecycle
  binding: Readonly<{
    sessionRef: string
    attachmentRef: string
    generation: number
    agents: ReadonlyArray<Readonly<{ agentRef: string; controlSessionRef: string }>>
  }>
  destination?: PylonPortableDestinationLifecycle
  checkpointArtifacts?: Pick<PylonPortableCheckpointArtifactStore, "register">
}>): Promise<PylonOwnerLocalExecutionTarget> => {
  try {
    const fence = await Effect.runPromise(input.ledger.readSession(input.binding.sessionRef))
    if (fence.attachmentRef !== input.binding.attachmentRef || fence.generation !== input.binding.generation) {
      throw new PylonPortableTargetError("invalid_binding", "durable portable session fence is stale")
    }
  } catch (error) {
    if (error instanceof PylonPortableTargetError) throw error
    if (!(error instanceof Error) || !("reason" in error) || error.reason !== "not_found") {
      throw new PylonPortableTargetError("operation_failed", "portable operation ledger failed closed")
    }
    await runLedger(input.ledger.registerSession({
      sessionRef: input.binding.sessionRef,
      attachmentRef: input.binding.attachmentRef,
      generation: input.binding.generation,
      acceptingWork: true,
    }))
  }
  input.lifecycle.bind(input.binding)

  const destinationUnavailable = async (): Promise<never> => {
    throw new PylonPortableTargetError(
      "destination_not_configured",
      "owner-local destination rehydration is not configured",
    )
  }

  return {
    targetRef: input.targetRef,
    targetClass: "owner_local",
    quiesceGraph: async (operation) => {
      if (operation.sessionRef !== input.binding.sessionRef) {
        throw new PylonPortableTargetError("invalid_binding", "portable session binding does not match")
      }
      const agentRefs = operation.graph.nodes.map(node => node.agentRef)
      const quiesced = await input.lifecycle.quiesce({
        sessionRef: operation.sessionRef,
        attachmentRef: operation.attachmentRef,
        generation: operation.generation,
        agentRefs,
      })
      const result = await runLedger(input.ledger.quiesceGeneration({
        operationRef: operation.operationRef,
        sessionRef: operation.sessionRef,
        attachmentRef: operation.attachmentRef,
        generation: operation.generation,
        evidenceRefs: quiesced.evidenceRefs,
      }))
      return {
        quiescedAgentRefs: agentRefs,
        evidenceRefs: result.record.outcome?.evidenceRefs ?? quiesced.evidenceRefs,
      }
    },
    createCheckpoint: async (operation) => {
      const admitted = await runLedger(input.ledger.admitOperation({
        operationRef: operation.operationRef,
        sessionRef: operation.sessionRef,
        attachmentRef: operation.attachmentRef,
        generation: operation.generation,
        kind: "checkpoint",
      }))
      if (admitted.record.state === "completed") {
        const bundle = await runLedger(input.ledger.readCheckpointBundle(operation.operationRef))
        if (input.checkpointArtifacts !== undefined) {
          const source = await input.lifecycle.checkpointSource({
            sessionRef: operation.sessionRef,
            attachmentRef: operation.attachmentRef,
            generation: operation.generation,
            agentRefs: operation.graph.nodes.map(node => node.agentRef),
          })
          input.checkpointArtifacts.register({ bundle, workingDirectory: source.workingDirectory })
        }
        return bundle
      }
      const source = await input.lifecycle.checkpointSource({
        sessionRef: operation.sessionRef,
        attachmentRef: operation.attachmentRef,
        generation: operation.generation,
        agentRefs: operation.graph.nodes.map(node => node.agentRef),
      })
      const repository = await repositorySnapshot(source.workingDirectory)
      const receiptRef = stableRef(
        "receipt.pylon.portable.checkpoint",
        `${operation.sessionRef}:${operation.generation}:${repository.repositoryPostImageDigest}:${repository.diffDigest}`,
      )
      const payload: Omit<PortableCheckpoint, "digest"> = {
        schema: "openagents.portable_checkpoint.v1",
        checkpointRef: operation.checkpointRef,
        sessionRef: operation.sessionRef,
        sourceAttachmentRef: operation.attachmentRef,
        sourceGeneration: operation.generation,
        repositoryRef: operation.executionBinding.repositoryRef,
        repositoryRevisionRef: repository.repositoryRevisionRef,
        repositoryPostImageDigest: repository.repositoryPostImageDigest,
        diffDigest: repository.diffDigest,
        eventLogCursor: operation.eventLogCursor,
        catalogGenerationRef: stableRef(
          "catalog.pylon.portable",
          `${operation.sessionRef}:${operation.generation}:${source.workspaceRef}`,
        ),
        graphDigest: graphDigest(operation.graph),
        approvalRefs: source.approvalRefs,
        artifactRefs: source.artifactRefs,
        receiptRefs: [receiptRef],
        secretMaterial: "excluded",
        processState: "excluded",
      }
      const checkpoint: PortableCheckpoint = { ...payload, digest: checkpointDigest(payload) }
      const bundle: PylonPortableCheckpointBundle = {
        checkpoint,
        executionBinding: operation.executionBinding,
        graph: operation.graph,
        threadCursors: operation.threadCursors,
      }
      input.checkpointArtifacts?.register({ bundle, workingDirectory: source.workingDirectory })
      const stored = await runLedger(input.ledger.storeCheckpointBundle({
        operationRef: operation.operationRef,
        bundle,
      }))
      await runLedger(input.ledger.completeOperation({
        operationRef: operation.operationRef,
        outcome: {
          evidenceRefs: checkpoint.receiptRefs,
          checkpointRef: checkpoint.checkpointRef,
          repositoryPostImageDigest: checkpoint.repositoryPostImageDigest,
          diffDigest: checkpoint.diffDigest,
          graphDigest: checkpoint.graphDigest,
        },
      }))
      return stored.bundle
    },
    cleanupSource: async (operation) => {
      const admitted = await runLedger(input.ledger.admitOperation({
        operationRef: operation.operationRef,
        sessionRef: operation.sessionRef,
        attachmentRef: operation.attachmentRef,
        generation: operation.generation,
        kind: "cleanup",
      }))
      if (admitted.record.state === "completed") {
        return {
          cleanedAgentRefs: operation.agentRefs,
          processes: "released",
          scratch: "released",
          ports: "released",
          evidenceRefs: admitted.record.outcome?.evidenceRefs ?? [],
        }
      }
      const checkpointOperationRef = operation.operationRef.endsWith(".source.cleanup")
        ? `${operation.operationRef.slice(0, -".source.cleanup".length)}.checkpoint`
        : ""
      if (checkpointOperationRef.length === 0) {
        throw new PylonPortableTargetError("invalid_binding", "source cleanup operation does not bind a checkpoint")
      }
      const checkpointBundle = await runLedger(input.ledger.readCheckpointBundle(checkpointOperationRef))
      const cleanup = await input.lifecycle.cleanup({
        ...operation,
        checkpointRef: checkpointBundle.checkpoint.checkpointRef,
        checkpointDigest: checkpointBundle.checkpoint.digest,
      })
      const completed = await runLedger(input.ledger.completeOperation({
        operationRef: operation.operationRef,
        outcome: {
          evidenceRefs: cleanup.evidenceRefs,
          cleanupReceiptRef: cleanup.cleanupReceiptRef,
        },
      }))
      return {
        cleanedAgentRefs: cleanup.cleanedAgentRefs,
        processes: "released",
        scratch: "released",
        ports: "released",
        evidenceRefs: completed.record.outcome?.evidenceRefs ?? cleanup.evidenceRefs,
      }
    },
    stageCheckpoint: input.destination?.stageCheckpoint ?? destinationUnavailable,
    activate: input.destination?.activate ?? destinationUnavailable,
    abortStaged: input.destination?.abortStaged ?? destinationUnavailable,
  }
}
