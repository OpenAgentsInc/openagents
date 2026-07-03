#!/usr/bin/env bun
// Khala Code TUI — interactive terminal chat over the same local Codex
// app-server harness the desktop window uses. No window, no preview server:
// one readline loop, streamed deltas, multi-turn thread continuity.
//
// Usage:
//   bun scripts/khala-code-tui.ts                    # interactive REPL
//   echo "prompt" | bun scripts/khala-code-tui.ts    # piped turns, exit at EOF
//
// Slash commands: /new (fresh thread), /status (harness readiness), /exit

import { createInterface } from "node:readline/promises"
import { Buffer } from "node:buffer"

import { createCodexAppServerChatRuntime } from "../src/bun/codex-app-server-chat-runtime.js"
import { createCodexAppServerHost } from "../src/bun/codex-app-server-client.js"
import { inspectCodexHarnessStatus } from "../src/bun/codex-harness-status.js"
import { projectKhalaCodeDesktopEventToThreadEvents } from "../src/shared/headless-events.js"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc.js"

const interactive = process.stdin.isTTY === true && process.stdout.isTTY === true
const workingDirectory = process.cwd()
const env = Bun.env

let sessionId = `khala-code-tui-${Date.now().toString(36)}`
let threadId: string | undefined
let activeTurnId: string | undefined
let messageCounter = 0

const host = createCodexAppServerHost({ env })

const onEvent = (event: KhalaCodeDesktopChatTurnEvent): void => {
  for (const threadEvent of projectKhalaCodeDesktopEventToThreadEvents(event)) {
    const projected = threadEvent as { readonly delta?: string; readonly type?: string }
    if (projected.type === "item.delta" && typeof projected.delta === "string") {
      process.stdout.write(projected.delta)
    }
  }
}

const runtime = createCodexAppServerChatRuntime({
  env,
  host,
  onEvent,
  workingDirectory,
})

const printStatus = async (): Promise<boolean> => {
  const status = await inspectCodexHarnessStatus({ env })
  const auth = status.auth as
    | { readonly blockerRefs?: readonly string[]; readonly state?: string }
    | undefined
  if (status.available) {
    if (interactive) {
      process.stdout.write(
        `khala-code tui — codex harness ready (${status.binary?.version ?? "codex"}, home ${
          status.home?.path ?? "?"
        })\n`,
      )
    }
    return true
  }
  process.stderr.write(
    `khala-code tui: codex harness unavailable (${auth?.state ?? status.status}).\n` +
      `${status.reason ?? ""}\n` +
      (auth?.blockerRefs?.length ? `blockers: ${auth.blockerRefs.join(", ")}\n` : ""),
  )
  return false
}

const ensureThread = async (): Promise<void> => {
  if (threadId !== undefined) return
  const thread = await runtime.startThread({ cwd: workingDirectory, sessionId })
  threadId = thread.threadId
}

const runTurn = async (prompt: string): Promise<void> => {
  await ensureThread()
  const turnId = `turn-${Date.now().toString(36)}`
  activeTurnId = turnId
  messageCounter += 1
  try {
    const response = await runtime.startTurn({
      cwd: workingDirectory,
      messages: [{ body: prompt, id: `tui-user-${messageCounter}`, role: "user" }],
      sessionId,
      ...(threadId === undefined ? {} : { threadId }),
      turnId,
    })
    threadId = response.backend.threadId ?? threadId
    if (response.ok) {
      process.stdout.write("\n")
      return
    }
    const backend = response.backend as { readonly blockerRefs?: readonly string[] }
    const blockers = backend.blockerRefs?.length ? ` — ${backend.blockerRefs.join(", ")}` : ""
    process.stdout.write(`\n[turn ${response.backend.turnStatus ?? "failed"}${blockers}]\n`)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    process.stdout.write(`\n[turn failed: ${message}]\n`)
  } finally {
    activeTurnId = undefined
  }
}

const shutdown = (code: number): never => {
  try {
    host.dispose()
  } catch {
    // already stopped
  }
  process.exit(code)
}

process.on("SIGINT", () => {
  if (activeTurnId !== undefined) {
    const turnId = activeTurnId
    process.stdout.write("\n[interrupting turn]\n")
    void runtime.interruptTurn({ sessionId, turnId }).catch(() => undefined)
    return
  }
  process.stdout.write("\n")
  shutdown(0)
})

const ready = await printStatus()
if (!ready) shutdown(2)

if (interactive) {
  process.stdout.write("multi-turn thread; /new /status /exit; Ctrl-C interrupts a turn\n\n")
}

const handleLine = async (line: string): Promise<"continue" | "exit"> => {
  const trimmed = line.trim()
  if (trimmed.length === 0) return "continue"
  if (trimmed === "/exit" || trimmed === "/quit") return "exit"
  if (trimmed === "/new") {
    sessionId = `khala-code-tui-${Date.now().toString(36)}`
    threadId = undefined
    if (interactive) process.stdout.write("[new thread]\n")
    return "continue"
  }
  if (trimmed === "/status") {
    await printStatus()
    return "continue"
  }
  await runTurn(trimmed)
  return "continue"
}

const readPipedStdin = async (): Promise<string> => {
  const chunks: Array<Buffer> = []
  for await (const chunk of process.stdin) {
    chunks.push(typeof chunk === "string" ? Buffer.from(chunk) : chunk)
  }
  return Buffer.concat(chunks).toString("utf8")
}

if (interactive) {
  // Interactive REPL: readline owns the prompt.
  const rl = createInterface({ input: process.stdin, output: process.stdout })
  while (true) {
    let line: string
    try {
      line = await rl.question("you › ")
    } catch {
      break
    }
    if ((await handleLine(line)) === "exit") break
  }
  rl.close()
} else {
  // Piped mode: drain stdin first, then run each line as a sequential turn —
  // readline would drop lines that arrive while a turn is still streaming.
  const text = await readPipedStdin()
  for (const line of text.split("\n")) {
    if ((await handleLine(line)) === "exit") break
  }
}

shutdown(0)
