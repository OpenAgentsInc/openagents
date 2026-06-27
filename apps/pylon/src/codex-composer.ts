import { createHash } from "node:crypto"
import { spawn } from "node:child_process"
import { statSync } from "node:fs"
import { createRequire } from "node:module"
import path from "node:path"
import readline from "node:readline"
import {
  CODEX_AGENT_SDK_PACKAGE,
  probeCodexAgentReadiness,
  type CodexAgentConfig,
  type CodexAgentSandboxMode,
} from "./codex-agent.js"
import {
  hashPylonAccountRef,
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "./account-registry.js"
import type { BootstrapSummary } from "./bootstrap.js"
import {
  providerRateLimitSnapshotsFromEvent,
  recordPylonAccountUsageObservation,
} from "./account-usage.js"
import { installCodexRipgrepGuard } from "./codex-rg-guard.js"

export type CodexComposerSandboxMode = CodexAgentSandboxMode | "danger-full-access"
export type CodexComposerExecutionMode = "local_bounded" | "local_supervised_danger"
export const CODEX_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF =
  "blocker.codex.local_supervised_danger_public_path"
export const CODEX_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF =
  "blocker.codex.local_supervised_danger_requires_opt_in"

export interface CodexComposerCallbacks {
  onText?: (fullText: string) => void
  onEvent?: (summary: string, eventCount: number) => void
  onThreadId?: (threadId: string, externalSessionRef: string) => void
  onUsage?: (usage: { inputTokens: number; outputTokens: number; totalTokens: number }) => void
}

export interface CodexComposerOptions {
  cwd: string
  account?: ResolvedPylonAccountSelection | null
  accountHome?: string
  accountRef?: string
  model?: string
  sandboxMode?: CodexComposerSandboxMode
  executionMode?: CodexComposerExecutionMode
  approvalPolicy?: "never" | "on-request" | "on-failure" | "untrusted"
  networkAccessEnabled?: boolean
  timeoutMs?: number
  abortSignal?: AbortSignal
  usageStateSummary?: Pick<BootstrapSummary, "paths">
  config?: CodexAgentConfig
  importer?: (specifier: string) => Promise<unknown>
  env?: Record<string, string | undefined>
  platform?: string
  codexCliLoginPresent?: boolean
  humanReadableReasoning?: boolean
}

export function sandboxModeForCodexComposerExecutionMode(
  mode: CodexComposerExecutionMode,
  boundedMode: CodexAgentSandboxMode | undefined,
): CodexComposerSandboxMode {
  return mode === "local_supervised_danger" ? "danger-full-access" : boundedMode ?? "workspace-write"
}

export function rejectCodexLocalDangerForPublicPath(
  args: ReadonlyArray<string>,
  routeName: string,
): void {
  if (!args.includes("--codex-danger")) return
  throw new Error(`${routeName} rejects local_supervised_danger (${CODEX_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF})`)
}

export interface CodexComposerResult {
  text: string
  eventCount: number
  inputTokens: number
  outputTokens: number
  totalTokens: number
  commandCount: number
  editedFileCount: number
  threadId: string | null
}

type CodexSdkModule = {
  Codex: new (options?: {
    env?: Record<string, string | undefined>
    config?: Record<string, unknown>
  }) => {
    startThread: (options: Record<string, unknown>) => {
      runStreamed: (
        prompt: string,
        turnOptions?: Record<string, unknown>,
      ) => Promise<{ events: AsyncIterable<unknown> }>
    }
  }
}

type CodexThreadEvent = {
  type?: string
  thread_id?: unknown
  payload?: {
    type?: unknown
    role?: unknown
    message?: unknown
    info?: unknown
    summary?: unknown
    content?: unknown
    last_agent_message?: unknown
  }
  usage?: {
    input_tokens?: unknown
    output_tokens?: unknown
    reasoning_output_tokens?: unknown
  }
  error?: { message?: unknown }
  message?: unknown
  item?: {
    type?: string
    text?: unknown
    command?: unknown
    aggregated_output?: unknown
    exit_code?: unknown
    status?: unknown
    changes?: unknown
    server?: unknown
    tool?: unknown
    query?: unknown
    message?: unknown
    items?: unknown
  }
}

type CodexCliPath = {
  executablePath: string
  pathDirs: string[]
}

type CodexHumanOutputParserState = {
  phase: "header" | "user" | "reasoning" | "agent" | "tokens"
}

type CodexHumanOutputLine =
  | { type: "thread"; threadId: string }
  | { type: "reasoning"; text: string }
  | { type: "agent"; text: string }
  | { type: "tokens"; totalTokens: number }
  | null

const CODEX_PLATFORM_PACKAGE_BY_TARGET: Record<string, string> = {
  "x86_64-unknown-linux-musl": "@openai/codex-linux-x64",
  "aarch64-unknown-linux-musl": "@openai/codex-linux-arm64",
  "x86_64-apple-darwin": "@openai/codex-darwin-x64",
  "aarch64-apple-darwin": "@openai/codex-darwin-arm64",
  "x86_64-pc-windows-msvc": "@openai/codex-win32-x64",
  "aarch64-pc-windows-msvc": "@openai/codex-win32-arm64",
}

function numberOrZero(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function isFile(value: string): boolean {
  try {
    return statSync(value).isFile()
  } catch {
    return false
  }
}

function isDirectory(value: string): boolean {
  try {
    return statSync(value).isDirectory()
  } catch {
    return false
  }
}

function targetTripleFor(platform: string, arch: string): string | null {
  if ((platform === "linux" || platform === "android") && arch === "x64") return "x86_64-unknown-linux-musl"
  if ((platform === "linux" || platform === "android") && arch === "arm64") return "aarch64-unknown-linux-musl"
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin"
  if (platform === "darwin" && arch === "arm64") return "aarch64-apple-darwin"
  if (platform === "win32" && arch === "x64") return "x86_64-pc-windows-msvc"
  if (platform === "win32" && arch === "arm64") return "aarch64-pc-windows-msvc"
  return null
}

function resolveCodexCliPath(): CodexCliPath {
  const targetTriple = targetTripleFor(process.platform, process.arch)
  if (targetTriple === null) throw new Error(`Unsupported Codex CLI platform: ${process.platform} (${process.arch})`)
  const platformPackage = CODEX_PLATFORM_PACKAGE_BY_TARGET[targetTriple]
  if (platformPackage === undefined) throw new Error(`Unsupported Codex CLI target: ${targetTriple}`)

  const requireFromHere = createRequire(import.meta.url)
  const sdkPackageJsonPath = requireFromHere.resolve(`${CODEX_AGENT_SDK_PACKAGE}/package.json`)
  const sdkRequire = createRequire(sdkPackageJsonPath)
  const codexPackageJsonPath = sdkRequire.resolve("@openai/codex/package.json")
  const codexRequire = createRequire(codexPackageJsonPath)
  const platformPackageJsonPath = codexRequire.resolve(`${platformPackage}/package.json`)
  const vendorRoot = path.join(path.dirname(platformPackageJsonPath), "vendor")
  const packageRoot = path.join(vendorRoot, targetTriple)
  const executablePath = path.join(packageRoot, "bin", process.platform === "win32" ? "codex.exe" : "codex")
  if (!isFile(executablePath) || !isFile(path.join(packageRoot, "codex-package.json"))) {
    throw new Error(`Unable to locate Codex CLI binary for ${targetTriple}`)
  }
  const codexPathDir = path.join(packageRoot, "codex-path")
  return {
    executablePath,
    pathDirs: isDirectory(codexPathDir) ? [codexPathDir] : [],
  }
}

function cliEnvironment(
  env: Record<string, string | undefined>,
  pathDirs: ReadonlyArray<string>,
): Record<string, string> {
  const next: Record<string, string> = {}
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) next[key] = value
  }
  if (pathDirs.length > 0) {
    next.PATH = `${pathDirs.join(path.delimiter)}${next.PATH ? `${path.delimiter}${next.PATH}` : ""}`
  }
  if (next.CODEX_INTERNAL_ORIGINATOR_OVERRIDE === undefined) {
    next.CODEX_INTERNAL_ORIGINATOR_OVERRIDE = "pylon_codex_human_stream"
  }
  return next
}

