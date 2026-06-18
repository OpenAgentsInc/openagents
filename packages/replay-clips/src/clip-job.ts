/**
 * Replay clip job + manifest contracts (EPIC #5411, issue #5430).
 *
 * Specifies the programmatic contract an agent or operator uses to request a
 * directed replay clip and the public-safe manifest the render box emits when
 * the clip is produced.
 *
 * Boundary: this module is schema/validation only. A clip job is an
 * observation/projection request over an existing public replay bundle (or
 * timeline range). Decoding or validating a clip job/manifest grants no
 * settlement, payout, deployment, accepted-work, provider, wallet, or
 * public-claim authority. Rendering runs on owned local/CI/Container
 * infrastructure (see issue #5431), never inside the Cloudflare Worker; the
 * Worker may only host job records and read finished refs (issue #5432).
 *
 * Public-safety: every ref in a job or manifest must be a public-safe
 * reference (bundle slug/ref, public timeline cursor, sha256 digest, public
 * storage URL, ISO timestamp, source ref). Raw traces, prompts, seeds,
 * provider material, payout targets, invoices, preimages, tokens, wallet
 * material, mnemonics, local filesystem paths, and customer-private data are
 * rejected before a record can be treated as public-safe.
 */
import { Schema as S } from "effect"

import { ReplayCameraPath } from "./camera-path.js"

export const REPLAY_CLIP_JOB_SCHEMA_VERSION = "openagents.replay_clip_job.v1"

export const REPLAY_CLIP_MANIFEST_SCHEMA_VERSION =
  "openagents.replay_clip_manifest.v1"

export const REPLAY_CLIP_STALENESS_CONTRACT_VERSION = "projection_staleness.v1"

/**
 * A clip job is an evidence/observation request only. The claim scope mirrors
 * the proof-replay bundle's `evidence_presentation_only` posture so a clip can
 * never be read as an authority grant.
 */
export const REPLAY_CLIP_CLAIM_SCOPE = "evidence_presentation_only"

/** Render output container. mp4 is the only productized output today. */
export const ReplayClipOutputKind = S.Literals(["mp4"])
export type ReplayClipOutputKind = typeof ReplayClipOutputKind.Type

/**
 * Where the clip's source frames come from:
 * - `replay_bundle`: a named/published `proof_replay_bundle.v1` (slug or ref).
 * - `timeline_range`: a public activity-timeline cursor range (EPIC #1), from
 *   which a replay bundle is generated (EPIC #2) before rendering.
 */
export const ReplayClipSourceKind = S.Literals([
  "replay_bundle",
  "timeline_range",
])
export type ReplayClipSourceKind = typeof ReplayClipSourceKind.Type

/** A public replay bundle reference: a known slug or a `proof_replay_bundle.*` ref. */
export const ReplayBundleSource = S.Struct({
  kind: S.Literal("replay_bundle"),
  bundleRef: S.String,
})
export type ReplayBundleSource = typeof ReplayBundleSource.Type

/**
 * A public activity-timeline range source. Cursors are the opaque public
 * `{ts}:{sourceKind}:{eventRef}` keys from `openagents.public_activity_timeline.v1`;
 * they are public-safe by contract.
 */
export const ReplayTimelineRangeSource = S.Struct({
  kind: S.Literal("timeline_range"),
  fromCursor: S.String,
  toCursor: S.String,
  runRef: S.optional(S.String),
  windowRef: S.optional(S.String),
  actorRef: S.optional(S.String),
})
export type ReplayTimelineRangeSource = typeof ReplayTimelineRangeSource.Type

export const ReplayClipSource = S.Union([
  ReplayBundleSource,
  ReplayTimelineRangeSource,
])
export type ReplayClipSource = typeof ReplayClipSource.Type

/** Render window + encoding parameters. Mirrors the render-box CLI inputs. */
export const ReplayClipRenderSpec = S.Struct({
  startSecond: S.Number,
  durationSecond: S.Number,
  fps: S.Number,
  width: S.Number,
  height: S.Number,
  outputKind: ReplayClipOutputKind,
})
export type ReplayClipRenderSpec = typeof ReplayClipRenderSpec.Type

