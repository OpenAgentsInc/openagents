import { Effect } from "effect"
import { appendAssistantTurn, prepareUserTurn } from "./bounds.js"
import { KHALA_CLI_VERSION, formatKhalaChangelog } from "./changelog.js"
import { fetchModels, fetchTokensServed, mintFreeKey, runChatTurn, submitFeedback, toKhalaCliError } from "./client.js"
import {
  connectKhalaCodex,
  resolveKhalaCodexStatus,
  runKhalaCodexTask,
  selectKhalaRoute,
  type KhalaCodexDisplayEvent,
  type KhalaCodexStatus,
} from "./codex.js"
import { appendPromptHistory, readPromptFromTerminal } from "./input.js"
import { renderMarkdownForTerminal, renderReasoningMarkdownDeltaForTerminal, terminalStyle } from "./terminal.js"
import { ensureStoredAgentToken, traceTokenPath } from "./token-store.js"
import { DEFAULT_BASE_URL, type ChatMode, type ChatTurnMetadata, type KhalaChatMessage, type KhalaCliError, type KhalaTokensResponse } from "./types.js"
import { startKhalaAutoUpdate } from "./updater.js"

type ParsedCommand =
  | { readonly kind: "chat" }
  | { readonly kind: "codex"; readonly text: string | undefined }
  | { readonly kind: "codexAuth" }
  | { readonly kind: "codexStatus" }
  | { readonly kind: "changelog" }
  | { readonly kind: "feedback"; readonly text: string | undefined }
  | { readonly kind: "help" }
  | { readonly kind: "info" }
  | { readonly kind: "tokens" }
  | { readonly kind: "version" }

interface ParsedArgs {
  readonly help: boolean
  readonly version: boolean
  readonly command: ParsedCommand
  readonly mode: ChatMode
  readonly baseUrl: string
  readonly token: string | undefined
  readonly prompt: string | undefined
  readonly headless: boolean
  readonly json: boolean
  readonly models: boolean
  readonly mintFreeKey: boolean
}