function clipOneLine(value: string, max: number): string {
  const oneLine = value.replace(/\s+/g, " ").trim()
  return oneLine.length > max ? `${oneLine.slice(0, max)}…` : oneLine
}

function cleanHumanReasoningLine(value: string): string {
  return value
    .replace(/^\*\*(.+)\*\*$/, "$1")
    .replace(/\s+/g, " ")
    .trim()
}

function isCodexHumanMetadataLine(line: string): boolean {
  return (
    line === "" ||
    line === "--------" ||
    line.startsWith("OpenAI Codex v") ||
    line.startsWith("workdir: ") ||
    line.startsWith("model: ") ||
    line.startsWith("provider: ") ||
    line.startsWith("approval: ") ||
    line.startsWith("sandbox: ") ||
    line.startsWith("reasoning effort: ") ||
    line.startsWith("reasoning summaries: ") ||
    line.startsWith("hook: ") ||
    /^\d{4}-\d{2}-\d{2}T.*\sERROR\s/.test(line)
  )
}

export function createCodexHumanOutputParser(): (line: string) => CodexHumanOutputLine {
  const state: CodexHumanOutputParserState = { phase: "header" }
  return (rawLine: string): CodexHumanOutputLine => {
    const line = rawLine.trim()
    const sessionMatch = /^session id:\s*(\S+)/.exec(line)
    if (sessionMatch?.[1]) return { type: "thread", threadId: sessionMatch[1] }
    if (line === "user") {
      state.phase = "user"
      return null
    }
    if (line === "codex") {
      state.phase = "agent"
      return null
    }
    if (line === "tokens used") {
      state.phase = "tokens"
      return null
    }
    if (line === "hook: UserPromptSubmit Completed") {
      state.phase = "reasoning"
      return null
    }
    if (isCodexHumanMetadataLine(line)) return null

    if (state.phase === "tokens") {
      const totalTokens = Number.parseInt(line.replace(/,/g, ""), 10)
      return Number.isFinite(totalTokens) ? { type: "tokens", totalTokens } : null
    }
    if (state.phase === "agent") {
      return { type: "agent", text: line }
    }
    if (state.phase === "reasoning") {
      const text = cleanHumanReasoningLine(line)
      return text.length > 0 ? { type: "reasoning", text } : null
    }
    return null
  }
}

