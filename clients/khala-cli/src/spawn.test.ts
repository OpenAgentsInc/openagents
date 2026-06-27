import { mkdtemp } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { describe, expect, test } from "bun:test"

import {
  cancelKhalaSpawn,
  listKhalaSpawnRuns,
  readKhalaSpawnRun,
  refreshKhalaPylonSpawnRun,
  runKhalaSpawn,
  summarizeSpawnRun,
  type KhalaSpawnLifecycleEvent,
  type KhalaSpawnMcpCaller,
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

  test("dispatches pylon strategy through khala.spawn MCP and persists child assignments", async () => {
    const home = await mkdtemp(join(tmpdir(), "khala-spawn-pylon-test-"))
    const env = { KHALA_HOME: home }
    const calls: Array<{ readonly args: Record<string, unknown>; readonly tool: string }> = []
    const mcpCaller: KhalaSpawnMcpCaller = async input => {
      calls.push({ args: input.args, tool: input.tool })
      expect(input.baseUrl).toBe("https://example.test")
      expect(input.token).toBe("oa_agent_test")
      expect(input.tool).toBe("khala.spawn")
      return {
        assignedCount: 2,
        blockerRefs: [],
        children: [
          {
            assignmentRef: "assignment.public.khala_coding.one",
            durableRequestId: "chatcmpl_one",
            durableStreamUrl: "/v1/chat/completions/durable/chatcmpl_one",
            ok: true,
            pylonRef: "pylon.owner.codex",
            slotIndex: 0,
            state: "running",
            workerRef: "worker.public.khala_coding.spawn.01",
          },
          {
            assignmentRef: "assignment.public.khala_coding.two",
            durableRequestId: "chatcmpl_two",
            durableStreamUrl: "/v1/chat/completions/durable/chatcmpl_two",
            ok: true,
            pylonRef: "pylon.owner.codex",
            slotIndex: 1,
            state: "offered",
            workerRef: "worker.public.khala_coding.spawn.02",
          },
        ],
        ok: true,
        requestedCount: 2,
        schema: "openagents.khala_mcp.spawn.v1",
        spawnRef: "spawn.public.khala_coding.test_spawn",
      }
    }

    const run = await runKhalaSpawn({
      baseUrl: "https://example.test",
      count: 2,
      cwd: home,
      env,
      fixture: true,
      maxParallel: 2,
      mcpCaller,
      objective: "audit the public fixture",
      pylonRef: "pylon.owner.codex",
      strategy: "pylon",
      token: "oa_agent_test",
    })

    expect(calls).toHaveLength(1)
    expect(calls[0]?.args).toMatchObject({
      count: 2,
      fixture: true,
      maxParallel: 2,
      objective: "audit the public fixture",
      targetPylonRef: "pylon.owner.codex",
      workflow: "codex_agent_task",
    })
    expect(run.runRef).toBe("spawn.public.khala_coding.test_spawn")
    expect(run.strategy).toBe("pylon_codex_assignments")
    expect(run.workers).toHaveLength(2)
    expect(run.workers.map(worker => worker.assignmentRef)).toEqual([
      "assignment.public.khala_coding.one",
      "assignment.public.khala_coding.two",
    ])
    expect(run.workers.every(worker => worker.state === "running")).toBe(true)

    const stored = await readKhalaSpawnRun(env, run.runRef)
    expect(stored.workers[0]?.durableRequestId).toBe("chatcmpl_one")
    expect(summarizeSpawnRun(stored)).toContain("strategy: pylon_codex_assignments")
  })

  test("refreshes pylon spawn status through khala.spawnStatus", async () => {
    const home = await mkdtemp(join(tmpdir(), "khala-spawn-pylon-refresh-test-"))
    const env = { KHALA_HOME: home }
    const spawnCaller: KhalaSpawnMcpCaller = async () => ({
      assignedCount: 1,
      blockerRefs: [],
      children: [
        {
          assignmentRef: "assignment.public.khala_coding.one",
          durableRequestId: "chatcmpl_one",
          ok: true,
          pylonRef: "pylon.owner.codex",
          slotIndex: 0,
          state: "running",
          workerRef: "worker.public.khala_coding.spawn.01",
        },
      ],
      ok: true,
      requestedCount: 1,
      schema: "openagents.khala_mcp.spawn.v1",
      spawnRef: "spawn.public.khala_coding.refresh",
    })
    const run = await runKhalaSpawn({
      baseUrl: "https://example.test",
      count: 1,
      cwd: home,
      env,
      mcpCaller: spawnCaller,
      objective: "audit status",
      strategy: "pylon",
      token: "oa_agent_test",
    })
    const refreshed = await refreshKhalaPylonSpawnRun({
      baseUrl: "https://example.test",
      env,
      mcpCaller: async input => {
        expect(input.tool).toBe("khala.spawnStatus")
        expect(input.args).toEqual({ spawnRef: run.runRef })
        return {
          childCount: 1,
          children: [
            {
              assignmentRef: "assignment.public.khala_coding.one",
              durableRequestId: "chatcmpl_one",
              pylonRef: "pylon.owner.codex",
              state: "accepted",
            },
          ],
          ok: true,
          schema: "openagents.khala_mcp.spawn_status.v1",
          spawnRef: run.runRef,
          state: "accepted",
        }
      },
      run,
      token: "oa_agent_test",
    })

    expect(refreshed.state).toBe("completed")
    expect(refreshed.workers[0]?.state).toBe("accepted")
    expect(refreshed.workers[0]?.resultText).toContain("pylon assignment state: accepted")
  })
})
