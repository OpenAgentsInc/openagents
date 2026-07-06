#!/usr/bin/env bun
// Khala Code TUI — interactive terminal chat over the same local Codex
// app-server harness the desktop window uses. No window, no preview server:
// one readline loop, streamed deltas, multi-turn thread continuity.
//
// Usage:
//   bun scripts/khala-code-tui.ts                    # interactive REPL
//   bun scripts/khala-code-tui.ts "explain this bug" # one-shot: run a prompt, print, exit
//   echo "prompt" | bun scripts/khala-code-tui.ts    # piped: whole stdin is ONE prompt
//   printf 'a\nb\n' | bun scripts/khala-code-tui.ts --lines   # one turn per line (scripting)
//   bun scripts/khala-code-tui.ts --resume <threadId>          # continue an existing thread
//   bun scripts/khala-code-tui.ts --run <name>                 # one-shot: run a saved prompt
//
// Slash commands (interactive/--lines): /new /status /help /exit and the saved-prompt
// library /save /prompts /run /del /<name>. stdout carries assistant text only; tool
// activity, token usage, and status go to stderr, so `... | tui` pipes cleanly.

import { homedir } from "node:os"
import { createInterface } from "node:readline/promises"

import { createCodexAppServerChatRuntime } from "../src/bun/codex-app-server-chat-runtime.js"
import { createCodexAppServerHost } from "../src/bun/codex-app-server-client.js"
import { inspectCodexHarnessStatus } from "../src/bun/codex-harness-status.js"
import { projectKhalaCodeDesktopEventToThreadEvents } from "../src/shared/headless-events.js"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc.js"

// ---- argv ------------------------------------------------------------------
const rawArgs = process.argv.slice(2)
const wantHelp = rawArgs.includes("-h") || rawArgs.includes("--help")
const perLine = rawArgs.includes("--lines")
let resumeThreadId: string | undefined
let runSavedName: string | undefined
const positional: string[] = []
for (let i = 0; i < rawArgs.length; i += 1) {
  const arg = rawArgs[i]
  if (arg === "--resume") {
    resumeThreadId = rawArgs[i + 1]
    i += 1
    continue
  }
  if (arg === "--run") {
    runSavedName = rawArgs[i + 1]
    i += 1
    continue
  }
  if (arg.startsWith("-")) continue
  positional.push(arg)
}
const oneShotPrompt = positional.length > 0 ? positional.join(" ") : undefined

// ---- saved-prompt library (persists across launches) -----------------------
// A named, reusable prompt store — the terminal analog of a saved-command
// library. Lives in the app home so a prompt saved once is available every
// session. stdout stays clean; all library chatter goes to stderr.
const PROMPT_STORE = `${homedir()}/.khala-code/tui-prompts.json`
const RESERVED = new Set(["new", "status", "help", "exit", "quit", "save", "prompts", "run", "del"])

const loadPrompts = async (): Promise<Record<string, string>> => {
  try {
    const parsed: unknown = JSON.parse(await Bun.file(PROMPT_STORE).text())
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, string>
    }
  } catch {
    // no store yet, or unreadable/corrupt — treat as empty
  }
  return {}
}
const savePrompts = async (prompts: Record<string, string>): Promise<void> => {
  await Bun.write(PROMPT_STORE, `${JSON.stringify(prompts, null, 2)}\n`)
}

const HELP = `khala-code tui — terminal chat over the local Codex app-server harness

  bun scripts/khala-code-tui.ts                 interactive REPL
  bun scripts/khala-code-tui.ts "<prompt>"      one-shot: run a prompt and exit
  echo "<prompt>" | bun scripts/khala-code-tui.ts   piped: whole stdin is one prompt
  printf 'a\\nb\\n' | ... --lines               one turn per line (scripting)
  ... --resume <threadId>                       continue an existing app-server thread
  ... --run <name>                              one-shot: run a saved prompt and exit

Slash commands: /new  /status  /help  /exit
Saved prompts:  /save <name> [text]   (text omitted = save your last prompt)
                /prompts              list saved prompts
                /run <name>  or  /<name>   run a saved prompt
                /del <name>           delete a saved prompt
Saved prompts persist in ~/.khala-code/tui-prompts.json.
stdout = assistant text only; tool activity, token usage and status print to stderr.`

const interactive = process.stdin.isTTY === true && process.stdout.isTTY === true
const workingDirectory = process.cwd()
const env = Bun.env

let sessionId = `khala-code-tui-${Date.now().toString(36)}`
let threadId: string | undefined = resumeThreadId
let activeTurnId: string | undefined
let messageCounter = 0
let interruptArmed = false
let lastPrompt = ""

