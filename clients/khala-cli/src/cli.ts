import { Effect } from "effect"
import { appendAssistantTurn, prepareUserTurn } from "./bounds.js"
import { fetchModels, mintFreeKey, runChatTurn, toKhalaCliError } from "./client.js"
import { readPromptWithOpenTui } from "./input.js"
import { DEFAULT_BASE_URL, type ChatMode, type KhalaChatMessage, type KhalaCliError } from "./types.js"

interface ParsedArgs {
  readonly help: boolean
  readonly version: boolean
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
    process.stdout.write("khala 0.1.0\n")
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

    if (shouldRunHeadless(args)) {
      const prompt = args.prompt ?? await readStdinPrompt()
      const text = await runOneTurn(args, [], prompt, args.json ? undefined : (delta) => process.stdout.write(delta))
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

    await runInteractive(args)
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

  if (prompt === undefined && positional.length > 0) {
    prompt = positional.join(" ")
  }

  return {
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

async function runInteractive(args: ParsedArgs): Promise<void> {
  let messages: ReadonlyArray<KhalaChatMessage> = []
  process.stdout.write("Khala CLI. Type /exit to quit.\n")
  while (true) {
    const prompt = await readPromptWithOpenTui()
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
    process.stdout.write("\nKhala: ")
    const prepared = prepareUserTurn(messages, prompt)
    const result = await Effect.runPromise(runChatTurn({
      mode: args.mode,
      baseUrl: args.baseUrl,
      token: args.token,
      messages: prepared,
      onDelta: (delta) => process.stdout.write(delta),
    }))
    process.stdout.write("\n")
    messages = appendAssistantTurn(prepared, result.text)
  }
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
  process.stderr.write(`khala: ${prefix}: ${error.reason}\n`)
}

function usage(): string {
  return `Khala CLI

Usage:
  khala
  khala --prompt "Say hello"
  khala --headless --json < prompt.txt
  khala --api --token "$OPENAGENTS_AGENT_TOKEN" --prompt "Hello"

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
