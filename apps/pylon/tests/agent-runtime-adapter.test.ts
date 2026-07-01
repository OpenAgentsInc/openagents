import { describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { Effect, Stream } from "effect"

import {
  assertAgentRuntimePublicEventSafe,
  decodeAgentRuntimeEvent,
  type AgentRuntimeEvent,
} from "@openagentsinc/agent-runtime-schema"
import {
  agentRuntimeToolDeniedEvent,
  createClaudeCodeAgentRuntimeAdapter,
  createCodexAgentRuntimeAdapter,
  createHermesReservedAgentRuntimeAdapter,
  createTestFixtureAgentRuntimeAdapter,
  replayAgentRuntimeEventLog,
  type AgentRuntimeRunRequest,
} from "../src/agent-runtime-adapter"
import {
  AGENT_RUNNER_KINDS,
  AGENT_RUNNER_REGISTRY,
  agentRunnerForLease,
  agentRunnerResolutionForLease,
  agentRunnerServiceForLease,
  isAgentRunnerKind,
  normalizeAgentRunnerKind,
} from "../src/agent-runner-registry"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import {
  CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
  CLAUDE_AGENT_TASK_SCHEMA,
  type ClaudeAgentRunner,
} from "../src/claude-agent-executor"
import { CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import {
  CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  CODEX_AGENT_TASK_SCHEMA,
  type CodexAgentRunner,
} from "../src/codex-agent-executor"
import { ensurePylonLocalState } from "../src/state"

const now = new Date("2026-06-11T12:30:00.000Z")

const readyClaudeProbe = {
  env: { ANTHROPIC_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
}

const readyCodexProbe = {
  env: { CODEX_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
  codexCliLoginPresent: false,
}

const fixingClaudeRunner: ClaudeAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return { outcome: "completed", turnCount: 2, editedFileCount: 1, commandCount: 1, sessionRef: "session.public.claude.test" }
}

const fixingCodexRunner: CodexAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return { outcome: "completed", turnCount: 1, editedFileCount: 1, commandCount: 1, sessionRef: "session.public.codex.test" }
}

async function collect(stream: Stream.Stream<AgentRuntimeEvent>) {
  const chunk = await Effect.runPromise(Stream.runCollect(stream))
  return Array.from(chunk)
}

async function withRequest<T>(
  codingAssignment: Record<string, unknown>,
  fn: (request: AgentRuntimeRunRequest) => Promise<T>,
) {
  const home = await mkdtemp(join(tmpdir(), "pylon-rk2-test-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const state = await ensurePylonLocalState(summary)
    return await fn({
      runId: `run.public.rk2.${crypto.randomUUID()}`,
      lease: {
        schema: "openagents.pylon.assignment_lease.v0.3",
        assignmentRef: "assignment.public.rk2.test",
        leaseRef: "lease.public.rk2.test",
        goal: "goal.public.rk2.test",
        paymentMode: "no-spend",
        capabilityRefs: [],
        codingAssignment,
        expiresAt: new Date(now.getTime() + 60_000).toISOString(),
      },
      now,
      state,
    })
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("AgentRuntimeAdapter", () => {
  test("Claude and Codex runners are selected from the declarative registry", () => {
    expect(AGENT_RUNNER_KINDS).toEqual(["claude_agent", "codex"])
    expect(isAgentRunnerKind("claude_agent")).toBe(true)
    expect(isAgentRunnerKind("codex")).toBe(true)
    expect(isAgentRunnerKind("generic")).toBe(false)
    expect(normalizeAgentRunnerKind("claude")).toBe("claude_agent")

    expect(
      AGENT_RUNNER_REGISTRY.map((runner) => ({
        agentKind: runner.agentKind,
        adapterKind: runner.adapterKind,
        accountProvider: runner.accountProvider,
        kind: runner.kind,
        runtime: runner.runtime,
        serviceRef: runner.serviceRef,
      })),
    ).toEqual([
      {
        accountProvider: "claude_agent",
        agentKind: "claude_agent_sdk",
        adapterKind: "claude_code",
        kind: "claude_agent",
        runtime: {
          sdkPackage: CLAUDE_AGENT_SDK_PACKAGE,
          readinessProbe: "claude_agent_sdk_import",
          executionPolicy: {
            approvalPolicy: "pre_tool_use_deny",
            networkAccess: "runner_default",
            sandboxMode: "bounded_tool_allowlist",
          },
          turnReporter: {
            endpointPath: "/api/pylon/claude/turns",
            failSoft: true,
            kind: "pylon_claude_turn_reporter",
            usageTruth: "exact",
          },
          workspaceBoundary: {
            enforcement: "deny_before_tool_use",
            strategy: "pre_tool_use_hook",
          },
        },
        serviceRef: "claude",
      },
      {
        accountProvider: "codex",
        agentKind: "codex_sdk",
        adapterKind: "codex",
        kind: "codex",
        runtime: {
          sdkPackage: CODEX_AGENT_SDK_PACKAGE,
          readinessProbe: "codex_sdk_import_or_cli_login",
          executionPolicy: {
            approvalPolicy: "never",
            networkAccess: "enabled",
            sandboxMode: "danger-full-access",
          },
          turnReporter: {
            endpointPath: "/api/pylon/codex/turns",
            failSoft: true,
            kind: "pylon_codex_turn_reporter",
            usageTruth: "exact",
          },
          workspaceBoundary: {
            enforcement: "reject_closeout_on_escape",
            strategy: "post_hoc_workspace_validation",
          },
        },
        serviceRef: "codex",
      },
    ])

    const claudeLease = {
      schema: "openagents.pylon.assignment_lease.v0.3",
      assignmentRef: "assignment.public.registry.claude",
      leaseRef: "lease.public.registry.claude",
      goal: "goal.public.registry.claude",
      paymentMode: "no-spend",
      capabilityRefs: [],
      codingAssignment: {
        claudeAgent: {
          schema: CLAUDE_AGENT_TASK_SCHEMA,
          agentKind: "claude_agent_sdk",
          fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
        },
      },
      expiresAt: now.toISOString(),
    } as const
    const codexLease = {
      ...claudeLease,
      assignmentRef: "assignment.public.registry.codex",
      leaseRef: "lease.public.registry.codex",
      codingAssignment: {
        codex: {
          schema: CODEX_AGENT_TASK_SCHEMA,
          agentKind: "codex_sdk",
          fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
        },
      },
    }

    expect(agentRunnerForLease(claudeLease)?.adapterKind).toBe("claude_code")
    expect(agentRunnerServiceForLease(claudeLease)).toBe("claude")
    expect(agentRunnerForLease(codexLease)?.adapterKind).toBe("codex")
    expect(agentRunnerServiceForLease(codexLease)).toBe("codex")
  })

  test("registry resolution rejects ambiguous mixed-runner assignment payloads", () => {
    const lease = {
      schema: "openagents.pylon.assignment_lease.v0.3",
      assignmentRef: "assignment.public.registry.ambiguous",
      leaseRef: "lease.public.registry.ambiguous",
      goal: "goal.public.registry.ambiguous",
      paymentMode: "no-spend",
      capabilityRefs: [],
      codingAssignment: {
        claudeAgent: {
          schema: CLAUDE_AGENT_TASK_SCHEMA,
          agentKind: "claude_agent_sdk",
          fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
        },
        codex: {
          schema: CODEX_AGENT_TASK_SCHEMA,
          agentKind: "codex_sdk",
          fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
        },
      },
      expiresAt: now.toISOString(),
    } as const

    expect(agentRunnerResolutionForLease(lease)).toEqual({
      status: "ambiguous",
      runnerKinds: ["claude_agent", "codex"],
      blockerRef: "blocker.assignment.agent_runner_ambiguous",
    })
    expect(agentRunnerForLease(lease)).toBeNull()
    expect(agentRunnerServiceForLease(lease)).toBeNull()
  })

  test("fixture, Codex, and Claude wrappers emit the same kernel event contract", async () => {
    const fixtureAdapter = createTestFixtureAgentRuntimeAdapter()
    await withRequest({}, async (request) => {
      expect(await Effect.runPromise(fixtureAdapter.canRun(request))).toBe(true)
      const events = await collect(fixtureAdapter.start(request))
      expect(events.map((event) => event.tag)).toEqual([
        "run.started",
        "external_agent.started",
        "external_agent.artifact_recorded",
        "external_agent.completed",
        "run.completed",
      ])
      for (const event of events) {
        expect(decodeAgentRuntimeEvent(event)).toMatchObject({ runId: request.runId })
        expect(assertAgentRuntimePublicEventSafe(event)).toBe(event)
      }
    })

    const codexAdapter = createCodexAgentRuntimeAdapter({
      codexAgentProbe: readyCodexProbe,
      codexAgentRunner: fixingCodexRunner,
    })
    await withRequest({
      codex: {
        schema: CODEX_AGENT_TASK_SCHEMA,
        agentKind: "codex_sdk",
        fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
      },
    }, async (request) => {
      expect(await Effect.runPromise(codexAdapter.canRun(request))).toBe(true)
      const events = await collect(codexAdapter.start(request))
      expect(events.map((event) => event.tag)).toEqual([
        "run.started",
        "external_agent.started",
        "external_agent.artifact_recorded",
        "external_agent.completed",
        "run.completed",
      ])
      expect(replayAgentRuntimeEventLog(events)).toMatchObject({
        state: "completed",
        externalStatus: "completed",
        eventCount: 5,
      })
    })

    const claudeAdapter = createClaudeCodeAgentRuntimeAdapter({
      claudeAgentProbe: readyClaudeProbe,
      claudeAgentRunner: fixingClaudeRunner,
    })
    await withRequest({
      claudeAgent: {
        schema: CLAUDE_AGENT_TASK_SCHEMA,
        agentKind: "claude_agent_sdk",
        fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
      },
    }, async (request) => {
      expect(await Effect.runPromise(claudeAdapter.canRun(request))).toBe(true)
      const events = await collect(claudeAdapter.start(request))
      expect(events.map((event) => event.tag)).toEqual([
        "run.started",
        "external_agent.started",
        "external_agent.artifact_recorded",
        "external_agent.completed",
        "run.completed",
      ])
      expect(replayAgentRuntimeEventLog(events).artifactRefs[0]).toStartWith(
        "artifact.pylon.claude_agent_task.patch.",
      )
    })
  })

  test("replay rebuilds failed projection state from kernel events alone", async () => {
    const codexAdapter = createCodexAgentRuntimeAdapter({
      codexAgentProbe: readyCodexProbe,
      codexAgentRunner: async () => ({
        outcome: "workspace_escape_blocked",
        turnCount: 1,
        editedFileCount: 0,
        commandCount: 0,
        sessionRef: null,
      }),
    })
    await withRequest({
      codex: {
        schema: CODEX_AGENT_TASK_SCHEMA,
        agentKind: "codex_sdk",
        fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
      },
    }, async (request) => {
      const projection = replayAgentRuntimeEventLog(await collect(codexAdapter.start(request)))
      expect(projection.state).toBe("failed")
      expect(projection.externalStatus).toBe("failed")
      expect(projection.blockerRefs).toContain("blocker.assignment.codex_agent_workspace_escape_blocked")
    })
  })

  test("tool denial is a typed event and does not invoke a side effect", () => {
    let sideEffectCount = 0
    const denied = agentRuntimeToolDeniedEvent({
      runId: "run.public.tool_denied",
      generatedAt: now.toISOString(),
      invocationId: "tool.public.denied.1",
      sequence: 7,
      toolName: "write_file",
      toolRef: "tool.public.write_file",
      blockerRefs: ["blocker.assignment.tool_policy_denied"],
    })
    expect(denied.tag).toBe("tool.denied")
    expect(denied.toolInvocation?.status).toBe("denied")
    expect(sideEffectCount).toBe(0)
    expect(() => {
      sideEffectCount += 1
    }).not.toThrow()
    expect(sideEffectCount).toBe(1)
  })

  test("cancel emits run.cancelled and reserved Hermes stays unimplemented", async () => {
    const adapter = createTestFixtureAgentRuntimeAdapter()
    await withRequest({}, async (request) => {
      await Effect.runPromise(adapter.cancel(request.runId))
      const events = await collect(adapter.start(request))
      expect(events.map((event) => event.tag)).toEqual(["run.started", "run.cancelled"])
      expect(replayAgentRuntimeEventLog(events)).toMatchObject({
        state: "cancelled",
        externalStatus: "idle",
      })
    })

    const hermes = createHermesReservedAgentRuntimeAdapter()
    await withRequest({}, async (request) => {
      expect(await Effect.runPromise(hermes.canRun(request))).toBe(false)
      const events = await collect(hermes.start(request))
      expect(events.map((event) => event.tag)).toEqual([
        "run.started",
        "external_agent.failed",
        "run.failed",
      ])
      expect(replayAgentRuntimeEventLog(events).blockerRefs).toContain(
        "blocker.agent_runtime.hermes.unsupported_assignment",
      )
    })
  })
})