export async function runKhalaCli(argv: ReadonlyArray<string>, env: Record<string, string | undefined> = Bun.env): Promise<number> {
  let args: ParsedArgs
  try {
    args = parseArgs(argv, env)
  } catch (error) {
    printError(toKhalaCliError(error, "Could not parse arguments."))
    return 2
  }

  if (args.help) {
    process.stdout.write(usage())
    return 0
  }
  if (args.version) {
    process.stdout.write(`khala ${KHALA_CLI_VERSION}\n`)
    return 0
  }

  try {
    if (args.models) {
      const models = await Effect.runPromise(fetchModels({ baseUrl: args.baseUrl }))
      process.stdout.write(`${JSON.stringify(models, null, 2)}\n`)
      return 0
    }
    if (args.mintFreeKey) {
      const payload = await Effect.runPromise(mintFreeKey({ baseUrl: args.baseUrl }))
      process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`)
      return 0
    }
    if (args.command.kind === "changelog") {
      process.stdout.write(`${formatKhalaChangelog()}\n`)
      return 0
    }
    if (args.command.kind === "help") {
      process.stdout.write(usage())
      return 0
    }
    if (args.command.kind === "codexAuth") {
      const connected = await connectKhalaCodex({ env })
      process.stdout.write(
        connected.status === "already_connected"
          ? `Codex already connected: ${connected.codexHome}\n`
          : `Codex connected: ${connected.codexHome}\n`,
      )
      return 0
    }
    if (args.command.kind === "codexStatus") {
      process.stdout.write(`${formatCodexStatus(await resolveKhalaCodexStatus(env))}\n`)
      return 0
    }
    if (args.command.kind === "codex") {
      const prompt = args.command.text?.trim()
      if (prompt === undefined || prompt.length === 0) {
        throw new Error("khala codex requires a task, for example: khala codex \"read README.md\"")
      }
      const result = await runKhalaCodexTask({
        cwd: process.cwd(),
        env,
        prompt,
        onEvent: args.json ? undefined : event => writeCodexEvent(event),
      })
      if (args.json) {
        process.stdout.write(`${JSON.stringify({ kind: "local_codex", ...result })}\n`)
      } else if (result.text.trim().length === 0) {
        process.stdout.write(`${terminalStyle.assistant("Khala:")} Codex completed with no final text.\n`)
      } else {
        process.stdout.write("\n")
      }
      return 0
    }
    if (args.command.kind === "info") {
      process.stdout.write(`${await formatInfo({
        args,
        env,
        lastMessageInfo: undefined,
        lastTraceRef: undefined,
        sessionId: createCliSessionId(),
      })}\n`)
      return 0
    }
    if (args.command.kind === "tokens") {
      const tokens = await Effect.runPromise(fetchTokensServed({ baseUrl: args.baseUrl }))
      process.stdout.write(args.json ? `${JSON.stringify(tokens)}\n` : `${formatTokensServed(tokens)}\n`)
      return 0
    }
    if (args.command.kind === "version") {
      process.stdout.write(`khala ${KHALA_CLI_VERSION}\n`)
      return 0
    }
    if (args.command.kind === "feedback") {
      const feedback = args.command.text?.trim()
      if (feedback === undefined || feedback.length === 0) {
        throw new Error("khala feedback requires a message, for example: khala feedback \"the input ate my transcript\"")
      }
      const response = await Effect.runPromise(submitFeedback({
        baseUrl: args.baseUrl,
        clientVersion: KHALA_CLI_VERSION,
        feedback,
        source: "khala-cli",
      }))
      if (args.json) {
        process.stdout.write(`${JSON.stringify(response)}\n`)
      } else {
        process.stdout.write(`Feedback saved: ${response.feedbackRef}\n`)
      }
      return 0
    }

    if (shouldRunHeadless(args)) {
      const prompt = args.prompt ?? await readStdinPrompt()
      const text = await runOneTurn(
        args,
        [],
        prompt,
        args.json ? undefined : (delta) => process.stdout.write(delta),
        args.json ? undefined : (delta) => process.stderr.write(renderReasoningMarkdownDeltaForTerminal(delta)),
      )
      if (args.json) {
        process.stdout.write(`${JSON.stringify({ text })}\n`)
      } else {
        process.stdout.write("\n")
      }
      return 0
    }

    if (!process.stdin.isTTY || !process.stdout.isTTY) {
      throw new Error("Interactive Khala requires a TTY. Pass --headless or --prompt for programmatic use.")
    }

    await runInteractive(args, env)
    return 0
  } catch (error) {
    printError(toKhalaCliError(error, "Khala CLI failed."))
    return 1
  }
}

function parseArgs(argv: ReadonlyArray<string>, env: Record<string, string | undefined>): ParsedArgs {
  let help = false
  let version = false
  let mode: ChatMode = "public"
  let baseUrl = env.KHALA_BASE_URL ?? DEFAULT_BASE_URL
  let token = env.OPENAGENTS_AGENT_TOKEN
  let prompt: string | undefined
  let headless = false
  let json = false
  let models = false
  let mintFreeKey = false
  const positional: Array<string> = []

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === "--") continue
    if (arg === "--help" || arg === "-h") help = true
    else if (arg === "--version") version = true
    else if (arg === "--public") mode = "public"
    else if (arg === "--api") mode = "api"
    else if (arg === "--headless") headless = true
    else if (arg === "--json" || arg === "--no-stream") json = true
    else if (arg === "--models") models = true
    else if (arg === "--mint-free-key") mintFreeKey = true
    else if (arg === "--base-url") {
      baseUrl = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--base-url=")) {
      baseUrl = arg.slice("--base-url=".length)
    } else if (arg === "--token") {
      token = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--token=")) {
      token = arg.slice("--token=".length)
    } else if (arg === "--prompt" || arg === "-p") {
      prompt = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--prompt=")) {
      prompt = arg.slice("--prompt=".length)
    } else if (arg.startsWith("-")) {
      throw new Error(`Unknown flag: ${arg}`)
    } else {
      positional.push(arg)
    }
  }

  let command: ParsedCommand = { kind: "chat" }
  const maybeCommand = positional[0]
  if (maybeCommand === "feedback") {
    command = {
      kind: "feedback",
      text: positional.slice(1).join(" ") || prompt,
    }
  } else if (maybeCommand === "changelog") {
    command = { kind: "changelog" }
  } else if (maybeCommand === "auth" && positional[1] === "codex") {
    command = { kind: "codexAuth" }
  } else if (maybeCommand === "codex") {
    command = positional[1] === "status"
      ? { kind: "codexStatus" }
      : { kind: "codex", text: positional.slice(1).join(" ") || prompt }
  } else if (maybeCommand === "help") {
    command = { kind: "help" }
  } else if (maybeCommand === "info") {
    command = { kind: "info" }
  } else if (maybeCommand === "tokens") {
    command = { kind: "tokens" }
  } else if (maybeCommand === "version") {
    command = { kind: "version" }
  } else if (prompt === undefined && positional.length > 0) {
    prompt = positional.join(" ")
  }

  return {
    command,
    help,
    version,
    mode,
    baseUrl,
    token,
    prompt,
    headless,
    json,
    models,
    mintFreeKey,
  }
}

async function runInteractive(args: ParsedArgs, env: Record<string, string | undefined>): Promise<void> {
  let messages: ReadonlyArray<KhalaChatMessage> = []
  let promptHistory: ReadonlyArray<string> = []
  let lastTraceRef: string | undefined
  let lastMessageInfo: ChatTurnMetadata | undefined
  const sessionId = createCliSessionId()
  process.stdout.write(
    `Khala CLI v${KHALA_CLI_VERSION}. Type /help for commands, /exit to quit.\n\n`,
  )
  startKhalaAutoUpdate({
    currentVersion: KHALA_CLI_VERSION,
    env,
    notify: line => process.stdout.write(`\n${terminalStyle.assistant("Khala:")} ${line}\n\n`),
  })
  while (true) {
    const prompt = await readPromptFromTerminal(terminalStyle.user("> "), {
      history: promptHistory,
    })
    if (prompt === null) {
      process.stdout.write("\n")
      return
    }
    if (prompt.trim() === "/exit") {
      return
    }
    if (prompt.trim().length === 0) {
      continue
    }
    promptHistory = appendPromptHistory(promptHistory, prompt)
    if (prompt.trim().startsWith("/")) {
      const outcome = await handleSlashCommand(args, env, prompt.trim(), lastTraceRef, lastMessageInfo, sessionId)
      if (outcome === "exit") return
      continue
    }

    const routed = await maybeRunLocalCodexTurn(args, env, messages, prompt)
    if (routed.handled) {
      messages = appendAssistantTurn(
        prepareUserTurn(messages, prompt),
        routed.text,
      )
      continue
    }

    const prepared = prepareUserTurn(messages, prompt)
    const waitingDots = startWaitingDots()
    const beforeTurnOutput = (): void => {
      waitingDots.stop()
    }
    try {
      let streamed = false
      let reasoningStreamed = false
      let markdownStreamBuffer = ""
      let reasoningStreamBuffer = ""
      const result = await Effect.runPromise(runChatTurn({
        mode: args.mode,
        baseUrl: args.baseUrl,
        token: args.token,
        messages: prepared,
        onDelta: (delta) => {
          beforeTurnOutput()
          if (!streamed) {
            if (reasoningStreamed) {
              process.stdout.write("\n")
            }
            process.stdout.write(`${terminalStyle.assistant("Khala:")} `)
            streamed = true
          }
          const rendered = renderStreamingMarkdownDelta(markdownStreamBuffer, delta)
          markdownStreamBuffer = rendered.buffer
          process.stdout.write(rendered.text)
        },
        onReasoning: (delta) => {
          beforeTurnOutput()
          if (!reasoningStreamed) {
            process.stdout.write(`${terminalStyle.meta("Khala reasoning:")} `)
            reasoningStreamed = true
          }
          const rendered = renderStreamingMarkdownDelta(reasoningStreamBuffer, delta)
          reasoningStreamBuffer = rendered.buffer
          process.stdout.write(terminalStyle.reasoning(rendered.text))
        },
        onRetry: (event) => {
          beforeTurnOutput()
          process.stdout.write(`${streamed || reasoningStreamed ? "\n" : ""}${terminalStyle.assistant("Khala:")} ${formatRetryNotice(event)}\n`)
        },
      }))
      beforeTurnOutput()
      if (streamed && markdownStreamBuffer.length > 0) {
        process.stdout.write(renderMarkdownForTerminal(markdownStreamBuffer))
      }
      if (reasoningStreamed && reasoningStreamBuffer.length > 0) {
        process.stdout.write(terminalStyle.reasoning(renderMarkdownForTerminal(reasoningStreamBuffer)))
      }
      if (streamed || reasoningStreamed) {
        process.stdout.write("\n\n")
      } else {
        process.stdout.write(`${terminalStyle.assistant("Khala:")} ${renderMarkdownForTerminal(result.text)}\n\n`)
      }
      messages = appendAssistantTurn(prepared, result.text)
      lastTraceRef = result.traceRef ?? lastTraceRef
      lastMessageInfo = result.metadata
    } catch (error) {
      beforeTurnOutput()
      const khalaError = toKhalaCliError(error, "Khala turn failed.")
      lastTraceRef = khalaError.traceRef ?? lastTraceRef
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatInteractiveError(khalaError)}\n\n`)
    }
  }
}

