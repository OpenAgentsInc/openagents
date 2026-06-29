import { describe, expect, test } from "bun:test"

import {
  REPLAY_CAMERA_PATH_MAX_FOV,
  REPLAY_CAMERA_PATH_MAX_KEYFRAMES,
  REPLAY_CAMERA_PATH_MAX_SECOND,
  REPLAY_CAMERA_PATH_MIN_FOV,
  REPLAY_CAMERA_PATH_SCHEMA_VERSION,
  REPLAY_CLIP_CLAIM_SCOPE,
  REPLAY_CLIP_JOB_SCHEMA_VERSION,
  REPLAY_CLIP_MANIFEST_SCHEMA_VERSION,
  assertReplayClipJobRecordSafe,
  assertReplayClipJobRequestSafe,
  assertReplayClipManifestSafe,
  compileReplayCameraPath,
  makeReplayCameraPath,
  parseReplayCameraPath,
  replayCameraVerbs,
  replayClipHasUnsafeMaterial,
  replayClipJobStatusCanTransition,
  replayClipJobStatuses,
} from "./index.js"
import {
  blockedClipJobRecord,
  curatedBundleClipJobRequest,
  followActorCameraPath,
  launchMomentCameraPath,
  queuedClipJobRecord,
  replayCameraPathFixtures,
  replayClipJobRecordFixtures,
  replayClipJobRequestFixtures,
  succeededClipJobRecord,
  succeededClipManifest,
  timelineRangeClipJobRequest,
} from "./fixtures.js"

describe("@openagentsinc/replay-clips camera-path DSL (#5433)", () => {
  test("parses and orders every camera-path fixture", () => {
    for (const path of replayCameraPathFixtures) {
      const parsed = parseReplayCameraPath(path)
      expect(parsed.schemaVersion).toBe(REPLAY_CAMERA_PATH_SCHEMA_VERSION)
      const seconds = parsed.keyframes.map(keyframe => keyframe.second)
      expect(seconds).toEqual([...seconds].sort((a, b) => a - b))
    }
  })

  test("sorts out-of-order keyframes by second", () => {
    const parsed = makeReplayCameraPath([
      { second: 4, verb: "frame_settlement" },
      { second: 0, verb: "hold" },
      { second: 2, verb: "orbit" },
    ])
    expect(parsed.keyframes.map(k => k.second)).toEqual([0, 2, 4])
  })

  test("compiles DSL verbs into render-box camera modes", () => {
    const compiled = compileReplayCameraPath(launchMomentCameraPath)
    expect(compiled.keyframes).toEqual([
      { second: 0, mode: "director_track" },
      { second: 2, mode: "orbit_proof", fov: 55 },
      { second: 4, mode: "zap_focus" },
    ])
  })

  test("compiles follow/frame_actor to follow_actor mode", () => {
    const compiled = compileReplayCameraPath(followActorCameraPath)
    expect(compiled.keyframes.map(k => k.mode)).toEqual([
      "follow_actor",
      "follow_actor",
    ])
  })

  test("clamps fov to bounds", () => {
    const high = makeReplayCameraPath([
      { second: 0, verb: "orbit", fov: 999 },
    ])
    expect(high.keyframes[0]?.fov).toBe(REPLAY_CAMERA_PATH_MAX_FOV)
    const low = makeReplayCameraPath([{ second: 0, verb: "orbit", fov: 1 }])
    expect(low.keyframes[0]?.fov).toBe(REPLAY_CAMERA_PATH_MIN_FOV)
  })

  test("fails closed on empty keyframes", () => {
    expect(() =>
      parseReplayCameraPath({
        schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
        keyframes: [],
      }),
    ).toThrow(/at least one keyframe/)
  })

  test("fails closed past the keyframe ceiling", () => {
    const keyframes = Array.from(
      { length: REPLAY_CAMERA_PATH_MAX_KEYFRAMES + 1 },
      (_, index) => ({ second: index, verb: "hold" as const }),
    )
    expect(() =>
      parseReplayCameraPath({
        schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
        keyframes,
      }),
    ).toThrow(/exceeds .* keyframes/)
  })

  test("fails closed when follow/frame_actor lacks an actorRef", () => {
    for (const verb of ["follow", "frame_actor"] as const) {
      expect(() =>
        parseReplayCameraPath({
          schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
          keyframes: [{ second: 0, verb }],
        }),
      ).toThrow(/requires an actorRef/)
    }
  })

  test("fails closed when a non-actor verb carries an actorRef", () => {
    for (const verb of ["hold", "orbit", "frame_settlement"] as const) {
      expect(() =>
        parseReplayCameraPath({
          schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
          keyframes: [{ second: 0, verb, actorRef: "actor.x" }],
        }),
      ).toThrow(/must not carry an actorRef/)
    }
  })

  test("fails closed on negative or over-long seconds", () => {
    expect(() =>
      makeReplayCameraPath([{ second: -1, verb: "hold" }]),
    ).toThrow(/non-negative/)
    expect(() =>
      makeReplayCameraPath([
        { second: REPLAY_CAMERA_PATH_MAX_SECOND + 1, verb: "hold" },
      ]),
    ).toThrow(/exceeds/)
  })

  test("fails closed on unknown verb", () => {
    expect(() =>
      parseReplayCameraPath({
        schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
        keyframes: [{ second: 0, verb: "teleport" }],
      }),
    ).toThrow()
  })

  test("fails closed on raw/private material in actorRef", () => {
    expect(() =>
      parseReplayCameraPath({
        schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
        keyframes: [
          { second: 0, verb: "follow", actorRef: "/Users/secret/path" },
        ],
      }),
    ).toThrow(/raw\/private material/)
  })

  test("two different camera paths compile to different keyframe modes", () => {
    const a = compileReplayCameraPath(launchMomentCameraPath)
    const b = compileReplayCameraPath(followActorCameraPath)
    expect(JSON.stringify(a)).not.toBe(JSON.stringify(b))
  })

  test("exposes the bounded verb set", () => {
    expect(replayCameraVerbs).toEqual([
      "hold",
      "orbit",
      "follow",
      "frame_actor",
      "frame_settlement",
    ])
  })
})

