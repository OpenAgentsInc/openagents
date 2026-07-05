import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { resolve } from "node:path"
import { afterEach, describe, expect, test } from "bun:test"
import { HarnessAgent } from "@ai-sdk/harness/agent"
import type {
  HarnessV1,
  HarnessV1ContinueTurnOptions,
  HarnessV1LifecycleState,
  HarnessV1Prompt,
  HarnessV1PromptTurnOptions,
  HarnessV1Session,
  HarnessV1StartOptions,
  HarnessV1StreamPart,
} from "@ai-sdk/harness"
import { createLocalAiSdkSandboxProvider } from "./index.js"

const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, {
    force: true,
    recursive: true,
  })))
})

describe("createLocalAiSdkSandboxProvider", () => {
  test("runs a public HarnessAgent.stream fixture without Vercel", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "openagents-local-provider-"))
    roots.push(root)
    const provider = createLocalAiSdkSandboxProvider({
      defaultPorts: [4321],
      rootDirectory: root,
    })
    const agent = new HarnessAgent({
      harness: fixtureHarness,
      sandbox: provider,
      sandboxConfig: {
        onSession: async ({ session, sessionWorkDir }) => {
          await session.writeTextFile({
            content: "public fixture\n",
            path: `${sessionWorkDir}/README.md`,
          })
        },
      },
    })
    const session = await agent.createSession({
      sessionId: "fixture-session",
    })

    try {
      const result = await agent.stream({
        prompt: "repair the public fixture",
        session,
      })
      let text = ""
      for await (const delta of result.textStream) {
        text += delta
      }
      expect(text).toContain("public fixture repaired")

      const sandbox = await provider.resumeSession?.({
        sessionId: "fixture-session",
      })
      expect(sandbox).toBeDefined()
      if (sandbox === undefined) throw new Error("Missing resumed sandbox.")
      expect(sandbox.defaultWorkingDirectory).toStartWith(root)
      expect(await sandbox.getPortUrl({ port: 4321, protocol: "ws" })).toBe(
        "ws://127.0.0.1:4321/",
      )
      expect(
        await sandbox.readTextFile({
          path: `${sandbox.defaultWorkingDirectory}/codex-fixture-session/result.txt`,
        }),
      ).toBe("repair the public fixture\n")
      const env = await sandbox.run({
        command: "printf '%s\\n%s\\n%s' \"$HOME\" \"$CODEX_HOME\" \"$CLAUDE_CONFIG_DIR\"",
      })
      expect(env.stdout).toContain(`${sandbox.defaultWorkingDirectory}/.openagents-home`)
      expect(env.stdout).toContain("/codex")
      expect(env.stdout).toContain("/claude-code")
      await expect(
        sandbox.readTextFile({ path: resolve(tmpdir(), "outside-openagents.txt") }),
      ).rejects.toThrow("escapes local sandbox")
    } finally {
      await session.destroy()
    }
  })

  test("restricted view keeps infra controls off the tool-safe surface", async () => {
    const root = await mkdtemp(resolve(tmpdir(), "openagents-local-provider-"))
    roots.push(root)
    const provider = createLocalAiSdkSandboxProvider({ rootDirectory: root })
    const sandbox = await provider.createSession({ sessionId: "restricted" })

    try {
      const restricted = sandbox.restricted()
      await restricted.writeTextFile({
        content: "a\nb\nc\n",
        path: "notes.txt",
      })
      expect(await restricted.readTextFile({
        endLine: 2,
        path: "notes.txt",
        startLine: 2,
      })).toBe("b")
      expect("stop" in restricted).toBe(false)
      expect("setNetworkPolicy" in restricted).toBe(false)
      await expect(restricted.readTextFile({ path: "../outside.txt" })).rejects.toThrow(
        "escapes local sandbox",
      )
    } finally {
      await sandbox.destroy?.()
    }
  })
})

const fixtureHarness: HarnessV1 = {
  builtinTools: {},
  harnessId: "codex",
  specificationVersion: "harness-v1",
  doStart: async (options) => makeFixtureSession(options),
}

function makeFixtureSession(options: HarnessV1StartOptions): HarnessV1Session {
  const lifecycle = (): HarnessV1LifecycleState => ({
    data: { sessionId: options.sessionId },
    harnessId: "codex",
    specificationVersion: "harness-v1",
    type: "resume-session",
  })
  const promptTurn = async (turn: HarnessV1PromptTurnOptions) => {
    const text = promptToText(turn.prompt)
    await options.sandboxSession.restricted().writeTextFile({
      content: `${text}\n`,
      path: `${options.sessionWorkDir}/result.txt`,
    })
    turn.emit({ type: "stream-start", modelId: "fixture-model" })
    turn.emit({ type: "text-start", id: "text-1" })
    turn.emit({
      delta: `public fixture repaired: ${text}`,
      id: "text-1",
      type: "text-delta",
    })
    turn.emit({ type: "text-end", id: "text-1" })
    turn.emit({
      finishReason: stopFinishReason,
      type: "finish-step",
      usage: fixtureUsage,
    })
    turn.emit({
      finishReason: stopFinishReason,
      totalUsage: fixtureUsage,
      type: "finish",
    })
    return {
      done: Promise.resolve(),
      submitToolResult: async () => {},
    }
  }
  const continueTurn = async (turn: HarnessV1ContinueTurnOptions) => {
    turn.emit({ type: "stream-start", modelId: "fixture-model" })
    turn.emit({
      finishReason: stopFinishReason,
      totalUsage: zeroUsage,
      type: "finish",
    })
    return {
      done: Promise.resolve(),
      submitToolResult: async () => {},
    }
  }
  return {
    doCompact: async () => {},
    doContinueTurn: continueTurn,
    doDestroy: async () => {},
    doDetach: async () => lifecycle() as Extract<HarnessV1LifecycleState, { type: "resume-session" }>,
    doPromptTurn: promptTurn,
    doStop: async () => lifecycle() as Extract<HarnessV1LifecycleState, { type: "resume-session" }>,
    doSuspendTurn: async () => ({
      data: { sessionId: options.sessionId },
      harnessId: "codex",
      specificationVersion: "harness-v1",
      type: "continue-turn",
    }),
    isResume: options.resumeFrom !== undefined || options.continueFrom !== undefined,
    sessionId: options.sessionId,
  }
}

function promptToText(prompt: HarnessV1Prompt): string {
  if (typeof prompt === "string") return prompt
  if (typeof prompt.content === "string") return prompt.content
  return prompt.content
    .map((part) => (part.type === "text" ? part.text : ""))
    .join("")
}

const stopFinishReason: Extract<
  HarnessV1StreamPart,
  { type: "finish" }
>["finishReason"] = {
  raw: undefined,
  unified: "stop",
}

const fixtureUsage: Extract<
  HarnessV1StreamPart,
  { type: "finish" }
>["totalUsage"] = {
  inputTokens: {
    cacheRead: undefined,
    cacheWrite: undefined,
    noCache: 1,
    total: 1,
  },
  outputTokens: {
    reasoning: undefined,
    text: 1,
    total: 1,
  },
}

const zeroUsage: Extract<
  HarnessV1StreamPart,
  { type: "finish" }
>["totalUsage"] = {
  inputTokens: {
    cacheRead: undefined,
    cacheWrite: undefined,
    noCache: undefined,
    total: undefined,
  },
  outputTokens: {
    reasoning: undefined,
    text: undefined,
    total: undefined,
  },
}
