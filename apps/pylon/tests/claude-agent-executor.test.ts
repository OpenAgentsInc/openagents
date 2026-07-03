import { describe, expect, test } from "bun:test"
import { realpathSync } from "node:fs"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
  CLAUDE_AGENT_TASK_SCHEMA,
  claudeAgentTaskFrom,
  claudeUsageFrom,
  executeClaudeAgentAssignment,
  toolInputEscapesWorkspace,
  type ClaudeAgentCheckoutRunner,
  type ClaudeAgentRunner,
} from "../src/claude-agent-executor"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import type { ClaudeTurnReport } from "../src/claude-turn-reporter"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { ensurePylonLocalState, assertPublicProjectionSafe } from "../src/state"

const now = new Date("2026-06-10T22:00:00.000Z")

const readyProbe = {
  env: { ANTHROPIC_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
}

const notReadyProbe = {
  env: {},
  platform: "darwin",
  importer: async () => {
    throw new Error("Cannot find module")
  },
}

const claudeAgentCodingAssignment = {
  claudeAgent: {
    schema: CLAUDE_AGENT_TASK_SCHEMA,
    agentKind: "claude_agent_sdk",
    fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
    maxTurns: 8,
    timeoutSeconds: 120,
  },
}

const gitCheckoutCodingAssignment = {
  objective: {
    publicSummary: "Repair the public sum fixture.",
  },
  claudeAgent: {
    schema: CLAUDE_AGENT_TASK_SCHEMA,
    agentKind: "claude_agent_sdk",
    allowedToolKinds: ["edit", "file", "git", "shell", "test_runner"],
    maxTurns: 8,
    timeoutSeconds: 120,
  },
  workspace: {
    kind: "git_checkout",
    repository: {
      branch: "main",
      commitSha: "3333333333333333333333333333333333333333",
      fullName: "OpenAgentsInc/public-sum-fixture",
      provider: "github",
      visibility: "public",
    },
    verificationCommand: {
      args: ["bun", "test", "sum.test.ts"],
      commandRef: "command.public.autopilot_coder.bun_test_sum",
    },
  },
}

const lease = {
  assignmentRef: "assignment.public.claude_agent.test",
  leaseRef: "lease.public.claude_agent.test",
  codingAssignment: claudeAgentCodingAssignment,
}

const successfulClaudeTurnReporter = async (_report: ClaudeTurnReport) => {}

const fixingRunner: ClaudeAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return {
    outcome: "completed",
    turnCount: 3,
    editedFileCount: 1,
    commandCount: 1,
    sessionRef: null,
    usage: { inputTokens: 1200, cachedInputTokens: 0, outputTokens: 340 },
  }
}

const fixingRunnerWithoutUsage: ClaudeAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return {
    outcome: "completed",
    turnCount: 3,
    editedFileCount: 1,
    commandCount: 1,
    sessionRef: null,
    usage: null,
  }
}

const checkoutRunner: ClaudeAgentCheckoutRunner = async (workspace) => {
  await mkdir(workspace, { recursive: true })
  await writeFile(
    join(workspace, "package.json"),
    `${JSON.stringify({ private: true, scripts: { test: "bun test sum.test.ts" }, type: "module" }, null, 2)}\n`,
  )
  await writeFile(join(workspace, "sum.ts"), "export const sum = (left: number, right: number) => left - right\n")
  await writeFile(
    join(workspace, "sum.test.ts"),
    [
      'import { describe, expect, test } from "bun:test"',
      'import { sum } from "./sum"',
      "",
      'describe("sum checkout", () => {',
      '  test("adds two numbers", () => {',
      "    expect(sum(2, 3)).toBe(5)",
      "  })",
      "})",
      "",
    ].join("\n"),
  )
}

const idleRunner: ClaudeAgentRunner = async () => ({
  outcome: "completed",
  turnCount: 1,
  editedFileCount: 0,
  commandCount: 0,
  sessionRef: null,
  usage: null,
})

