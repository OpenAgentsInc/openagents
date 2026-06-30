import { mkdtemp } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createExecCommandTool,
  createWriteStdinTool,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-write-stdin-tool-"))
}

async function startSession(workspace: string, cmd = "cat", sessionId = "s1") {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createExecCommandTool()]),
      { arguments: { cmd, tty: true, yield_time_ms: 20 }, id: "call_exec", name: "exec_command", sessionId },
      makeKhalaToolServices({ workingDirectory: workspace }),
    ),
  )
}

async function writeStdin(
  workspace: string,
  args: Readonly<Record<string, unknown>>,
  sessionId = "s1",
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createWriteStdinTool()]),
      { arguments: args, id: "call_stdin", name: "write_stdin", sessionId },
      makeKhalaToolServices({ workingDirectory: workspace }),
    ),
  )
}

function sessionIdFrom(result: Awaited<ReturnType<typeof startSession>>): string {
  const ui = result.ui as { sessionId?: string }
  if (ui.sessionId === undefined) throw new Error("missing session id")
  return ui.sessionId
}

describe("write_stdin tool", () => {
  test("starts interactive sessions with terminal stdin", async () => {
    const workspace = await makeWorkspace()
    const started = await startSession(workspace, "test -t 0; echo tty:$?; cat")
    const sessionId = sessionIdFrom(started)
    const polled = await writeStdin(workspace, { session_id: sessionId, yield_time_ms: 150 })

    expect(started.status).toBe("ok")
    expect(polled.status).toBe("ok")
    expect(polled.modelOutput.text).toContain(process.platform === "darwin" ? "tty:1" : "tty:0")
    await writeStdin(workspace, { chars: "\u0003", session_id: sessionId, yield_time_ms: 10 })
  })

  test("writes input to an interactive session", async () => {
    const workspace = await makeWorkspace()
    const started = await startSession(workspace)
    const sessionId = sessionIdFrom(started)

    const result = await writeStdin(workspace, { chars: "hello\n", session_id: sessionId, yield_time_ms: 50 })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("hello")
    expect(result.ui).toMatchObject({
      events: expect.arrayContaining([
        expect.objectContaining({ kind: "stdin_chunk" }),
        expect.objectContaining({ kind: "stdout_chunk" }),
      ]),
      sessionId,
    })
    await writeStdin(workspace, { chars: "\u0003", session_id: sessionId, yield_time_ms: 10 })
  })

  test("polls without input", async () => {
    const workspace = await makeWorkspace()
    const started = await startSession(workspace, "sh -c 'sleep 0.03; echo later; cat'")
    const sessionId = sessionIdFrom(started)

    const result = await writeStdin(workspace, { session_id: sessionId, yield_time_ms: 200 })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("later")
    await writeStdin(workspace, { chars: "\u0003", session_id: sessionId, yield_time_ms: 10 })
  })

  test("polls closed sessions but rejects writes to closed sessions", async () => {
    const workspace = await makeWorkspace()
    const started = await startSession(workspace, "printf done")
    const sessionId = sessionIdFrom(started)

    const poll = await writeStdin(workspace, { session_id: sessionId, yield_time_ms: 120 })
    const write = await writeStdin(workspace, { chars: "again\n", session_id: sessionId, yield_time_ms: 10 })

    expect(poll.status).toBe("ok")
    expect(poll.modelOutput.text).toContain("done")
    expect(write.status).toBe("failed")
    expect(write.publicSummary).toContain("process session is closed")
  })

  test("fails unknown sessions", async () => {
    const workspace = await makeWorkspace()

    const result = await writeStdin(workspace, { chars: "hello\n", session_id: "missing", yield_time_ms: 10 })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("unknown process session")
  })

  test("cannot attach to another Khala session", async () => {
    const workspace = await makeWorkspace()
    const started = await startSession(workspace, "cat", "owner-session")
    const sessionId = sessionIdFrom(started)

    const result = await writeStdin(workspace, { chars: "hello\n", session_id: sessionId, yield_time_ms: 10 }, "other-session")

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("active Khala session")
    await writeStdin(workspace, { chars: "\u0003", session_id: sessionId, yield_time_ms: 10 }, "owner-session")
  })

  test("truncates previews and spills oversized session output", async () => {
    const workspace = await makeWorkspace()
    const started = await startSession(workspace)
    const sessionId = sessionIdFrom(started)

    const result = await writeStdin(workspace, {
      chars: `${"abcdefghijklmnopqrstuvwxyz".repeat(30)}\n`,
      max_output_tokens: 2,
      session_id: sessionId,
      yield_time_ms: 50,
    })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("[stdin output truncated; see private artifact]")
    expect(result.artifacts).toHaveLength(1)
    expect(result.privateDataRefs).toHaveLength(1)
    await writeStdin(workspace, { chars: "\u0003", session_id: sessionId, yield_time_ms: 10 })
  })

  test("propagates cancellation through Ctrl-C input", async () => {
    const workspace = await makeWorkspace()
    const started = await startSession(workspace)
    const sessionId = sessionIdFrom(started)

    const result = await writeStdin(workspace, { chars: "\u0003", session_id: sessionId, yield_time_ms: 20 })

    expect(result.status).toBe("failed")
    expect(result.ui).toMatchObject({ cancelled: true, sessionId })
  })
})
