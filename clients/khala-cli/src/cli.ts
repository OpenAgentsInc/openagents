import { Effect } from "effect"
import { appendAssistantTurn, prepareUserTurn } from "./bounds.js"
import { KHALA_CLI_VERSION, formatKhalaChangelog } from "./changelog.js"
import { fetchModels, fetchTokensServed, mintFreeKey, runChatTurn, submitFeedback, toKhalaCliError } from "./client.js"
import { readPromptFromTerminal } from "./input.js"
import { DEFAULT_BASE_URL, type ChatMode, type KhalaChatMessage, type KhalaCliError, type KhalaTokensResponse } from "./types.js"
import { startKhalaAutoUpdate } from "./updater.js"

type ParsedCommand =
  | { readonly kind: "chat" }
  | { readonly kind: "changelog" }
  | { readonly kind: "feedback"; readonly text: string | undefined }
  | { readonly kind: "tokens" }

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
    if (args.command.kind === "tokens") {
      const tokens = await Effect.runPromise(fetchTokensServed({ baseUrl: args.baseUrl }))
      process.stdout.write(args.json ? `${JSON.stringify(tokens)}\n` : `${formatTokensServed(tokens)}\n`)
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
  } else if (maybeCommand === "tokens") {
    command = { kind: "tokens" }
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
  let lastTraceRef: string | undefined
  process.stdout.write("Khala CLI. Type /exit to quit.\n\n")
  startKhalaAutoUpdate({
    currentVersion: KHALA_CLI_VERSION,
    env,
    notify: line => process.stdout.write(`\nKhala: ${line}\n\n`),
  })
  while (true) {
    const prompt = await readPromptFromTerminal()
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
    if (prompt.trim().startsWith("/")) {
      const outcome = await handleSlashCommand(args, prompt.trim(), lastTraceRef)
      if (outcome === "exit") return
      continue
    }

    let wroteAssistantPrefix = false
    const writeAssistantPrefix = () => {
      if (!wroteAssistantPrefix) {
        process.stdout.write("Khala: ")
        wroteAssistantPrefix = true
      }
    }
    const prepared = prepareUserTurn(messages, prompt)
    try {
      const result = await Effect.runPromise(runChatTurn({
        mode: args.mode,
        baseUrl: args.baseUrl,
        token: args.token,
        messages: prepared,
        onDelta: (delta) => {
          writeAssistantPrefix()
          process.stdout.write(delta)
        },
        onRetry: (event) => {
          process.stdout.write(`Khala: inference unavailable; retrying ${event.retry}/${event.maxRetries} in ${formatDelay(event.delayMs)}...\n`)
        },
      }))
      writeAssistantPrefix()
      process.stdout.write("\n\n")
      messages = appendAssistantTurn(prepared, result.text)
      lastTraceRef = result.traceRef ?? lastTraceRef
    } catch (error) {
      writeAssistantPrefix()
      process.stdout.write(`${formatInteractiveError(toKhalaCliError(error, "Khala turn failed."))}\n\n`)
    }
  }
}

async function handleSlashCommand(
  args: ParsedArgs,
  input: string,
  lastTraceRef: string | undefined,
): Promise<"handled" | "exit"> {
  const [command = "", ...rest] = input.split(/\s+/)
  const argument = rest.join(" ").trim()

  if (command === "/exit" || command === "/quit") {
    return "exit"
  }
  if (command === "/changelog") {
    process.stdout.write(`${formatKhalaChangelog()}\n\n`)
    return "handled"
  }
  if (command === "/tokens") {
    try {
      const tokens = await Effect.runPromise(fetchTokensServed({ baseUrl: args.baseUrl }))
      process.stdout.write(`Khala: ${formatTokensServed(tokens)}\n\n`)
    } catch (error) {
      process.stdout.write(`Khala: ${formatInteractiveError(toKhalaCliError(error, "Tokens fetch failed."))}\n\n`)
    }
    return "handled"
  }
  if (command === "/feedback") {
    if (argument.length === 0) {
      process.stdout.write("Khala: Usage: /feedback <message>\n\n")
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
      process.stdout.write(`Khala: Feedback saved: ${response.feedbackRef}\n\n`)
    } catch (error) {
      process.stdout.write(`Khala: ${formatInteractiveError(toKhalaCliError(error, "Feedback failed."))}\n\n`)
    }
    return "handled"
  }

  process.stdout.write(`Khala: Unknown command ${command}. Try /feedback <message>, /tokens, /changelog, or /exit.\n\n`)
  return "handled"
}

async function runOneTurn(
  args: ParsedArgs,
  history: ReadonlyArray<KhalaChatMessage>,
  prompt: string,
  onDelta?: (text: string) => void,
): Promise<string> {
  const messages = prepareUserTurn(history, prompt)
  const result = await Effect.runPromise(runChatTurn({
    mode: args.mode,
    baseUrl: args.baseUrl,
    token: args.token,
    messages,
    onDelta,
    onRetry: (event) => {
      process.stderr.write(`khala: inference unavailable; retrying ${event.retry}/${event.maxRetries} in ${formatDelay(event.delayMs)}...\n`)
    },
  }))
  return result.text
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
  process.stderr.write(`khala: ${prefix}: ${humanReadableReason(error)}\n`)
}

function formatInteractiveError(error: KhalaCliError): string {
  if (error.statusCode === 429) {
    return "Rate limited after retrying. Please wait a moment and try again."
  }
  if (error.statusCode === 502 || error.statusCode === 503 || error.statusCode === 504 || error.code === "inference_unavailable") {
    return "Inference is unavailable after retrying. Please try again in a moment."
  }
  return humanReadableReason(error)
}

function humanReadableReason(error: KhalaCliError): string {
  if (error.reason === "inference_unavailable" || error.code === "inference_unavailable") {
    return "Inference is unavailable after retrying. Please try again in a moment."
  }
  return error.reason
}

function formatDelay(delayMs: number): string {
  if (delayMs < 1_000) return `${delayMs}ms`
  return `${(delayMs / 1_000).toFixed(1)}s`
}

function formatTokensServed(tokens: KhalaTokensResponse): string {
  const formatted = new Intl.NumberFormat().format(tokens.tokensServed)
  return `Khala tokens served: ${formatted} (as of ${formatTokensTimestamp(tokens.generatedAt)})`
}

function formatTokensTimestamp(iso: string): string {
  return new Intl.DateTimeFormat("en-US", {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(iso))
}

function usage(): string {
  return `Khala CLI

Usage:
  khala
  khala changelog
  khala tokens
  khala feedback "The transcript disappeared"
  khala --prompt "Say hello"
  khala --headless --json < prompt.txt
  khala --api --token "$OPENAGENTS_AGENT_TOKEN" --prompt "Hello"

Interactive commands:
  /feedback <text>   Save product feedback without sending it to inference
  /tokens            Show global Khala tokens served
  /changelog         Show recent Khala CLI changes
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