async function withState<T>(fn: (state: Awaited<ReturnType<typeof ensurePylonLocalState>>) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-claude-exec-test-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const state = await ensurePylonLocalState(summary)
    return await fn(state)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("claude agent task recognition", () => {
  test("decodes Claude SDK usage with snake_case or camelCase token fields", () => {
    expect(
      claudeUsageFrom({
        input_tokens: 1200,
        cache_read_input_tokens: 70,
        cache_creation_input_tokens: 30,
        output_tokens: 340,
      }),
    ).toEqual({ cachedInputTokens: 100, inputTokens: 1200, outputTokens: 340 })
    expect(
      claudeUsageFrom({
        inputTokens: 800,
        cacheReadInputTokens: 20,
        cacheCreationInputTokens: 10,
        outputTokens: 200,
      }),
    ).toEqual({ cachedInputTokens: 30, inputTokens: 800, outputTokens: 200 })
    expect(
      claudeUsageFrom({
        input_tokens: 900,
        inputTokens: 800,
        cachedInputTokens: 15,
        output_tokens: 250,
        outputTokens: 200,
      }),
    ).toEqual({ cachedInputTokens: 15, inputTokens: 900, outputTokens: 250 })
    expect(claudeUsageFrom({ input_tokens: 0, output_tokens: 0 })).toBeNull()
    expect(claudeUsageFrom(undefined)).toBeNull()
  })

  test("recognizes the typed work class and passes everything else through", async () => {
    expect(claudeAgentTaskFrom(claudeAgentCodingAssignment)).not.toBeNull()
    expect(claudeAgentTaskFrom({ kind: "job.public.tassadar_executor_trace", tassadar: {} })).toBeNull()
    expect(claudeAgentTaskFrom({ claudeAgent: { schema: "wrong", agentKind: "claude_agent_sdk", fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF } })).toBeNull()
    expect(claudeAgentTaskFrom({ claudeAgent: { schema: CLAUDE_AGENT_TASK_SCHEMA, agentKind: "claude_agent_sdk", fixtureRef: "fixture.unknown" } })).toBeNull()
    expect(claudeAgentTaskFrom(gitCheckoutCodingAssignment)?.workspace?.kind).toBe("git_checkout")
    expect(claudeAgentTaskFrom(undefined)).toBeNull()

    await withState(async (state) => {
      const passthrough = await executeClaudeAgentAssignment(
        state,
        { ...lease, codingAssignment: { kind: "something_else" } },
        now,
        { claudeAgentRunner: fixingRunner, claudeAgentProbe: readyProbe },
      )
      expect(passthrough).toBeNull()
    })
  })

  test("executes the fixture task, verifies with the real test command, accepts", async () => {
    await withState(async (state) => {
      const reports: ClaudeTurnReport[] = []
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: readyProbe,
        claudeTurnReporter: async (report) => {
          reports.push(report)
        },
      })
      expect(record).not.toBeNull()
      expect(record?.status).toBe("accepted")
      expect(record?.blockerRefs).toEqual([])
      expect(record?.resultRefs).toContain("result.public.pylon.claude_agent_task.fixture_repair_passed")
      expect(record?.resultRefs).toContain("result.public.pylon.claude_agent_task.token_usage_reported")
      expect(record?.summaryRefs).toContain("summary.public.pylon.claude_agent_task.token_usage_reported")
      expect(record?.artifactRefs[0]).toStartWith("artifact.pylon.claude_agent_task.patch.")
      expect(record?.testRefs[0]).toStartWith("command.pylon.claude_agent_task.verification.")
      expect(reports).toHaveLength(1)
      expect(reports[0]).toMatchObject({
        assignmentRef: lease.assignmentRef,
        leaseRef: lease.leaseRef,
        observedAt: now.toISOString(),
        pylonRef: state.identity.pylonRef,
        roleRef: "coder",
        turnIndex: 1,
        usage: { cachedInputTokens: 0, inputTokens: 1200, outputTokens: 340 },
      })
      expect(reports[0]?.runRef).toStartWith("run.pylon.claude_agent_task.")
      expect(reports[0]?.workspaceRef).toStartWith("workspace.pylon.claude_agent_task.")
      assertPublicProjectionSafe(record)
      const projected = JSON.stringify(record)
      expect(projected).not.toContain(state.paths.cache)
      expect(projected).not.toContain("bounded fixture workspace")
    })
  })

  test("keeps accepted work visible when token reporting fails, with a typed blocker", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: readyProbe,
        claudeTurnReporter: async () => {
          throw new Error("turn ingest down")
        },
      })
      expect(record?.status).toBe("accepted")
      expect(record?.blockerRefs).toContain("blocker.assignment.claude_agent_token_usage_report_failed")
      expect(record?.resultRefs).toContain("result.public.pylon.claude_agent_task.token_usage_report_failed")
      expect(record?.summaryRefs).toContain("summary.public.pylon.claude_agent_task.token_usage_report_failed")
      assertPublicProjectionSafe(record)
    })
  })

  test("surfaces missing or unconfigured Claude token reporting as public-safe closeout diagnostics", async () => {
    await withState(async (state) => {
      const missingUsageRecord = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunnerWithoutUsage,
        claudeAgentProbe: readyProbe,
      })
      expect(missingUsageRecord?.status).toBe("accepted")
      expect(missingUsageRecord?.blockerRefs).toContain("blocker.assignment.claude_agent_token_usage_missing")
      expect(missingUsageRecord?.resultRefs).toContain("result.public.pylon.claude_agent_task.token_usage_missing")
      assertPublicProjectionSafe(missingUsageRecord)

      const unconfiguredRecord = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: readyProbe,
      })
      expect(unconfiguredRecord?.status).toBe("accepted")
      expect(unconfiguredRecord?.blockerRefs).toContain("blocker.assignment.claude_agent_token_usage_reporter_unconfigured")
      expect(unconfiguredRecord?.resultRefs).toContain(
        "result.public.pylon.claude_agent_task.token_usage_reporter_unconfigured",
      )
      assertPublicProjectionSafe(unconfiguredRecord)
    })
  })

  test("executes a public git_checkout task and verifies with caller-supplied argv", async () => {
    await withState(async (state) => {
      let workspaceDir: string | null = null
      const observingRunner: ClaudeAgentRunner = async (input) => {
        workspaceDir = input.cwd
        expect(input.instructions).toContain("command.public.autopilot_coder.bun_test_sum")
        expect(input.instructions).toContain("Repair the public sum fixture.")
        expect(input.allowedTools).toEqual(["Edit", "Read", "Bash"])
        return fixingRunner(input)
      }
      const record = await executeClaudeAgentAssignment(
        state,
        {
          ...lease,
          codingAssignment: gitCheckoutCodingAssignment,
          leaseRef: "lease.public.claude_agent.git_checkout",
        },
        now,
        {
          checkoutRunner,
          claudeAgentRunner: observingRunner,
          claudeAgentProbe: readyProbe,
          claudeTurnReporter: successfulClaudeTurnReporter,
        },
      )

      expect(record?.status).toBe("accepted")
      expect(record?.blockerRefs).toEqual([])
      expect(record?.resultRefs).toContain("result.public.pylon.claude_agent_task.git_checkout_verified_passed")
      expect(record?.artifactRefs[0]).toStartWith("artifact.pylon.claude_agent_task.patch.")
      expect(record?.testRefs[0]).toStartWith("command.pylon.claude_agent_task.verification.")
      expect(record?.previewRefs[0]).toStartWith("workspace.pylon.claude_agent_task.")
      expect(workspaceDir).not.toBeNull()
      const fixed = await readFile(join(workspaceDir as unknown as string, "sum.ts"), "utf8")
      expect(fixed).toContain("left + right")
      assertPublicProjectionSafe(record)
      const projected = JSON.stringify(record)
      expect(projected).not.toContain(state.paths.cache)
      expect(projected).not.toContain("OpenAgentsInc/public-sum-fixture")
      expect(projected).not.toContain("Repair the public sum fixture.")
    })
  })

  test("rejects with claude_agent_test_failed when the agent does not fix the fixture", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: idleRunner,
        claudeAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual([
        "blocker.assignment.claude_agent_test_failed",
        "blocker.assignment.claude_agent_token_usage_missing",
      ])
      expect(record?.resultRefs).toContain("result.public.pylon.claude_agent_task.token_usage_missing")
      assertPublicProjectionSafe(record)
    })
  })

  test("refuses with typed blockers when the lane is not ready", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: notReadyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toContain("blocker.assignment.claude_agent_unavailable")
      expect(record?.blockerRefs).toContain("blocker.claude_agent.sdk_missing")
      assertPublicProjectionSafe(record)
    })
  })

  test("maps escape, budget, refusal, and thrown-runner outcomes to typed blockers", async () => {
    await withState(async (state) => {
      const outcomes = [
        { outcome: "workspace_escape_blocked", blocker: "blocker.assignment.claude_agent_workspace_escape_blocked" },
        { outcome: "budget_exceeded", blocker: "blocker.assignment.claude_agent_budget_exceeded" },
        { outcome: "refused", blocker: "blocker.assignment.claude_agent_execution_refused" },
      ] as const
      for (const { outcome, blocker } of outcomes) {
        const runner: ClaudeAgentRunner = async () => ({
          outcome,
          turnCount: 2,
          editedFileCount: 0,
          commandCount: 0,
          sessionRef: null,
          usage: { inputTokens: 10, cachedInputTokens: 0, outputTokens: 5 },
        })
        const record = await executeClaudeAgentAssignment(state, lease, now, {
          claudeAgentRunner: runner,
          claudeAgentProbe: readyProbe,
          claudeTurnReporter: successfulClaudeTurnReporter,
        })
        expect(record?.status).toBe("rejected")
        expect(record?.blockerRefs).toEqual([blocker])
        assertPublicProjectionSafe(record)
      }

      const throwingRunner: ClaudeAgentRunner = async () => {
        throw new Error("sdk exploded")
      }
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: throwingRunner,
        claudeAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual(["blocker.assignment.claude_agent_execution_refused"])
    })
  })

  test("redaction: unsafe-shaped digests are rejected before any POST", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: readyProbe,
        claudeTurnReporter: successfulClaudeTurnReporter,
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
      const observingRunner: ClaudeAgentRunner = async (input) => {
        workspaceDir = input.cwd
        return fixingRunner(input)
      }
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: observingRunner,
        claudeAgentProbe: readyProbe,
        claudeTurnReporter: successfulClaudeTurnReporter,
      })
      expect(record?.status).toBe("accepted")
      expect(workspaceDir).not.toBeNull()
      const fixed = await readFile(join(workspaceDir as unknown as string, "sum.ts"), "utf8")
      expect(fixed).toContain("left + right")
    })
  })

  test("the runner receives the selected Claude account env", async () => {
    await withState(async (state) => {
      let seenClaudeConfigDir: string | undefined
      const account = {
        provider: "claude_agent" as const,
        selector: "direct_home" as const,
        accountRef: null,
        accountRefHash: "account.pylon.claude_agent.test",
        home: "/tmp/pylon-claude-account",
      }
      const observingRunner: ClaudeAgentRunner = async (input) => {
        seenClaudeConfigDir = input.env?.CLAUDE_CONFIG_DIR
        expect(input.account).toBe(account)
        return fixingRunner(input)
      }
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        account,
        claudeAgentRunner: observingRunner,
        claudeAgentProbe: readyProbe,
        claudeTurnReporter: successfulClaudeTurnReporter,
      })
      expect(record?.status).toBe("accepted")
      expect(seenClaudeConfigDir).toBe("/tmp/pylon-claude-account")
    })
  })
})