async function handleSlashCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  input: string,
  lastTraceRef: string | undefined,
  lastMessageInfo: ChatTurnMetadata | undefined,
  sessionId: string,
): Promise<"handled" | "exit"> {
  const [command = "", ...rest] = input.split(/\s+/)
  const argument = rest.join(" ").trim()

  if (command === "/exit" || command === "/quit") {
    return "exit"
  }
  if (command === "/help") {
    process.stdout.write(`${interactiveHelp()}\n`)
    return "handled"
  }
  if (command === "/version") {
    process.stdout.write(`${terminalStyle.assistant("Khala:")} khala ${KHALA_CLI_VERSION}\n\n`)
    return "handled"
  }
  if (command === "/codex") {
    if (argument === "status") {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatCodexStatus(await resolveKhalaCodexStatus(env))}\n\n`)
      return "handled"
    }
    if (argument === "connect" || argument === "auth") {
      try {
        const connected = await connectKhalaCodex({ env })
        process.stdout.write(`${terminalStyle.assistant("Khala:")} ${
          connected.status === "already_connected"
            ? `Codex already connected: ${connected.codexHome}`
            : `Codex connected: ${connected.codexHome}`
        }\n\n`)
      } catch (error) {
        process.stdout.write(`${terminalStyle.assistant("Khala:")} ${String(error instanceof Error ? error.message : error)}\n\n`)
      }
      return "handled"
    }
    if (argument.length === 0) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Usage: /codex <task>, /codex status, or /codex connect\n\n`)
      return "handled"
    }
    try {
      const result = await runKhalaCodexTask({
        cwd: process.cwd(),
        env,
        prompt: argument,
        onEvent: event => writeCodexEvent(event),
      })
      if (result.text.trim().length === 0) {
        process.stdout.write(`${terminalStyle.assistant("Khala:")} Codex completed with no final text.\n\n`)
      } else {
        process.stdout.write("\n")
      }
    } catch (error) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${String(error instanceof Error ? error.message : error)}\n\n`)
    }
    return "handled"
  }
  if (command === "/msginfo") {
    process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatMessageInfo(lastMessageInfo)}\n\n`)
    return "handled"
  }
  if (command === "/info") {
    process.stdout.write(`${terminalStyle.assistant("Khala:")} ${await formatInfo({
      args,
      env,
      lastMessageInfo,
      lastTraceRef,
      sessionId,
    })}\n\n`)
    return "handled"
  }
  if (command === "/changelog") {
    process.stdout.write(`${formatKhalaChangelog()}\n\n`)
    return "handled"
  }
  if (command === "/tokens") {
    try {
      const tokens = await Effect.runPromise(fetchTokensServed({ baseUrl: args.baseUrl }))
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatTokensServed(tokens)}\n\n`)
    } catch (error) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatInteractiveError(toKhalaCliError(error, "Tokens fetch failed."))}\n\n`)
    }
    return "handled"
  }
  if (command === "/feedback") {
    if (argument.length === 0) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Usage: /feedback <message>\n\n`)
      return "handled"
    }
    try {
      const response = await Effect.runPromise(submitFeedback({
        baseUrl: args.baseUrl,
        clientVersion: KHALA_CLI_VERSION,
        feedback: argument,
        source: "khala-cli-interactive",
        ...(lastTraceRef === undefined ? {} : { traceRef: lastTraceRef }),
      }))
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Feedback saved: ${response.feedbackRef}\n\n`)
    } catch (error) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatInteractiveError(toKhalaCliError(error, "Feedback failed."))}\n\n`)
    }
    return "handled"
  }

  process.stdout.write(`${terminalStyle.assistant("Khala:")} Unknown command ${command}. Try /help.\n\n`)
  return "handled"
}

async function runOneTurn(
  args: ParsedArgs,
  history: ReadonlyArray<KhalaChatMessage>,
  prompt: string,
  onDelta?: (text: string) => void,
  onReasoning?: (text: string) => void,
): Promise<string> {
  const routed = await maybeRunLocalCodexTurn(args, Bun.env, history, prompt, {
    silent: args.json,
    onEvent: onDelta === undefined
      ? undefined
      : event => {
          if (event.kind === "message") onDelta(event.text)
          else if (event.kind === "reasoning") onReasoning?.(event.text)
        },
  })
  if (routed.handled) return routed.text
  const messages = prepareUserTurn(history, prompt)
  const result = await Effect.runPromise(runChatTurn({
    mode: args.mode,
    baseUrl: args.baseUrl,
    token: args.token,
    messages,
    onDelta,
    onReasoning,
    onRetry: (event) => {
      process.stderr.write(`khala: ${formatRetryNotice(event)}\n`)
    },
  }))
  return result.text
}

async function maybeRunLocalCodexTurn(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  history: ReadonlyArray<KhalaChatMessage>,
  prompt: string,
  options: {
    readonly onEvent?: ((event: KhalaCodexDisplayEvent) => void) | undefined
    readonly silent?: boolean | undefined
  } = {},
): Promise<{ readonly handled: false } | { readonly handled: true; readonly text: string }> {
  const selection = await selectKhalaRoute({
    baseUrl: args.baseUrl,
    env,
    history,
    mode: args.mode,
    prompt,
    token: args.token,
  })
  if (selection.route !== "local_codex") {
    return { handled: false }
  }
  const status = await resolveKhalaCodexStatus(env)
  if (!status.ready) {
    const message = status.blocker === "codex_sdk_missing"
      ? "Local Codex routing selected, but this Khala build is missing @openai/codex-sdk."
      : "Local Codex routing selected, but Codex is not connected. Run `khala auth codex` or `/codex connect`."
    if (options.silent !== true) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${message}\n\n`)
    }
    return { handled: true, text: message }
  }
  if (options.silent !== true) {
    process.stdout.write(`${terminalStyle.assistant("Khala:")} ${terminalStyle.meta(`Blueprint selected local Codex: ${selection.reason}`)}\n`)
  }
  const result = await runKhalaCodexTask({
    cwd: process.cwd(),
    env,
    prompt,
    onEvent: options.silent === true ? undefined : options.onEvent ?? (event => writeCodexEvent(event)),
  })
  if (options.silent === true) {
    return { handled: true, text: result.text }
  }
  if (result.text.trim().length === 0) {
    process.stdout.write(`${terminalStyle.assistant("Khala:")} Codex completed with no final text.\n\n`)
  } else {
    process.stdout.write("\n")
  }
  return { handled: true, text: result.text }
}