describe("@openagentsinc/replay-clips clip job + manifest (#5430)", () => {
  test("decodes and validates every clip-job request fixture", () => {
    for (const request of replayClipJobRequestFixtures) {
      const safe = assertReplayClipJobRequestSafe(request)
      expect(safe.schemaVersion).toBe(REPLAY_CLIP_JOB_SCHEMA_VERSION)
      expect(safe.sourceRefs.length).toBeGreaterThan(0)
    }
  })

  test("accepts both bundle and timeline-range sources", () => {
    expect(curatedBundleClipJobRequest.source.kind).toBe("replay_bundle")
    expect(timelineRangeClipJobRequest.source.kind).toBe("timeline_range")
    assertReplayClipJobRequestSafe(curatedBundleClipJobRequest)
    assertReplayClipJobRequestSafe(timelineRangeClipJobRequest)
  })

  test("decodes and validates every clip-job record fixture", () => {
    for (const record of replayClipJobRecordFixtures) {
      const safe = assertReplayClipJobRecordSafe(record)
      expect(safe.claimScope).toBe(REPLAY_CLIP_CLAIM_SCOPE)
    }
  })

  test("blocked records keep blocker refs", () => {
    expect(blockedClipJobRecord.status).toBe("blocked")
    expect(blockedClipJobRecord.blockerRefs.length).toBeGreaterThan(0)
    assertReplayClipJobRecordSafe(blockedClipJobRecord)
  })

  test("fails closed when blocked/failed record lacks blocker refs", () => {
    expect(() =>
      assertReplayClipJobRecordSafe({
        ...queuedClipJobRecord,
        status: "failed",
        blockerRefs: [],
      }),
    ).toThrow(/must carry blockerRefs/)
  })

  test("fails closed when succeeded record lacks a manifest ref", () => {
    expect(() =>
      assertReplayClipJobRecordSafe({
        ...queuedClipJobRecord,
        status: "succeeded",
      }),
    ).toThrow(/must carry a manifestRef/)
    assertReplayClipJobRecordSafe(succeededClipJobRecord)
  })

  test("fails closed on out-of-bounds render parameters", () => {
    expect(() =>
      assertReplayClipJobRequestSafe({
        ...curatedBundleClipJobRequest,
        render: { ...curatedBundleClipJobRequest.render, durationSecond: 0 },
      }),
    ).toThrow(/durationSecond must be greater than 0/)
    expect(() =>
      assertReplayClipJobRequestSafe({
        ...curatedBundleClipJobRequest,
        render: { ...curatedBundleClipJobRequest.render, fps: -1 },
      }),
    ).toThrow(/fps must be greater than 0/)
    expect(() =>
      assertReplayClipJobRequestSafe({
        ...curatedBundleClipJobRequest,
        render: { ...curatedBundleClipJobRequest.render, startSecond: -5 },
      }),
    ).toThrow(/startSecond must be a non-negative number/)
  })

  test("fails closed when a request carries no source refs", () => {
    expect(() =>
      assertReplayClipJobRequestSafe({
        ...curatedBundleClipJobRequest,
        sourceRefs: [],
      }),
    ).toThrow(/at least one sourceRef/)
  })

  test("fails closed on raw/private material in a request", () => {
    expect(() =>
      assertReplayClipJobRequestSafe({
        ...curatedBundleClipJobRequest,
        sourceRefs: ["/Users/chris/secret/trace.json"],
      }),
    ).toThrow(/raw\/private material/)
  })

  test("detects unsafe material helper directly", () => {
    expect(
      replayClipHasUnsafeMaterial({ note: "lnbc1pabc payment invoice" }),
    ).toBe(true)
    expect(
      replayClipHasUnsafeMaterial({ bundleRef: "first-real-settlement" }),
    ).toBe(false)
  })
})