/**
 * Job lifecycle. A job is `queued` at creation, moves to `rendering` when the
 * render box claims it, `succeeded` once the mp4 + manifest are uploaded, and
 * `failed`/`blocked` otherwise. `blocked` is a typed projection-gap state
 * (e.g. missing source coverage, owner-gated infra not provisioned) and is
 * never a guessed success.
 */
export const ReplayClipJobStatus = S.Literals([
  "queued",
  "rendering",
  "succeeded",
  "failed",
  "blocked",
])
export type ReplayClipJobStatus = typeof ReplayClipJobStatus.Type

export const replayClipJobStatuses: ReadonlyArray<ReplayClipJobStatus> = [
  "queued",
  "rendering",
  "succeeded",
  "failed",
  "blocked",
]

const REPLAY_CLIP_JOB_STATUS_TRANSITIONS: Readonly<
  Record<ReplayClipJobStatus, ReadonlyArray<ReplayClipJobStatus>>
> = {
  queued: ["rendering", "blocked", "failed"],
  rendering: ["succeeded", "failed", "blocked"],
  succeeded: [],
  failed: [],
  blocked: ["queued"],
}

/** Whether a lifecycle transition is allowed by the clip-job state machine. */
export const replayClipJobStatusCanTransition = (
  from: ReplayClipJobStatus,
  to: ReplayClipJobStatus,
): boolean => REPLAY_CLIP_JOB_STATUS_TRANSITIONS[from].includes(to)

/** A clip-job request as an agent or operator submits it. */
export const ReplayClipJobRequest = S.Struct({
  schemaVersion: S.Literal(REPLAY_CLIP_JOB_SCHEMA_VERSION),
  source: ReplayClipSource,
  render: ReplayClipRenderSpec,
  cameraPath: ReplayCameraPath,
  sourceRefs: S.Array(S.String),
})
export type ReplayClipJobRequest = typeof ReplayClipJobRequest.Type

/**
 * A persisted clip-job record (the public-safe projection a read API returns).
 * `manifest` is present only on `succeeded`; `blockerRefs` carry typed blocker
 * reasons for `blocked`/`failed` states.
 */
