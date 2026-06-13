import { describe, expect, test } from "bun:test"

import {
  BackgroundRunTransitionError,
  supervise,
  type BackgroundRun,
} from "../src/tas/task-supervision"

const queuedRun: BackgroundRun = {
  runId: "run.pylon.background.fixture",
  kind: "fixture",
  state: "queued",
}

describe("task background run supervision", () => {
  test("legal lifecycle moves queued run through running to completed", () => {
    const started = supervise(queuedRun, {
      type: "started",
      at: "2026-06-11T12:00:00.000Z",
      evidenceRefs: ["artifact.task.start"],
    })

    const completed = supervise(started.run, {
      type: "completed",
      at: "2026-06-11T12:05:00.000Z",
      evidenceRefs: ["artifact.task.complete"],
    })

    expect(started).toEqual({
      run: {
        ...queuedRun,
        state: "running",
        startedAt: "2026-06-11T12:00:00.000Z",
        evidenceRefs: ["artifact.task.start"],
      },
      emitReceipt: false,
    })
    expect(completed).toEqual({
      run: {
        ...queuedRun,
        state: "completed",
        startedAt: "2026-06-11T12:00:00.000Z",
        endedAt: "2026-06-11T12:05:00.000Z",
        evidenceRefs: ["artifact.task.start", "artifact.task.complete"],
      },
      emitReceipt: true,
    })
  })

  test("illegal transition is rejected", () => {
    expect(() =>
      supervise(queuedRun, {
        type: "completed",
        at: "2026-06-11T12:05:00.000Z",
      }),
    ).toThrow(BackgroundRunTransitionError)
  })

  test("terminal state is final", () => {
    const completed: BackgroundRun = {
      ...queuedRun,
      state: "completed",
      startedAt: "2026-06-11T12:00:00.000Z",
      endedAt: "2026-06-11T12:05:00.000Z",
    }

    expect(() =>
      supervise(completed, {
        type: "started",
        at: "2026-06-11T12:06:00.000Z",
      }),
    ).toThrow(BackgroundRunTransitionError)
    expect(() =>
      supervise(completed, {
        type: "evidence.recorded",
        evidenceRefs: ["artifact.task.late"],
      }),
    ).toThrow(BackgroundRunTransitionError)
  })

  test("receipt emission is signaled only for terminal transitions", () => {
    const started = supervise(queuedRun, {
      type: "started",
      at: "2026-06-11T12:00:00.000Z",
    })
    const evidence = supervise(started.run, {
      type: "evidence.recorded",
      evidenceRefs: ["artifact.task.log"],
    })
    const failed = supervise(evidence.run, {
      type: "failed",
      at: "2026-06-11T12:03:00.000Z",
      evidenceRefs: ["artifact.task.failure"],
    })

    expect(started.emitReceipt).toBe(false)
    expect(evidence.emitReceipt).toBe(false)
    expect(failed.emitReceipt).toBe(true)
    expect(failed.run.evidenceRefs).toEqual([
      "artifact.task.log",
      "artifact.task.failure",
    ])
  })
})
