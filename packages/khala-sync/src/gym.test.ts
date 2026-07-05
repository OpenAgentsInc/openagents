import { describe, expect, test } from "bun:test"
import {
  canonicalJson,
  decodeGymRunProgressEntity,
  encodeGymRunProgressEntity,
  GYM_RUN_PROGRESS_ENTITY_TYPE,
  GYM_RUN_PROGRESS_PUBLIC_CHANNEL,
} from "./index.js"

/**
 * Gym run-progress entity contract (KS-6.5, #8415). The load-bearing
 * property here is SPEC §7 invariant 9: this shape must structurally
 * REFUSE raw private material — emails, filesystem paths, bearer strings,
 * URLs — so a redaction bug upstream fails to decode instead of
 * replicating, mirroring the Worker's own
 * `checkGymRunProgressPublicSafety` tripwire.
 */

// Deliberately NOT a bare "token" match: this entity legitimately carries
// `promptTokens`/`completionTokens`/`totalTokens` COUNT fields (mirroring the
// Worker's own `checkGymRunProgressPublicSafety`, which reserves leak SHAPES
// rather than the bare field names).
const FORBIDDEN = /apikey|authorization:|bearer[:\s]|mnemonic|secret|\/users\/|https?:\/\//i

const validPublished = {
  agent: "opencode",
  blockerRefs: [],
  caveatRefs: [],
  completionFraction: 0.1685,
  configId: "config.run.web.alpha",
  counts: {
    cancelled: 0,
    completed: 13,
    completedFailed: 0,
    completedPassed: 13,
    error: 0,
    officialDenominator: 89,
    pending: 74,
    running: 2,
  },
  decisionGrade: false,
  elapsedMs: 540_000,
  inProgress: true,
  jobRef: "job.run.web.alpha",
  lastUpdatedAt: "2026-06-25T00:00:00.000Z",
  passRateOverCompleted: 1,
  phase: "running",
  profile: {
    attribution: "Z.ai GLM-5.2 (REAP-504B)",
    contextWindowTokens: 65_536,
    hardwareProfile: "hydralisk-g4-4x-rtx-pro-6000",
    model: "openagents/glm-5.2-reap-504b",
    profileRef: "khala-public-heuristic",
    publicLabel: "Khala public heuristic",
  },
  publication: "web_authorized",
  runRef: "run.web.alpha",
  tokens: {
    completionTokens: null,
    promptTokens: null,
    totalTokens: null,
  },
}

const validUnpublished = {
  blockerRefs: ["blocker.gym.run_progress.not_authorized_for_web_publication"],
  decisionGrade: false,
  inProgress: true,
  lastUpdatedAt: "2026-06-25T00:00:00.000Z",
  publication: "local_only",
  runRef: "run.local.beta",
}

describe("gym run-progress entity contract", () => {
  test("entity type + shared public channel are the expected constants", () => {
    expect(GYM_RUN_PROGRESS_ENTITY_TYPE).toBe("gym_run_progress")
    expect(GYM_RUN_PROGRESS_PUBLIC_CHANNEL).toBe("gym-run-progress")
  })

  test("published (web_authorized) shape decodes and re-encodes canonically", () => {
    const entity = decodeGymRunProgressEntity(validPublished)
    expect(entity.runRef).toBe("run.web.alpha")
    expect(entity.publication).toBe("web_authorized")
    expect(canonicalJson(encodeGymRunProgressEntity(entity))).not.toMatch(
      FORBIDDEN,
    )
  })

  test("unpublished (local_only) shape decodes with NO live counts", () => {
    const entity = decodeGymRunProgressEntity(validUnpublished)
    expect(entity.runRef).toBe("run.local.beta")
    expect(entity.publication).toBe("local_only")
    expect("counts" in entity).toBe(false)
  })

  test("rejects a model ref with more than one path segment", () => {
    expect(() =>
      decodeGymRunProgressEntity({
        ...validPublished,
        profile: { ...validPublished.profile, model: "a/b/c" },
      }),
    ).toThrow()
  })

  test("rejects out-of-range unit fractions and negative counts", () => {
    expect(() =>
      decodeGymRunProgressEntity({
        ...validPublished,
        completionFraction: 1.5,
      }),
    ).toThrow()
    expect(() =>
      decodeGymRunProgressEntity({
        ...validPublished,
        counts: { ...validPublished.counts, completed: -1 },
      }),
    ).toThrow()
  })

  test("refs structurally refuse emails, paths, and whitespace", () => {
    expect(() =>
      decodeGymRunProgressEntity({
        ...validPublished,
        runRef: "/Users/alice/run",
      }),
    ).toThrow()
    expect(() =>
      decodeGymRunProgressEntity({
        ...validPublished,
        runRef: "user@example.com",
      }),
    ).toThrow()
    expect(() =>
      decodeGymRunProgressEntity({
        ...validPublished,
        blockerRefs: ["bearer token-should-not-decode"],
      }),
    ).toThrow()
  })

  test("rejects an unknown publication literal", () => {
    expect(() =>
      decodeGymRunProgressEntity({
        ...validPublished,
        publication: "exploded",
      }),
    ).toThrow()
  })
})