function summarizeChanges(changes: unknown): string {
  if (!Array.isArray(changes) || changes.length === 0) return "no file paths reported"
  return changes
    .slice(0, 3)
    .map((change) => {
      if (change === null || typeof change !== "object") return "unknown"
      const record = change as Record<string, unknown>
      const kind = typeof record.kind === "string" ? record.kind : "change"
      const path = typeof record.path === "string" ? record.path : "unknown"
      return `${kind} ${path}`
    })
    .join(", ")
}

function completedTodos(items: unknown): string {
  if (!Array.isArray(items)) return "todo list updated"
  const total = items.length
  const done = items.filter((item) => item !== null && typeof item === "object" && (item as { completed?: unknown }).completed === true).length
  return `todo list ${done}/${total}`
}

function textFromContent(content: unknown, blockType: string): string {
  if (typeof content === "string") return content
  if (!Array.isArray(content)) return ""
  const block = content.find((entry: any) => entry?.type === blockType && typeof entry.text === "string")
  return block?.text ?? ""
}

function reasoningSummaryText(summary: unknown): string {
  if (typeof summary === "string") return summary.replace(/\s+/g, " ").trim()
  if (!Array.isArray(summary)) return ""
  return summary
    .map((entry) => {
      if (typeof entry === "string") return entry
      if (entry !== null && typeof entry === "object" && typeof (entry as { text?: unknown }).text === "string") {
        return (entry as { text: string }).text
      }
      return ""
    })
    .join(" ")
    .replace(/\s+/g, " ")
    .trim()
}

