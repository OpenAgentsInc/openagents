import { createHash } from "node:crypto"

import {
  verifySignedReleaseSet,
  type ReleaseSet,
  type ReleaseSetArtifact,
} from "../../openagents-desktop/src/release-set-contract.ts"
import type {
  PinnedReleaseKey,
  UpdateSignature,
} from "../../openagents-desktop/src/update-contract.ts"
import { isMonotonicUpgrade } from "../../openagents-desktop/src/update-contract.ts"

export type ReleaseSetChannel = "stable" | "rc"

export const RELEASE_SET_PAYLOAD_LIMIT = 512 * 1024
export const RELEASE_SET_SIGNATURE_LIMIT = 8 * 1024
export const RELEASE_SET_POINTER_LIMIT = 16 * 1024

export type ReleaseSetPointer = Readonly<{
  schema: "openagents.desktop.release_pointer.v2"
  channel: ReleaseSetChannel
  revision: number
  generation: string
  previousGeneration: string | null
  payloadSha256: string
  signatureSha256: string
  publishedAt: string
}>

export type ReleaseSetCandidate = Readonly<{
  channel: ReleaseSetChannel
  generation: string
  releaseSet: ReleaseSet
  payloadBytes: Uint8Array
  signatureBytes: Uint8Array
  signature: UpdateSignature
}>

export type ArtifactObservation = Readonly<{
  byteLength: number
  sha256: string
}>

/**
 * DIST-04 integration port. The coordinator supplies a public-style verifier
 * that downloads the credential-free signed URL without forwarding secrets,
 * applies its own response/body limits, and returns only the observed digest
 * and length. A candidate cannot enter storage until every signed artifact
 * has passed this seam.
 */
export type ReleaseSetArtifactVerifier = (
  artifact: ReleaseSetArtifact,
) => Promise<ArtifactObservation>

export type ReleaseSetFeedLog = (entry: Readonly<{
  event:
    | "candidate_admitted"
    | "candidate_rejected"
    | "pointer_promoted"
    | "pointer_rolled_back"
    | "route_resolved"
    | "route_failed"
  channel?: ReleaseSetChannel
  generation?: string
  revision?: number
  targetCount?: number
  artifactCount?: number
  leaf?: "pointer" | "payload" | "signature"
  cacheMode?: "bounded" | "no_store" | "immutable"
  outcome?: "success" | "failure"
  reason?: string
}>) => void

type StoredCandidate = ReleaseSetCandidate

export interface ReleaseSetFeedStore {
  readCandidate(channel: ReleaseSetChannel, generation: string): Promise<StoredCandidate | null>
  createCandidate(candidate: StoredCandidate): Promise<"created" | "exists" | "conflict">
  readPointer(channel: ReleaseSetChannel): Promise<ReleaseSetPointer | null>
  compareAndSwapPointer(
    channel: ReleaseSetChannel,
    expectedRevision: number | null,
    next: ReleaseSetPointer,
  ): Promise<boolean>
  listCandidateGenerations(channel: ReleaseSetChannel): Promise<readonly string[]>
}

const copyCandidate = (candidate: StoredCandidate): StoredCandidate => ({
  ...candidate,
  payloadBytes: Uint8Array.from(candidate.payloadBytes),
  signatureBytes: Uint8Array.from(candidate.signatureBytes),
})

export const createInMemoryReleaseSetFeedStore = (): ReleaseSetFeedStore => {
  const candidates = new Map<string, StoredCandidate>()
  const pointers = new Map<ReleaseSetChannel, ReleaseSetPointer>()
  const key = (channel: ReleaseSetChannel, generation: string): string =>
    `${channel}/${generation}`
  return {
    async readCandidate(channel, generation) {
      const candidate = candidates.get(key(channel, generation))
      return candidate === undefined ? null : copyCandidate(candidate)
    },
    async createCandidate(candidate) {
      const candidateKey = key(candidate.channel, candidate.generation)
      const current = candidates.get(candidateKey)
      if (current === undefined) {
        candidates.set(candidateKey, copyCandidate(candidate))
        return "created"
      }
      return bytesEqual(current.payloadBytes, candidate.payloadBytes) &&
        bytesEqual(current.signatureBytes, candidate.signatureBytes)
        ? "exists"
        : "conflict"
    },
    async readPointer(channel) {
      return pointers.get(channel) ?? null
    },
    async compareAndSwapPointer(channel, expectedRevision, next) {
      const current = pointers.get(channel)
      if ((current?.revision ?? null) !== expectedRevision) return false
      pointers.set(channel, { ...next })
      return true
    },
    async listCandidateGenerations(channel) {
      return [...candidates.keys()]
        .filter((candidateKey) => candidateKey.startsWith(`${channel}/`))
        .map((candidateKey) => candidateKey.slice(channel.length + 1))
        .toSorted()
    },
  }
}