describe("@openagentsinc/replay-clips manifest (#5430)", () => {
  test("validates a source-ref-complete manifest", () => {
    const safe = assertReplayClipManifestSafe(succeededClipManifest)
    expect(safe.schemaVersion).toBe(REPLAY_CLIP_MANIFEST_SCHEMA_VERSION)
    expect(safe.claimScope).toBe(REPLAY_CLIP_CLAIM_SCOPE)
    expect(safe.artifacts.length).toBeGreaterThan(0)
  })

  test("fails closed on missing source refs", () => {
    expect(() =>
      assertReplayClipManifestSafe({
        ...succeededClipManifest,
        sourceRefs: [],
      }),
    ).toThrow(/source-ref complete/)
  })

  test("fails closed on non-https storage URL", () => {
    expect(() =>
      assertReplayClipManifestSafe({
        ...succeededClipManifest,
        artifacts: [
          {
            ...succeededClipManifest.artifacts[0]!,
            storageUrl: "http://clips.openagents.com/x.mp4",
          },
        ],
      }),
    ).toThrow(/public https URL/)
  })

  test("fails closed on malformed sha256", () => {
    expect(() =>
      assertReplayClipManifestSafe({
        ...succeededClipManifest,
        artifacts: [
          { ...succeededClipManifest.artifacts[0]!, sha256: "deadbeef" },
        ],
      }),
    ).toThrow(/64-char hex digest/)
  })

  test("fails closed when manifest has no artifacts", () => {
    expect(() =>
      assertReplayClipManifestSafe({
        ...succeededClipManifest,
        artifacts: [],
      }),
    ).toThrow(/at least one artifact/)
  })
})

describe("@openagentsinc/replay-clips lifecycle", () => {
  test("exposes the status set", () => {
    expect(replayClipJobStatuses).toEqual([
      "queued",
      "rendering",
      "succeeded",
      "failed",
      "blocked",
    ])
  })

  test("enforces the lifecycle state machine", () => {
    expect(replayClipJobStatusCanTransition("queued", "rendering")).toBe(true)
    expect(replayClipJobStatusCanTransition("rendering", "succeeded")).toBe(true)
    expect(replayClipJobStatusCanTransition("blocked", "queued")).toBe(true)
    expect(replayClipJobStatusCanTransition("succeeded", "rendering")).toBe(
      false,
    )
    expect(replayClipJobStatusCanTransition("queued", "succeeded")).toBe(false)
  })
})