async function readStdinPrompt(): Promise<string> {
  return await new Response(Bun.stdin.stream()).text()
}

function shouldRunHeadless(args: ParsedArgs): boolean {
  return args.headless || args.prompt !== undefined || args.json || !process.stdin.isTTY
}

function requireValue(argv: ReadonlyArray<string>, index: number, flag: string): string {
  const value = argv[index + 1]
  if (value === undefined || value.startsWith("-")) {
    throw new Error(`${flag} requires a value`)
  }
  return value
}

function printError(error: KhalaCliError): void {
  const prefix = error.statusCode === 429
    ? "rate limited"
    : error.statusCode === 502
      ? "inference unavailable"
      : "error"
  process.stderr.write(`khala: ${prefix}: ${humanReadableReason(error)}${formatTraceSuffix(error)}\n`)
}

function formatInteractiveError(error: KhalaCliError): string {
  if (error.statusCode === 429) {
    return "Rate limited after retrying. Please wait a moment and try again."
  }
  if (error.statusCode === 502 || error.statusCode === 503 || error.statusCode === 504 || error.code === "inference_unavailable") {
    const backend = error.reason === "inference_unavailable" ? "" : ` Backend: ${error.reason}.`
    return `Inference is unavailable after retrying.${backend}${formatTraceSuffix(error)}`
  }
  return `${humanReadableReason(error)}${formatTraceSuffix(error)}`
}

