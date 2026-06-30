import { describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"
import {
  CODEX_AGENT_SDK_PACKAGE,
  CODEX_AGENT_CAPABILITY_REF,
} from "./codex-agent.js"
import {
  ciHarness,
  codexAgentSmokeLease,
} from "./codex-agent-task-smoke.js"
import { sendHeartbeat } from "./presence.js"
import { runNoSpendAssignment } from "./assignment.js"
import { ensurePylonLocalState } from "./state.js"
import type { CodexAgentRunner } from "./codex-agent-executor.js"

const waitFor = async (predicate: () => boolean, timeoutMs = 1_000) => {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (predicate()) return
    await new Promise(resolve => setTimeout(resolve, 10))
  }
  expect(predicate()).toBe(true)
}

const slowRunner: CodexAgentRunner = async (input) => {
  await new Promise(resolve => setTimeout(resolve, 80))
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return {
    commandCount: 0,
    editedFileCount: 1,
    outcome: "completed",
    sessionRef: null,
    turnCount: 1,
  }
}

describe("assignment runtime progress", () => {
  test("publishes server-visible running progress while a silent Codex worker is active", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-runtime-progress-"))
    const lease = codexAgentSmokeLease({
      assignmentRef: "assignment.public.codex_agent_task.runtime_progress",
      leaseRef: "lease.public.codex_agent_task.runtime_progress",
    })
    const harness = ciHarness(lease)
    try {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Runtime Progress Test"]),
        { PYLON_HOME: home },
        "darwin",
      )
      const state = await ensurePylonLocalState(summary)
      await writeFile(
        state.paths.runtimeState,
        `${JSON.stringify({
          blockerRefs: [],
          capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
          displayName: "Runtime Progress Test",
          lifecycle: "assignment-ready",
          resourceMode: "background_20",
          updatedAt: new Date().toISOString(),
        })}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: harness.baseUrl })

      const run = await runNoSpendAssignment(summary, {
        baseUrl: harness.baseUrl,
        codexAgentProbe: {
          codexCliLoginPresent: false,
          env: { CODEX_API_KEY: "ci-runtime-progress-key" },
          importer: async (specifier: string) => {
            if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
            return {}
          },
          platform: "darwin",
        },
        codexAgentRunner: slowRunner,
        codexAuthValidityProbe: async () => ({ valid: true }),
        runtimeProgressIntervalMs: 20,
      })

      expect(run.ok).toBe(true)
      await waitFor(() =>
        harness.retained.some(line =>
          line.includes("/progress") &&
          line.includes('"status":"running"') &&
          line.includes('"phase":"runtime_active"')
        ),
      )
    } finally {
      harness.stop()
      await rm(home, { recursive: true, force: true })
    }
  })
})
