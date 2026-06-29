import { describe, expect, mock, test } from "bun:test"
import { existsSync } from "node:fs"
import { lstat, mkdir, mkdtemp, readdir, readFile, readlink, rm, writeFile } from "node:fs/promises"
import { join, relative } from "node:path"
import { tmpdir } from "node:os"
import {
  CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  CODEX_AGENT_TASK_SCHEMA,
  classifyCodexExecutionFailure,
  codexAgentTaskFrom,
  effectiveSandboxMode,
  executeCodexAgentAssignment,
  fileChangeEscapesWorkspace,
  prepareWorkspaceDependencies,
  runWithCodexSdk,
  type CodexAgentRunner,
} from "../src/codex-agent-executor"
import { CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import { hashPylonAccountRef, type ResolvedPylonAccountSelection } from "../src/account-registry"
import { loadQuotaRecord } from "../src/account-quota-ledger"
import type {
  CodexEventChunkReport,
  CodexTurnReport,
} from "../src/codex-turn-reporter"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { ensurePylonLocalState, assertPublicProjectionSafe } from "../src/state"
import {
  WorkspaceCheckoutError,
  workspaceLeaseRecordFor,
} from "../src/workspace-materializer"

const now = new Date("2026-06-11T22:00:00.000Z")

const readyProbe = {
  env: { CODEX_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
  codexCliLoginPresent: false,
}

const notReadyProbe = {
  env: {},
  platform: "darwin",
  importer: async () => {
    throw new Error("Cannot find module")
  },
  codexCliLoginPresent: false,
}

const codexAgentCodingAssignment = {
  codex: {
    schema: CODEX_AGENT_TASK_SCHEMA,
    agentKind: "codex_sdk",
    fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
    timeoutSeconds: 120,
  },
}

const lease = {
  assignmentRef: "assignment.public.codex_agent.test",
  leaseRef: "lease.public.codex_agent.test",
  codingAssignment: codexAgentCodingAssignment,
}

const fixingRunner: CodexAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return { outcome: "completed", turnCount: 1, editedFileCount: 1, commandCount: 1, sessionRef: null }
}

const idleRunner: CodexAgentRunner = async () => ({
  outcome: "completed",
  turnCount: 1,
  editedFileCount: 0,
  commandCount: 0,
  sessionRef: null,
})

const usageLimitMessage =
  "Codex turn failed: You've hit your usage limit. Visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at Jun 14th, 2026 9:58 PM."

async function withState<T>(fn: (state: Awaited<ReturnType<typeof ensurePylonLocalState>>) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-codex-exec-test-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const state = await ensurePylonLocalState(summary)
    return await fn(state)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("codex agent task recognition", () => {
  test("classifies concrete Codex execution failures without retaining raw text", () => {
    expect(
      classifyCodexExecutionFailure(
        new Error("Your access token could not be refreshed because your refresh token was revoked."),
      ),
    ).toMatchObject({
      blockerRef: "blocker.assignment.codex_agent_execution_credentials_revoked",
      reason: "credentials_revoked",
    })
    expect(classifyCodexExecutionFailure(new Error(usageLimitMessage))).toMatchObject({
      blockerRef: "blocker.assignment.codex_agent_execution_usage_limited",
      reason: "usage_limited",
    })
    expect(classifyCodexExecutionFailure(new Error("429 Too Many Requests"))).toMatchObject({
      blockerRef: "blocker.assignment.codex_agent_execution_rate_limited",
      reason: "rate_limited",
    })
    expect(classifyCodexExecutionFailure(new Error("WebSocket network failure"))).toMatchObject({
      blockerRef: "blocker.assignment.codex_agent_execution_network",
      reason: "network",
    })
  })

  test("recognizes the typed work class and passes everything else through", async () => {
    expect(codexAgentTaskFrom(codexAgentCodingAssignment)).not.toBeNull()
    expect(codexAgentTaskFrom({ kind: "job.public.tassadar_executor_trace", tassadar: {} })).toBeNull()
    expect(codexAgentTaskFrom({ claudeAgent: { schema: "openagents.pylon.claude_agent_task.v0.3" } })).toBeNull()
    expect(codexAgentTaskFrom({ codex: { schema: "wrong", agentKind: "codex_sdk", fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF } })).toBeNull()
    expect(codexAgentTaskFrom({ codex: { schema: CODEX_AGENT_TASK_SCHEMA, agentKind: "codex_sdk", fixtureRef: "fixture.unknown" } })).toBeNull()
    expect(codexAgentTaskFrom(undefined)).toBeNull()

    await withState(async (state) => {
      const passthrough = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: { kind: "something_else" } },
        now,
        { codexAgentRunner: fixingRunner, codexAgentProbe: readyProbe },
      )
      expect(passthrough).toBeNull()
    })
  })

  test("a claude_agent_task lease passes through this gate untouched", async () => {
    await withState(async (state) => {
      const passthrough = await executeCodexAgentAssignment(
        state,
        {
          ...lease,
          codingAssignment: {
            kind: "claude_agent_task",
            claudeAgent: {
              schema: "openagents.pylon.claude_agent_task.v0.3",
              agentKind: "claude_agent_sdk",
              fixtureRef: "fixture.public.pylon.claude_agent.sum_repair.v1",
            },
          },
        },
        now,
        { codexAgentRunner: fixingRunner, codexAgentProbe: readyProbe },
      )
      expect(passthrough).toBeNull()
    })
  })

  test("executes the fixture task, verifies with the real test command, accepts", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: fixingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record).not.toBeNull()
      expect(record?.status).toBe("accepted")
      expect(record?.blockerRefs).toEqual([])
      expect(record?.resultRefs).toContain("result.public.pylon.codex_agent_task.fixture_repair_passed")
      expect(record?.artifactRefs[0]).toStartWith("artifact.pylon.codex_agent_task.patch.")
      expect(record?.testRefs[0]).toStartWith("command.pylon.codex_agent_task.verification.")
      assertPublicProjectionSafe(record)
      const projected = JSON.stringify(record)
      expect(projected).not.toContain(state.paths.cache)
      expect(projected).not.toContain("bounded fixture workspace")
    })
  })

  test("rejects with codex_agent_test_failed when the agent does not fix the fixture", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: idleRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual(["blocker.assignment.codex_agent_test_failed"])
      assertPublicProjectionSafe(record)
    })
  })

  test("refuses with typed blockers when the lane is not ready", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: fixingRunner,
        codexAgentProbe: notReadyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toContain("blocker.assignment.codex_agent_unavailable")
      expect(record?.blockerRefs).toContain("blocker.codex_agent.sdk_missing")
      assertPublicProjectionSafe(record)
    })
  })

  test("maps escape, budget, refusal, and thrown-runner outcomes to typed blockers", async () => {
    await withState(async (state) => {
      const outcomes = [
        { outcome: "workspace_escape_blocked", blocker: "blocker.assignment.codex_agent_workspace_escape_blocked" },
        { outcome: "budget_exceeded", blocker: "blocker.assignment.codex_agent_budget_exceeded" },
        {
          outcome: "refused",
          blocker: "blocker.assignment.codex_agent_execution_refused",
          extraBlocker: "blocker.assignment.codex_agent_execution_other",
        },
      ] as const
      for (const { outcome, blocker, extraBlocker } of outcomes) {
        const runner: CodexAgentRunner = async () => ({
          outcome,
          turnCount: 1,
          editedFileCount: 0,
          commandCount: 0,
          sessionRef: null,
        })
        const record = await executeCodexAgentAssignment(state, lease, now, {
          codexAgentRunner: runner,
          codexAgentProbe: readyProbe,
        })
        expect(record?.status).toBe("rejected")
        expect(record?.blockerRefs).toContain(blocker)
        if (extraBlocker !== undefined) expect(record?.blockerRefs).toContain(extraBlocker)
        else expect(record?.blockerRefs).toEqual([blocker])
        assertPublicProjectionSafe(record)
      }

      const throwingRunner: CodexAgentRunner = async () => {
        throw new Error("sdk exploded")
      }
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: throwingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toContain("blocker.assignment.codex_agent_execution_refused")
      expect(record?.blockerRefs).toContain("blocker.assignment.codex_agent_execution_other")
    })
  })

  test("usage-limit refusals surface the reason and mark the account unavailable", async () => {
    await withState(async (state) => {
      const accountRef = "codex-usage-limited"
      const accountRefHash = hashPylonAccountRef("codex", accountRef)
      const account: ResolvedPylonAccountSelection = {
        provider: "codex",
        selector: "registry_ref",
        accountRef,
        accountRefHash,
        home: join(state.paths.home, "accounts", "codex", accountRef),
      }
      const refusingRunner: CodexAgentRunner = async () => ({
        outcome: "refused",
        turnCount: 1,
        editedFileCount: 0,
        commandCount: 0,
        sessionRef: null,
        errorMessage: usageLimitMessage,
      })

      const record = await executeCodexAgentAssignment(state, lease, now, {
        account,
        codexAgentRunner: refusingRunner,
        codexAgentProbe: readyProbe,
      })

      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toContain("blocker.assignment.codex_agent_execution_usage_limited")
      expect(record?.resultRefs).toContain("result.public.pylon.codex_agent_task.execution_usage_limited")
      expect(JSON.stringify(record)).not.toContain(usageLimitMessage)
      const quotaRecord = await loadQuotaRecord(state, accountRefHash)
      expect(quotaRecord).not.toBeNull()
      expect(quotaRecord?.provider).toBe("codex")
      expect(quotaRecord?.sourceDigestRef).toStartWith("diagnostic.pylon.codex_agent_task.execution_error.")
      assertPublicProjectionSafe(record)
    })
  })

  test("bounds a hung Codex runner with a typed budget-exceeded closeout", async () => {
    await withState(async (state) => {
      const hungLease = {
        ...lease,
        codingAssignment: {
          codex: {
            schema: CODEX_AGENT_TASK_SCHEMA,
            agentKind: "codex_sdk",
            fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
            timeoutSeconds: 0.001,
          },
        },
      }
      const neverSettles: CodexAgentRunner = async () => new Promise(() => {})

      const record = await executeCodexAgentAssignment(state, hungLease, now, {
        codexAgentRunner: neverSettles,
        codexAgentProbe: readyProbe,
      })

      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual([
        "blocker.assignment.codex_agent_budget_exceeded",
      ])
      expect(record?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.budget_exceeded",
      )
      assertPublicProjectionSafe(record)
    })
  })

  test("redaction: unsafe-shaped digests are rejected before any POST", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: fixingRunner,
        codexAgentProbe: readyProbe,
      })
      const tampered = { ...record, message: `${record?.message} raw prompt follows` }
      expect(() => assertPublicProjectionSafe(tampered)).toThrow()
      const keyTampered = { ...record, cachePath: "/leak" }
      expect(() => assertPublicProjectionSafe(keyTampered)).toThrow()
      const credentialTampered = { ...record, summaryRefs: ["bearer abc123"] }
      expect(() => assertPublicProjectionSafe(credentialTampered)).toThrow()
    })
  })

  test("the agent's edit is real: workspace file changed and bun test passes", async () => {
    await withState(async (state) => {
      let workspaceDir: string | null = null
      const observingRunner: CodexAgentRunner = async (input) => {
        workspaceDir = input.cwd
        return fixingRunner(input)
      }
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: observingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("accepted")
      expect(workspaceDir).not.toBeNull()
      const fixed = await readFile(join(workspaceDir as unknown as string, "sum.ts"), "utf8")
      expect(fixed).toContain("left + right")
    })
  })

  test("the runner receives the owner-local full-access mode", async () => {
    await withState(async (state) => {
      let seenMode: string | null = null
      let seenCodeHome: string | undefined
      let seenNetworkAccess: boolean | null = null
      const account = {
        provider: "codex" as const,
        selector: "direct_home" as const,
        accountRef: null,
        accountRefHash: "account.pylon.codex.test",
        home: "/tmp/pylon-codex-account",
      }
      const observingRunner: CodexAgentRunner = async (input) => {
        seenMode = input.sandboxMode
        seenCodeHome = input.env?.CODEX_HOME
        seenNetworkAccess = input.networkAccessEnabled
        expect(input.account).toBe(account)
        return fixingRunner(input)
      }
      await executeCodexAgentAssignment(state, lease, now, {
        account,
        codexAgentRunner: observingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(seenMode).toBe("danger-full-access")
      expect(seenNetworkAccess).toBe(true)
      expect(seenCodeHome).toBe("/tmp/pylon-codex-account")
    })
  })

  test("passes turn reports to the injected reporter with assignment context", async () => {
    await withState(async (state) => {
      const reports: Array<CodexTurnReport> = []
      const reportingRunner: CodexAgentRunner = async (input) => {
        await input.eventReporter?.({
          assignmentRef: input.assignmentRef ?? "missing-assignment",
          leaseRef: input.leaseRef ?? "missing-lease",
          pylonRef: input.pylonRef ?? "missing-pylon",
          ...(input.runRef === undefined ? {} : { runRef: input.runRef }),
          sessionRef: "session.pylon.codex_agent.test",
          ...(input.workspaceRef === undefined ? {} : { workspaceRef: input.workspaceRef }),
          turnIndex: 1,
          observedAt: now.toISOString(),
          usage: {
            cachedInputTokens: 2,
            inputTokens: 30,
            outputTokens: 4,
            reasoningOutputTokens: 6,
          },
          items: [
            {
              itemType: "agent_message",
              message: "Fixed the fixture.",
              ordinal: 1,
              status: "completed",
            },
          ],
        })
        return fixingRunner(input)
      }

      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentProbe: readyProbe,
        codexAgentRunner: reportingRunner,
        codexTurnReporter: async report => {
          reports.push(report)
        },
      })

      expect(record?.status).toBe("accepted")
      expect(reports).toHaveLength(1)
      expect(reports[0]).toMatchObject({
        assignmentRef: lease.assignmentRef,
        leaseRef: lease.leaseRef,
        pylonRef: state.identity.pylonRef,
        sessionRef: "session.pylon.codex_agent.test",
        turnIndex: 1,
        usage: {
          cachedInputTokens: 2,
          inputTokens: 30,
          outputTokens: 4,
          reasoningOutputTokens: 6,
        },
      })
      expect(reports[0]?.runRef).toStartWith("run.pylon.codex_agent_task.")
      expect(reports[0]?.workspaceRef).toStartWith("workspace.pylon.codex_agent_task.")
    })
  })

  test("SDK turn reporter failures do not fail the local Codex task", async () => {
    const reports: Array<unknown> = []
    mock.module(CODEX_AGENT_SDK_PACKAGE, () => ({
      Codex: class {
        startThread() {
          return {
            runStreamed: async () => ({
              events: (async function* () {
                yield { type: "thread.started", thread_id: "thread-codex-reporter-fail-soft" }
                yield { type: "turn.started" }
                yield {
                  type: "item.completed",
                  item: {
                    status: "completed",
                    text: "Completed the turn.",
                    type: "agent_message",
                  },
                }
                yield {
                  type: "item.completed",
                  item: {
                    aggregated_output: "raw shell output stays local",
                    exit_code: 0,
                    status: "completed",
                    type: "command_execution",
                  },
                }
                yield {
                  type: "turn.completed",
                  usage: {
                    cached_input_tokens: 2,
                    input_tokens: 50,
                    output_tokens: 7,
                    reasoning_output_tokens: 3,
                  },
                }
              })(),
            }),
          }
        }
      },
    }))

    const result = await runWithCodexSdk({
      assignmentRef: "assignment.public.codex_agent.reporter",
      cwd: "/tmp",
      eventReporter: async report => {
        reports.push(report)
        throw new Error("ingest unavailable")
      },
      instructions: "Run a mocked Codex turn.",
      leaseRef: "lease.public.codex_agent.reporter",
      networkAccessEnabled: true,
      pylonRef: "pylon.public.codex_agent.reporter",
      runRef: "run.public.codex_agent.reporter",
      sandboxMode: "danger-full-access",
      timeoutMs: 1_000,
      workspaceRef: "workspace.public.codex_agent.reporter",
    })

    expect(result).toMatchObject({
      commandCount: 1,
      editedFileCount: 0,
      outcome: "completed",
      turnCount: 1,
    })
    expect(result.sessionRef).toStartWith("session.pylon.codex_agent.")
    expect(reports).toHaveLength(1)
    expect(JSON.stringify(reports[0])).toContain("raw shell output stays local")
    expect(reports[0]).toMatchObject({
      assignmentRef: "assignment.public.codex_agent.reporter",
      leaseRef: "lease.public.codex_agent.reporter",
      pylonRef: "pylon.public.codex_agent.reporter",
      turnIndex: 1,
      usage: {
        cachedInputTokens: 2,
        inputTokens: 50,
        outputTokens: 7,
        reasoningOutputTokens: 3,
      },
    })
    expect((reports[0] as CodexTurnReport).rawEvents).toEqual([
      { type: "thread.started", thread_id: "thread-codex-reporter-fail-soft" },
      { type: "turn.started" },
      {
        type: "item.completed",
        item: {
          status: "completed",
          text: "Completed the turn.",
          type: "agent_message",
        },
      },
      {
        type: "item.completed",
        item: {
          aggregated_output: "raw shell output stays local",
          exit_code: 0,
          status: "completed",
          type: "command_execution",
        },
      },
      {
        type: "turn.completed",
        usage: {
          cached_input_tokens: 2,
          input_tokens: 50,
          output_tokens: 7,
          reasoning_output_tokens: 3,
        },
      },
    ])
  })

  test("streams raw event chunks during the SDK turn before final closeout", async () => {
    const chunks: Array<CodexEventChunkReport> = []
    const turns: Array<CodexTurnReport> = []
    mock.module(CODEX_AGENT_SDK_PACKAGE, () => ({
      Codex: class {
        startThread() {
          return {
            runStreamed: async () => ({
              events: (async function* () {
                yield { type: "thread.started", thread_id: "thread-codex-stream-chunks" }
                yield { type: "turn.started" }
                yield {
                  type: "item.completed",
                  item: {
                    status: "completed",
                    text: "Streaming item one.",
                    type: "agent_message",
                  },
                }
                yield {
                  type: "item.completed",
                  item: {
                    aggregated_output: "raw command output stored privately",
                    exit_code: 0,
                    status: "completed",
                    type: "command_execution",
                  },
                }
                yield {
                  type: "turn.completed",
                  usage: {
                    cached_input_tokens: 4,
                    input_tokens: 70,
                    output_tokens: 11,
                    reasoning_output_tokens: 5,
                  },
                }
              })(),
            }),
          }
        }
      },
    }))

    const result = await runWithCodexSdk({
      assignmentRef: "assignment.public.codex_agent.stream_chunks",
      cwd: "/tmp",
      eventChunkReporter: async chunk => {
        chunks.push(chunk)
      },
      eventReporter: async report => {
        turns.push(report)
      },
      instructions: "Run a mocked Codex turn.",
      leaseRef: "lease.public.codex_agent.stream_chunks",
      networkAccessEnabled: true,
      pylonRef: "pylon.public.codex_agent.stream_chunks",
      runRef: "run.public.codex_agent.stream_chunks",
      sandboxMode: "danger-full-access",
      timeoutMs: 1_000,
      workspaceRef: "workspace.public.codex_agent.stream_chunks",
    })

    expect(result.outcome).toBe("completed")
    expect(chunks.map(chunk => chunk.chunkIndex)).toEqual([1, 2, 3, 4])
    expect(chunks[0]?.rawEvents.map(event => event.type)).toEqual([
      "thread.started",
      "turn.started",
    ])
    expect(chunks[1]).toMatchObject({
      items: [{ itemType: "agent_message", message: "Streaming item one." }],
      turnIndex: 1,
    })
    expect(chunks[2]).toMatchObject({
      items: [{ itemType: "command_execution", exitCode: 0 }],
      turnIndex: 1,
    })
    expect(chunks[3]?.rawEvents).toEqual([
      {
        type: "turn.completed",
        usage: {
          cached_input_tokens: 4,
          input_tokens: 70,
          output_tokens: 11,
          reasoning_output_tokens: 5,
        },
      },
    ])
    expect(turns).toHaveLength(1)
    expect(turns[0]?.usage).toMatchObject({
      cachedInputTokens: 4,
      inputTokens: 70,
      outputTokens: 11,
      reasoningOutputTokens: 5,
    })
    expect(turns[0]?.rawEvents).toHaveLength(5)
  })
})

