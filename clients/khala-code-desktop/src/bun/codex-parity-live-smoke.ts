import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import type { KhalaCodeDesktopCodexHarnessStatus } from "../shared/rpc.js"
import { createCodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"
import {
  createCodexAppServerHost,
  type CodexAppServerHost,
} from "./codex-app-server-client.js"
import { inspectCodexHarnessStatus } from "./codex-harness-status.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"

export const KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS = "codex_app_server_live_parity"

export type KhalaCodeCodexParityLiveSmokeResult = Readonly<{
  codexTurnId?: string
  eventCount?: number
  harness: typeof KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS
  ok: boolean
  reason?: string
  required: boolean
  resumedThreadId?: string
  skipped: boolean
  status: "failed" | "ok" | "skipped"
  threadId?: string
  turnStatus?: string
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

const notRequested = (): KhalaCodeCodexParityLiveSmokeResult => ({
  harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
  ok: true,
  reason:
    "Live Codex parity smoke not requested. Set KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE=1 or pass --require-live.",
  required: false,
  skipped: true,
  status: "skipped",
})

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
    return {
      ...(response.backend.turnId === undefined ? {} : { codexTurnId: response.backend.turnId }),
      eventCount: events.length,
      harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
      ok,
      required: true,
      resumedThreadId: resumed.threadId,
      skipped: false,
      status: ok ? "ok" : "failed",
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
