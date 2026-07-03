import { decodeAgentDefinition } from "@openagentsinc/agent-runtime-schema"
import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  AGENT_HARNESS_ADAPTER_SCHEMA,
  assignmentCarriesCodexHarnessTask,
  codexAgentHarnessAdapter,
} from "../src/agent-harness-adapter"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { CODEX_AGENT_CAPABILITY_REF, CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import {
  CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  executeCodexAgentAssignment,
  type CodexAgentRunner,
} from "../src/codex-agent-executor"
import { ensurePylonLocalState } from "../src/state"

const now = new Date("2026-07-03T00:00:00.000Z")

const definition = decodeAgentDefinition({
  schema: "openagents.agent_definition.v1",
  id: "agent_definition.pylon.codex_harness_test",
  ownerRef: "agent:agent_user_owner",
  name: "Pylon Codex Harness Adapter",
  slug: "pylon-codex-harness-adapter",
  goal: "Run a bounded Codex fixture through the Pylon codex_agent_task lane.",
  harness: {
    kind: "codex",
    modelHint: "openagents/pylon-codex",
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
      triggerRef: "trigger.public.pylon.codex_harness.manual",
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
  sourceRefs: ["issue.public.github.OpenAgentsInc.openagents.8190"],
  createdAt: now.toISOString(),
  updatedAt: now.toISOString(),
})

const readyProbe = {
  env: { CODEX_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
  codexCliLoginPresent: false,
}

const fixingRunner: CodexAgentRunner = async (input) => {
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

describe("agent_harness_adapter.v1 Codex adapter", () => {
  test("starts a definition-backed Codex session on the existing codex_agent_task lane", async () => {
    const started = await codexAgentHarnessAdapter.start({
      assignmentRef: "assignment.public.agent_harness.codex_fixture",
      definition,
      leaseRef: "lease.public.agent_harness.codex_fixture",
      now,
      triggerPayload: {
        fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
      },
      triggerRef: "trigger.public.pylon.codex_harness.manual",
    })

    expect(started.schema).toBe(AGENT_HARNESS_ADAPTER_SCHEMA)
    expect(started.status).toBe("started")
    if (started.status !== "started") return

    expect(started.sessionRef).toStartWith("session.agent_harness.codex.")
    expect(started.assignment.capabilityRefs).toContain(CODEX_AGENT_CAPABILITY_REF)
    expect(started.assignment.assignmentRef).toBe("assignment.public.agent_harness.codex_fixture")
    expect(started.assignment.leaseRef).toBe("lease.public.agent_harness.codex_fixture")
    expect(assignmentCarriesCodexHarnessTask(started.assignment)).toBe(true)
    expect(started.initialEvents.map((event) => event.tag)).toEqual([
      "run.input_accepted",
      "external_agent.started",
    ])

    await withState(async (state) => {
      const closeout = await executeCodexAgentAssignment(state, started.assignment, now, {
        codexAgentProbe: readyProbe,
        codexAgentRunner: fixingRunner,
      })

      expect(closeout?.status).toBe("accepted")
      expect(closeout?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.fixture_repair_passed",
      )

      const progress = codexAgentHarnessAdapter.normalizeEvent({
        event: {
          schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
          event: "assignment_run.runtime_progress",
          observedAt: now.toISOString(),
          assignmentRef: started.assignment.assignmentRef,
          leaseRef: started.assignment.leaseRef,
          phase: "running",
          elapsedMs: 1000,
        },
        sequence: 2,
        sessionRef: started.sessionRef,
      })

      expect(progress.tag).toBe("external_agent.event")
      expect(progress.runId).toBe(started.sessionRef)
      expect(progress.refs).toContain(started.assignment.assignmentRef)

      const terminal = codexAgentHarnessAdapter.reportTerminalState({
        closeout: closeout!,
        generatedAt: now.toISOString(),
        sequence: 3,
        sessionRef: started.sessionRef,
      })

      expect(terminal.schema).toBe(AGENT_HARNESS_ADAPTER_SCHEMA)
      expect(terminal.state).toBe("completed")
      expect(terminal.event.tag).toBe("external_agent.completed")
      expect(terminal.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.fixture_repair_passed",
      )
    })
  })

  test("refuses non-own-pylon definitions instead of inventing a lane", async () => {
    const refused = await codexAgentHarnessAdapter.start({
      definition: decodeAgentDefinition({ ...definition, lane: "cloud_workroom" }),
      now,
      triggerPayload: {},
    })

    expect(refused).toEqual({
      schema: AGENT_HARNESS_ADAPTER_SCHEMA,
      status: "refused",
      adapterKind: "codex",
      blockerRefs: ["blocker.agent_harness_adapter.own_pylon_required"],
      reasonRef: "reason.agent_harness_adapter.codex_start_refused",
    })
  })
})
