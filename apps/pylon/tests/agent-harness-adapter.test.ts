import { decodeAgentDefinition } from "@openagentsinc/agent-runtime-schema"
import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  AGENT_HARNESS_ADAPTER_SCHEMA,
  assignmentCarriesClaudeHarnessTask,
  assignmentCarriesCodexHarnessTask,
  claudeCodeAgentHarnessAdapter,
  codexAgentHarnessAdapter,
} from "../src/agent-harness-adapter"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { CLAUDE_AGENT_CAPABILITY_REF, CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import {
  executeClaudeAgentAssignment,
  type ClaudeAgentRunner,
} from "../src/claude-agent-executor"
import { CODEX_AGENT_CAPABILITY_REF, CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import {
  executeCodexAgentAssignment,
  type CodexAgentRunner,
} from "../src/codex-agent-executor"
import { ensurePylonLocalState } from "../src/state"

const now = new Date("2026-07-03T00:00:00.000Z")

const definition = decodeAgentDefinition({
  schema: "openagents.agent_definition.v1",
  id: "agent_definition.pylon.harness_swap_test",
  ownerRef: "agent:agent_user_owner",
  name: "Pylon Harness Swap Adapter",
  slug: "pylon-harness-swap-adapter",
  goal: "Run the same bounded fixture definition through Pylon harness adapters.",
  harness: {
    kind: "khala",
    modelHint: "openagents/pylon-harness-swap",
  },
  toolset: {
    allow: ["tool.openagents.issue.read"],
    deny: ["tool.openagents.payment.*"],
    ask: ["tool.openagents.github.comment"],
    networkPolicy: "owner_scoped",
    secretPolicy: "owner_scoped_refs_only",
  },
  triggers: [
    {
      kind: "manual",
      triggerRef: "trigger.public.pylon.harness_swap.manual",
    },
  ],
  lane: "own_pylon",
  budget: {
    maxRunSeconds: 120,
    maxRunsPerDay: 3,
    maxCreditsPerDay: 0,
  },
  escalation: {
    channel: "operator",
    askPolicy: {
      policyRef: "policy.public.agent_definition.operator_required.v1",
      mode: "operator_required",
    },
  },
  sourceRefs: ["issue.public.github.OpenAgentsInc.openagents.8191"],
  createdAt: now.toISOString(),
  updatedAt: now.toISOString(),
})

const codexReadyProbe = {
  env: { CODEX_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
  codexCliLoginPresent: false,
}

const claudeReadyProbe = {
  env: { ANTHROPIC_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
}

const fixingCodexRunner: CodexAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return {
    outcome: "completed",
    turnCount: 1,
    editedFileCount: 1,
    commandCount: 1,
    sessionRef: "codex.session.fixture.8190",
  }
}

const fixingClaudeRunner: ClaudeAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return {
    outcome: "completed",
    turnCount: 2,
    editedFileCount: 1,
    commandCount: 1,
    sessionRef: "session.pylon.claude_agent.fixture.8191",
    usage: { cachedInputTokens: 0, inputTokens: 1200, outputTokens: 240 },
  }
}

async function withState<T>(fn: (state: Awaited<ReturnType<typeof ensurePylonLocalState>>) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-agent-harness-adapter-"))
  try {
    await mkdir(join(home, "codex-home"), { recursive: true })
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
      CODEX_HOME: join(home, "codex-home"),
      PYLON_HOME: home,
    }, "darwin")
    const state = await ensurePylonLocalState(summary)
    return await fn(state)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("agent_harness_adapter.v1 harness adapters", () => {
  test("runs one unchanged definition through Codex and Claude task lanes", async () => {
    const codexStarted = await codexAgentHarnessAdapter.start({
      assignmentRef: "assignment.public.agent_harness.codex_fixture",
      definition,
      leaseRef: "lease.public.agent_harness.codex_fixture",
      now,
      triggerPayload: {},
      triggerRef: "trigger.public.pylon.harness_swap.manual",
    })
    const claudeStarted = await claudeCodeAgentHarnessAdapter.start({
      assignmentRef: "assignment.public.agent_harness.claude_fixture",
      definition,
      leaseRef: "lease.public.agent_harness.claude_fixture",
      now,
      triggerPayload: {},
      triggerRef: "trigger.public.pylon.harness_swap.manual",
    })

    expect(codexStarted.schema).toBe(AGENT_HARNESS_ADAPTER_SCHEMA)
    expect(claudeStarted.schema).toBe(AGENT_HARNESS_ADAPTER_SCHEMA)
    expect(codexStarted.status).toBe("started")
    expect(claudeStarted.status).toBe("started")
    if (codexStarted.status !== "started" || claudeStarted.status !== "started") return

    expect(codexStarted.sessionRef).toStartWith("session.agent_harness.codex.")
    expect(claudeStarted.sessionRef).toStartWith("session.agent_harness.claude_code.")
    expect(codexStarted.assignment.capabilityRefs).toContain(CODEX_AGENT_CAPABILITY_REF)
    expect(claudeStarted.assignment.capabilityRefs).toContain(CLAUDE_AGENT_CAPABILITY_REF)
    expect(codexStarted.assignment.assignmentRef).toBe("assignment.public.agent_harness.codex_fixture")
    expect(codexStarted.assignment.leaseRef).toBe("lease.public.agent_harness.codex_fixture")
    expect(claudeStarted.assignment.assignmentRef).toBe("assignment.public.agent_harness.claude_fixture")
    expect(claudeStarted.assignment.leaseRef).toBe("lease.public.agent_harness.claude_fixture")
    expect(assignmentCarriesCodexHarnessTask(codexStarted.assignment)).toBe(true)
    expect(assignmentCarriesClaudeHarnessTask(claudeStarted.assignment)).toBe(true)
    expect(codexStarted.initialEvents.map((event) => event.tag)).toEqual([
      "run.input_accepted",
      "external_agent.started",
    ])
    expect(claudeStarted.initialEvents.map((event) => event.tag)).toEqual([
      "run.input_accepted",
      "external_agent.started",
    ])

    await withState(async (state) => {
      const codexCloseout = await executeCodexAgentAssignment(state, codexStarted.assignment, now, {
        codexAgentProbe: codexReadyProbe,
        codexAgentRunner: fixingCodexRunner,
      })
      const claudeCloseout = await executeClaudeAgentAssignment(state, claudeStarted.assignment, now, {
        claudeAgentProbe: claudeReadyProbe,
        claudeAgentRunner: fixingClaudeRunner,
        claudeTurnReporter: async () => {},
      })

      expect(codexCloseout?.status).toBe("accepted")
      expect(claudeCloseout?.status).toBe("accepted")
      expect(codexCloseout?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.fixture_repair_passed",
      )
      expect(claudeCloseout?.resultRefs).toContain(
        "result.public.pylon.claude_agent_task.fixture_repair_passed",
      )
      expect(claudeCloseout?.resultRefs).toContain(
        "result.public.pylon.claude_agent_task.token_usage_reported",
      )

      const progress = codexAgentHarnessAdapter.normalizeEvent({
        event: {
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
          event: "assignment_run.runtime_progress",
          observedAt: now.toISOString(),
          assignmentRef: codexStarted.assignment.assignmentRef,
          leaseRef: codexStarted.assignment.leaseRef,
          phase: "running",
          elapsedMs: 1000,
        },
        sequence: 2,
        sessionRef: codexStarted.sessionRef,
      })

      expect(progress.tag).toBe("external_agent.event")
      expect(progress.runId).toBe(codexStarted.sessionRef)
      expect(progress.refs).toContain(codexStarted.assignment.assignmentRef)

      const codexTerminal = codexAgentHarnessAdapter.reportTerminalState({
        closeout: codexCloseout!,
        generatedAt: now.toISOString(),
        sequence: 3,
        sessionRef: codexStarted.sessionRef,
      })
      const claudeTerminal = claudeCodeAgentHarnessAdapter.reportTerminalState({
        closeout: claudeCloseout!,
        generatedAt: now.toISOString(),
        sequence: 3,
        sessionRef: claudeStarted.sessionRef,
      })

      expect(codexTerminal.schema).toBe(AGENT_HARNESS_ADAPTER_SCHEMA)
      expect(claudeTerminal.schema).toBe(AGENT_HARNESS_ADAPTER_SCHEMA)
      expect(codexTerminal.state).toBe("completed")
      expect(claudeTerminal.state).toBe("completed")
      expect(codexTerminal.event.tag).toBe("external_agent.completed")
      expect(claudeTerminal.event.tag).toBe("external_agent.completed")
      expect(codexTerminal.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.fixture_repair_passed",
      )
      expect(claudeTerminal.resultRefs).toContain(
        "result.public.pylon.claude_agent_task.fixture_repair_passed",
      )
    })
  })

  test("refuses non-own-pylon definitions instead of inventing a lane", async () => {
    const codexRefused = await codexAgentHarnessAdapter.start({
      definition: decodeAgentDefinition({ ...definition, lane: "cloud_workroom" }),
      now,
      triggerPayload: {},
    })
    const claudeRefused = await claudeCodeAgentHarnessAdapter.start({
      definition: decodeAgentDefinition({ ...definition, lane: "cloud_workroom" }),
      now,
      triggerPayload: {},
    })

    expect(codexRefused).toEqual({
      schema: AGENT_HARNESS_ADAPTER_SCHEMA,
      status: "refused",
      adapterKind: "codex",
      blockerRefs: ["blocker.agent_harness_adapter.own_pylon_required"],
      reasonRef: "reason.agent_harness_adapter.codex_start_refused",
    })
    expect(claudeRefused).toEqual({
      schema: AGENT_HARNESS_ADAPTER_SCHEMA,
      status: "refused",
      adapterKind: "claude_code",
      blockerRefs: ["blocker.agent_harness_adapter.own_pylon_required"],
      reasonRef: "reason.agent_harness_adapter.claude_code_start_refused",
    })
  })
})