// ---- thinking spinner (interactive only; stderr, no ANSI escapes) ----------
const spinnerFrames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
let spinnerTimer: ReturnType<typeof setInterval> | undefined
const startSpinner = (): void => {
  if (!interactive || spinnerTimer !== undefined) return
  let i = 0
  spinnerTimer = setInterval(() => {
    process.stderr.write(`\r${spinnerFrames[i++ % spinnerFrames.length]} thinking… `)
  }, 90)
}
const stopSpinner = (): void => {
  if (spinnerTimer === undefined) return
  clearInterval(spinnerTimer)
  spinnerTimer = undefined
  process.stderr.write("\r             \r")
}

// ---- event stream ----------------------------------------------------------
const toolGlyph: Record<string, string> = {
  command_execution: "⚙",
  file_change: "✎",
  mcp_tool_call: "⚙",
  todo_list: "☑",
  error: "⚠",
}

const host = createCodexAppServerHost({ env })

const onEvent = (event: KhalaCodeDesktopChatTurnEvent): void => {
  for (const threadEvent of projectKhalaCodeDesktopEventToThreadEvents(event)) {
    const projected = threadEvent as {
      readonly delta?: string
      readonly type?: string
      readonly item?: { readonly kind?: string; readonly tool_name?: string }
    }
    if (projected.type === "item.delta" && typeof projected.delta === "string") {
      stopSpinner()
      process.stdout.write(projected.delta)
    } else if (
      projected.type === "item.started" &&
      projected.item?.kind &&
      projected.item.kind !== "message"
    ) {
      // Surface tool activity (previously dropped) — meta goes to stderr.
      stopSpinner()
      const kind = projected.item.kind
      const glyph = toolGlyph[kind] ?? "•"
      const name = projected.item.tool_name ? `: ${projected.item.tool_name}` : ""
      process.stderr.write(`  ${glyph} ${kind}${name}\n`)
    }
  }
}

const runtime = createCodexAppServerChatRuntime({
  env,
  host,
  onEvent,
  workingDirectory,
})

const formatUsage = (response: unknown): string => {
  const usage = (response as {
    usage?: { input?: number; output?: number; reasoningOutput?: number; cachedInput?: number }
  }).usage
  const input = usage?.input ?? 0
  const output = usage?.output ?? 0
  const reasoning = usage?.reasoningOutput ?? 0
  const cached = usage?.cachedInput ?? 0
  // The codex app-server path currently reports all-zero usage for turns that
  // plainly consumed tokens; report unknown rather than fabricate zeros, per
  // the exact/estimated/unknown honesty rule.
  if (usage === undefined || (input === 0 && output === 0 && reasoning === 0 && cached === 0)) {
    return "[tokens: unknown]"
  }
  const parts = [`in ${input}`, `out ${output}`]
  if (reasoning) parts.push(`reasoning ${reasoning}`)
  if (cached) parts.push(`cached ${cached}`)
  return `[tokens: ${parts.join(" · ")}]`
}

