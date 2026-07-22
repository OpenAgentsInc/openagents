import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import { createUnsandboxedKhalaProcessService } from "./index.js"

describe("Khala process session termination", () => {
  test("binds termination to the owning session and escalates to a proven hard exit", async () => {
    const workspace = await mkdtemp(join(tmpdir(), "khala-process-termination-"))
    const service = createUnsandboxedKhalaProcessService()
    const khalaSessionId = "session.khala.termination.owner"
    let processSessionId: string | undefined
    try {
      const started = await Effect.runPromise(service.startSession({
        argv: ["/bin/sh", "-c", "trap '' INT; echo ready; sleep 30"],
        command: "trap '' INT; echo ready; sleep 30",
        cwd: workspace,
        khalaSessionId,
        maxCaptureBytes: 1_024,
        timeoutMs: 60_000,
        workspaceRoot: workspace,
        yieldTimeMs: 10,
      }))
      processSessionId = started.sessionId
      const ready = await Effect.runPromise(service.writeStdin({
        khalaSessionId,
        maxCaptureBytes: 1_024,
        sessionId: started.sessionId,
        yieldTimeMs: 500,
      }))
      expect(ready.stdout).toContain("ready")

      await expect(Effect.runPromise(service.terminateSession({
        khalaSessionId: "session.khala.termination.swapped",
        sessionId: started.sessionId,
      }))).rejects.toMatchObject({ code: "process_session_mismatch" })

      const terminated = await Effect.runPromise(service.terminateSession({
        khalaSessionId,
        sessionId: started.sessionId,
      }))
      expect(terminated).toMatchObject({
        cancelled: true,
        exitObserved: true,
        khalaSessionId,
        sessionId: started.sessionId,
        termination: "hard_kill",
      })

      const replay = await Effect.runPromise(service.terminateSession({
        khalaSessionId,
        sessionId: started.sessionId,
      }))
      expect(replay).toMatchObject({
        exitObserved: true,
        sessionId: started.sessionId,
        termination: "already_exited",
      })
    } finally {
      if (processSessionId !== undefined) {
        await Effect.runPromise(service.terminateSession({
          khalaSessionId,
          sessionId: processSessionId,
        })).catch(() => undefined)
      }
      await rm(workspace, { force: true, recursive: true })
    }
  })
})