function humanReadableReason(error: KhalaCliError): string {
  if (error.reason === "inference_unavailable" || error.code === "inference_unavailable") {
    return "Inference is unavailable after retrying. Please try again in a moment."
  }
  return error.reason
}

function formatTraceSuffix(error: KhalaCliError): string {
  return error.traceRef === undefined ? "" : ` Trace: ${error.traceRef}.`
}

function formatDelay(delayMs: number): string {
  if (delayMs < 1_000) return `${delayMs}ms`
  return `${(delayMs / 1_000).toFixed(1)}s`
}

function formatRetryNotice(event: {
  readonly retry: number
  readonly maxRetries: number
  readonly delayMs: number
}): string {
  return `inference unavailable; retrying ${event.retry}/${event.maxRetries} in ${formatDelay(event.delayMs)}...`
}

function startWaitingDots(): { readonly stop: () => void } {
  let printed = 0
  const timer = setInterval(() => {
    printed += 1
    process.stdout.write(".")
  }, 1_000)
  return {
    stop: () => {
      clearInterval(timer)
      if (printed > 0) {
        process.stdout.write("\n")
      }
    },
  }
}

function formatTokensServed(tokens: KhalaTokensResponse): string {
  const formatted = new Intl.NumberFormat().format(tokens.tokensServed)
  return `Khala tokens served: ${formatted}`
}

