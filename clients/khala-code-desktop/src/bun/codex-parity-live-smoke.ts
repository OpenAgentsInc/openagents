import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import type { KhalaCodeDesktopCodexHarnessStatus } from "../shared/rpc.js"
import { createCodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"
import {
  createCodexAppServerHost,
  type CodexAppServerHost,
} from "./codex-app-server-client.js"
import {
  validateKhalaCodeHeadlessJsonl,
} from "../shared/headless-events.js"
import {
  runKhalaCodeDesktopHeadlessJsonl,
} from "./headless.js"
import { inspectCodexHarnessStatus } from "./codex-harness-status.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"

export const KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS = "codex_app_server_live_parity"

export type KhalaCodeCodexParityLiveSmokeResult = Readonly<{
  codexTurnId?: string
  eventCount?: number
  harness: typeof KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS
  modeH?: KhalaCodeCodexModeHLiveSmokeResult
  ok: boolean
  reason?: string
  required: boolean
  resumedThreadId?: string
  skipped: boolean
  status: "failed" | "ok" | "skipped"
  threadId?: string
  turnStatus?: string
}>

export type KhalaCodeCodexModeHLiveSmokeResult = Readonly<{
  codexTurnId?: string
  eventCount: number
  eventTypes: readonly string[]
  finalOk: boolean
  jsonlSchemaOk: boolean
  ok: boolean
  reason?: string
  threadId?: string
  turnId?: string
  validationErrors: readonly string[]
}>

export type RunKhalaCodeCodexParityLiveSmokeInput = Readonly<{
  createHost?: () => CodexAppServerHost
  env?: NodeJS.ProcessEnv
  inspectHarness?: () => Promise<KhalaCodeDesktopCodexHarnessStatus>
  interruptAfterMs?: number
  prompt?: string
  requireLive?: boolean
  timeoutMs?: number
  workingDirectory?: string
}>

const defaultPrompt =
  "Khala Codex parity live smoke: acknowledge with one short sentence, then stop."

const defaultHeadlessPrompt =
  "Khala Code Mode H live smoke: acknowledge with one short sentence, then stop."

const notRequested = (): KhalaCodeCodexParityLiveSmokeResult => ({
  harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
  ok: true,
  reason:
    "Live Codex parity smoke not requested. Set KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE=1 or pass --require-live.",
  required: false,
  skipped: true,
  status: "skipped",
})

const writableBuffer = (): { readonly sink: { write: (chunk: string) => boolean }; readonly text: () => string } => {
  let value = ""
  return {
    sink: {
      write: (chunk: string) => {
        value += chunk
        return true
      },
    },
    text: () => value,
  }
}

const isRecord = (value: unknown): value is Readonly<Record<string, unknown>> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringField = (value: unknown, field: string): string | undefined => {
  if (!isRecord(value)) return undefined
  const candidate = value[field]
  return typeof candidate === "string" && candidate.length > 0 ? candidate : undefined
}

const booleanField = (value: unknown, field: string): boolean | undefined => {
  if (!isRecord(value)) return undefined
  const candidate = value[field]
  return typeof candidate === "boolean" ? candidate : undefined
}

async function runModeHLiveSmoke(input: {
  readonly createHost?: () => CodexAppServerHost
  readonly env: NodeJS.ProcessEnv
  readonly prompt?: string
  readonly timeoutMs?: number
  readonly workingDirectory: string
}): Promise<KhalaCodeCodexModeHLiveSmokeResult> {
  const stdout = writableBuffer()
  const stderr = writableBuffer()
  const host = input.createHost?.() ?? createCodexAppServerHost({ env: input.env })

  try {
    await runKhalaCodeDesktopHeadlessJsonl({
      createCodexChatRuntime: ({ onEvent }) =>
        createCodexAppServerChatRuntime({
          env: input.env,
          host,
          onEvent,
          statePath: join(input.workingDirectory, "codex-mode-h-sessions.json"),
          tokenUsageReporter: null,
          turnTimeoutMs: input.timeoutMs ?? 120_000,
          workingDirectory: input.workingDirectory,
        }),
      env: input.env,
      prompt: input.prompt ?? defaultHeadlessPrompt,
      sessionId: "khala-code-mode-h-live-smoke-session",
      stderr: stderr.sink,
      stdout: stdout.sink,
      turnId: "khala-code-mode-h-live-smoke-turn",
      workingDirectory: input.workingDirectory,
    })

    const validation = validateKhalaCodeHeadlessJsonl(stderr.text())
    const final: unknown = JSON.parse(stdout.text())
    const requiredTypes = ["thread.started", "turn.started", "turn.completed"]
    const missingTypes = requiredTypes.filter(type => !validation.eventTypes.includes(type as never))
    const validationErrors = [
      ...validation.errors,
      ...missingTypes.map(type => `missing ${type}`),
    ]
    const finalOk = booleanField(final, "ok") === true
    const codexTurnId = stringField(final, "codexTurnId")
    const threadId = stringField(final, "threadId")
    const turnId = stringField(final, "turnId")

    return {
      ...(codexTurnId === undefined ? {} : { codexTurnId }),
      eventCount: validation.eventCount,
      eventTypes: validation.eventTypes,
      finalOk,
      jsonlSchemaOk: validation.ok && missingTypes.length === 0,
      ok: finalOk && validation.ok && missingTypes.length === 0,
      ...(threadId === undefined ? {} : { threadId }),
      ...(turnId === undefined ? {} : { turnId }),
      validationErrors,
    }
  } catch (error) {
    const validation = validateKhalaCodeHeadlessJsonl(stderr.text())
    return {
      eventCount: validation.eventCount,
      eventTypes: validation.eventTypes,
      finalOk: false,
      jsonlSchemaOk: validation.ok,
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
      validationErrors: validation.errors,
    }
  } finally {
    host.dispose()
  }
}

export async function runKhalaCodeCodexParityLiveSmoke(
  input: RunKhalaCodeCodexParityLiveSmokeInput = {},
): Promise<KhalaCodeCodexParityLiveSmokeResult> {
  const env = input.env ?? khalaCodeConfigFromRuntimeEnv().env
  const requireLive = input.requireLive === true ||
    env.KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE === "1"
  if (!requireLive) return notRequested()

  const harness = await (input.inspectHarness ?? (() => inspectCodexHarnessStatus({ env })))()
  if (!harness.available) {
    return {
      harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
      ok: false,
      reason: `Explicit live Codex parity smoke requested, but Codex is unavailable: ${harness.reason}`,
      required: true,
      skipped: false,
      status: "failed",
    }
  }

  const tempRoot = input.workingDirectory ?? await mkdtemp(join(tmpdir(), "khala-code-codex-parity-"))
  const ownsTempRoot = input.workingDirectory === undefined
  const host = input.createHost?.() ?? createCodexAppServerHost({ env })
  const events: unknown[] = []
  let interruptTimer: ReturnType<typeof setTimeout> | undefined
  try {
    const runtime = createCodexAppServerChatRuntime({
      env,
      host,
      onEvent: event => events.push(event),
      statePath: join(tempRoot, "codex-sessions.json"),
      turnTimeoutMs: input.timeoutMs ?? 120_000,
      workingDirectory: tempRoot,
    })
    const sessionId = "khala-code-parity-live-smoke-session"
    const turnId = "khala-code-parity-live-smoke-turn"
    const thread = await runtime.startThread({ cwd: tempRoot, sessionId })
    const resumed = await runtime.resumeThread({
      cwd: tempRoot,
      sessionId,
      threadId: thread.threadId,
    })
    const turn = runtime.startTurn({
      cwd: tempRoot,
      messages: [{
        body: input.prompt ?? defaultPrompt,
        id: "khala-code-parity-live-smoke-user",
        role: "user",
      }],
      sessionId,
      turnId,
    })
    const interruptAfterMs = input.interruptAfterMs ?? 750
    if (interruptAfterMs >= 0) {
      interruptTimer = setTimeout(() => {
        void runtime.interruptTurn({ sessionId, turnId }).catch(() => undefined)
      }, interruptAfterMs)
    }
    const response = await turn
    const ok = response.ok || response.backend.turnStatus === "interrupted"
    const modeH = ok
      ? await runModeHLiveSmoke({
        env,
        ...(input.createHost === undefined ? {} : { createHost: input.createHost }),
        ...(input.prompt === undefined ? {} : { prompt: `${input.prompt} Mode H JSONL pass.` }),
        ...(input.timeoutMs === undefined ? {} : { timeoutMs: input.timeoutMs }),
        workingDirectory: tempRoot,
      })
      : undefined
    return {
      ...(response.backend.turnId === undefined ? {} : { codexTurnId: response.backend.turnId }),
      eventCount: events.length,
      harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
      ...(modeH === undefined ? {} : { modeH }),
      ok: ok && (modeH?.ok ?? true),
      required: true,
      resumedThreadId: resumed.threadId,
      skipped: false,
      status: ok && (modeH?.ok ?? true) ? "ok" : "failed",
      threadId: response.backend.threadId ?? thread.threadId,
      ...(response.backend.turnStatus === undefined ? {} : { turnStatus: response.backend.turnStatus }),
    }
  } catch (error) {
    return {
      harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
      required: true,
      skipped: false,
      status: "failed",
    }
  } finally {
    if (interruptTimer !== undefined) clearTimeout(interruptTimer)
    host.dispose()
    if (ownsTempRoot) await rm(tempRoot, { force: true, recursive: true })
  }
}