describe("sandbox mode resolution", () => {
  test("caller-owned Khala assignments use the Codex full-access SDK equivalent", () => {
    expect(effectiveSandboxMode(undefined, undefined)).toBe("danger-full-access")
    expect(effectiveSandboxMode("workspace-write", undefined)).toBe("danger-full-access")
    expect(effectiveSandboxMode("read-only", "workspace-write")).toBe("danger-full-access")
    expect(effectiveSandboxMode("workspace-write", "read-only")).toBe("danger-full-access")
  })
})

describe("post-hoc workspace boundary checks", () => {
  const workspace = "/var/pylon-test/workspace"

  test("file changes outside the workspace escape", () => {
    expect(fileChangeEscapesWorkspace("/etc/passwd", workspace)).toBe(true)
    expect(fileChangeEscapesWorkspace(`${workspace}/sum.ts`, workspace)).toBe(false)
    expect(fileChangeEscapesWorkspace("../outside.ts", workspace)).toBe(true)
    expect(fileChangeEscapesWorkspace("sum.ts", workspace)).toBe(false)
    expect(fileChangeEscapesWorkspace(`${workspace}-sibling/sum.ts`, workspace)).toBe(true)
  })
})

describe("codex git_checkout workspace (shared B2 contract)", () => {
  const checkoutAssignment = {
    kind: "codex_agent_task",
    objective: { publicSummary: "Repair the failing sum test." },
    codex: {
      schema: CODEX_AGENT_TASK_SCHEMA,
      agentKind: "codex_sdk",
      timeoutSeconds: 120,
    },
    workspace: {
      kind: "git_checkout",
      repository: {
        branch: "main",
        commitSha: "1745cd4b54b8a12a50922f80b5d345314c91d70d",
        fullName: "AtlantisPleb/openagents-b2-git-checkout-fixture-20260611144040",
        provider: "github",
        visibility: "public",
      },
      verificationCommand: {
        args: ["bun", "test", "sum.test.ts"],
        commandRef: "command.public.autopilot_coder.bun_test",
      },
    },
  }

  test("recognizes a workspace-bearing payload without a fixture ref", () => {
    const task = codexAgentTaskFrom(checkoutAssignment)
    expect(task).not.toBeNull()
    expect(task?.workspace?.repository.fullName).toBe(
      "AtlantisPleb/openagents-b2-git-checkout-fixture-20260611144040",
    )
    expect(task?.objectiveSummary).toBe("Repair the failing sum test.")
  })

  test("rejects an invalid workspace exactly like the claude gate (shared validator)", () => {
    const tampered = {
      ...checkoutAssignment,
      workspace: {
        ...checkoutAssignment.workspace,
        repository: { ...checkoutAssignment.workspace.repository, commitSha: "main" },
      },
    }
    expect(codexAgentTaskFrom(tampered)).toBeNull()
  })

  test("executes a checkout task end to end with an injected checkout runner", async () => {
    await withState(async (state) => {
      const checkoutRunner = async (workspace: string) => {
        const { mkdir } = await import("node:fs/promises")
        await mkdir(workspace, { recursive: true })
        await writeFile(
          join(workspace, "package.json"),
          `${JSON.stringify({ private: true, type: "module" })}\n`,
        )
        await writeFile(
          join(workspace, "sum.ts"),
          "export const sum = (left: number, right: number) => left - right\n",
        )
        await writeFile(
          join(workspace, "sum.test.ts"),
          [
            'import { describe, expect, test } from "bun:test"',
            'import { sum } from "./sum"',
            'describe("sum", () => { test("adds", () => { expect(sum(2, 3)).toBe(5) }) })',
            "",
          ].join("\n"),
        )
      }
      const record = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: checkoutAssignment },
        now,
        { checkoutRunner, codexAgentRunner: fixingRunner, codexAgentProbe: readyProbe },
      )
      expect(record?.status).toBe("accepted")
      expect(record?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.git_checkout_verified_passed",
      )
      assertPublicProjectionSafe(record)
      const projected = JSON.stringify(record)
      expect(projected).not.toContain(state.paths.cache)
      expect(projected).not.toContain("Repair the failing sum test.")
    })
  })

  test("records the opened PR refs in the closeout for a verified diff (#6439)", async () => {
    await withState(async (state) => {
      const checkoutRunner = async (workspace: string) => {
        const { mkdir } = await import("node:fs/promises")
        await mkdir(workspace, { recursive: true })
        await writeFile(
          join(workspace, "package.json"),
          `${JSON.stringify({ private: true, type: "module" })}\n`,
        )
        await writeFile(
          join(workspace, "sum.ts"),
          "export const sum = (left: number, right: number) => left - right\n",
        )
        await writeFile(
          join(workspace, "sum.test.ts"),
          [
            'import { describe, expect, test } from "bun:test"',
            'import { sum } from "./sum"',
            'describe("sum", () => { test("adds", () => { expect(sum(2, 3)).toBe(5) }) })',
            "",
          ].join("\n"),
        )
      }
      const seen: Array<{ assignmentRef: string; passed: boolean }> = []
      const record = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: checkoutAssignment },
        now,
        {
          checkoutRunner,
          codexAgentRunner: fixingRunner,
          codexAgentProbe: readyProbe,
          pullRequestPublisher: async (input) => {
            seen.push({ assignmentRef: input.assignmentRef, passed: input.verification.passed })
            return {
              state: "opened",
              prUrl: "https://github.com/OpenAgentsInc/openagents/pull/99001",
              prNumber: 99001,
              branch: "pylon/assignment-deadbeefdeadbeef",
              changedCount: 1,
              reused: false,
            }
          },
        },
      )
      expect(record?.status).toBe("accepted")
      expect(seen.length).toBe(1)
      expect(seen[0]?.passed).toBe(true)
      expect(record?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.pull_request_opened",
      )
      expect(record?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.pull_request_changed_files.1",
      )
      expect(record?.previewRefs).toContain(
        "https://github.com/OpenAgentsInc/openagents/pull/99001",
      )
      assertPublicProjectionSafe(record)
    })
  })

  test("releases a clean git checkout workspace after closeout (#6524)", async () => {
    await withState(async (state) => {
      const checkoutRunner = async (workspace: string) => {
        await mkdir(workspace, { recursive: true })
        await writeFile(
          join(workspace, "package.json"),
          `${JSON.stringify({ private: true, type: "module" })}\n`,
        )
        await writeFile(
          join(workspace, "sum.ts"),
          "export const sum = (left: number, right: number) => left + right\n",
        )
        await writeFile(
          join(workspace, "sum.test.ts"),
          [
            'import { describe, expect, test } from "bun:test"',
            'import { sum } from "./sum"',
            'describe("sum", () => { test("adds", () => { expect(sum(2, 3)).toBe(5) }) })',
            "",
          ].join("\n"),
        )
      }
      let workspaceRef: string | null = null
      const record = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: checkoutAssignment },
        now,
        {
          checkoutRunner,
          codexAgentProbe: readyProbe,
          codexAgentRunner: async (input) => {
            workspaceRef = input.workspaceRef ?? null
            return idleRunner(input)
          },
          pullRequestPublisher: async () => ({ state: "no_change" }),
        },
      )

      expect(record?.status).toBe("accepted")
      expect(record?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.workspace_cleaned_on_closeout",
      )
      expect(workspaceRef).not.toBeNull()
      const leaseRecord = await workspaceLeaseRecordFor({
        workspaceRef: workspaceRef as string,
        workspaceStateRoot: join(state.paths.cache, "workspace-leases"),
      })
      expect(leaseRecord?.state).toBe("cleaned")
      expect(leaseRecord?.cleanupReceiptRef).toStartWith("receipt.pylon.workspace_cleanup.")
      expect(existsSync(leaseRecord?.local.workingDirectory ?? "")).toBe(false)
      assertPublicProjectionSafe(record)
    })
  })

  test("does not open a PR for the public fixture lane (no workspace)", async () => {
    await withState(async (state) => {
      let publisherCalls = 0
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: fixingRunner,
        codexAgentProbe: readyProbe,
        pullRequestPublisher: async () => {
          publisherCalls += 1
          return { state: "no_change" }
        },
      })
      expect(record?.status).toBe("accepted")
      expect(publisherCalls).toBe(0)
      expect(
        (record?.resultRefs ?? []).some((ref) => ref.includes("pull_request")),
      ).toBe(false)
    })
  })

  test("prepares locked Bun checkout dependencies before running Codex", async () => {
    await withState(async (state) => {
      let checkoutRoot = ""
      const checkoutRunner = async (workspace: string) => {
        checkoutRoot = workspace
        await mkdir(workspace, { recursive: true })
        await writeFile(
          join(workspace, "package.json"),
          `${JSON.stringify({ private: true, type: "module" })}\n`,
        )
        await writeFile(join(workspace, "bun.lock"), "")
        await mkdir(join(workspace, "apps/openagents.com/workers/api"), { recursive: true })
        await writeFile(
          join(workspace, "apps/openagents.com/package.json"),
          `${JSON.stringify(
            {
              private: true,
              devDependencies: { vitest: "^4.1.8" },
              workspaces: ["workers/*"],
            },
            null,
            2,
          )}\n`,
        )
        await writeFile(
          join(workspace, "apps/openagents.com/workers/api/package.json"),
          `${JSON.stringify(
            {
              private: true,
              scripts: { test: "bun test sum.test.ts" },
              dependencies: { "@openagentsinc/atif": "workspace:*" },
            },
            null,
            2,
          )}\n`,
        )
        await writeFile(
          join(workspace, "sum.ts"),
          "export const sum = (left: number, right: number) => left - right\n",
        )
        await writeFile(
          join(workspace, "sum.test.ts"),
          [
            'import { describe, expect, test } from "bun:test"',
            'import { sum } from "./sum"',
            'describe("sum", () => { test("adds", () => { expect(sum(2, 3)).toBe(5) }) })',
            "",
          ].join("\n"),
        )
        await writeFile(
          join(workspace, "apps/openagents.com/workers/api/sum.test.ts"),
          [
            'import { describe, expect, test } from "bun:test"',
            'import { sum } from "../../../../sum"',
            'describe("nested sum", () => { test("adds", () => { expect(sum(2, 3)).toBe(5) }) })',
            "",
          ].join("\n"),
        )
      }
      const nestedVerifierAssignment = {
        ...checkoutAssignment,
        workspace: {
          ...checkoutAssignment.workspace,
          verificationCommand: {
            args: ["bun", "--cwd", "apps/openagents.com/workers/api", "test"],
            commandRef: "command.public.autopilot_coder.nested_bun_test",
          },
        },
      }
      let installerCalled = false
      const dependencyCommands: Array<string> = []
      let runnerSawPreparedWorkspace = false
      const dependencyInstaller = async (input: { args: string[]; cwd: string }) => {
        installerCalled = true
        dependencyCommands.push(`${relative(checkoutRoot, input.cwd) || "."}: ${input.args.join(" ")}`)
        if (input.args[0] === "bun") {
          await writeFile(join(input.cwd, "dependency-ready.txt"), "ready\n")
        }
        return { exitCode: 0, stderrBytes: 0, stdoutBytes: 12, timedOut: false }
      }
      const observingRunner: CodexAgentRunner = async (input) => {
        const rootReady =
          (await readFile(join(input.cwd, "dependency-ready.txt"), "utf8")) === "ready\n"
        const appReady =
          (await readFile(join(input.cwd, "apps/openagents.com/dependency-ready.txt"), "utf8")) ===
          "ready\n"
        runnerSawPreparedWorkspace = rootReady && appReady
        return fixingRunner(input)
      }

      const record = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: nestedVerifierAssignment },
        now,
        {
          checkoutRunner,
          codexAgentProbe: readyProbe,
          codexAgentRunner: observingRunner,
          dependencyInstaller,
        },
      )

      expect(record?.status).toBe("accepted")
      expect(installerCalled).toBe(true)
      expect(dependencyCommands).toEqual([
        ".: bun install --no-save --ignore-scripts",
        "apps/openagents.com: bun install --no-save --ignore-scripts",
        ".: git restore --source=HEAD --staged --worktree .",
      ])
      expect(runnerSawPreparedWorkspace).toBe(true)
      assertPublicProjectionSafe(record)
    })
  })

  test("dependency preparation failure returns a typed public-safe blocker", async () => {
    await withState(async (state) => {
      const checkoutRunner = async (workspace: string) => {
        await mkdir(workspace, { recursive: true })
        await writeFile(
          join(workspace, "package.json"),
          `${JSON.stringify({ private: true, type: "module" })}\n`,
        )
        await writeFile(join(workspace, "bun.lock"), "")
      }
      let runnerCalled = false
      const record = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: checkoutAssignment },
        now,
        {
          checkoutRunner,
          codexAgentProbe: readyProbe,
          codexAgentRunner: async () => {
            runnerCalled = true
            return idleRunner()
          },
          dependencyInstaller: async () => ({
            exitCode: 1,
            stderrBytes: 24,
            stdoutBytes: 0,
            timedOut: false,
          }),
        },
      )

      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual([
        "blocker.assignment.codex_agent_workspace_dependency_install_failed",
      ])
      expect(record?.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.workspace_dependency_install_failed",
      )
      expect(runnerCalled).toBe(false)
      assertPublicProjectionSafe(record)
      expect(JSON.stringify(record)).not.toContain(state.paths.cache)
    })
  })

  test("a failed checkout produces the typed checkout refusal arm", async () => {
    await withState(async (state) => {
      const failingCheckout = async () => {
        throw new WorkspaceCheckoutError("reason.workspace_checkout.branch_fetch_failed")
      }
      const record = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: checkoutAssignment },
        now,
        { checkoutRunner: failingCheckout, codexAgentRunner: fixingRunner, codexAgentProbe: readyProbe },
      )
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual([
        "blocker.assignment.codex_agent_workspace_checkout_failed",
        "reason.workspace_checkout.branch_fetch_failed",
      ])
      assertPublicProjectionSafe(record)
      expect(JSON.stringify(record)).not.toContain(state.paths.cache)
    })
  })
})