const sha256 = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex")

const bytesEqual = (left: Uint8Array, right: Uint8Array): boolean =>
  left.byteLength === right.byteLength &&
  Buffer.from(left).equals(Buffer.from(right))

const parseSignature = (bytes: Uint8Array): UpdateSignature => {
  if (bytes.byteLength === 0 || bytes.byteLength > RELEASE_SET_SIGNATURE_LIMIT) {
    throw new Error("signature_size_invalid")
  }
  let value: unknown
  try {
    value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes))
  } catch {
    throw new Error("signature_json_invalid")
  }
  if (
    typeof value !== "object" || value === null || Array.isArray(value) ||
    (value as Record<string, unknown>).alg !== "ed25519" ||
    typeof (value as Record<string, unknown>).kid !== "string" ||
    typeof (value as Record<string, unknown>).sha256 !== "string" ||
    typeof (value as Record<string, unknown>).signature !== "string"
  ) {
    throw new Error("signature_schema_invalid")
  }
  return value as UpdateSignature
}

const generationPattern = /^[0-9a-f]{64}$/

const assertGeneration = (generation: string): void => {
  if (!generationPattern.test(generation)) throw new Error("generation_invalid")
}

const redactedReason = (error: unknown, fallback: string): string => {
  const message = error instanceof Error ? error.message : ""
  return /^[a-z0-9_]{1,80}$/.test(message) ? message : fallback
}

const pointerIsValid = (
  value: ReleaseSetPointer,
  channel: ReleaseSetChannel,
): boolean => {
  const keys = Object.keys(value).toSorted().join(",")
  return keys === [
    "channel",
    "generation",
    "payloadSha256",
    "previousGeneration",
    "publishedAt",
    "revision",
    "schema",
    "signatureSha256",
  ].toSorted().join(",") &&
    value.schema === "openagents.desktop.release_pointer.v2" &&
    value.channel === channel &&
    Number.isSafeInteger(value.revision) && value.revision > 0 &&
    generationPattern.test(value.generation) &&
    (value.previousGeneration === null || generationPattern.test(value.previousGeneration)) &&
    value.previousGeneration !== value.generation &&
    generationPattern.test(value.payloadSha256) &&
    generationPattern.test(value.signatureSha256) &&
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value.publishedAt)
}

export type CreateReleaseSetFeedInput = Readonly<{
  store: ReleaseSetFeedStore
  pins: ReadonlyMap<string, PinnedReleaseKey>
  verifyArtifact: ReleaseSetArtifactVerifier
  now?: () => string
  log?: ReleaseSetFeedLog
}>

export type AdmitReleaseSetCandidateInput = Readonly<{
  channel: ReleaseSetChannel
  payloadBytes: Uint8Array
  signatureBytes: Uint8Array
}>

export type ReleaseSetFeed = Readonly<{
  admitCandidate(input: AdmitReleaseSetCandidateInput): Promise<ReleaseSetCandidate>
  promote(channel: ReleaseSetChannel, generation: string, expectedRevision: number | null): Promise<ReleaseSetPointer>
  rollback(channel: ReleaseSetChannel, expectedRevision: number): Promise<ReleaseSetPointer>
  listGarbageCandidates(channel: ReleaseSetChannel): Promise<readonly string[]>
  metrics(): Readonly<Record<string, number>>
  fetch(request: Request): Promise<Response | null>
}>

const immutableHeaders = {
  "access-control-allow-origin": "*",
  "cache-control": "public, max-age=31536000, immutable",
  "cross-origin-resource-policy": "cross-origin",
} as const

const pointerHeaders = {
  "access-control-allow-origin": "*",
  "cache-control": "public, max-age=15, stale-while-revalidate=15",
  "cross-origin-resource-policy": "cross-origin",
} as const

const currentAliasHeaders = {
  "access-control-allow-origin": "*",
  "cache-control": "no-store",
  "cross-origin-resource-policy": "cross-origin",
} as const

const jsonBytesResponse = (
  bytes: Uint8Array,
  cacheMode: "immutable" | "pointer" | "current",
  requestMethod: string,
  generation?: string,
): Response => new Response(
  requestMethod === "HEAD" ? null : Uint8Array.from(bytes).buffer,
  {
    headers: {
      ...(cacheMode === "immutable"
        ? immutableHeaders
        : cacheMode === "pointer" ? pointerHeaders : currentAliasHeaders),
      "content-length": String(bytes.byteLength),
      "content-type": "application/json; charset=utf-8",
      "x-content-type-options": "nosniff",
      ...(generation === undefined ? {} : { "x-openagents-release-generation": generation }),
    },
  },
)