describe("workspace boundary checks", () => {
  const workspace = "/var/pylon-test/workspace"

  test("path fields outside the workspace escape", () => {
    expect(toolInputEscapesWorkspace("Edit", { file_path: "/etc/passwd" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Edit", { file_path: `${workspace}/sum.ts` }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Read", { path: "../outside.ts" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Read", { path: "sum.ts" }, workspace)).toBe(false)
  })

  test("bash commands with traversal or foreign absolute paths escape", () => {
    expect(toolInputEscapesWorkspace("Bash", { command: "cat ../secrets" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "cat ../../etc/hosts" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "cd .." }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "cat /etc/passwd" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "bun test sum.test.ts" }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: `cat ${workspace}/sum.ts` }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: "/usr/bin/env bun test" }, workspace)).toBe(false)
  })

  test("bash commands with benign dot-dot syntax do not escape", () => {
    expect(toolInputEscapesWorkspace("Bash", { command: "for n in {1..5}; do echo $n; done" }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: "git diff main..HEAD" }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: "printf 'working... done\\n'" }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: "cat src/../sum.ts" }, workspace)).toBe(false)
  })

  test("dash-flag glued traversal is denied while benign flag values pass", () => {
    const workspace = "/private/tmp/pylon-claude-guard-ws"
    expect(toolInputEscapesWorkspace("Bash", { command: "curl --output=../secret http://x" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "tar --directory=../../evil -xf a.tar" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "git --git-dir=../../.git log" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "cc -o../escape main.c" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "curl --output=out/result.bin http://x" }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: "bun test --filter=sum src/sum.test.ts" }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: `cp file ${workspace}/dest.txt` }, workspace)).toBe(false)
  })

  test("symlinked workspace roots accept realpath spellings without widening", async () => {
    // macOS /tmp is a symlink to /private/tmp; the SDK canonicalizes its cwd,
    // so tool paths arrive realpath'd. Found live on 2026-06-11: the first
    // real local-session run was falsely refused as a workspace escape.
    const symlinked = await mkdtemp(join(tmpdir(), "pylon-claude-boundary-"))
    try {
      const real = realpathSync(symlinked)
      // inside, spelled both ways: never an escape
      expect(toolInputEscapesWorkspace("Edit", { file_path: join(real, "sum.ts") }, symlinked)).toBe(false)
      expect(toolInputEscapesWorkspace("Edit", { file_path: join(symlinked, "sum.ts") }, symlinked)).toBe(false)
      expect(
        toolInputEscapesWorkspace("Bash", { command: `cat ${join(real, "sum.ts")}` }, symlinked),
      ).toBe(false)
      // outside stays outside under both spellings
      expect(toolInputEscapesWorkspace("Edit", { file_path: "/etc/passwd" }, symlinked)).toBe(true)
      expect(
        toolInputEscapesWorkspace("Edit", { file_path: join(real, "..", "outside.ts") }, symlinked),
      ).toBe(true)
    } finally {
      await rm(symlinked, { recursive: true, force: true })
    }
  })
})
