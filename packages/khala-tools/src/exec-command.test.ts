import { mkdir, mkdtemp } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createExecCommandTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaPermissionRequest,
  type KhalaPermissionService,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-exec-command-tool-"))
}

async function runExec(
  workspace: string,
  args: Readonly<Record<string, unknown>>,
  permission?: KhalaPermissionService,
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createExecCommandTool()]),
      { arguments: args, id: "call_1", name: "exec_command", sessionId: "s1" },
      makeKhalaToolServices({
        ...(permission === undefined ? {} : { permission }),
        workingDirectory: workspace,
      }),
    ),
  )
}

describe("exec_command tool", () => {
  test("runs successful commands through the process service", async () => {
    const workspace = await makeWorkspace()

    const result = await runExec(workspace, { cmd: "printf hello" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("hello")
    expect(result.ui).toMatchObject({ exitCode: 0, kind: "terminal_exec", timedOut: false })
  })

  test("returns failed status and stderr for non-zero exit codes", async () => {
    const workspace = await makeWorkspace()

    const result = await runExec(workspace, { cmd: "printf problem >&2; exit 7" })

    expect(result.status).toBe("failed")
    expect(result.modelOutput.text).toContain("problem")
    expect(result.ui).toMatchObject({ exitCode: 7, stderrBytes: 7 })
  })

  test("times out long-running commands", async () => {
    const workspace = await makeWorkspace()

    const result = await runExec(workspace, { cmd: "sleep 1", timeout_ms: 20 })

    expect(result.status).toBe("failed")
    expect(result.ui).toMatchObject({ timedOut: true })
  })

  test("supports cancellation deadlines", async () => {
    const workspace = await makeWorkspace()

    const result = await runExec(workspace, { cancel_after_ms: 20, cmd: "sleep 1", timeout_ms: 1000 })

    expect(result.status).toBe("failed")
    expect(result.ui).toMatchObject({ cancelled: true })
  })

  test("streams stdout and stderr chunks for terminal rendering", async () => {
    const workspace = await makeWorkspace()

    const result = await runExec(workspace, { cmd: "printf out; printf err >&2" })

    expect(result.status).toBe("ok")
    expect(result.ui).toMatchObject({
      events: expect.arrayContaining([
        expect.objectContaining({ kind: "stdout_chunk" }),
        expect.objectContaining({ kind: "stderr_chunk" }),
      ]),
    })
  })

  test("truncates model output and spills oversized output to a private artifact", async () => {
    const workspace = await makeWorkspace()

    const result = await runExec(workspace, {
      cmd: "i=0; while [ $i -lt 80 ]; do printf \"line-$i-abcdefghijklmnopqrstuvwxyz\\n\"; i=$((i+1)); done",
      max_output_tokens: 2,
    })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("[exec output truncated; see private artifact]")
    expect(result.artifacts).toHaveLength(1)
    expect(result.privateDataRefs).toHaveLength(1)
  })

  test("scopes workdir to workspace-relative directories", async () => {
    const workspace = await makeWorkspace()
    await mkdir(join(workspace, "sub"))

    const result = await runExec(workspace, { cmd: "pwd", workdir: "sub" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain(join(workspace, "sub"))
    expect(result.ui).toMatchObject({ cwd: "sub" })
  })

  test("requires approval for external working directories", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-exec-outside-"))

    const result = await runExec(workspace, { cmd: "pwd", workdir: outside }, denyAllKhalaPermissionService)

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("exec_external_cwd_denied")
  })

  test("marks destructive command approval material without executing when denied", async () => {
    const workspace = await makeWorkspace()
    const requests: KhalaPermissionRequest[] = []
    const permission: KhalaPermissionService = {
      decide: request => Effect.sync(() => {
        requests.push(request)
        return "deny" as const
      }),
    }

    const result = await runExec(workspace, { cmd: "rm -rf build" }, permission)

    expect(result.status).toBe("denied")
    expect(requests[0]).toMatchObject({
      action: "shell",
      resources: expect.arrayContaining(["risk:destructive"]),
    })
  })

  test("does not claim sandbox enforcement or accept public owner-full escalation fields", async () => {
    const workspace = await makeWorkspace()

    const result = await runExec(workspace, { cmd: "printf ok", owner_full_access: true })

    expect(result.status).toBe("ok")
    expect(result.ui).toMatchObject({
      sandbox: {
        enforced: false,
        kind: "none",
      },
    })
    expect(JSON.stringify(result.ui)).not.toContain("owner_full_access")
  })
})
