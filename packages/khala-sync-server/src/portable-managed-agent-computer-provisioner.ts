import { createHash } from "node:crypto"

import type { ManagedAgentComputerPortableProvisioner } from "./portable-managed-agent-computer-target.js"
import type { PortableCheckpointBundle } from "./portable-session-move.js"

type PortableCheckpointArtifact = Readonly<{
  artifactRef: string
  digest: `sha256:${string}`
  bytes: Uint8Array
}>

/** Runtime-neutral artifact seam; Pylon supplies its Bun-backed implementation. */
type PortableCheckpointArtifactStore = Readonly<{
  resolve: (input: Readonly<{
    ownerRef: string
    targetRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    checkpointRef: string
    bundle: PortableCheckpointBundle
  }>) => Promise<PortableCheckpointArtifact>
  registerArtifact: (input: Readonly<{
    bundle: PortableCheckpointBundle
    artifact: PortableCheckpointArtifact
  }>) => Promise<void>
}>

const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|password|secret|credential|mnemonic|hostname|processId|socket|authHome)"\s*:/iu
const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u

export class OaCodexControlPortableProvisionerError extends Error {
  readonly _tag = "OaCodexControlPortableProvisionerError"
  override readonly name = "OaCodexControlPortableProvisionerError"

  constructor(
    readonly code: "invalid" | "unavailable" | "rejected" | "unsafe_response",
    message: string,
  ) {
    super(message)
  }
}

type FetchLike = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>

export type OaCodexControlPortableProvisionerConfig = Readonly<{
  baseUrl: string
  bearerToken: string
  fetch?: FetchLike
  timeoutMs?: number
  checkpointArtifacts: PortableCheckpointArtifactStore
}>

type OperationBase = Readonly<{
  operationRef: string
  action: "stage" | "abortPrepared" | "activate" | "abort" | "quiesce" | "checkpoint" | "reclaim"
  ownerRef: string
  targetRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  resourceRef?: string
  payload: unknown
}>

const object = (value: unknown): Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new OaCodexControlPortableProvisionerError(
      "rejected",
      "oa-codex-control returned a non-object portable provisioner response",
    )
  }
  return value as Record<string, unknown>
}

const publicSafe = <A>(value: A): A => {
  const encoded = JSON.stringify(value)
  if (FORBIDDEN_PRIVATE_MATERIAL.test(encoded)) {
    throw new OaCodexControlPortableProvisionerError(
      "unsafe_response",
      "portable provisioner response contains forbidden private material",
    )
  }
  return value
}

/**
 * Concrete PORT-03 binding to oa-codex-control's retained Firecracker route.
 *
 * The bearer token is carried only in the HTTP Authorization header. Operation
 * bodies and responses contain the portable public-safe bundle plus opaque
 * capability/resource refs; the client never serializes credential material.
 */