function tokenUsageMessage(info: unknown): string | null {
  if (info === null || typeof info !== "object") return null
  const record = info as Record<string, unknown>
  const usage =
    record.last_token_usage !== null && typeof record.last_token_usage === "object"
      ? record.last_token_usage as Record<string, unknown>
      : record.total_token_usage !== null && typeof record.total_token_usage === "object"
        ? record.total_token_usage as Record<string, unknown>
        : record
  const output = numberOrZero(usage.output_tokens)
  const reasoning = numberOrZero(usage.reasoning_output_tokens)
  return output === 0 && reasoning === 0
    ? null
    : `thinking tokens: ${reasoning}; output tokens: ${output}`
}

export function summarizeCodexThreadEvent(raw: unknown): string {
  const event = raw as CodexThreadEvent
  const type = typeof event.type === "string" ? event.type : "event"
  if (type === "thread.started") return "thread started"
  if (type === "turn.started") return "turn started"
  if (type === "turn.completed") return "turn completed"
  if (type === "turn.failed") {
    const message = typeof event.error?.message === "string" ? event.error.message : "turn failed"
    return `turn failed: ${message}`
  }
  if (type === "error") {
    const message = typeof event.message === "string" ? event.message : "stream error"
    return `error: ${message}`
  }

  const payload = event.payload && typeof event.payload === "object" ? event.payload : null
  const payloadType = typeof payload?.type === "string" ? payload.type : ""
  if (type === "event_msg") {
    if (payloadType === "task_started") return "task started"
    if (payloadType === "task_complete") {
      const text = typeof payload?.last_agent_message === "string"
        ? payload.last_agent_message.replace(/\s+/g, " ").trim()
        : ""
      return text.length > 0 ? `agent: ${clipOneLine(text, 200)}` : "task complete"
    }
    if (payloadType === "agent_message") {
      const text = typeof payload?.message === "string"
        ? payload.message.replace(/\s+/g, " ").trim()
        : ""
      return text.length > 0 ? `agent: ${clipOneLine(text, 200)}` : "agent message"
    }
    if (payloadType === "token_count") return tokenUsageMessage(payload?.info) ?? "token count"
  }
  if (type === "response_item" && payload !== null) {
    if (payloadType === "reasoning") {
      const text = reasoningSummaryText(payload.summary)
      return text.length > 0 ? `thinking: ${clipOneLine(text, 1800)}` : "thinking…"
    }
    if (payloadType === "message" && payload.role === "assistant") {
      const text = textFromContent(payload.content, "output_text").replace(/\s+/g, " ").trim()
      return text.length > 0 ? `agent: ${clipOneLine(text, 200)}` : "agent message"
    }
  }

  const item = event.item
  if (!item || typeof item.type !== "string") return type
  // Surface the actual text so a remote viewer (the phone timeline) sees WHAT
  // the agent is doing, not just an opaque "agent message" label. Bounded +
  // whitespace-collapsed; the proof-serialization scanner still redacts secrets.
  const itemText = (() => {
    const t = (item as Record<string, unknown>).text
    if (typeof t === "string") return t.replace(/\s+/g, " ").trim()
    const contentText = textFromContent((item as Record<string, unknown>).content, "output_text")
    return contentText.replace(/\s+/g, " ").trim()
  })()
  if (item.type === "agent_message") return itemText ? `agent: ${clipOneLine(itemText, 200)}` : "agent message"
  if (item.type === "reasoning") {
    const text = itemText || reasoningSummaryText((item as Record<string, unknown>).summary)
    return text ? `thinking: ${clipOneLine(text, 1800)}` : "thinking…"
  }
  if (item.type === "command_execution") {
    const command = typeof item.command === "string" ? item.command : "command"
    const status = typeof item.status === "string" ? item.status : "running"
    const exit = typeof item.exit_code === "number" ? ` exit ${item.exit_code}` : ""
    return `${status}: ${command}${exit}`
  }
  if (item.type === "file_change") {
    const status = typeof item.status === "string" ? item.status : "file change"
    return `${status}: ${summarizeChanges(item.changes)}`
  }
  if (item.type === "mcp_tool_call") {
    const server = typeof item.server === "string" ? item.server : "mcp"
    const tool = typeof item.tool === "string" ? item.tool : "tool"
    const status = typeof item.status === "string" ? item.status : "running"
    return `${status}: ${server}.${tool}`
  }
  if (item.type === "web_search") {
    const query = typeof item.query === "string" ? item.query : "search"
    return `web search: ${query}`
  }
  if (item.type === "todo_list") return completedTodos(item.items)
  if (item.type === "error") {
    const message = typeof item.message === "string" ? item.message : "item error"
    return `error: ${message}`
  }
  return `${type}: ${item.type}`
}

