import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  CODEX_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF,
  CODEX_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF,
  createCodexHumanOutputParser,
  rejectCodexLocalDangerForPublicPath,
  runCodexComposerStream,
  sandboxModeForCodexComposerExecutionMode,
  summarizeCodexThreadEvent,
} from "../src/codex-composer"
import { CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { collectPylonAccountsUsage, parsePylonAccountsUsageArgs } from "../src/account-usage"

async function* fakeCodexEvents() {
  yield { type: "thread.started", thread_id: "thread.test.codex" }
  yield { type: "turn.started" }
  yield {
    type: "event_msg",
    payload: {
      type: "token_count",
      info: {
        last_token_usage: {
          output_tokens: 11,
          reasoning_output_tokens: 7,
        },
      },
    },
  }
  yield {
    type: "response_item",
    payload: {
      type: "reasoning",
      summary: [{ text: "checking the patch path" }],
    },
  }
  yield {
    type: "item.completed",
    item: {
      id: "cmd-1",
      type: "command_execution",
      command: "bun test",
      aggregated_output: "ok",
      exit_code: 0,
      status: "completed",
    },
  }
  yield {
    type: "item.completed",
    item: {
      id: "patch-1",
      type: "file_change",
      status: "completed",
      changes: [
        { kind: "update", path: "src/index.ts" },
        { kind: "add", path: "tests/index.test.ts" },
      ],
    },
  }
  yield {
    type: "item.completed",
    item: {
      id: "msg-1",
      type: "agent_message",
      text: "Patched the composer.",
    },
  }
  yield {
    type: "turn.completed",
    usage: {
      input_tokens: 10,
      cached_input_tokens: 0,
      output_tokens: 5,
      reasoning_output_tokens: 2,
    },
    rate_limits: {
      primary: {
        used_percent: 9,
        window_minutes: 10080,
        reset_at: 1_780_000_000,
      },
    },
  }
}

describe("Codex composer SDK stream", () => {
  test("runs the TypeScript SDK in the selected cwd and surfaces typed events", async () => {
    let threadOptions: Record<string, unknown> | null = null
    let clientOptions: { env?: Record<string, string | undefined>; config?: Record<string, unknown> } | null = null
    let promptSeen = ""
    let signalSeen = false
    const eventSummaries: string[] = []
    const threadRefs: string[] = []
    const textUpdates: string[] = []
    const usageUpdates: number[] = []
    const importer = async (specifier: string) => {
      if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        Codex: class {
          constructor(options?: { env?: Record<string, string | undefined>; config?: Record<string, unknown> }) {
            clientOptions = options ?? null
          }
          startThread(options: Record<string, unknown>) {
            threadOptions = options
            return {
              runStreamed: async (prompt: string, turnOptions: Record<string, unknown>) => {
                promptSeen = prompt
                signalSeen = turnOptions.signal instanceof AbortSignal
                return { events: fakeCodexEvents() }
              },
            }
          }
        },
      }
    }

    const result = await runCodexComposerStream(
      "fix the composer",
      {
        approvalPolicy: "never",
        codexCliLoginPresent: false,
        cwd: "/tmp/current-repo",
        env: { CODEX_API_KEY: "test-key-shape" },
        importer,
        model: "gpt-5-codex",
        networkAccessEnabled: false,
        platform: "darwin",
        sandboxMode: "workspace-write",
      },
      {
        onEvent: (summary) => eventSummaries.push(summary),
        onText: (text) => textUpdates.push(text),
        onThreadId: (_threadId, ref) => threadRefs.push(ref),
        onUsage: (usage) => usageUpdates.push(usage.totalTokens),
      },
    )

    expect(promptSeen).toBe("fix the composer")
    expect(signalSeen).toBe(true)
    expect(threadOptions).toMatchObject({
      approvalPolicy: "never",
      model: "gpt-5-codex",
      networkAccessEnabled: false,
      sandboxMode: "workspace-write",
      skipGitRepoCheck: true,
      workingDirectory: "/tmp/current-repo",
    })
    expect(clientOptions?.env).toMatchObject({ CODEX_API_KEY: "test-key-shape" })
    expect(clientOptions?.config).toMatchObject({
      model_reasoning_summary: "detailed",
      show_raw_agent_reasoning: true,
    })
    expect(textUpdates).toEqual(["Patched the composer."])
    expect(threadRefs).toEqual(["session.pylon.codex_composer.17d51880c7bad062ae498c8e"])
    expect(usageUpdates).toEqual([15])
    expect(eventSummaries).toContain("thinking tokens: 7; output tokens: 11")
    expect(eventSummaries).toContain("thinking: checking the patch path")
    expect(eventSummaries).toContain("completed: bun test exit 0")
    expect(eventSummaries).toContain("completed: update src/index.ts, add tests/index.test.ts")
    expect(result).toMatchObject({
      commandCount: 1,
      editedFileCount: 2,
      eventCount: 8,
      inputTokens: 10,
      outputTokens: 5,
      text: "Patched the composer.",
      threadId: "thread.test.codex",
      totalTokens: 15,
    })
  })

  test("summarizes raw reasoning item text instead of a placeholder", () => {
    expect(summarizeCodexThreadEvent({
      type: "item.completed",
      item: {
        id: "reasoning-1",
        type: "reasoning",
        text: "Inspect the current process and compare its health capabilities.",
      },
    })).toBe("thinking: Inspect the current process and compare its health capabilities.")
  })

  test("parses readable reasoning from Codex human output", () => {
    const parse = createCodexHumanOutputParser()
    expect(parse("session id: 019ee33e-5a66-7ac2-ad23-bf6cc50ef822")).toEqual({
      type: "thread",
      threadId: "019ee33e-5a66-7ac2-ad23-bf6cc50ef822",
    })
    expect(parse("hook: UserPromptSubmit Completed")).toBeNull()
    expect(parse("**Verifying prime status**")).toEqual({
      type: "reasoning",
      text: "Verifying prime status",
    })
    expect(parse("I checked primes up to the square root.")).toEqual({
      type: "reasoning",
      text: "I checked primes up to the square root.",
    })
    expect(parse("codex")).toBeNull()
    expect(parse("ready")).toEqual({ type: "agent", text: "ready" })
    expect(parse("tokens used")).toBeNull()
    expect(parse("11,868")).toEqual({ type: "tokens", totalTokens: 11868 })
  })

  test("reports readiness blockers before starting the SDK thread", async () => {
    const importer = async (specifier: string) => {
      if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        Codex: class {
          startThread() {
            throw new Error("must not start")
          }
        },
      }
    }

    await expect(
      runCodexComposerStream("fix", {
        codexCliLoginPresent: false,
        cwd: "/tmp/current-repo",
        env: {},
        importer,
        platform: "darwin",
      }),
    ).rejects.toThrow("Codex composer unavailable: credentials_missing")
  })

  test("danger-full-access requires the local supervised execution mode", async () => {
    const importer = async (specifier: string) => {
      if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        Codex: class {
          startThread() {
            throw new Error("must not start")
          }
        },
      }
    }

    await expect(
      runCodexComposerStream("fix", {
        codexCliLoginPresent: false,
        cwd: "/tmp/current-repo",
        env: { CODEX_API_KEY: "test-key-shape" },
        importer,
        platform: "darwin",
        sandboxMode: "danger-full-access",
      }),
    ).rejects.toThrow(CODEX_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF)
  })

  test("local supervised mode maps to SDK danger-full-access", async () => {
    let threadOptions: Record<string, unknown> | null = null
    const importer = async (specifier: string) => {
      if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        Codex: class {
          startThread(options: Record<string, unknown>) {
            threadOptions = options
            return {
              runStreamed: async () => ({ events: fakeCodexEvents() }),
            }
          }
        },
      }
    }

    await runCodexComposerStream("fix", {
      approvalPolicy: "never",
      codexCliLoginPresent: false,
      cwd: "/tmp/current-repo",
      env: { CODEX_API_KEY: "test-key-shape" },
      executionMode: "local_supervised_danger",
      importer,
      platform: "darwin",
      sandboxMode: sandboxModeForCodexComposerExecutionMode("local_supervised_danger", "workspace-write"),
    })

    expect(threadOptions).toMatchObject({
      approvalPolicy: "never",
      sandboxMode: "danger-full-access",
      workingDirectory: "/tmp/current-repo",
    })
  })

  test("injects per-session CODEX_HOME without mutating the process env", async () => {
    let clientEnv: Record<string, string | undefined> | null = null
    const original = Bun.env.CODEX_HOME
    const importer = async (specifier: string) => {
      if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        Codex: class {
          constructor(options?: { env?: Record<string, string | undefined> }) {
            clientEnv = options?.env ?? null
          }
          startThread() {
            return {
              runStreamed: async () => ({ events: fakeCodexEvents() }),
            }
          }
        },
      }
    }

    const result = await runCodexComposerStream("fix", {
      accountHome: "/tmp/codex-home-a",
      codexCliLoginPresent: true,
      cwd: "/tmp/current-repo",
      env: { PATH: "/bin" },
      importer,
      platform: "darwin",
    })

    expect(result.threadId).toBe("thread.test.codex")
    expect(clientEnv).toMatchObject({ PATH: "/bin", CODEX_HOME: "/tmp/codex-home-a" })
    expect(Bun.env.CODEX_HOME).toBe(original)
  })

  test("injects the ripgrep guard into the Codex environment", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-codex-composer-rg-"))
    try {
      const realRg = join(root, "real-rg")
      const guardBin = join(root, "guard-bin")
      await writeFile(realRg, "#!/usr/bin/env bash\nexit 0\n")
      await chmod(realRg, 0o755)

      let clientEnv: Record<string, string | undefined> | null = null
      const importer = async (specifier: string) => {
        if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
        return {
          Codex: class {
            constructor(options?: { env?: Record<string, string | undefined> }) {
              clientEnv = options?.env ?? null
            }
            startThread() {
              return {
                runStreamed: async () => ({ events: fakeCodexEvents() }),
              }
            }
          },
        }
      }

      await runCodexComposerStream("fix", {
        codexCliLoginPresent: true,
        cwd: "/tmp/current-repo",
        env: {
          OPENAGENTS_CODEX_REAL_RG: realRg,
          OPENAGENTS_CODEX_RG_GUARD_BIN_DIR: guardBin,
          PATH: "/bin",
        },
        importer,
        platform: "darwin",
      })

      expect(clientEnv?.OPENAGENTS_CODEX_RG_GUARD).toBe("1")
      expect(clientEnv?.OPENAGENTS_CODEX_REAL_RG).toBe(realRg)
      expect(clientEnv?.PATH?.startsWith(`${guardBin}:`)).toBe(true)
      expect(existsSync(join(guardBin, "rg"))).toBe(true)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("persists streamed usage snapshots when a Pylon home is provided", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-usage-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const importer = async (specifier: string) => {
        if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
        return {
          Codex: class {
            startThread() {
              return {
                runStreamed: async () => ({ events: fakeCodexEvents() }),
              }
            }
          },
        }
      }

      await runCodexComposerStream("fix", {
        codexCliLoginPresent: false,
        cwd: "/tmp/current-repo",
        env: { CODEX_API_KEY: "test-key-shape", PYLON_HOME: home },
        importer,
        platform: "darwin",
        usageStateSummary: summary,
      })

      const usage = await collectPylonAccountsUsage(
        summary,
        parsePylonAccountsUsageArgs(["--json"]),
        {
          env: { CODEX_API_KEY: "test-key-shape", PYLON_HOME: home },
        },
      )
      const codex = usage.accounts.find((account) => account.provider === "codex")
      expect(codex?.truth.provider.state).toBe("available")
      expect(codex?.truth.provider.snapshots[0]?.primary?.label).toBe("weekly")
      expect(codex?.truth.localSession.usage?.totalTokens).toBe(15)
      expect(JSON.stringify(usage)).not.toContain("test-key-shape")
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("public command paths reject the local dangerous flag", () => {
    expect(() => rejectCodexLocalDangerForPublicPath(["submit", "--codex-danger"], "pylon work")).toThrow(
      CODEX_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF,
    )
    expect(() => rejectCodexLocalDangerForPublicPath(["submit"], "pylon work")).not.toThrow()
  })

  test("summarizes todo and error stream events without raw payload dumps", () => {
    expect(
      summarizeCodexThreadEvent({
        type: "item.updated",
        item: {
          type: "todo_list",
          items: [
            { text: "inspect", completed: true },
            { text: "patch", completed: false },
          ],
        },
      }),
    ).toBe("todo list 1/2")
    expect(summarizeCodexThreadEvent({ type: "error", message: "bad stream" })).toBe("error: bad stream")
  })
})