export const ReplayClipJobRecord = S.Struct({
  schemaVersion: S.Literal(REPLAY_CLIP_JOB_SCHEMA_VERSION),
  jobRef: S.String,
  status: ReplayClipJobStatus,
  claimScope: S.Literal(REPLAY_CLIP_CLAIM_SCOPE),
  source: ReplayClipSource,
  render: ReplayClipRenderSpec,
  cameraPath: ReplayCameraPath,
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  manifestRef: S.optional(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type ReplayClipJobRecord = typeof ReplayClipJobRecord.Type

/** A rendered artifact reference inside a manifest (public-safe refs only). */
export const ReplayClipArtifact = S.Struct({
  kind: ReplayClipOutputKind,
  storageUrl: S.String,
  sha256: S.String,
  byteSize: S.Number,
})
export type ReplayClipArtifact = typeof ReplayClipArtifact.Type

/** Renderer + codec provenance recorded with every clip. */
export const ReplayClipRenderer = S.Struct({
  renderer: S.String,
  rendererVersion: S.String,
  runLocation: S.String,
  videoCodec: S.String,
  pixelFormat: S.String,
})
export type ReplayClipRenderer = typeof ReplayClipRenderer.Type

/**
 * The public-safe output manifest the render box writes alongside the mp4.
 * Source-ref complete: it must name the bundle ref / timeline source, the
 * camera path, the renderer version, and the sha256 of each artifact so a
 * skeptic can dereference and verify the clip without trusting the renderer.
 */
export const ReplayClipManifest = S.Struct({
  schemaVersion: S.Literal(REPLAY_CLIP_MANIFEST_SCHEMA_VERSION),
  jobRef: S.String,
  claimScope: S.Literal(REPLAY_CLIP_CLAIM_SCOPE),
  bundleRef: S.String,
  source: ReplayClipSource,
  render: ReplayClipRenderSpec,
  cameraPath: ReplayCameraPath,
  renderer: ReplayClipRenderer,
  artifacts: S.Array(ReplayClipArtifact),
  frameCount: S.Number,
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  generatedAt: S.String,
})
export type ReplayClipManifest = typeof ReplayClipManifest.Type

export const decodeReplayClipJobRequest = S.decodeUnknownSync(
  ReplayClipJobRequest,
)
export const decodeReplayClipJobRecord = S.decodeUnknownSync(ReplayClipJobRecord)
export const decodeReplayClipManifest = S.decodeUnknownSync(ReplayClipManifest)

const unsafeReplayClipMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer\s+[A-Za-z0-9._-]+|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|prompt|provider|record|repo|runner|run[_-]?log|shell|source|state|target|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

/** True when a job/manifest payload contains raw or private material. */
export const replayClipHasUnsafeMaterial = (value: unknown): boolean =>
  unsafeReplayClipMaterialPattern.test(JSON.stringify(value))

const positiveFinite = (value: number): boolean =>
  Number.isFinite(value) && value > 0

const assertRenderSpecBounds = (render: ReplayClipRenderSpec): void => {
  if (!Number.isFinite(render.startSecond) || render.startSecond < 0) {
    throw new Error(
      "Replay clip render startSecond must be a non-negative number",
    )
  }
  if (!positiveFinite(render.durationSecond)) {
    throw new Error("Replay clip render durationSecond must be greater than 0")
  }
  if (!positiveFinite(render.fps)) {
    throw new Error("Replay clip render fps must be greater than 0")
  }
  if (!positiveFinite(render.width) || !positiveFinite(render.height)) {
    throw new Error("Replay clip render width and height must be greater than 0")
  }
}

const httpsUrlPattern = /^https:\/\//i
const sha256Pattern = /^[a-f0-9]{64}$/i

/**
 * Validate a clip-job request and return the decoded value. Fails closed on
 * raw/private material or out-of-bounds render parameters.
 */
export const assertReplayClipJobRequestSafe = (
  input: unknown,
): ReplayClipJobRequest => {
  const request = decodeReplayClipJobRequest(input)

  if (replayClipHasUnsafeMaterial(request)) {
    throw new Error("Replay clip job request contains raw/private material")
  }

  assertRenderSpecBounds(request.render)

  if (request.sourceRefs.length === 0) {
    throw new Error("Replay clip job request must carry at least one sourceRef")
  }

  return request
}

/** Validate a persisted clip-job record (the public read projection). */
export const assertReplayClipJobRecordSafe = (
  input: unknown,
): ReplayClipJobRecord => {
  const record = decodeReplayClipJobRecord(input)

  if (replayClipHasUnsafeMaterial(record)) {
    throw new Error("Replay clip job record contains raw/private material")
  }

  assertRenderSpecBounds(record.render)

  if (record.sourceRefs.length === 0) {
    throw new Error("Replay clip job record must carry at least one sourceRef")
  }

  if (
    (record.status === "blocked" || record.status === "failed") &&
    record.blockerRefs.length === 0
  ) {
    throw new Error(
      "Replay clip job record in blocked/failed status must carry blockerRefs",
    )
  }

  if (record.status === "succeeded" && record.manifestRef === undefined) {
    throw new Error(
      "Replay clip job record in succeeded status must carry a manifestRef",
    )
  }

  return record
}

/**
 * Validate an output manifest. Fails closed on raw/private material, missing
 * source refs, non-https storage URLs, or malformed sha256 digests.
 */
export const assertReplayClipManifestSafe = (
  input: unknown,
): ReplayClipManifest => {
  const manifest = decodeReplayClipManifest(input)

  if (replayClipHasUnsafeMaterial(manifest)) {
    throw new Error("Replay clip manifest contains raw/private material")
  }

  assertRenderSpecBounds(manifest.render)

  if (manifest.sourceRefs.length === 0) {
    throw new Error("Replay clip manifest must be source-ref complete")
  }

  if (manifest.artifacts.length === 0) {
    throw new Error("Replay clip manifest must reference at least one artifact")
  }

  for (const artifact of manifest.artifacts) {
    if (!httpsUrlPattern.test(artifact.storageUrl)) {
      throw new Error(
        "Replay clip manifest artifact storageUrl must be a public https URL",
      )
    }
    if (!sha256Pattern.test(artifact.sha256)) {
      throw new Error(
        "Replay clip manifest artifact sha256 must be a 64-char hex digest",
      )
    }
    if (!positiveFinite(artifact.byteSize)) {
      throw new Error(
        "Replay clip manifest artifact byteSize must be greater than 0",
      )
    }
  }

  if (!positiveFinite(manifest.frameCount)) {
    throw new Error("Replay clip manifest frameCount must be greater than 0")
  }

  return manifest
}