async function runCodexHumanComposerStream(
  prompt: string,
  options: CodexComposerOptions,
  resolved: {
    account: ResolvedPylonAccountSelection | null
    config: CodexAgentConfig
    env: Record<string, string | undefined>
    sandboxMode: CodexComposerSandboxMode
  },
  callbacks: CodexComposerCallbacks,
): Promise<CodexComposerResult> {
  const codexPath = resolveCodexCliPath()
  const cliEnv = cliEnvironment(resolved.env, codexPath.pathDirs)
  const args = [
    "exec",
    "-C",
    options.cwd,
    "--skip-git-repo-check",
    "-s",
    resolved.sandboxMode,
    "--color",
    "never",
    "-c",
    `approval_policy="${options.approvalPolicy ?? "never"}"`,
    "-c",
    `sandbox_workspace_write.network_access=${options.networkAccessEnabled ?? false}`,
    "-c",
    `model_reasoning_summary="detailed"`,
    "-c",
    "show_raw_agent_reasoning=true",
    "-c",
    "hide_agent_reasoning=false",
  ]
  const model = options.model ?? resolved.config.model
  if (model !== undefined) args.push("--model", model)
  args.push("-")

  const abort = new AbortController()
  const abortFromCaller = () => abort.abort()
  if (options.abortSignal?.aborted) abort.abort()
  else options.abortSignal?.addEventListener("abort", abortFromCaller, { once: true })
  const timer = options.timeoutMs === undefined
    ? null
    : setTimeout(() => abort.abort(), options.timeoutMs)

  let eventCount = 0
  let textResult = ""
  let threadId: string | null = null
  let totalTokens = 0
  const emit = (summary: string) => {
    eventCount += 1
    callbacks.onEvent?.(summary, eventCount)
  }

  emit("thread started")
  emit("turn started")

  const child = spawn(codexPath.executablePath, args, {
    env: cliEnv,
    stdio: ["pipe", "pipe", "pipe"],
  })
  if (!child.stdin || !child.stdout || !child.stderr) {
    child.kill()
    throw new Error("Codex exec failed to open stdio pipes")
  }
  const killChild = () => {
    try {
      child.kill("SIGTERM")
    } catch {
      // best effort cancellation
    }
  }
  abort.signal.addEventListener("abort", killChild, { once: true })
  child.stdin.end(prompt)

  child.stdout?.resume()
  const stderrLines: string[] = []
  const exitPromise = new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolve) => {
    child.once("exit", (code, signal) => resolve({ code, signal }))
  })

  const parser = createCodexHumanOutputParser()
  const transcript = readline.createInterface({ input: child.stderr, crlfDelay: Infinity })
  try {
    for await (const line of transcript) {
      stderrLines.push(line)
      const parsed = parser(line)
      if (parsed === null) continue
      if (parsed.type === "thread") {
        threadId = parsed.threadId
        callbacks.onThreadId?.(threadId, stableRef("session.pylon.codex_composer", threadId))
      } else if (parsed.type === "reasoning") {
        emit(`thinking: ${clipOneLine(parsed.text, 1800)}`)
      } else if (parsed.type === "agent") {
        textResult = textResult.length === 0 ? parsed.text : `${textResult}\n${parsed.text}`
        emit(`agent: ${clipOneLine(parsed.text, 200)}`)
        callbacks.onText?.(textResult)
      } else if (parsed.type === "tokens") {
        // Capture the token total for usage accounting (onUsage below), but do
        // NOT emit it as a transcript event: a raw "tokens used: 51342" line is
        // exactly the scrollback noise the readable stream must keep out (it
        // belongs in a footer, not the transcript). #stream-ux 2026-06-19.
        totalTokens = parsed.totalTokens
      }
    }
    const { code, signal } = await exitPromise
    if (abort.signal.aborted) {
      throw new Error(options.abortSignal?.aborted ? "Codex composer cancelled" : "Codex composer timed out")
    }
    if (code !== 0 || signal !== null) {
      const detail = signal === null ? `code ${code ?? 1}` : `signal ${signal}`
      throw new Error(`Codex exec exited with ${detail}: ${stderrLines.join("\n")}`)
    }
    if (totalTokens > 0) {
      callbacks.onUsage?.({ inputTokens: 0, outputTokens: totalTokens, totalTokens })
      if (options.usageStateSummary) {
        await recordPylonAccountUsageObservation(options.usageStateSummary, {
          provider: "codex",
          account: resolved.account,
          localSessionUsage: {
            provider: "codex",
            sessionRef: threadId === null ? null : stableRef("session.pylon.codex_composer", threadId),
            inputTokens: 0,
            outputTokens: totalTokens,
            totalTokens,
          },
        })
      }
    }
    emit("turn completed")
    return {
      commandCount: 0,
      editedFileCount: 0,
      eventCount,
      inputTokens: 0,
      outputTokens: totalTokens,
      text: textResult,
      threadId,
      totalTokens,
    }
  } finally {
    transcript.close()
    if (timer !== null) clearTimeout(timer)
    options.abortSignal?.removeEventListener("abort", abortFromCaller)
    abort.signal.removeEventListener("abort", killChild)
  }
}

