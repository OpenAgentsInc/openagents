import { describe, expect, test } from "bun:test"
import { runOneWorker, type SpawnedWorkerProcess } from "./concurrent-checkout-proof.js"

function fakeProcess(exitCode: number, stdout: string, stderr = ""): SpawnedWorkerProcess {
  return {
    exited: Promise.resolve(exitCode),
    stderr: new Response(stderr).body,
    stdout: new Response(stdout).body,
  }
}

describe("runOneWorker", () => {
  test("returns a normal ok result when the worker process exits 0", async () => {
    const result = await runOneWorker(0, { PROOF_FULL_NAME: "x" }, () =>
      fakeProcess(0, `${JSON.stringify({ leaseRef: "lease.public.proof.0", ok: true })}\n`),
    )
    expect(result).toEqual({
      exitCode: 0,
      index: 0,
      stderr: "",
      stdout: JSON.stringify({ leaseRef: "lease.public.proof.0", ok: true }),
    })
  })

  test("returns a failed result (not a rejection) when spawning the worker throws", async () => {
    const result = await runOneWorker(3, { PROOF_FULL_NAME: "x" }, () => {
      throw new Error("EMFILE: too many open files")
    })
    expect(result.index).toBe(3)
    expect(result.exitCode).toBe(1)
    expect(result.stderr).toContain("harness_spawn_failed")
    expect(result.stderr).toContain("EMFILE")
  })

  test("one worker's spawn failure does not discard the other workers' results under Promise.all", async () => {
    // Regression for the Promise.all cron-landmine audit: a harness-level
    // throw for one worker must not abort visibility into sibling workers.
    const baseEnv = { PROOF_FULL_NAME: "x" }
    const results = await Promise.all([
      runOneWorker(0, baseEnv, () => fakeProcess(0, JSON.stringify({ ok: true }))),
      runOneWorker(1, baseEnv, () => {
        throw new Error("spawn EAGAIN")
      }),
      runOneWorker(2, baseEnv, () => fakeProcess(0, JSON.stringify({ ok: true }))),
    ])

    expect(results).toHaveLength(3)
    expect(results[0]?.exitCode).toBe(0)
    expect(results[1]?.exitCode).toBe(1)
    expect(results[1]?.stderr).toContain("harness_spawn_failed")
    expect(results[2]?.exitCode).toBe(0)
  })
})