const printStatus = async (): Promise<boolean> => {
  const status = await inspectCodexHarnessStatus({ env })
  const auth = status.auth as
    | { readonly blockerRefs?: readonly string[]; readonly state?: string }
    | undefined
  if (status.available) {
    if (interactive) {
      process.stderr.write(
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
  lastPrompt = prompt
  await ensureThread()
  const turnId = `turn-${Date.now().toString(36)}`
  activeTurnId = turnId
  interruptArmed = false
  messageCounter += 1
  startSpinner()
  try {
    const response = await runtime.startTurn({
      cwd: workingDirectory,
      messages: [{ body: prompt, id: `tui-user-${messageCounter}`, role: "user" }],
      sessionId,
      ...(threadId === undefined ? {} : { threadId }),
      turnId,
    })
    threadId = response.backend.threadId ?? threadId
    stopSpinner()
    if (response.ok) {
      process.stdout.write("\n")
      process.stderr.write(`${formatUsage(response)}\n`)
      return
    }
    const backend = response.backend as { readonly blockerRefs?: readonly string[] }
    const blockers = backend.blockerRefs?.length ? ` — ${backend.blockerRefs.join(", ")}` : ""
    process.stderr.write(
      `[turn ${response.backend.turnStatus ?? "failed"}${blockers}] ${formatUsage(response)}\n`,
    )
  } catch (error) {
    stopSpinner()
    const message = error instanceof Error ? error.message : String(error)
    process.stderr.write(`[turn failed: ${message}]\n`)
  } finally {
    activeTurnId = undefined
    interruptArmed = false
  }
}

const shutdown = (code: number): never => {
  stopSpinner()
  try {
    host.dispose()
  } catch {
    // already stopped
  }
  process.exit(code)
}

process.on("SIGINT", () => {
  if (activeTurnId !== undefined) {
    if (interruptArmed) {
      process.stderr.write("\n[force quit]\n")
      shutdown(130)
    }
    interruptArmed = true
    const turnId = activeTurnId
    stopSpinner()
    process.stderr.write("\n[interrupting turn — Ctrl-C again to force quit]\n")
    void runtime.interruptTurn({ sessionId, turnId }).catch(() => undefined)
    return
  }
  process.stdout.write("\n")
  shutdown(0)
})

if (wantHelp) {
  process.stdout.write(`${HELP}\n`)
  shutdown(0)
}

const ready = await printStatus()
if (!ready) shutdown(2)

// One-shot: a prompt passed as argv runs once and exits (works with a TTY too).
if (oneShotPrompt !== undefined) {
  await runTurn(oneShotPrompt)
  shutdown(0)
}

// One-shot: run a saved prompt by name and exit.
if (runSavedName !== undefined) {
  const body = (await loadPrompts())[runSavedName]
  if (body === undefined) {
    process.stderr.write(`no saved prompt '${runSavedName}' (see /prompts)\n`)
    shutdown(2)
  }
  await runTurn(body)
  shutdown(0)
}

const handleLine = async (line: string): Promise<"continue" | "exit"> => {
  const trimmed = line.trim()
  if (trimmed.length === 0) return "continue"
  if (trimmed === "/exit" || trimmed === "/quit") return "exit"
  if (trimmed === "/help") {
    process.stderr.write(`${HELP}\n`)
    return "continue"
  }
  if (trimmed === "/new") {
    sessionId = `khala-code-tui-${Date.now().toString(36)}`
    threadId = undefined
    if (interactive) process.stderr.write("[new thread]\n")
    return "continue"
  }
  if (trimmed === "/status") {
    await printStatus()
    return "continue"
  }
  if (trimmed === "/prompts") {
    const prompts = await loadPrompts()
    const names = Object.keys(prompts).sort()
    if (names.length === 0) {
      process.stderr.write("[no saved prompts — /save <name> to add one]\n")
    } else {
      process.stderr.write(
        `${names
          .map(n => `  /${n}  — ${prompts[n].replace(/\s+/gu, " ").slice(0, 60)}`)
          .join("\n")}\n`,
      )
    }
    return "continue"
  }
  if (trimmed.startsWith("/save")) {
    const rest = trimmed.slice("/save".length).trim()
    const sep = rest.indexOf(" ")
    const name = sep === -1 ? rest : rest.slice(0, sep)
    const inline = sep === -1 ? "" : rest.slice(sep + 1).trim()
    if (name.length === 0) {
      process.stderr.write("usage: /save <name> [prompt text]  (omit text to save your last prompt)\n")
      return "continue"
    }
    if (RESERVED.has(name)) {
      process.stderr.write(`'${name}' is a reserved command name — pick another\n`)
      return "continue"
    }
    const body = inline.length > 0 ? inline : lastPrompt
    if (body.length === 0) {
      process.stderr.write("nothing to save yet — send a prompt first, or /save <name> <text>\n")
      return "continue"
    }
    const prompts = await loadPrompts()
    prompts[name] = body
    await savePrompts(prompts)
    process.stderr.write(`[saved '/${name}' → ${body.length} chars]\n`)
    return "continue"
  }
  if (trimmed.startsWith("/run ")) {
    const name = trimmed.slice("/run ".length).trim()
    const body = (await loadPrompts())[name]
    if (body === undefined) {
      process.stderr.write(`[no saved prompt '${name}' — /prompts to list]\n`)
      return "continue"
    }
    await runTurn(body)
    return "continue"
  }
  if (trimmed.startsWith("/del ")) {
    const name = trimmed.slice("/del ".length).trim()
    const prompts = await loadPrompts()
    if (prompts[name] === undefined) {
      process.stderr.write(`[no saved prompt '${name}']\n`)
      return "continue"
    }
    delete prompts[name]
    await savePrompts(prompts)
    process.stderr.write(`[deleted '${name}']\n`)
    return "continue"
  }
  // Shorthand: /<name> runs a saved prompt when one exists; otherwise the line
  // falls through and is sent to the model verbatim (unchanged prior behavior).
  if (trimmed.startsWith("/") && !/\s/u.test(trimmed)) {
    const name = trimmed.slice(1)
    if (!RESERVED.has(name)) {
      const body = (await loadPrompts())[name]
      if (body !== undefined) {
        await runTurn(body)
        return "continue"
      }
    }
  }
  await runTurn(trimmed)
  return "continue"
}

if (interactive) {
  process.stderr.write("multi-turn thread; /new /status /help /save /prompts /exit; Ctrl-C interrupts a turn\n\n")
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
  const text = await Bun.stdin.text()
  if (perLine) {
    // Scripting mode: one turn per line, slash-commands active. Piped mode
    // drains stdin first — readline would drop lines that arrive mid-stream.
    for (const line of text.split("\n")) {
      if ((await handleLine(line)) === "exit") break
    }
  } else {
    // Default piped mode: the whole of stdin is a single prompt, so a
    // multi-line heredoc reaches the model intact instead of being split
    // into one turn per line.
    const prompt = text.replace(/\n+$/u, "")
    if (prompt.trim().length > 0) await runTurn(prompt)
  }
}

shutdown(0)