describe("shared node_modules cache across codex worktrees", () => {
  // Each git worktree gets a fresh checkout but never shares node_modules, so the
  // fleet used to run one full `bun install` per assignment and thrash concurrency.
  // The cache lets the first task install once and later matching tasks symlink.
  const LOCK_A = "lockfile-contents-A\n"
  const LOCK_B = "lockfile-contents-B-different\n"

  async function makeWorktree(root: string, lockContents: string): Promise<string> {
    const dir = await mkdtemp(join(root, "worktree-"))
    await writeFile(join(dir, "package.json"), `${JSON.stringify({ private: true, type: "module" })}\n`)
    await writeFile(join(dir, "bun.lock"), lockContents)
    return dir
  }

  // A real installer that materializes a node_modules tree (a marker file) so the
  // cache populate path has something to move, unlike the public-fixture mock.
  function countingInstaller() {
    const commands: string[] = []
    const installer = async (input: { args: string[]; cwd: string }) => {
      commands.push(input.args.join(" "))
      if (input.args[0] === "bun" && input.args[1] === "install") {
        await mkdir(join(input.cwd, "node_modules"), { recursive: true })
        await writeFile(join(input.cwd, "node_modules", ".installed"), "ok\n")
      }
      return { exitCode: 0, stderrBytes: 0, stdoutBytes: 8, timedOut: false }
    }
    const installCount = () => commands.filter((c) => c.startsWith("bun install")).length
    return { commands, installCount, installer }
  }

  test("(a) a matching shared cache skips bun install and symlinks node_modules in", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-nm-cache-a-"))
    try {
      const sharedCacheRoot = join(root, "shared")
      const first = countingInstaller()
      const wtA = await makeWorktree(root, LOCK_A)
      const prepA = await prepareWorkspaceDependencies({
        installer: first.installer,
        sharedCacheRoot,
        verificationArgs: ["bun", "test", "sum.test.ts"],
        workspace: wtA,
      })
      expect(prepA.ok).toBe(true)
      // First task installs once, then publishes the tree into the shared cache.
      expect(first.installCount()).toBe(1)
      const linkedA = await lstat(join(wtA, "node_modules"))
      expect(linkedA.isSymbolicLink()).toBe(true)

      // A second, independent worktree with the SAME lockfile must reuse the cache.
      const second = countingInstaller()
      const wtB = await makeWorktree(root, LOCK_A)
      const prepB = await prepareWorkspaceDependencies({
        installer: second.installer,
        sharedCacheRoot,
        verificationArgs: ["bun", "test", "sum.test.ts"],
        workspace: wtB,
      })
      expect(prepB.ok).toBe(true)
      // No install, and not even the post-install git restore should run.
      expect(second.commands).toEqual([])
      const linkedB = await lstat(join(wtB, "node_modules"))
      expect(linkedB.isSymbolicLink()).toBe(true)
      const targetB = await readlink(join(wtB, "node_modules"))
      expect(existsSync(join(targetB, ".installed"))).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("(b) a lockfile-hash mismatch falls back to a fresh install", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-nm-cache-b-"))
    try {
      const sharedCacheRoot = join(root, "shared")
      const first = countingInstaller()
      const wtA = await makeWorktree(root, LOCK_A)
      await prepareWorkspaceDependencies({
        installer: first.installer,
        sharedCacheRoot,
        verificationArgs: ["bun", "test", "sum.test.ts"],
        workspace: wtA,
      })
      expect(first.installCount()).toBe(1)

      // Different bun.lock -> different hash -> the LOCK_A cache must not be reused.
      const second = countingInstaller()
      const wtB = await makeWorktree(root, LOCK_B)
      await prepareWorkspaceDependencies({
        installer: second.installer,
        sharedCacheRoot,
        verificationArgs: ["bun", "test", "sum.test.ts"],
        workspace: wtB,
      })
      expect(second.installCount()).toBe(1)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("(c) concurrent materializations do not corrupt the shared cache", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-nm-cache-c-"))
    try {
      const sharedCacheRoot = join(root, "shared")
      const worktrees = await Promise.all(
        Array.from({ length: 6 }, () => makeWorktree(root, LOCK_A)),
      )
      const installers = worktrees.map(() => countingInstaller())
      const results = await Promise.all(
        worktrees.map((workspace, index) =>
          prepareWorkspaceDependencies({
            installer: installers[index].installer,
            sharedCacheRoot,
            verificationArgs: ["bun", "test", "sum.test.ts"],
            workspace,
          }),
        ),
      )

      // Every materialization succeeded and ended with a usable node_modules.
      for (const result of results) expect(result.ok).toBe(true)
      for (const workspace of worktrees) {
        const stat = await lstat(join(workspace, "node_modules"))
        const dir = stat.isSymbolicLink()
          ? await readlink(join(workspace, "node_modules"))
          : join(workspace, "node_modules")
        expect(existsSync(join(dir, ".installed"))).toBe(true)
      }

      // Exactly one valid, uncorrupted shared cache entry exists for this lockfile,
      // with no leftover populate locks or half-written temp directories. Layout is
      // <sharedCacheRoot>/<lockHash>/<install-dir-key>/node_modules.
      const lockHashDirs = await readdir(sharedCacheRoot)
      expect(lockHashDirs).toHaveLength(1)
      const installKeyDirs = await readdir(join(sharedCacheRoot, lockHashDirs[0]))
      expect(installKeyDirs).toHaveLength(1)
      const keyDir = join(sharedCacheRoot, lockHashDirs[0], installKeyDirs[0])
      const keyEntries = await readdir(keyDir)
      expect(keyEntries).toEqual(["node_modules"])
      expect(existsSync(join(keyDir, "node_modules", ".installed"))).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})
