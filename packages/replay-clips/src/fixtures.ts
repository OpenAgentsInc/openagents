/**
 * Public-safe fixtures for `@openagentsinc/replay-clips`.
 *
 * These mirror common launch moments: a curated proof-replay bundle clip, a
 * generated timeline-range clip, a succeeded clip with a manifest, and a
 * blocked clip whose source coverage is missing. Every fixture is public-safe
 * by construction (slugs, public timeline cursors, sha256 digests, public
 * storage URLs, source refs only).
 */
import {
  REPLAY_CAMERA_PATH_SCHEMA_VERSION,
  type ReplayCameraPath,
} from "./camera-path.js"
import {
  REPLAY_CLIP_CLAIM_SCOPE,
  REPLAY_CLIP_JOB_SCHEMA_VERSION,
  REPLAY_CLIP_MANIFEST_SCHEMA_VERSION,
  type ReplayClipJobRecord,
  type ReplayClipJobRequest,
  type ReplayClipManifest,
} from "./clip-job.js"

const generatedAt = "2026-06-18T18:00:00.000Z"

/** Camera path: open wide, orbit the proof, then frame the settlement. */
export const launchMomentCameraPath: ReplayCameraPath = {
  schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
  keyframes: [
    { second: 0, verb: "hold" },
    { second: 2, verb: "orbit", fov: 55 },
    { second: 4, verb: "frame_settlement" },
  ],
}

/** Camera path: follow a named actor across the claim->verify beat. */
export const followActorCameraPath: ReplayCameraPath = {
  schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
  keyframes: [
    { second: 0, verb: "follow", actorRef: "actor.worker.orrery", easing: "ease_in" },
    { second: 3, verb: "frame_actor", actorRef: "actor.validator.whitefang" },
  ],
}

/** A clip-job request over the curated first-real-settlement bundle. */
export const curatedBundleClipJobRequest: ReplayClipJobRequest = {
  schemaVersion: REPLAY_CLIP_JOB_SCHEMA_VERSION,
  source: { kind: "replay_bundle", bundleRef: "first-real-settlement" },
  render: {
    startSecond: 20,
    durationSecond: 5,
    fps: 12,
    width: 1280,
    height: 720,
    outputKind: "mp4",
  },
  cameraPath: launchMomentCameraPath,
  sourceRefs: [
    "https://openagents.com/api/public/tassadar-replays/first-real-settlement",
  ],
}

/** A clip-job request over a generated public timeline range. */
export const timelineRangeClipJobRequest: ReplayClipJobRequest = {
  schemaVersion: REPLAY_CLIP_JOB_SCHEMA_VERSION,
  source: {
    kind: "timeline_range",
    fromCursor: "2026-06-16T00:00:00.000Z:training_window:window.opened.1",
    toCursor: "2026-06-16T00:05:00.000Z:settlement_receipt:receipt.nexus.1",
    runRef: "run.tassadar.executor.20260615",
  },
  render: {
    startSecond: 0,
    durationSecond: 8,
    fps: 24,
    width: 1920,
    height: 1080,
    outputKind: "mp4",
  },
  cameraPath: followActorCameraPath,
  sourceRefs: [
    "https://openagents.com/api/public/activity-timeline?from=2026-06-16T00:00:00.000Z&to=2026-06-16T00:05:00.000Z",
  ],
}

/** A queued clip-job record (the public read projection at submission). */
export const queuedClipJobRecord: ReplayClipJobRecord = {
  schemaVersion: REPLAY_CLIP_JOB_SCHEMA_VERSION,
  jobRef: "replay_clip_job.first-real-settlement.0001",
  status: "queued",
  claimScope: REPLAY_CLIP_CLAIM_SCOPE,
  source: curatedBundleClipJobRequest.source,
  render: curatedBundleClipJobRequest.render,
  cameraPath: curatedBundleClipJobRequest.cameraPath,
  sourceRefs: curatedBundleClipJobRequest.sourceRefs,
  caveatRefs: [
    "Clip is evidence-presentation only and grants no settlement or payout authority.",
  ],
  blockerRefs: [],
  createdAt: generatedAt,
  updatedAt: generatedAt,
}

/** A succeeded clip-job record carrying a manifest ref. */
export const succeededClipJobRecord: ReplayClipJobRecord = {
  ...queuedClipJobRecord,
  jobRef: "replay_clip_job.first-real-settlement.0002",
  status: "succeeded",
  manifestRef:
    "https://clips.openagents.com/replay_clip_job.first-real-settlement.0002.render.json",
  updatedAt: "2026-06-18T18:02:00.000Z",
}

/** A blocked clip-job record whose source coverage is missing. */
export const blockedClipJobRecord: ReplayClipJobRecord = {
  ...queuedClipJobRecord,
  jobRef: "replay_clip_job.unknown-range.0003",
  status: "blocked",
  blockerRefs: [
    "projection_gap.replay_clip.source_coverage_missing",
    "https://openagents.com/api/public/activity-timeline?from=2026-06-01T00:00:00.000Z&to=2026-06-01T00:01:00.000Z",
  ],
  updatedAt: "2026-06-18T18:01:00.000Z",
}

/** A public-safe output manifest for a succeeded clip. */
export const succeededClipManifest: ReplayClipManifest = {
  schemaVersion: REPLAY_CLIP_MANIFEST_SCHEMA_VERSION,
  jobRef: "replay_clip_job.first-real-settlement.0002",
  claimScope: REPLAY_CLIP_CLAIM_SCOPE,
  bundleRef: "proof_replay_bundle.first-real-settlement",
  source: curatedBundleClipJobRequest.source,
  render: curatedBundleClipJobRequest.render,
  cameraPath: curatedBundleClipJobRequest.cameraPath,
  renderer: {
    renderer: "playwright-chromium-screenshot-plus-ffmpeg",
    rendererVersion: "replay-r1",
    runLocation:
      "local_or_ci_render_box_with_bun_node_headless_chromium_and_ffmpeg_not_cloudflare_worker",
    videoCodec: "libx264",
    pixelFormat: "yuv420p",
  },
  artifacts: [
    {
      kind: "mp4",
      storageUrl:
        "https://clips.openagents.com/replay_clip_job.first-real-settlement.0002.mp4",
      sha256:
        "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
      byteSize: 1_482_944,
    },
  ],
  frameCount: 60,
  sourceRefs: curatedBundleClipJobRequest.sourceRefs,
  caveatRefs: [
    "Clip is evidence-presentation only and grants no settlement or payout authority.",
  ],
  generatedAt: "2026-06-18T18:02:00.000Z",
}

export const replayClipJobRequestFixtures: ReadonlyArray<ReplayClipJobRequest> =
  [curatedBundleClipJobRequest, timelineRangeClipJobRequest]

export const replayClipJobRecordFixtures: ReadonlyArray<ReplayClipJobRecord> = [
  queuedClipJobRecord,
  succeededClipJobRecord,
  blockedClipJobRecord,
]

export const replayCameraPathFixtures: ReadonlyArray<ReplayCameraPath> = [
  launchMomentCameraPath,
  followActorCameraPath,
]