function renderStreamingMarkdownDelta(
  previousBuffer: string,
  delta: string,
): { readonly buffer: string; readonly text: string } {
  const combined = `${previousBuffer}${delta}`.replace(/\r\n/g, "\n").replace(/\r/g, "\n")
  const lastNewline = combined.lastIndexOf("\n")
  if (lastNewline < 0) {
    return { buffer: combined, text: "" }
  }
  const complete = combined.slice(0, lastNewline + 1)
  const buffer = combined.slice(lastNewline + 1)
  return { buffer, text: renderMarkdownForTerminal(complete) }
}

function formatMessageInfo(info: ChatTurnMetadata | undefined): string {
  if (info === undefined) {
    return terminalStyle.meta("No Khala message metadata yet.")
  }
  const usageNote = info.estimatedUsage ? "estimated" : "provider-reported"
  const lines = [
    "Last message metadata:",
    `trace: ${info.traceRef ?? "not reported"}`,
    `orchestrator: ${info.requestedModel ?? "openagents/khala"}`,
    `backend model: ${info.servedModel ?? "not reported"}`,
    `backend adapter: ${info.servedAdapterId ?? "not reported"}`,
    `primary adapter: ${info.primaryAdapterId ?? "same as backend or not reported"}`,
    `route: ${formatFallback(info)}`,
    `tokens: ${info.usage.totalTokens} total (${info.usage.promptTokens} prompt, ${info.usage.completionTokens} completion${info.usage.cachedPromptTokens === undefined ? "" : `, ${info.usage.cachedPromptTokens} cached`}; ${usageNote})`,
    `latency: first byte ${formatOptionalMs(info.timeToFirstByteMs)}, first token ${formatOptionalMs(info.timeToFirstTokenMs)}, stream ${formatOptionalMs(info.streamDurationMs)}, total ${formatMs(info.durationMs)}`,
    `speed: ${info.tokensPerSecond === undefined ? "not measured" : `${info.tokensPerSecond.toFixed(1)} tok/s`} over ${formatMs(info.streamDurationMs ?? info.durationMs)}`,
    `finish: ${info.finishReason ?? "not reported"}`,
  ]
  return terminalStyle.meta(lines.join("\n"))
}

