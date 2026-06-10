import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import {
  TassadarNumericExecutionError,
  collectInterpreterOutputs,
  executeTassadarNumericModel,
} from "./numeric-executor.js"
import { verifyTassadarFullReplay, verifyTassadarWindow } from "./replay.js"

const fixture = JSON.parse(
  readFileSync(new URL("../fixtures/tassadar-poc-loop-sum-v1.json", import.meta.url), "utf8"),
)

describe("tassadar numeric executor (TS leg)", () => {
  test("reproduces the Rust trace digest byte-for-byte on the committed fixture", async () => {
    const trace = await executeTassadarNumericModel(fixture.model, fixture.steps)
    expect(trace.traceDigest).toBe(fixture.expectedTraceDigest)
    expect(trace.stepCount).toBe(fixture.steps.length)
    const { outputs, halted } = collectInterpreterOutputs(trace.stepOutputs)
    expect(halted).toBe(true)
    expect(outputs.map((value) => Number(value))).toEqual(fixture.expectedOutputs)
  })

  test("full replay verdicts verify honest digests and reject forged ones", async () => {
    const honest = await verifyTassadarFullReplay({
      claimedTraceDigest: fixture.expectedTraceDigest,
      model: fixture.model,
      steps: fixture.steps,
      validatorDeviceRef: "device.test",
    })
    expect(honest.outcome).toBe("verified")
    const forged = await verifyTassadarFullReplay({
      claimedTraceDigest: "forged",
      model: fixture.model,
      steps: fixture.steps,
      validatorDeviceRef: "device.test",
    })
    expect(forged.outcome).toBe("rejected")
    expect(forged.rejection?.reason).toBe("trace_digest_mismatch")
  })

  test("window spot-checks name the exact tampered step", async () => {
    const trace = await executeTassadarNumericModel(fixture.model, fixture.steps)
    const honestRows = trace.stepOutputs.slice(10, 13)
    const honest = await verifyTassadarWindow({
      claimedRows: honestRows,
      model: fixture.model,
      steps: fixture.steps,
      validatorDeviceRef: "device.test",
      windowStart: 10,
    })
    expect(honest.outcome).toBe("verified")
    const tamperedRows = honestRows.map((row, offset) =>
      offset === 1 ? row.map((value, i) => (i === 1 ? value + 1n : value)) : row,
    )
    const tampered = await verifyTassadarWindow({
      claimedRows: tamperedRows,
      model: fixture.model,
      steps: fixture.steps,
      validatorDeviceRef: "device.test",
      windowStart: 10,
    })
    expect(tampered.outcome).toBe("rejected")
    expect(tampered.rejection).toEqual({ reason: "row_mismatch", step: 11 })
  })

  test("input arity mismatches refuse with typed errors", async () => {
    await expect(
      executeTassadarNumericModel(fixture.model, [[0, 1]]),
    ).rejects.toBeInstanceOf(TassadarNumericExecutionError)
  })
})
