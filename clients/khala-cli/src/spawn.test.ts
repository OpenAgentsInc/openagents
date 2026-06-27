import { mkdtemp } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { describe, expect, test } from "bun:test"

import {
  cancelKhalaSpawn,
  listKhalaSpawnRuns,
  readKhalaSpawnRun,
  runKhalaSpawn,
  summarizeSpawnRun,
  type KhalaSpawnLifecycleEvent,
  type KhalaSpawnWorkerRunner,
} from "./spawn.js"

describe("Khala spawn supervisor", () => {
  test("creates a persisted parent run with local child worker closeouts", async () => {
    const home = await mkdtemp(join(tmpdir(), "khala-spawn-test-"))
    const env = { KHALA_HOME: home }
    const runner: KhalaSpawnWorkerRunner = async input => ({
      commandCount: input.worker.slotIndex,
      editedFileCount: 0,
      sessionRef: `session.${input.worker.slotIndex}`,
      text: `worker ${input.worker.slotIndex} done`,
      turnCount: 1,
    })

    const run = await runKhalaSpawn({
      count: 2,
      cwd: home,
      env,
      maxParallel: 2,
      objective: "audit the checkout flow",
      runner,
      strategy: "local",
      workspaceFactory: async input => join(home, "workspace", input.workerRef),
    })

    expect(run.schema).toBe("openagents.khala.spawn_run.v0.1")
    expect(run.state).toBe("completed")
    expect(run.workers).toHaveLength(2)
    expect(run.workers.map(worker => worker.state)).toEqual(["accepted", "accepted"])
    expect(run.workers[0]?.sessionRef).toBe("session.1")

    const stored = await readKhalaSpawnRun(env, run.runRef)
    expect(stored.runRef).toBe(run.runRef)
    expect(stored.workers[1]?.resultText).toBe("worker 2 done")

    const listed = await listKhalaSpawnRuns(env)
    expect(listed.runs.map(candidate => candidate.runRef)).toContain(run.runRef)
    expect(summarizeSpawnRun(run)).toContain("2 accepted")
  })

  test("records cancellation and aborts running workers", async () => {
    const home = await mkdtemp(join(tmpdir(), "khala-spawn-cancel-test-"))
    const env = { KHALA_HOME: home }
    let runRef = ""
    const events: KhalaSpawnLifecycleEvent[] = []
    const runner: KhalaSpawnWorkerRunner = async input => {
      while (!input.signal.aborted) {
        await Bun.sleep(10)
      }
      throw new Error("aborted")
    }

    const running = runKhalaSpawn({
      count: 2,
      cwd: home,
      env,
      maxParallel: 1,
      objective: "audit cancellation",
      onEvent: async event => {
        events.push(event)
        runRef = event.runRef
      },
      runner,
      strategy: "local",
      workspaceFactory: async input => join(home, "workspace", input.workerRef),
    })

    for (let index = 0; index < 200 && !events.some(event => event.state === "starting"); index += 1) {
      await Bun.sleep(10)
    }
    expect(runRef).not.toBe("")

    const cancel = await cancelKhalaSpawn(env, runRef)
    expect(cancel.ok).toBe(true)

    const run = await running
    expect(run.state).toBe("cancelled")
    expect(run.workers.every(worker => worker.state === "cancelled")).toBe(true)
    expect(run.workers.flatMap(worker => worker.blockerRefs)).toContain("blocker.khala_spawn.cancelled")
  })

  test("keeps a run-level cancellation visible after an earlier worker accepted", async () => {
    const home = await mkdtemp(join(tmpdir(), "khala-spawn-partial-cancel-test-"))
    const env = { KHALA_HOME: home }
    let runRef = ""
    const events: KhalaSpawnLifecycleEvent[] = []
    const runner: KhalaSpawnWorkerRunner = async input => {
      if (input.worker.slotIndex === 1) {
        return {
          commandCount: 1,
          editedFileCount: 0,
          sessionRef: "session.accepted",
          text: "first worker done",
          turnCount: 1,
        }
      }
      while (!input.signal.aborted) {
        await Bun.sleep(10)
      }
      throw new Error("aborted")
    }

    const running = runKhalaSpawn({
      count: 2,
      cwd: home,
      env,
      maxParallel: 1,
      objective: "audit partial cancellation",
      onEvent: async event => {
        events.push(event)
        runRef = event.runRef
      },
      runner,
      strategy: "local",
      workspaceFactory: async input => join(home, "workspace", input.workerRef),
    })

    for (let index = 0; index < 200 && !events.some(event => event.workerRef?.endsWith(".02") && event.state === "starting"); index += 1) {
      await Bun.sleep(10)
    }
    expect(runRef).not.toBe("")

    await cancelKhalaSpawn(env, runRef)

    const run = await running
    expect(run.state).toBe("cancelled")
    expect(run.workers.map(worker => worker.state)).toEqual(["accepted", "cancelled"])
  })
})