const safeError = (reason: string, status?: number): Response =>
  Response.json(
    { error: reason },
    {
      status: status ?? (reason === "not_found" ? 404 : 400),
      headers: {
        "access-control-allow-origin": "*",
        "cache-control": "no-store",
        "content-type": "application/json; charset=utf-8",
        "x-content-type-options": "nosniff",
      },
    },
  )

export const createReleaseSetFeed = (
  input: CreateReleaseSetFeedInput,
): ReleaseSetFeed => {
  const now = input.now ?? (() => new Date().toISOString())
  const log = input.log ?? (() => undefined)
  const counters = new Map<string, number>()
  const emit = (entry: Parameters<ReleaseSetFeedLog>[0]): void => {
    const channel = entry.channel === undefined ? "none" : entry.channel
    const key = `${entry.event}.${channel}`
    counters.set(key, (counters.get(key) ?? 0) + 1)
    const detailKey = `${key}.${entry.leaf ?? "all"}.${entry.cacheMode ?? "none"}.${entry.outcome ?? "none"}`
    counters.set(detailKey, (counters.get(detailKey) ?? 0) + 1)
    if (entry.revision !== undefined && entry.channel !== undefined) {
      counters.set(`pointer_revision.${entry.channel}`, entry.revision)
    }
    if (entry.targetCount !== undefined && entry.channel !== undefined) {
      counters.set(`target_count.${entry.channel}`, entry.targetCount)
    }
    if (entry.artifactCount !== undefined && entry.channel !== undefined) {
      counters.set(`artifact_count.${entry.channel}`, entry.artifactCount)
    }
    log(entry)
  }
  const candidateIsAuthentic = (
    candidate: StoredCandidate,
    channel: ReleaseSetChannel,
  ): boolean => {
    if (candidate.channel !== channel || sha256(candidate.payloadBytes) !== candidate.generation) {
      return false
    }
    try {
      const signature = parseSignature(candidate.signatureBytes)
      const pin = input.pins.get(signature.kid)
      return pin !== undefined &&
        verifySignedReleaseSet(candidate.payloadBytes, signature, pin, channel).ok
    } catch {
      return false
    }
  }

  const readCurrent = async (
    channel: ReleaseSetChannel,
  ): Promise<{ pointer: ReleaseSetPointer; candidate: StoredCandidate } | null> => {
    const pointer = await input.store.readPointer(channel)
    if (pointer === null) return null
    if (!pointerIsValid(pointer, channel)) {
      emit({ event: "route_failed", channel, reason: "pointer_invalid" })
      throw new Error("pointer_invalid")
    }
    const candidate = await input.store.readCandidate(channel, pointer.generation)
    if (
      candidate === null || !candidateIsAuthentic(candidate, channel) ||
      sha256(candidate.payloadBytes) !== pointer.payloadSha256 ||
      sha256(candidate.signatureBytes) !== pointer.signatureSha256
    ) {
      emit({ event: "route_failed", channel, reason: "pointer_object_mismatch" })
      throw new Error("pointer_object_mismatch")
    }
    return { pointer, candidate }
  }

  return {
    async admitCandidate(candidateInput) {
      try {
        if (
          candidateInput.payloadBytes.byteLength === 0 ||
          candidateInput.payloadBytes.byteLength > RELEASE_SET_PAYLOAD_LIMIT
        ) throw new Error("payload_size_invalid")
        const signature = parseSignature(candidateInput.signatureBytes)
        const pin = input.pins.get(signature.kid)
        if (pin === undefined) throw new Error("signing_key_not_pinned")
        const verified = verifySignedReleaseSet(
          candidateInput.payloadBytes,
          signature,
          pin,
          candidateInput.channel,
        )
        if (!verified.ok) throw new Error(`release_set_${verified.reason}`)
        const artifacts = verified.releaseSet.targets.flatMap((row) => row.artifacts)
        for (const artifact of artifacts) {
          const observed = await input.verifyArtifact(artifact)
          if (
            observed.byteLength !== artifact.byteLength ||
            observed.sha256 !== artifact.sha256
          ) throw new Error("artifact_observation_mismatch")
        }
        const generation = sha256(candidateInput.payloadBytes)
        const candidate: StoredCandidate = {
          channel: candidateInput.channel,
          generation,
          releaseSet: verified.releaseSet,
          payloadBytes: Uint8Array.from(candidateInput.payloadBytes),
          signatureBytes: Uint8Array.from(candidateInput.signatureBytes),
          signature,
        }
        const result = await input.store.createCandidate(candidate)
        if (result === "conflict") throw new Error("immutable_candidate_conflict")
        emit({
          event: "candidate_admitted",
          channel: candidate.channel,
          generation,
          targetCount: candidate.releaseSet.targets.length,
          artifactCount: artifacts.length,
        })
        return copyCandidate(candidate)
      } catch (error) {
        const reason = redactedReason(error, "candidate_rejected")
        emit({ event: "candidate_rejected", channel: candidateInput.channel, reason })
        throw new Error(reason)
      }
    },

    async promote(channel, generation, expectedRevision) {
      assertGeneration(generation)
      const candidate = await input.store.readCandidate(channel, generation)
      if (candidate === null) throw new Error("candidate_not_found")
      if (!candidateIsAuthentic(candidate, channel)) throw new Error("candidate_storage_invalid")
      const current = await input.store.readPointer(channel)
      if ((current?.revision ?? null) !== expectedRevision) {
        throw new Error("pointer_revision_conflict")
      }
      if (current?.generation === generation) throw new Error("candidate_already_current")
      if (current !== null) {
        const currentCandidate = await input.store.readCandidate(channel, current.generation)
        if (currentCandidate === null) throw new Error("current_candidate_missing")
        const monotonic = isMonotonicUpgrade(
          currentCandidate.releaseSet.version,
          candidate.releaseSet.version,
          channel,
        )
        if (!monotonic.admissible) throw new Error(`promotion_${monotonic.reason}`)
      }
      const next: ReleaseSetPointer = {
        schema: "openagents.desktop.release_pointer.v2",
        channel,
        revision: (current?.revision ?? 0) + 1,
        generation,
        previousGeneration: current?.generation ?? null,
        payloadSha256: sha256(candidate.payloadBytes),
        signatureSha256: sha256(candidate.signatureBytes),
        publishedAt: now(),
      }
      if (!await input.store.compareAndSwapPointer(channel, expectedRevision, next)) {
        throw new Error("pointer_revision_conflict")
      }
      emit({ event: "pointer_promoted", channel, generation, revision: next.revision })
      return next
    },

    async rollback(channel, expectedRevision) {
      const current = await input.store.readPointer(channel)
      if (current === null || current.revision !== expectedRevision) {
        throw new Error("pointer_revision_conflict")
      }
      if (!pointerIsValid(current, channel)) throw new Error("current_pointer_invalid")
      const currentCandidate = await input.store.readCandidate(channel, current.generation)
      if (
        currentCandidate === null || !candidateIsAuthentic(currentCandidate, channel) ||
        currentCandidate.generation !== current.generation ||
        sha256(currentCandidate.payloadBytes) !== current.payloadSha256 ||
        sha256(currentCandidate.signatureBytes) !== current.signatureSha256
      ) throw new Error("current_candidate_invalid")
      if (current.previousGeneration === null) throw new Error("rollback_slot_empty")
      const previous = await input.store.readCandidate(channel, current.previousGeneration)
      if (previous === null) throw new Error("rollback_candidate_missing")
      if (
        previous.generation !== current.previousGeneration ||
        !candidateIsAuthentic(previous, channel)
      ) throw new Error("rollback_candidate_invalid")
      const next: ReleaseSetPointer = {
        schema: "openagents.desktop.release_pointer.v2",
        channel,
        revision: current.revision + 1,
        generation: previous.generation,
        previousGeneration: current.generation,
        payloadSha256: sha256(previous.payloadBytes),
        signatureSha256: sha256(previous.signatureBytes),
        publishedAt: now(),
      }
      if (!await input.store.compareAndSwapPointer(channel, expectedRevision, next)) {
        throw new Error("pointer_revision_conflict")
      }
      emit({
        event: "pointer_rolled_back",
        channel,
        generation: next.generation,
        revision: next.revision,
      })
      return next
    },

    async listGarbageCandidates(channel) {
      const pointer = await input.store.readPointer(channel)
      const retained = new Set(
        pointer === null
          ? []
          : [pointer.generation, pointer.previousGeneration].filter(
              (value): value is string => value !== null,
            ),
      )
      return (await input.store.listCandidateGenerations(channel))
        .filter((generation) => !retained.has(generation))
    },

    metrics() {
      return Object.freeze(Object.fromEntries(counters.entries()))
    },

    async fetch(request) {
      const url = new URL(request.url)
      if (request.method === "OPTIONS" && url.pathname.startsWith("/desktop/openagents/")) {
        return new Response(null, {
          status: 204,
          headers: {
            "access-control-allow-headers": "content-type",
            "access-control-allow-methods": "GET, HEAD, OPTIONS",
            "access-control-allow-origin": "*",
            "access-control-max-age": "86400",
          },
        })
      }
      if (request.method !== "GET" && request.method !== "HEAD") return null

      if (url.pathname === "/metrics/release-set.json") {
        const bytes = new TextEncoder().encode(JSON.stringify({
          schema: "openagents.desktop.release_feed_metrics.v1",
          counters: Object.fromEntries(counters.entries()),
        }))
        return jsonBytesResponse(bytes, "current", request.method)
      }

      const match = url.pathname.match(
        /^\/desktop\/openagents\/(stable|rc)\/(?:v2\/)?(pointer|release-set|release-set\.sig)\.json$/,
      )
      if (match !== null) {
        const channel = match[1] as ReleaseSetChannel
        let current: Awaited<ReturnType<typeof readCurrent>>
        try {
          current = await readCurrent(channel)
        } catch {
          emit({
            event: "route_failed",
            channel,
            leaf: match[2] === "pointer"
              ? "pointer"
              : match[2] === "release-set" ? "payload" : "signature",
            cacheMode: match[2] === "pointer" ? "bounded" : "no_store",
            outcome: "failure",
            reason: "storage_unavailable",
          })
          return safeError("feed_unavailable", 503)
        }
        if (current === null) {
          emit({
            event: "route_failed",
            channel,
            leaf: match[2] === "pointer"
              ? "pointer"
              : match[2] === "release-set" ? "payload" : "signature",
            cacheMode: match[2] === "pointer" ? "bounded" : "no_store",
            outcome: "failure",
            reason: "not_found",
          })
          return safeError("not_found")
        }
        let bytes: Uint8Array
        if (match[2] === "pointer") {
          bytes = new TextEncoder().encode(JSON.stringify(current.pointer))
          if (bytes.byteLength > RELEASE_SET_POINTER_LIMIT) return safeError("pointer_oversized")
        } else if (match[2] === "release-set") {
          bytes = current.candidate.payloadBytes
        } else {
          bytes = current.candidate.signatureBytes
        }
        emit({
          event: "route_resolved",
          channel,
          generation: current.pointer.generation,
          revision: current.pointer.revision,
          leaf: match[2] === "pointer"
            ? "pointer"
            : match[2] === "release-set" ? "payload" : "signature",
          cacheMode: match[2] === "pointer" ? "bounded" : "no_store",
          outcome: "success",
        })
        return jsonBytesResponse(
          bytes,
          match[2] === "pointer" ? "pointer" : "current",
          request.method,
          current.pointer.generation,
        )
      }

      const candidateMatch = url.pathname.match(
        /^\/desktop\/openagents\/(stable|rc)\/candidates\/([0-9a-f]{64})\/(release-set|release-set\.sig)\.json$/,
      )
      if (candidateMatch !== null) {
        const channel = candidateMatch[1] as ReleaseSetChannel
        let candidate: StoredCandidate | null
        try {
          candidate = await input.store.readCandidate(channel, candidateMatch[2])
        } catch {
          emit({
            event: "route_failed",
            channel,
            leaf: candidateMatch[3] === "release-set" ? "payload" : "signature",
            cacheMode: "immutable",
            outcome: "failure",
            reason: "storage_unavailable",
          })
          return safeError("feed_unavailable", 503)
        }
        if (candidate === null) {
          emit({
            event: "route_failed",
            channel,
            leaf: candidateMatch[3] === "release-set" ? "payload" : "signature",
            cacheMode: "immutable",
            outcome: "failure",
            reason: "candidate_missing",
          })
          return safeError("not_found")
        }
        if (!candidateIsAuthentic(candidate, channel)) {
          emit({
            event: "route_failed",
            channel,
            leaf: candidateMatch[3] === "release-set" ? "payload" : "signature",
            cacheMode: "immutable",
            outcome: "failure",
            reason: "candidate_invalid",
          })
          return safeError("feed_unavailable", 503)
        }
        const bytes = candidateMatch[3] === "release-set"
          ? candidate.payloadBytes
          : candidate.signatureBytes
        emit({
          event: "route_resolved",
          channel,
          generation: candidate.generation,
          leaf: candidateMatch[3] === "release-set" ? "payload" : "signature",
          cacheMode: "immutable",
          outcome: "success",
        })
        return jsonBytesResponse(bytes, "immutable", request.method, candidate.generation)
      }
      return null
    },
  }
}