export async function runCodexComposerStream(
  prompt: string,
  options: CodexComposerOptions,
  callbacks: CodexComposerCallbacks = {},
): Promise<CodexComposerResult> {
  const config = options.config ?? {}
  const account =
    options.account ??
    (options.accountHome
      ? {
          provider: "codex" as const,
          selector: "direct_home" as const,
          accountRef: options.accountRef ?? null,
          accountRefHash: hashPylonAccountRef("codex", options.accountRef ?? options.accountHome),
          home: options.accountHome,
        }
      : null)
  const accountEnv = pylonAccountEnvironment(
    options.env ?? (Bun.env as Record<string, string | undefined>),
    account,
  )
  const env = installCodexRipgrepGuard({ env: accountEnv }).env
  const executionMode = options.executionMode ?? "local_bounded"
  const sandboxMode = options.sandboxMode ?? sandboxModeForCodexComposerExecutionMode(executionMode, config.sandboxMode)
  if (sandboxMode === "danger-full-access" && executionMode !== "local_supervised_danger") {
    throw new Error(
      `danger-full-access requires local_supervised_danger (${CODEX_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF})`,
    )
  }
  const readiness = await probeCodexAgentReadiness({
    config,
    env,
    importer: options.importer,
    platform: options.platform,
    codexCliLoginPresent: options.codexCliLoginPresent,
  })
  if (readiness.state !== "ready") {
    const blockers = readiness.blockerRefs.length > 0 ? ` (${readiness.blockerRefs.join(", ")})` : ""
    throw new Error(`Codex composer unavailable: ${readiness.state}${blockers}`)
  }
  if (options.humanReadableReasoning === true && options.importer === undefined) {
    return runCodexHumanComposerStream(
      prompt,
      options,
      { account, config, env, sandboxMode },
      callbacks,
    )
  }

  const importer = options.importer ?? ((specifier: string) => import(specifier))
  const sdk = (await importer(CODEX_AGENT_SDK_PACKAGE)) as CodexSdkModule
  const abort = new AbortController()
  const abortFromCaller = () => abort.abort()
  if (options.abortSignal?.aborted) abort.abort()
  else options.abortSignal?.addEventListener("abort", abortFromCaller, { once: true })
  const timer = options.timeoutMs === undefined
    ? null
    : setTimeout(() => abort.abort(), options.timeoutMs)
  let textResult = ""
  let eventCount = 0
  let inputTokens = 0
  let outputTokens = 0
  let commandCount = 0
  let editedFileCount = 0
  let threadId: string | null = null

  try {
    const codex = new sdk.Codex({
      env,
      config: {
        model_reasoning_summary: "detailed",
        show_raw_agent_reasoning: true,
      },
    })
    const thread = codex.startThread({
      workingDirectory: options.cwd,
      sandboxMode,
      approvalPolicy: options.approvalPolicy ?? "never",
      skipGitRepoCheck: true,
      networkAccessEnabled: options.networkAccessEnabled ?? false,
      ...(options.model ?? config.model ? { model: options.model ?? config.model } : {}),
    })
    const { events } = await thread.runStreamed(prompt, { signal: abort.signal })
    for await (const raw of events) {
      eventCount += 1
      const event = raw as CodexThreadEvent
      const providerSnapshots = providerRateLimitSnapshotsFromEvent("codex", raw)
      if (options.usageStateSummary && providerSnapshots.length > 0) {
        await recordPylonAccountUsageObservation(options.usageStateSummary, {
          provider: "codex",
          account,
          providerSnapshots,
        })
      }
      callbacks.onEvent?.(summarizeCodexThreadEvent(raw), eventCount)
      if (event.type === "thread.started" && typeof event.thread_id === "string") {
        threadId = event.thread_id
        callbacks.onThreadId?.(threadId, stableRef("session.pylon.codex_composer", threadId))
      }
      if (event.type === "turn.completed") {
        inputTokens = numberOrZero(event.usage?.input_tokens)
        outputTokens = numberOrZero(event.usage?.output_tokens)
        callbacks.onUsage?.({ inputTokens, outputTokens, totalTokens: inputTokens + outputTokens })
        if (options.usageStateSummary) {
          await recordPylonAccountUsageObservation(options.usageStateSummary, {
            provider: "codex",
            account,
            localSessionUsage: {
              provider: "codex",
              sessionRef: threadId === null ? null : stableRef("session.pylon.codex_composer", threadId),
              inputTokens,
              outputTokens,
              totalTokens: inputTokens + outputTokens,
            },
          })
        }
      }
      if (event.type === "item.completed" && event.item?.type === "agent_message" && typeof event.item.text === "string") {
        textResult = event.item.text
        callbacks.onText?.(textResult)
      }
      if (event.type === "item.completed" && event.item?.type === "command_execution") {
        commandCount += 1
      }
      if (event.type === "item.completed" && event.item?.type === "file_change") {
        editedFileCount += Array.isArray(event.item.changes) ? event.item.changes.length : 0
      }
      if (event.type === "turn.failed") {
        const message = typeof event.error?.message === "string" ? event.error.message : "turn failed"
        throw new Error(`Codex turn failed: ${message}`)
      }
      if (event.type === "error") {
        const message = typeof event.message === "string" ? event.message : "stream error"
        throw new Error(`Codex stream error: ${message}`)
      }
    }
  } catch (error) {
    if (abort.signal.aborted) {
      throw new Error(options.abortSignal?.aborted ? "Codex composer cancelled" : "Codex composer timed out")
    }
    throw error
  } finally {
    if (timer !== null) clearTimeout(timer)
    options.abortSignal?.removeEventListener("abort", abortFromCaller)
  }

  return {
    text: textResult.trim(),
    eventCount,
    inputTokens,
    outputTokens,
    totalTokens: inputTokens + outputTokens,
    commandCount,
    editedFileCount,
    threadId,
  }
}