function formatOptionalMs(value: number | undefined): string {
  return value === undefined ? "not measured" : formatMs(value)
}

function formatMs(value: number): string {
  if (value < 1_000) return `${Math.round(value)}ms`
  return `${(value / 1_000).toFixed(2)}s`
}

function formatFallback(info: ChatTurnMetadata): string {
  if (info.fallbackReason === undefined || info.fallbackReason === null) {
    return "primary backend, no fallback reported"
  }
  const primary = info.primaryAdapterId ?? "primary adapter"
  const backend = info.servedAdapterId ?? "fallback adapter"
  return `${primary} reported ${info.fallbackReason}; Khala used ${backend}`
}

function formatCodexStatus(status: KhalaCodexStatus): string {
  if (status.ready) {
    return terminalStyle.meta([
      "Local Codex: connected",
      `home: ${status.codexHome}`,
      `credentials: ${formatCodexCredentialSource(status.credentialSource)}`,
      "workspace delegation: enabled",
    ].join("\n"))
  }
  const lines = [
    "Local Codex: not connected",
    `home: ${status.codexHome}`,
    `sdk: ${status.sdk}`,
    status.blocker === "codex_sdk_missing"
      ? "fix: install a Khala build that includes @openai/codex-sdk"
      : "fix: run khala auth codex or /codex connect",
  ]
  return terminalStyle.meta(lines.join("\n"))
}

function formatCodexCredentialSource(source: Extract<KhalaCodexStatus, { readonly ready: true }>["credentialSource"]): string {
  if (source === "khala_codex_home") return "Khala Codex account"
  if (source === "codex_home_env") return "CODEX_HOME"
  if (source === "default_codex_home") return "~/.codex"
  return "Pylon Codex account"
}

function writeCodexEvent(event: KhalaCodexDisplayEvent): void {
  if (event.kind === "message") {
    process.stdout.write(`${renderMarkdownForTerminal(event.text)}\n`)
    return
  }
  if (event.kind === "reasoning") {
    process.stdout.write(`${terminalStyle.reasoning(renderMarkdownForTerminal(event.text))}\n`)
    return
  }
  if (event.kind === "command") {
    process.stdout.write(`${terminalStyle.meta(`Codex: ${event.text}`)}\n`)
    return
  }
  if (event.kind === "file_change") {
    process.stdout.write(`${terminalStyle.meta(`Codex: ${event.text}`)}\n`)
    return
  }
  process.stdout.write(`${terminalStyle.meta(`Codex: ${event.text}`)}\n`)
}

