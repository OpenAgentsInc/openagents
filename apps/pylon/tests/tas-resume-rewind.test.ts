import { describe, expect, test } from "bun:test"

import { resolveResume, type ResumeCheckpoint } from "../src/tas/resume-rewind"

const checkpoints: readonly ResumeCheckpoint[] = [
  { seq: 1, ref: "checkpoint.001" },
  { seq: 2, ref: "checkpoint.002" },
  { seq: 5, ref: "checkpoint.005" },
]

describe("tas resume and rewind core", () => {
  test("resumes to the latest checkpoint when no target is provided", () => {
    expect(resolveResume(checkpoints)).toEqual({
      resumedRef: "checkpoint.005",
      truncatedRefs: [],
    })
  })

  test("rewinds to a mid seq and drops later checkpoints", () => {
    expect(resolveResume(checkpoints, 2)).toEqual({
      resumedRef: "checkpoint.002",
      truncatedRefs: ["checkpoint.005"],
    })
  })

  test("rejects invalid targets and non-monotonic checkpoint sequences", () => {
    expect(() => resolveResume(checkpoints, 3)).toThrow("resume target seq does not match")
    expect(() => resolveResume(checkpoints, 0)).toThrow("resume target seq must be")
    expect(() =>
      resolveResume([
        { seq: 1, ref: "checkpoint.001" },
        { seq: 1, ref: "checkpoint.duplicate" },
      ]),
    ).toThrow("strictly increasing")
  })
})