export const createOaCodexControlPortableProvisioner = (
  config: OaCodexControlPortableProvisionerConfig,
): ManagedAgentComputerPortableProvisioner => {
  let endpoint: URL
  let materializeEndpoint: URL
  let exportEndpoint: URL
  try {
    endpoint = new URL("/v1/portable-agent-computers/operations", config.baseUrl)
    materializeEndpoint = new URL("/v1/portable-agent-computers/checkpoints/materialize", config.baseUrl)
    exportEndpoint = new URL("/v1/portable-agent-computers/checkpoints/export", config.baseUrl)
  } catch {
    throw new OaCodexControlPortableProvisionerError("invalid", "oa-codex-control base URL is invalid")
  }
  if (endpoint.protocol !== "https:" && !["127.0.0.1", "localhost", "::1"].includes(endpoint.hostname)) {
    throw new OaCodexControlPortableProvisionerError(
      "invalid",
      "portable provisioner requires HTTPS or authenticated loopback HTTP",
    )
  }
  if (config.bearerToken.length < 16) {
    throw new OaCodexControlPortableProvisionerError("invalid", "oa-codex-control bearer token is missing")
  }
  const fetcher = config.fetch ?? globalThis.fetch
  const timeoutMs = config.timeoutMs ?? 120_000

  const run = async <A>(operation: OperationBase): Promise<A> => {
    publicSafe(operation)
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), timeoutMs)
    let response: Response
    try {
      response = await fetcher(endpoint, {
        method: "POST",
        headers: {
          authorization: `Bearer ${config.bearerToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(operation),
        signal: controller.signal,
      })
    } catch {
      throw new OaCodexControlPortableProvisionerError(
        "unavailable",
        "oa-codex-control portable provisioner is unavailable",
      )
    } finally {
      clearTimeout(timeout)
    }
    let decoded: unknown
    try {
      decoded = await response.json()
    } catch {
      throw new OaCodexControlPortableProvisionerError(
        "rejected",
        "oa-codex-control returned an invalid portable provisioner response",
      )
    }
    if (!response.ok) {
      throw new OaCodexControlPortableProvisionerError(
        "rejected",
        `oa-codex-control refused portable operation (${response.status})`,
      )
    }
    return publicSafe(object(decoded)) as A
  }

  const materialize = async (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    checkpointRef: string
    artifactRef: string
    artifactDigest: string
    bytes: Uint8Array
  }>): Promise<Record<string, unknown>> => {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), timeoutMs)
    const requestBytes = Uint8Array.from(input.bytes)
    let response: Response
    try {
      response = await fetcher(materializeEndpoint, {
        method: "POST",
        headers: {
          authorization: `Bearer ${config.bearerToken}`,
          "content-type": "application/octet-stream",
          "X-OA-Operation-Ref": input.operationRef,
          "X-OA-Owner-Ref": input.ownerRef,
          "X-OA-Target-Ref": input.targetRef,
          "X-OA-Session-Ref": input.sessionRef,
          "X-OA-Attachment-Ref": input.attachmentRef,
          "X-OA-Attachment-Generation": String(input.generation),
          "X-OA-Checkpoint-Ref": input.checkpointRef,
          "X-OA-Artifact-Ref": input.artifactRef,
          "X-OA-Artifact-Digest": input.artifactDigest,
        },
        body: requestBytes.buffer,
        signal: controller.signal,
      })
    } catch {
      throw new OaCodexControlPortableProvisionerError(
        "unavailable",
        "oa-codex-control checkpoint materializer is unavailable",
      )
    } finally {
      clearTimeout(timeout)
      requestBytes.fill(0)
    }
    let decoded: unknown
    try {
      decoded = await response.json()
    } catch {
      throw new OaCodexControlPortableProvisionerError(
        "rejected",
        "oa-codex-control returned an invalid checkpoint materialization response",
      )
    }
    if (!response.ok) {
      throw new OaCodexControlPortableProvisionerError(
        "rejected",
        `oa-codex-control refused checkpoint materialization (${response.status})`,
      )
    }
    return publicSafe(object(decoded))
  }

  const exportCheckpoint = async (input: Readonly<{
    operationRef: string
    ownerRef: string
    targetRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    checkpointRef: string
  }>): Promise<Readonly<{ artifactRef: string; digest: `sha256:${string}`; bytes: Uint8Array }>> => {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), timeoutMs)
    let response: Response
    try {
      response = await fetcher(exportEndpoint, {
        method: "POST",
        headers: {
          authorization: `Bearer ${config.bearerToken}`,
          "X-OA-Operation-Ref": input.operationRef,
          "X-OA-Owner-Ref": input.ownerRef,
          "X-OA-Target-Ref": input.targetRef,
          "X-OA-Session-Ref": input.sessionRef,
          "X-OA-Attachment-Ref": input.attachmentRef,
          "X-OA-Attachment-Generation": String(input.generation),
          "X-OA-Checkpoint-Ref": input.checkpointRef,
        },
        signal: controller.signal,
      })
    } catch {
      throw new OaCodexControlPortableProvisionerError("unavailable", "oa-codex-control checkpoint exporter is unavailable")
    } finally {
      clearTimeout(timeout)
    }
    if (!response.ok || !response.headers.get("content-type")?.toLowerCase().startsWith("application/octet-stream")) {
      throw new OaCodexControlPortableProvisionerError("rejected", `oa-codex-control refused checkpoint export (${response.status})`)
    }
    const artifactRef = response.headers.get("X-OA-Artifact-Ref") ?? ""
    const digest = response.headers.get("X-OA-Artifact-Digest") ?? ""
    const bytes = new Uint8Array(await response.arrayBuffer())
    if (!SAFE_REF.test(artifactRef) || !/^sha256:[a-f0-9]{64}$/u.test(digest) || bytes.byteLength === 0 ||
        `sha256:${createHash("sha256").update(bytes).digest("hex")}` !== digest) {
      bytes.fill(0)
      throw new OaCodexControlPortableProvisionerError("rejected", "oa-codex-control returned an invalid checkpoint artifact")
    }
    return { artifactRef, digest: digest as `sha256:${string}`, bytes }
  }

  return {
    stage: async input => {
      const prepared = await run<Readonly<{
        resourceRef: string
        materializationRequired: true
      }>>({
        operationRef: input.operationRef,
        action: "stage",
        ownerRef: input.ownerRef,
        targetRef: input.targetRef,
        sessionRef: input.bundle.checkpoint.sessionRef,
        attachmentRef: input.attachmentRef,
        generation: input.generation,
        payload: {
          bundle: input.bundle,
          capabilityLeaseRefs: input.capabilityLeaseRefs,
        },
      })
      if (!SAFE_REF.test(prepared.resourceRef) || prepared.materializationRequired !== true) {
        throw new OaCodexControlPortableProvisionerError("rejected", "oa-codex-control did not prepare an exact non-accepting resource")
      }
      try {
        const artifact = await config.checkpointArtifacts.resolve({
          ownerRef: input.ownerRef,
          targetRef: input.targetRef,
          sessionRef: input.bundle.checkpoint.sessionRef,
          attachmentRef: input.attachmentRef,
          generation: input.generation,
          checkpointRef: input.bundle.checkpoint.checkpointRef,
          bundle: input.bundle,
        })
        try {
          if (!SAFE_REF.test(artifact.artifactRef) ||
              !/^sha256:[a-f0-9]{64}$/u.test(artifact.digest) ||
              artifact.bytes.byteLength === 0 ||
              `sha256:${createHash("sha256").update(artifact.bytes).digest("hex")}` !== artifact.digest) {
            throw new OaCodexControlPortableProvisionerError("rejected", "checkpoint artifact resolver returned mismatched bytes")
          }
          const receipt = await materialize({
            operationRef: `${input.operationRef}.materialize`,
            ownerRef: input.ownerRef,
            targetRef: input.targetRef,
            sessionRef: input.bundle.checkpoint.sessionRef,
            attachmentRef: input.attachmentRef,
            generation: input.generation,
            checkpointRef: input.bundle.checkpoint.checkpointRef,
            artifactRef: artifact.artifactRef,
            artifactDigest: artifact.digest,
            bytes: artifact.bytes,
          })
          if (receipt.resourceRef !== prepared.resourceRef) {
            throw new OaCodexControlPortableProvisionerError("rejected", "checkpoint materialization used a different retained resource")
          }
          return receipt as Awaited<ReturnType<ManagedAgentComputerPortableProvisioner["stage"]>>
        } finally {
          artifact.bytes.fill(0)
        }
      } catch (error) {
        await run({
          operationRef: `${input.operationRef}.abort-prepared`,
          action: "abortPrepared",
          ownerRef: input.ownerRef,
          targetRef: input.targetRef,
          sessionRef: input.bundle.checkpoint.sessionRef,
          attachmentRef: input.attachmentRef,
          generation: input.generation,
          payload: { stageOperationRef: input.operationRef },
        }).catch(() => undefined)
        throw error
      }
    },
    activate: input => run({
      operationRef: input.operationRef,
      action: "activate",
      ownerRef: input.ownerRef,
      targetRef: input.targetRef,
      resourceRef: input.resourceRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      generation: input.generation,
      payload: {
        checkpointRef: input.checkpointRef,
        executionBinding: input.executionBinding,
        capabilityLeaseRefs: input.capabilityLeaseRefs,
        authorityEvidenceRef: input.authorityEvidenceRef,
      },
    }),
    abort: input => run({
      operationRef: input.operationRef,
      action: "abort",
      ownerRef: input.ownerRef,
      targetRef: input.targetRef,
      resourceRef: input.resourceRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      generation: input.generation,
      payload: {},
    }),
    quiesce: input => run({
      operationRef: input.operationRef,
      action: "quiesce",
      ownerRef: input.ownerRef,
      targetRef: input.targetRef,
      resourceRef: input.resourceRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      generation: input.generation,
      payload: { graph: input.graph, threadCursors: input.threadCursors },
    }),
    checkpoint: async input => {
      const bundle = await run<Awaited<ReturnType<ManagedAgentComputerPortableProvisioner["checkpoint"]>>>({
        operationRef: input.operationRef,
        action: "checkpoint",
        ownerRef: input.ownerRef,
        targetRef: input.targetRef,
        resourceRef: input.resourceRef,
        sessionRef: input.sessionRef,
        attachmentRef: input.attachmentRef,
        generation: input.generation,
        payload: {
          checkpointRef: input.checkpointRef,
          eventLogCursor: input.eventLogCursor,
          executionBinding: input.executionBinding,
          graph: input.graph,
          threadCursors: input.threadCursors,
        },
      })
      if (bundle.checkpoint.checkpointRef !== input.checkpointRef ||
          bundle.checkpoint.sessionRef !== input.sessionRef ||
          bundle.checkpoint.sourceAttachmentRef !== input.attachmentRef ||
          bundle.checkpoint.sourceGeneration !== input.generation) {
        throw new OaCodexControlPortableProvisionerError("rejected", "managed checkpoint bundle does not match the exact source generation")
      }
      const artifact = await exportCheckpoint({
        operationRef: `${input.operationRef}.export`,
        ownerRef: input.ownerRef,
        targetRef: input.targetRef,
        sessionRef: input.sessionRef,
        attachmentRef: input.attachmentRef,
        generation: input.generation,
        checkpointRef: input.checkpointRef,
      })
      try {
        await config.checkpointArtifacts.registerArtifact({ bundle, artifact })
      } finally {
        artifact.bytes.fill(0)
      }
      return bundle
    },
    reclaim: input => run({
      operationRef: input.operationRef,
      action: "reclaim",
      ownerRef: input.ownerRef,
      targetRef: input.targetRef,
      resourceRef: input.resourceRef,
      sessionRef: input.sessionRef,
      attachmentRef: input.attachmentRef,
      generation: input.generation,
      payload: { agentRefs: input.agentRefs },
    }),
  }
}
