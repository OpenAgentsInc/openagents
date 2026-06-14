import { describe, expect, test } from "bun:test"

import { laborPrompt, makeConfiguredLaborRuntime } from "../src/labor"
import type { LaborJobRequest, LaborWorkspace } from "../src/labor"

const request: LaborJobRequest = {
  jobType: "code_task",
  policyRef: "provider.compliant_usage_labor.v1",
  inputRefs: ["objective.public.pylon_work.abc123", "repo.public.github.OpenAgentsInc.openagents"],
  acceptanceCriteria: ["command.public.pylon.labor.bun_test"],
  expectedArtifacts: [],
  request: { content: "", params: [] },
} as unknown as LaborJobRequest

const workspace: LaborWorkspace = {
  absolutePath: "/tmp/labor-codex-test-ws",
  relativePath: "ws",
  root: "/tmp",
}

describe("labor codex execution", () => {
  test("laborPrompt injects resolved objective detail as an Objective section", () => {
    const prompt = laborPrompt(request, "Create sum.ts and sum.test.ts so bun test passes.")
    expect(prompt).toContain("Objective:")
    expect(prompt).toContain("Create sum.ts and sum.test.ts so bun test passes.")
    // The objective must precede the opaque refs so the agent reads it first.
    expect(prompt.indexOf("Objective:")).toBeLessThan(prompt.indexOf("Inputs:"))
  })

  test("laborPrompt without detail keeps the ref-only shape (no Objective section)", () => {
    const prompt = laborPrompt(request)
    expect(prompt).not.toContain("Objective:")
    expect(prompt).toContain("Inputs:")
  })

  test("codex labor command runs headless: skips git-repo-check, keeps the workspace sandbox", async () => {
    let captured: string[] = []
    const runtime = makeConfiguredLaborRuntime({
      which: (name) => (name === "codex" ? "/usr/bin/codex" : null),
      spawn: ((command: string[]) => {
        captured = command
        return {
          stdout: new Response("ok").body,
          stderr: new Response("").body,
          exited: Promise.resolve(0),
        }
      }) as unknown as typeof Bun.spawn,
    })
    await runtime.runLabor({
      agentKind: "codex",
      request,
      requestEventId: "event.test",
      workspace,
      objectiveDetail: "Create sum.ts and sum.test.ts so bun test passes.",
    })
    expect(captured.slice(0, 7)).toEqual([
      "/usr/bin/codex",
      "exec",
      "--skip-git-repo-check",
      "-s",
      "workspace-write",
      "-c",
      "sandbox_workspace_write.network_access=false",
    ])
    // It must NOT use the unsandboxed bypass for untrusted requester work.
    expect(captured).not.toContain("--dangerously-bypass-approvals-and-sandbox")
    // Network is denied so an untrusted job cannot clone/fetch into the sandbox.
    expect(captured).toContain("sandbox_workspace_write.network_access=false")
    // The injected objective reaches the agent prompt (last arg).
    expect(captured[captured.length - 1]).toContain("Create sum.ts and sum.test.ts so bun test passes.")
  })
})