async function formatInfo(input: {
  readonly args: ParsedArgs
  readonly env: Record<string, string | undefined>
  readonly lastMessageInfo: ChatTurnMetadata | undefined
  readonly lastTraceRef: string | undefined
  readonly sessionId: string
}): Promise<string> {
  const lines = [
    "Current Khala session:",
    `thread: ${input.sessionId}`,
    `last request trace: ${input.lastMessageInfo?.traceRef ?? input.lastTraceRef ?? "not reported yet"}`,
  ]
  const traceUrl = resolveTraceUrl(input.args.baseUrl, input.lastMessageInfo)
  if (traceUrl !== undefined) {
    lines.push(`trace: ${traceUrl}`)
    return terminalStyle.meta(lines.join("\n"))
  }

  try {
    const token = await ensureStoredAgentToken({
      baseUrl: input.args.baseUrl,
      env: input.env,
      explicitToken: input.args.token,
    })
    lines.push(`traces: ${baseUrlFor(input.args.baseUrl)}/traces?token=${encodeURIComponent(token)}`)
    lines.push(`token store: ${traceTokenPath(input.env)}`)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lines.push(`traces: unavailable (${message})`)
  }
  return terminalStyle.meta(lines.join("\n"))
}

function resolveTraceUrl(baseUrl: string, info: ChatTurnMetadata | undefined): string | undefined {
  if (info?.traceUrl !== undefined) {
    if (info.traceUrl.startsWith("http://") || info.traceUrl.startsWith("https://")) return info.traceUrl
    return `${baseUrlFor(baseUrl)}${info.traceUrl.startsWith("/") ? "" : "/"}${info.traceUrl}`
  }
  if (info?.traceUuid !== undefined) {
    return `${baseUrlFor(baseUrl)}/trace/${encodeURIComponent(info.traceUuid)}`
  }
  return undefined
}

function baseUrlFor(baseUrl: string): string {
  return baseUrl.replace(/\/+$/, "")
}

function createCliSessionId(): string {
  return `khala_cli.${crypto.randomUUID()}`
}

function interactiveHelp(): string {
  return `${terminalStyle.meta("Slash commands:")}
/help              Show this command list
/feedback <text>   Save product feedback with the last trace when available
/info              Show this CLI session and trace viewing link
/msginfo           Show metadata for the last Khala response
/codex status      Show local Codex connection status
/codex connect     Connect Codex to Khala with device auth
/codex <task>      Delegate a workspace task directly to Codex
/tokens            Show global Khala tokens served
/changelog         Show recent Khala CLI changes
/version           Show the installed Khala CLI version
/exit, /quit       Quit
`
}

function usage(): string {
  return `Khala CLI

Usage:
  khala
  khala help
  khala info
  khala version
  khala changelog
  khala tokens
  khala auth codex
  khala codex status
  khala codex "read README.md"
  khala feedback "The transcript disappeared"
  khala --prompt "Say hello"
  khala --headless --json < prompt.txt
  khala --api --token "$OPENAGENTS_AGENT_TOKEN" --prompt "Hello"

Interactive commands:
  /help              Show slash commands
  /feedback <text>   Save product feedback without sending it to inference
  /info              Show this CLI session and trace viewing link
  /msginfo           Show metadata for the last Khala response
  /codex status      Show local Codex connection status
  /codex connect     Connect Codex to Khala with device auth
  /codex <task>      Delegate a workspace task directly to Codex
  /tokens            Show global Khala tokens served
  /changelog         Show recent Khala CLI changes
  /version           Show the installed Khala CLI version
  /exit              Quit

Flags:
  --public             Use /api/khala/chat (default, no auth)
  --api                Use /api/v1/chat/completions with openagents/khala
  --base-url <url>     Override ${DEFAULT_BASE_URL}
  --token <token>      Bearer token for --api (env: OPENAGENTS_AGENT_TOKEN)
  --prompt, -p <text>  Run one headless prompt
  --headless           Read one prompt from stdin when --prompt is absent
  --json, --no-stream  Print final JSON instead of streaming deltas
  --models             Print /api/v1/models JSON
  --mint-free-key      Call POST /api/keys/free and print the response once
  --help, -h           Show this help
`
}
