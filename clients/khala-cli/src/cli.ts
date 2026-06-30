import { Effect } from "effect"
import { homedir } from "node:os"
import { join } from "node:path"
import {
  appendKhalaSessionModelItem,
  createKhalaSessionRollout,
  forkKhalaSessionRollout,
  khalaSessionModelItems,
  listKhalaSessionRollouts,
  readKhalaSessionRollout,
  runKhalaMcpServerStdio,
  type KhalaSessionRolloutLoaded,
} from "@openagentsinc/khala-tools"
import { appendAssistantTurn, prepareUserTurn } from "./bounds.js"
import { KHALA_CLI_VERSION, formatKhalaChangelog } from "./changelog.js"
import {
  armArtanisPylonDispatch,
  decideArtanisApprovalGate,
  fetchArtanisConsole,
  fetchModels,
  fetchTokensServed,
  mintFreeKey,
  runArtanisTurn,
  runChatTurn,
  submitFeedback,
  toKhalaCliError,
} from "./client.js"
import {
  connectKhalaCodex,
  resolveKhalaCodexStatus,
  runKhalaCodexTask,
  selectKhalaRoute,
  type KhalaCodexDisplayEvent,
  type KhalaCodexStatus,
  type KhalaRouteSelection,
} from "./codex.js"
import {
  CodexCliMissingError,
  connectFleetAccount,
  linkFleetPylon,
  listFleetAccounts,
  runOperatorFleetStatusLive,
  type KhalaFleetConnectResult,
  type KhalaFleetLinkResult,
  type KhalaFleetStatus,
} from "./fleet.js"
import {
  parseFleetIssueList,
  runKhalaFleetSupervisor,
  type KhalaFleetRunResult,
} from "./fleet-run.js"
import { appendPromptHistory, readPromptFromTerminal } from "./input.js"
import { readStdinText } from "./proc.js"
import { openVerificationUrl, runKhalaLogin, type KhalaLoginResult } from "./login.js"
import {
  cancelKhalaSpawn,
  listKhalaSpawnRuns,
  readKhalaSpawnRun,
  readKhalaSpawnWorker,
  refreshKhalaPylonSpawnRun,
  runKhalaSpawn,
  summarizeSpawnRun,
  type KhalaSpawnLifecycleEvent,
  type KhalaSpawnRun,
  type KhalaSpawnStrategy,
  type KhalaSpawnWorker,
  type KhalaSpawnWorkflow,
} from "./spawn.js"
import { renderMarkdownForTerminal, renderReasoningMarkdownDeltaForTerminal, terminalStyle } from "./terminal.js"
import { clearStoredAgentToken, ensureStoredAgentToken, readStoredAgentToken, traceTokenPath } from "./token-store.js"
import {
  clearProviderKey,
  describeStoredProviderKey,
  normalizeProviderName,
  providerKeyPath,
  readProviderKey,
  redactProviderKey,
  validateProviderKeyShape,
  writeProviderKey,
  type StoredProviderKey,
  BYOK_PROVIDERS,
} from "./provider-key.js"
import { DEFAULT_BASE_URL, type ChatMode, type ChatTurnMetadata, type KhalaChatMessage, type KhalaCliError, type KhalaTokensResponse } from "./types.js"
import { awaitSettledKhalaAutoUpdate, startKhalaAutoUpdate } from "./updater.js"

// Conversation channel. "khala" is the public collective-intelligence model;
// "artanis" is the owner-only operator channel (#6363, epic #6359) that talks
// to the real Artanis operator persona via POST /api/operator/artanis/chat.
type Channel = "khala" | "artanis"

type ParsedCommand =
  | { readonly kind: "chat" }
  | {
      readonly kind: "artanisApprovalGate"
      readonly action: "approve" | "reject" | "arm-pylon-dispatch" | "status"
      readonly gateRef?: string | undefined
    }
  | { readonly kind: "codex"; readonly text: string | undefined }
  | { readonly kind: "codexAuth" }
  | { readonly kind: "codexStatus" }
  | { readonly kind: "changelog" }
  | { readonly kind: "feedback"; readonly text: string | undefined }
  | { readonly kind: "fleetConnect"; readonly accountRef: string | undefined; readonly force: boolean }
  | { readonly kind: "fleetRun" }
  | { readonly kind: "fleetLink" }
  | { readonly kind: "fleetStatus"; readonly live: boolean }
  | { readonly kind: "help" }
  | { readonly kind: "info" }
  | {
      readonly kind: "key"
      readonly action: "add" | "list" | "remove"
      readonly provider: string | undefined
      readonly key: string | undefined
    }
  | { readonly kind: "login" }
  | { readonly kind: "logout" }
  | { readonly kind: "mcpServer" }
  | { readonly kind: "resume"; readonly sessionId: string | undefined }
  | { readonly kind: "fork"; readonly sessionId: string | undefined }
  | { readonly kind: "spawn"; readonly text: string | undefined }
  | { readonly kind: "spawnCancel"; readonly targetRef: string | undefined }
  | { readonly kind: "spawnJoin"; readonly runRef: string | undefined }
  | { readonly kind: "spawnWorker"; readonly workerRef: string | undefined }
  | { readonly kind: "spawnWorkers" }
  | { readonly kind: "tokens" }
  | { readonly kind: "version" }

interface ParsedArgs {
  readonly help: boolean
  readonly version: boolean
  readonly command: ParsedCommand
  readonly mode: ChatMode
  readonly channel: Channel
  readonly baseUrl: string
  readonly token: string | undefined
  readonly prompt: string | undefined
  readonly sessionDir: string | undefined
  readonly headless: boolean
  readonly json: boolean
  readonly models: boolean
  readonly mintFreeKey: boolean
  readonly artanisExpiresInHours: number | undefined
  readonly fleetDryRun: boolean
  readonly fleetIssues: readonly number[] | undefined
  readonly fleetOnce: boolean
  readonly fleetPerAccount: number | undefined
  readonly spawnCount: number | undefined
  readonly spawnBranch: string | undefined
  readonly spawnCommit: string | undefined
  readonly spawnFixture: boolean
  readonly spawnMaxParallel: number | undefined
  readonly spawnObjective: string | undefined
  readonly spawnPylonRef: string | undefined
  readonly spawnRepo: string | undefined
  readonly spawnStrategy: KhalaSpawnStrategy
  readonly spawnTimeoutMs: number | undefined
  readonly spawnVerify: string | undefined
  readonly spawnWorkflow: KhalaSpawnWorkflow
}

export async function runKhalaCli(argv: ReadonlyArray<string>, env: Record<string, string | undefined> = process.env): Promise<number> {
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
    if (args.command.kind === "artanisApprovalGate") {
      const result = await runArtanisApprovalGateCommand(args, env, args.command)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${formatArtanisApprovalGateResult(result)}\n`)
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
    if (args.command.kind === "fleetConnect") {
      try {
        const result = await connectFleetAccount({
          env,
          accountRef: args.command.accountRef,
          force: args.command.force,
        })
        if (args.json) {
          process.stdout.write(`${JSON.stringify(result)}\n`)
        } else {
          process.stdout.write(`${formatFleetConnect(result)}\n`)
        }
        return 0
      } catch (error) {
        if (error instanceof CodexCliMissingError) {
          process.stderr.write(`${terminalStyle.meta(error.message)}\n`)
          return 1
        }
        throw error
      }
    }
    if (args.command.kind === "fleetLink") {
      const result = await linkFleetPylon({
        env,
        baseUrl: args.baseUrl,
        token: args.token,
      })
      process.stdout.write(args.json ? `${JSON.stringify(result)}\n` : `${formatFleetLink(result)}\n`)
      return 0
    }
    if (args.command.kind === "fleetStatus") {
      if (args.command.live) {
        const token = await ensureStoredAgentToken({
          baseUrl: args.baseUrl,
          env,
          explicitToken: args.token,
        })
        await runOperatorFleetStatusLive({ baseUrl: args.baseUrl, token, env })
        return 0
      }
      const status = await listFleetAccounts({ env })
      process.stdout.write(args.json ? `${JSON.stringify(status)}\n` : `${formatFleetStatus(status)}\n`)
      return 0
    }
    if (args.command.kind === "fleetRun") {
      const result = await runFleetRunCommand(args, env)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${formatFleetRunResult(result)}\n`)
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
    if (args.command.kind === "spawn") {
      const result = await runSpawnCommand(args, env, args.command.text)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${summarizeSpawnRun(result)}\n`)
      return 0
    }
    if (args.command.kind === "spawnWorkers") {
      const result = await listKhalaSpawnRuns(env)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${formatSpawnRuns(result.runs)}\n`)
      return 0
    }
    if (args.command.kind === "spawnWorker") {
      const workerRef = args.command.workerRef?.trim()
      if (!workerRef) throw new Error("khala worker requires a worker ref")
      const result = await readKhalaSpawnWorker(env, workerRef)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${formatSpawnWorker(result)}\n`)
      return 0
    }
    if (args.command.kind === "spawnJoin") {
      const runRef = args.command.runRef?.trim()
      if (!runRef) throw new Error("khala join requires a run ref")
      const result = await readSpawnRunCommand(args, env, runRef)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${summarizeSpawnRun(result)}\n`)
      return 0
    }
    if (args.command.kind === "spawnCancel") {
      const targetRef = args.command.targetRef?.trim()
      if (!targetRef) throw new Error("khala cancel requires a run or worker ref")
      const result = await cancelKhalaSpawn(env, targetRef)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `Cancelled ${result.cancelledWorkers.length} worker(s) for ${targetRef}.\n`)
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
    if (args.command.kind === "key") {
      return await runKeyCommand(args, env, args.command)
    }
    if (args.command.kind === "login") {
      return await runLoginCommand(args, env)
    }
    if (args.command.kind === "logout") {
      const cleared = await clearStoredAgentToken(env)
      process.stdout.write(
        cleared
          ? `Signed out. Cleared the stored Khala token (${traceTokenPath(env)}).\n`
          : "You were not signed in (no stored Khala token to clear).\n",
      )
      return 0
    }
    if (args.command.kind === "mcpServer") {
      await runKhalaMcpServerStdio()
      return 0
    }
    if (args.command.kind === "resume") {
      const result = await runResumeCommand(args, env, args.command.sessionId)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${formatSessionCommandResult(result)}\n`)
      return 0
    }
    if (args.command.kind === "fork") {
      const result = await runForkCommand(args, env, args.command.sessionId)
      process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${formatSessionCommandResult(result)}\n`)
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
      const session = await createKhalaSessionRollout({
        sessionId: createCliSessionId(),
        stateDir: khalaSessionStateDir(args, env),
      })
      await appendCliModelItem(args, env, session.sessionId, "user", prompt)
      const text = await runOneTurn(
        args,
        env,
        [],
        prompt,
        args.json ? undefined : (delta) => process.stdout.write(delta),
        args.json ? undefined : (delta) => process.stderr.write(renderReasoningMarkdownDeltaForTerminal(delta)),
      )
      await appendCliModelItem(args, env, session.sessionId, "assistant", text)
      if (args.json) {
        process.stdout.write(`${JSON.stringify({ sessionId: session.sessionId, sessionPath: session.path, text })}\n`)
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
    const khalaError = toKhalaCliError(error, "Khala CLI failed.")
    if (args.channel === "artanis") {
      // Keep the headless --artanis failure as friendly as the interactive one:
      // explain it is an owner-only channel instead of printing "unauthorized".
      process.stderr.write(`khala: ${formatArtanisError(khalaError)}\n`)
    } else {
      printError(khalaError)
    }
    return 1
  }
}

function parseArgs(argv: ReadonlyArray<string>, env: Record<string, string | undefined>): ParsedArgs {
  let help = false
  let version = false
  let mode: ChatMode = "public"
  let channel: Channel = "khala"
  let baseUrl = env.KHALA_BASE_URL ?? DEFAULT_BASE_URL
  let token = env.OPENAGENTS_AGENT_TOKEN
  let prompt: string | undefined
  let sessionDir: string | undefined
  let headless = false
  let json = false
  let models = false
  let mintFreeKey = false
  let artanisExpiresInHours: number | undefined
  let fleetDryRun = false
  let fleetIssues: readonly number[] | undefined
  let fleetOnce = false
  let fleetPerAccount: number | undefined
  let spawnBranch: string | undefined
  let spawnCommit: string | undefined
  let spawnCount: number | undefined
  let spawnFixture = false
  let spawnMaxParallel: number | undefined
  let spawnObjective: string | undefined
  let spawnPylonRef: string | undefined
  let spawnRepo: string | undefined
  let spawnStrategy: KhalaSpawnStrategy = "auto"
  let spawnTimeoutMs: number | undefined
  let spawnVerify: string | undefined
  let spawnWorkflow: KhalaSpawnWorkflow = "codex_agent_task"
  let fleetAccount: string | undefined
  let fleetForce = false
  let fleetLive = false
  const positional: Array<string> = []

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === "--") continue
    if (arg === "--help" || arg === "-h") help = true
    else if (arg === "--version") version = true
    else if (arg === "--public") mode = "public"
    else if (arg === "--api") mode = "api"
    else if (arg === "--artanis") channel = "artanis"
    else if (arg === "--khala") channel = "khala"
    else if (arg === "--headless") headless = true
    else if (arg === "--json" || arg === "--no-stream") json = true
    else if (arg === "--models") models = true
    else if (arg === "--mint-free-key") mintFreeKey = true
    else if (arg === "--expires-hours") {
      artanisExpiresInHours = parsePositiveInteger(requireValue(argv, index, arg), arg)
      index += 1
    } else if (arg.startsWith("--expires-hours=")) {
      artanisExpiresInHours = parsePositiveInteger(arg.slice("--expires-hours=".length), "--expires-hours")
    }
    else if (arg === "--dry-run") fleetDryRun = true
    else if (arg === "--once") fleetOnce = true
    else if (arg === "--issues") {
      fleetIssues = parseFleetIssueList(requireValue(argv, index, arg))
      index += 1
    } else if (arg.startsWith("--issues=")) {
      fleetIssues = parseFleetIssueList(arg.slice("--issues=".length))
    } else if (arg === "--per-account") {
      fleetPerAccount = parsePositiveInteger(requireValue(argv, index, arg), arg)
      index += 1
    } else if (arg.startsWith("--per-account=")) {
      fleetPerAccount = parsePositiveInteger(arg.slice("--per-account=".length), "--per-account")
    }
    else if (arg === "--fixture") spawnFixture = true
    else if (arg === "--count") {
      spawnCount = parsePositiveInteger(requireValue(argv, index, arg), arg)
      index += 1
    } else if (arg.startsWith("--count=")) {
      spawnCount = parsePositiveInteger(arg.slice("--count=".length), "--count")
    } else if (arg === "--max-parallel") {
      spawnMaxParallel = parsePositiveInteger(requireValue(argv, index, arg), arg)
      index += 1
    } else if (arg.startsWith("--max-parallel=")) {
      spawnMaxParallel = parsePositiveInteger(arg.slice("--max-parallel=".length), "--max-parallel")
    } else if (arg === "--objective") {
      spawnObjective = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--objective=")) {
      spawnObjective = arg.slice("--objective=".length)
    } else if (arg === "--pylon-ref" || arg === "--target-pylon-ref") {
      spawnPylonRef = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--pylon-ref=")) {
      spawnPylonRef = arg.slice("--pylon-ref=".length)
    } else if (arg.startsWith("--target-pylon-ref=")) {
      spawnPylonRef = arg.slice("--target-pylon-ref=".length)
    } else if (arg === "--repo" || arg === "--repository") {
      spawnRepo = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--repo=")) {
      spawnRepo = arg.slice("--repo=".length)
    } else if (arg.startsWith("--repository=")) {
      spawnRepo = arg.slice("--repository=".length)
    } else if (arg === "--branch") {
      spawnBranch = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--branch=")) {
      spawnBranch = arg.slice("--branch=".length)
    } else if (arg === "--commit") {
      spawnCommit = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--commit=")) {
      spawnCommit = arg.slice("--commit=".length)
    } else if (arg === "--verify") {
      spawnVerify = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--verify=")) {
      spawnVerify = arg.slice("--verify=".length)
    } else if (arg === "--workflow") {
      spawnWorkflow = parseSpawnWorkflow(requireValue(argv, index, arg))
      index += 1
    } else if (arg.startsWith("--workflow=")) {
      spawnWorkflow = parseSpawnWorkflow(arg.slice("--workflow=".length))
    } else if (arg === "--strategy") {
      spawnStrategy = parseSpawnStrategy(requireValue(argv, index, arg))
      index += 1
    } else if (arg.startsWith("--strategy=")) {
      spawnStrategy = parseSpawnStrategy(arg.slice("--strategy=".length))
    } else if (arg === "--timeout") {
      spawnTimeoutMs = parsePositiveInteger(requireValue(argv, index, arg), arg) * 1000
      index += 1
    } else if (arg.startsWith("--timeout=")) {
      spawnTimeoutMs = parsePositiveInteger(arg.slice("--timeout=".length), "--timeout") * 1000
    } else if (arg === "--base-url") {
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
    } else if (arg === "--session-dir") {
      sessionDir = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--session-dir=")) {
      sessionDir = arg.slice("--session-dir=".length)
    } else if (arg === "--account") {
      fleetAccount = requireValue(argv, index, arg)
      index += 1
    } else if (arg.startsWith("--account=")) {
      fleetAccount = arg.slice("--account=".length)
    } else if (arg === "--force") {
      fleetForce = true
    } else if (arg === "--live") {
      fleetLive = true
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
  } else if (maybeCommand === "artanis") {
    const sub = positional[1]?.trim().toLowerCase()
    if (sub === "status" || sub === "approval-gates" || sub === "gates") {
      command = { kind: "artanisApprovalGate", action: "status" }
    } else if (sub === "arm-pylon-dispatch") {
      command = { kind: "artanisApprovalGate", action: "arm-pylon-dispatch" }
    } else if (sub === "approve" || sub === "reject") {
      command = {
        kind: "artanisApprovalGate",
        action: sub,
        gateRef: positional[2],
      }
    } else {
      throw new Error("Unknown Artanis command. Use `khala artanis status`, `khala artanis arm-pylon-dispatch`, `khala artanis approve <gateRef>`, or `khala artanis reject <gateRef>`.")
    }
  } else if (maybeCommand === "auth" && positional[1] === "codex") {
    command = { kind: "codexAuth" }
  } else if (maybeCommand === "fleet") {
    const sub = positional[1]?.trim().toLowerCase()
    if (sub === "status" || sub === "list" || sub === "ls") {
      command = { kind: "fleetStatus", live: fleetLive }
    } else if (sub === "run") {
      command = { kind: "fleetRun" }
    } else if (sub === "link") {
      command = { kind: "fleetLink" }
    } else if (sub === undefined || sub === "connect" || sub === "add") {
      // `khala fleet`, `khala fleet connect`, `khala fleet add` all connect.
      // Optional positional ref: `khala fleet connect codex-2` (or --account).
      command = {
        kind: "fleetConnect",
        accountRef: fleetAccount ?? (positional[2]?.trim() || undefined),
        force: fleetForce,
      }
    } else {
      throw new Error(`Unknown fleet command: ${sub}. Use \`khala fleet connect\`, \`khala fleet link\`, \`khala fleet status\`, or \`khala fleet run\`.`)
    }
  } else if (maybeCommand === "codex") {
    command = positional[1] === "status"
      ? { kind: "codexStatus" }
      : { kind: "codex", text: positional.slice(1).join(" ") || prompt }
  } else if (maybeCommand === "help") {
    command = { kind: "help" }
  } else if (maybeCommand === "info") {
    command = { kind: "info" }
  } else if (maybeCommand === "key") {
    const action = positional[1]?.trim().toLowerCase()
    command = {
      kind: "key",
      action: action === "add" || action === "remove" || action === "rm" || action === "delete"
        ? (action === "rm" || action === "delete" ? "remove" : action)
        : "list",
      provider: positional[2],
      key: positional[3],
    }
  } else if (maybeCommand === "login") {
    command = { kind: "login" }
  } else if (maybeCommand === "logout") {
    command = { kind: "logout" }
  } else if (maybeCommand === "mcp-server") {
    command = { kind: "mcpServer" }
  } else if (maybeCommand === "resume") {
    command = { kind: "resume", sessionId: positional[1] }
  } else if (maybeCommand === "fork") {
    command = { kind: "fork", sessionId: positional[1] }
  } else if (maybeCommand === "spawn") {
    command = {
      kind: "spawn",
      text: positional.slice(1).join(" ") || spawnObjective || prompt,
    }
  } else if (maybeCommand === "workers") {
    command = { kind: "spawnWorkers" }
  } else if (maybeCommand === "worker") {
    command = { kind: "spawnWorker", workerRef: positional[1] }
  } else if (maybeCommand === "join") {
    command = { kind: "spawnJoin", runRef: positional[1] }
  } else if (maybeCommand === "cancel") {
    command = { kind: "spawnCancel", targetRef: positional[1] }
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
    channel,
    baseUrl,
    token,
    prompt,
    sessionDir,
    headless,
    json,
    models,
    mintFreeKey,
    artanisExpiresInHours,
    fleetDryRun,
    fleetIssues,
    fleetOnce,
    fleetPerAccount,
    spawnCount,
    spawnBranch,
    spawnCommit,
    spawnFixture,
    spawnMaxParallel,
    spawnObjective,
    spawnPylonRef,
    spawnRepo,
    spawnStrategy,
    spawnTimeoutMs,
    spawnVerify,
    spawnWorkflow,
  }
}

async function runInteractive(args: ParsedArgs, env: Record<string, string | undefined>): Promise<void> {
  let messages: ReadonlyArray<KhalaChatMessage> = []
  let promptHistory: ReadonlyArray<string> = []
  let lastTraceRef: string | undefined
  let lastMessageInfo: ChatTurnMetadata | undefined
  // Active conversation channel. Starts from the parsed flag; flipped live by
  // the /artanis and /khala slash commands. Switching channels resets the
  // transcript so the two personas never share conversation context.
  let channel: Channel = args.channel
  const sessionId = createCliSessionId()
  await createKhalaSessionRollout({ sessionId, stateDir: khalaSessionStateDir(args, env) })
  process.stdout.write(`${welcomeBanner()}\n\n`)
  if (channel === "artanis") {
    process.stdout.write(`${artanisBanner()}\n\n`)
  }
  const updateHandle = startKhalaAutoUpdate({
    currentVersion: KHALA_CLI_VERSION,
    env,
    notifyMode: "defer",
    notify: line => process.stdout.write(`\n${terminalStyle.assistant("Khala:")} ${line}\n\n`),
  })
  await awaitSettledKhalaAutoUpdate(updateHandle)
  updateHandle.flushNotifications()
  const cleanExit = (): void => {
    updateHandle.flushNotifications()
  }
  while (true) {
    const prompt = await readPromptFromTerminal(terminalStyle.user("> "), {
      history: promptHistory,
    })
    if (prompt === null) {
      process.stdout.write("\n")
      cleanExit()
      return
    }
    if (prompt.trim() === "/exit") {
      cleanExit()
      return
    }
    if (prompt.trim().length === 0) {
      continue
    }
    promptHistory = appendPromptHistory(promptHistory, prompt)
    if (prompt.trim().startsWith("/")) {
      // Channel toggles are handled here because they mutate loop-local state.
      const channelSwitch = parseChannelSwitch(prompt.trim())
      if (channelSwitch !== undefined) {
        if (channelSwitch === channel) {
          process.stdout.write(`${terminalStyle.assistant(speakerLabel(channel))} Already in the ${channelName(channel)} channel.\n\n`)
        } else {
          channel = channelSwitch
          messages = []
          lastMessageInfo = undefined
          process.stdout.write(`${channelSwitchNotice(channel)}\n\n`)
        }
        continue
      }
      const outcome = await handleSlashCommand(args, env, prompt.trim(), lastTraceRef, lastMessageInfo, sessionId)
      if (outcome === "exit") {
        cleanExit()
        return
      }
      continue
    }

    if (channel === "artanis") {
      const prepared = prepareUserTurn(messages, prompt)
      await appendCliModelItem(args, env, sessionId, "user", prompt)
      const waitingDots = startWaitingDots()
      try {
        const token = await ensureStoredAgentToken({
          baseUrl: args.baseUrl,
          env,
          explicitToken: args.token,
        })
        const result = await Effect.runPromise(runArtanisTurn({
          baseUrl: args.baseUrl,
          token,
          messages: prepared,
        }))
        waitingDots.stop()
        process.stdout.write(`${terminalStyle.assistant("Artanis:")} ${renderMarkdownForTerminal(result.text)}\n\n`)
        messages = appendAssistantTurn(prepared, result.text)
        await appendCliModelItem(args, env, sessionId, "assistant", result.text)
        lastTraceRef = result.traceRef ?? lastTraceRef
      } catch (error) {
        waitingDots.stop()
        const artanisError = toKhalaCliError(error, "Artanis turn failed.")
        lastTraceRef = artanisError.traceRef ?? lastTraceRef
        process.stdout.write(`${terminalStyle.assistant("Artanis:")} ${formatArtanisError(artanisError)}\n\n`)
      }
      continue
    }

    const resolvedArgs = await withStoredApiToken(args, env)
    const routed = await maybeRunLocalCodexTurn(resolvedArgs, env, messages, prompt)
    if (routed.handled) {
      await appendCliModelItem(args, env, sessionId, "user", prompt)
      await appendCliModelItem(args, env, sessionId, "assistant", routed.text)
      messages = appendAssistantTurn(
        prepareUserTurn(messages, prompt),
        routed.text,
      )
      continue
    }

    const prepared = prepareUserTurn(messages, prompt)
    await appendCliModelItem(args, env, sessionId, "user", prompt)
    // Resolve BYOK per-turn so /key add and /key remove take effect live.
    // Built on resolvedArgs so a stored `khala login` token is still honored.
    const exec = await resolveChatExecution(resolvedArgs, env)
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
        mode: exec.mode,
        baseUrl: resolvedArgs.baseUrl,
        token: exec.token,
        ...(exec.providerName === undefined ? {} : { providerName: exec.providerName }),
        ...(exec.providerKey === undefined ? {} : { providerKey: exec.providerKey }),
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
      await appendCliModelItem(args, env, sessionId, "assistant", result.text)
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
  if (command === "/login") {
    await runLoginCommand(args, env)
    process.stdout.write("\n")
    return "handled"
  }
  if (command === "/logout") {
    const cleared = await clearStoredAgentToken(env)
    process.stdout.write(`${terminalStyle.assistant("Khala:")} ${
      cleared
        ? "Signed out. Cleared the stored Khala token."
        : "You were not signed in (no stored Khala token to clear)."
    }\n\n`)
    return "handled"
  }
  if (command === "/key") {
    const [sub = "", ...keyRest] = argument.split(/\s+/).filter(part => part.length > 0)
    const action = sub.toLowerCase()
    try {
      if (action === "" || action === "list" || action === "status") {
        process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatKeyStatus(await readProviderKey(env))}\n\n`)
        return "handled"
      }
      if (action === "remove" || action === "rm" || action === "delete") {
        const cleared = await clearProviderKey(env)
        process.stdout.write(`${terminalStyle.assistant("Khala:")} ${
          cleared
            ? "Removed your stored provider key. Khala chats use the shared collective again."
            : "No provider key was stored."
        }\n\n`)
        return "handled"
      }
      if (action === "add") {
        const provider = normalizeProviderName(keyRest[0])
        if (provider === undefined) {
          process.stdout.write(`${terminalStyle.assistant("Khala:")} Usage: /key add <provider> <api-key>. Known providers: ${BYOK_PROVIDERS.join(", ")}.\n\n`)
          return "handled"
        }
        const key = validateProviderKeyShape(keyRest.slice(1).join(" "))
        const record = await writeProviderKey(env, { provider, key })
        process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatKeyAdded(record, env)}\n\n`)
        return "handled"
      }
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Usage: /key, /key add <provider> <api-key>, or /key remove\n\n`)
    } catch (error) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${String(error instanceof Error ? error.message : error)}\n\n`)
    }
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
  if (command === "/spawn") {
    try {
      const parsed = parseInteractiveSpawn(argument)
      const result = await runKhalaSpawn({
        baseUrl: args.baseUrl,
        ...(args.spawnBranch === undefined ? {} : { branch: args.spawnBranch }),
        ...(args.spawnCommit === undefined ? {} : { commit: args.spawnCommit }),
        count: parsed.count,
        cwd: process.cwd(),
        env,
        fixture: args.spawnFixture,
        maxParallel: parsed.count,
        objective: parsed.objective,
        onEvent: event => writeSpawnEvent(event),
        ...(args.spawnPylonRef === undefined ? {} : { pylonRef: args.spawnPylonRef }),
        ...(args.spawnRepo === undefined ? {} : { repo: args.spawnRepo }),
        strategy: args.spawnStrategy,
        token: args.token,
        ...(args.spawnVerify === undefined ? {} : { verify: args.spawnVerify }),
        workflow: args.spawnWorkflow,
      })
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${terminalStyle.meta("Spawn complete.")}\n${summarizeSpawnRun(result)}\n\n`)
    } catch (error) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${String(error instanceof Error ? error.message : error)}\n\n`)
    }
    return "handled"
  }
  if (command === "/workers") {
    const result = await listKhalaSpawnRuns(env)
    process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatSpawnRuns(result.runs)}\n\n`)
    return "handled"
  }
  if (command === "/worker") {
    if (argument.length === 0) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Usage: /worker <workerRef>\n\n`)
      return "handled"
    }
    try {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${formatSpawnWorker(await readKhalaSpawnWorker(env, argument))}\n\n`)
    } catch (error) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${String(error instanceof Error ? error.message : error)}\n\n`)
    }
    return "handled"
  }
  if (command === "/join") {
    if (argument.length === 0) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Usage: /join <runRef>\n\n`)
      return "handled"
    }
    try {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${summarizeSpawnRun(await readSpawnRunCommand(args, env, argument))}\n\n`)
    } catch (error) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${String(error instanceof Error ? error.message : error)}\n\n`)
    }
    return "handled"
  }
  if (command === "/cancel") {
    if (argument.length === 0) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Usage: /cancel <runRef|workerRef>\n\n`)
      return "handled"
    }
    try {
      const result = await cancelKhalaSpawn(env, argument)
      process.stdout.write(`${terminalStyle.assistant("Khala:")} Cancelled ${result.cancelledWorkers.length} worker(s) for ${argument}.\n\n`)
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
  env: Record<string, string | undefined>,
  history: ReadonlyArray<KhalaChatMessage>,
  prompt: string,
  onDelta?: (text: string) => void,
  onReasoning?: (text: string) => void,
): Promise<string> {
  if (args.channel === "artanis") {
    const token = await ensureStoredAgentToken({
      baseUrl: args.baseUrl,
      env,
      explicitToken: args.token,
    })
    const result = await Effect.runPromise(runArtanisTurn({
      baseUrl: args.baseUrl,
      token,
      messages: prepareUserTurn(history, prompt),
    }))
    onDelta?.(result.text)
    return result.text
  }

  const resolvedArgs = await withStoredApiToken(args, env)
  const routed = await maybeRunLocalCodexTurn(resolvedArgs, env, history, prompt, {
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
  const exec = await resolveChatExecution(resolvedArgs, env)
  const result = await Effect.runPromise(runChatTurn({
    mode: exec.mode,
    baseUrl: resolvedArgs.baseUrl,
    token: exec.token,
    ...(exec.providerName === undefined ? {} : { providerName: exec.providerName }),
    ...(exec.providerKey === undefined ? {} : { providerKey: exec.providerKey }),
    messages,
    onDelta,
    onReasoning,
    onRetry: (event) => {
      process.stderr.write(`khala: ${formatRetryNotice(event)}\n`)
    },
  }))
  return result.text
}

type KhalaSessionCommandResult = Readonly<{
  action: "fork" | "resume"
  modelItemCount: number
  parentSessionId?: string | undefined
  recordCount: number
  sessionId: string
  sessionPath: string
  text?: string | undefined
}>

async function runResumeCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  requestedSessionId: string | undefined,
): Promise<KhalaSessionCommandResult> {
  const stateDir = khalaSessionStateDir(args, env)
  const sessionId = await resolveSessionCommandTarget(stateDir, requestedSessionId)
  const loaded = await readKhalaSessionRollout(stateDir, sessionId)
  const prompt = args.prompt?.trim()
  if (prompt === undefined || prompt.length === 0) {
    return sessionCommandResult("resume", loaded)
  }
  const history = messagesFromRollout(loaded)
  await appendCliModelItem(args, env, loaded.sessionId, "user", prompt)
  const text = await runOneTurn(args, env, history, prompt)
  await appendCliModelItem(args, env, loaded.sessionId, "assistant", text)
  return { ...sessionCommandResult("resume", await readKhalaSessionRollout(stateDir, loaded.sessionId)), text }
}

async function runForkCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  requestedSessionId: string | undefined,
): Promise<KhalaSessionCommandResult> {
  const stateDir = khalaSessionStateDir(args, env)
  const sourceSessionId = await resolveSessionCommandTarget(stateDir, requestedSessionId)
  const forked = await forkKhalaSessionRollout({
    fromSessionId: sourceSessionId,
    stateDir,
  })
  const prompt = args.prompt?.trim()
  if (prompt === undefined || prompt.length === 0) {
    return sessionCommandResult("fork", forked, sourceSessionId)
  }
  const history = messagesFromRollout(forked)
  await appendCliModelItem(args, env, forked.sessionId, "user", prompt)
  const text = await runOneTurn(args, env, history, prompt)
  await appendCliModelItem(args, env, forked.sessionId, "assistant", text)
  return { ...sessionCommandResult("fork", await readKhalaSessionRollout(stateDir, forked.sessionId), sourceSessionId), text }
}

async function resolveSessionCommandTarget(stateDir: string, requestedSessionId: string | undefined): Promise<string> {
  const explicit = requestedSessionId?.trim()
  if (explicit !== undefined && explicit.length > 0 && explicit !== "--last") return explicit
  const [latest] = await listKhalaSessionRollouts(stateDir)
  if (latest === undefined) {
    throw new Error("No Khala sessions found. Start a chat or pass a session id.")
  }
  return latest.sessionId
}

function sessionCommandResult(
  action: "fork" | "resume",
  loaded: KhalaSessionRolloutLoaded,
  parentSessionId?: string | undefined,
): KhalaSessionCommandResult {
  return {
    action,
    modelItemCount: khalaSessionModelItems(loaded).length,
    ...(parentSessionId === undefined ? {} : { parentSessionId }),
    recordCount: loaded.records.length,
    sessionId: loaded.sessionId,
    sessionPath: loaded.path,
  }
}

function formatSessionCommandResult(result: KhalaSessionCommandResult): string {
  const verb = result.action === "fork" ? "Forked" : "Resumed"
  const lines = [
    `${verb} Khala session: ${result.sessionId}`,
    ...(result.parentSessionId === undefined ? [] : [`parent: ${result.parentSessionId}`]),
    `records: ${result.recordCount}`,
    `model items: ${result.modelItemCount}`,
    `rollout: ${result.sessionPath}`,
  ]
  if (result.text !== undefined) {
    lines.push("")
    lines.push(result.text)
  }
  return lines.join("\n")
}

function messagesFromRollout(loaded: KhalaSessionRolloutLoaded): ReadonlyArray<KhalaChatMessage> {
  return khalaSessionModelItems(loaded)
    .flatMap(item => {
      if (item.role !== "user" && item.role !== "assistant") return []
      return [{ role: item.role, content: item.body }]
    })
}

async function appendCliModelItem(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  sessionId: string,
  role: "assistant" | "user",
  body: string,
): Promise<void> {
  await appendKhalaSessionModelItem(khalaSessionStateDir(args, env), sessionId, {
    body,
    id: `msg.${role}.${crypto.randomUUID()}`,
    role,
  })
}

function khalaSessionStateDir(args: ParsedArgs, env: Record<string, string | undefined>): string {
  const explicit = args.sessionDir?.trim() || env.KHALA_SESSION_DIR?.trim()
  if (explicit) return explicit
  const stateHome = env.XDG_STATE_HOME?.trim() || join(homedir(), ".local", "state")
  return join(stateHome, "khala")
}

async function withStoredApiToken(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
): Promise<ParsedArgs> {
  if (args.mode !== "api") return args
  const storedToken = await resolveConfiguredAgentToken(args.token, env)
  return storedToken === undefined ? args : { ...args, token: storedToken }
}

async function resolveConfiguredAgentToken(
  token: string | undefined,
  env: Record<string, string | undefined>,
): Promise<string | undefined> {
  const explicit = token?.trim()
  if (explicit !== undefined && explicit.length > 0) return explicit
  return await readStoredAgentToken(env)
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
  if (selection.route === "spawn_khala") {
    return await runSelectedKhalaSpawnTurn(args, env, prompt, selection, options)
  }
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

async function runSelectedKhalaSpawnTurn(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  prompt: string,
  selection: Extract<KhalaRouteSelection, { readonly route: "spawn_khala" }>,
  options: {
    readonly silent?: boolean | undefined
  } = {},
): Promise<{ readonly handled: true; readonly text: string }> {
  if (selection.intent === "explain_capability") {
    const text = formatKhalaSpawnCapabilityAnswer()
    if (options.silent !== true) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${renderMarkdownForTerminal(text)}\n\n`)
    }
    return { handled: true, text }
  }

  const objective = selection.objective?.trim() || prompt.trim()
  if (objective.length === 0) {
    const text = "Khala spawn needs a worker objective. Try `/spawn 5 audit this workspace` or `khala spawn --count 5 --objective \"audit this workspace\"`."
    if (options.silent !== true) {
      process.stdout.write(`${terminalStyle.assistant("Khala:")} ${text}\n\n`)
    }
    return { handled: true, text }
  }

  const count = selection.count ?? args.spawnCount ?? 1
  if (options.silent !== true) {
    process.stdout.write(`${terminalStyle.assistant("Khala:")} ${terminalStyle.meta(`Blueprint selected Khala spawn: ${selection.reason}`)}\n`)
  }
  const run = await runKhalaSpawn({
    baseUrl: args.baseUrl,
    ...(args.spawnBranch === undefined ? {} : { branch: args.spawnBranch }),
    ...(args.spawnCommit === undefined ? {} : { commit: args.spawnCommit }),
    count,
    cwd: process.cwd(),
    env,
    fixture: args.spawnFixture,
    maxParallel: args.spawnMaxParallel ?? count,
    objective,
    onEvent: options.silent === true ? undefined : event => writeSpawnEvent(event),
    ...(args.spawnPylonRef === undefined ? {} : { pylonRef: args.spawnPylonRef }),
    ...(args.spawnRepo === undefined ? {} : { repo: args.spawnRepo }),
    strategy: args.spawnStrategy,
    ...(args.spawnTimeoutMs === undefined ? {} : { timeoutMs: args.spawnTimeoutMs }),
    token: args.token,
    ...(args.spawnVerify === undefined ? {} : { verify: args.spawnVerify }),
    workflow: args.spawnWorkflow,
  })
  const text = `Spawn complete.\n${summarizeSpawnRun(run)}`
  if (options.silent !== true) {
    process.stdout.write(`${terminalStyle.assistant("Khala:")} ${terminalStyle.meta("Spawn complete.")}\n${summarizeSpawnRun(run)}\n\n`)
  }
  return { handled: true, text }
}

export function formatKhalaSpawnCapabilityAnswer(): string {
  return [
    "Yes. In this CLI, we can spawn supervised Khala child workers.",
    "Use `/spawn 5 audit X` in interactive mode or `khala spawn --count 5 --objective \"audit X\"` from the shell.",
    "Runs are bounded, recorded under the local Khala home, inspectable with `/workers` or `khala workers`, and cancellable with `/cancel` or `khala cancel`.",
    "Public/browser chat can explain that path, but it cannot execute local workers on your machine.",
  ].join(" ")
}

async function readStdinPrompt(): Promise<string> {
  return await readStdinText()
}

async function runSpawnCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  text: string | undefined,
): Promise<KhalaSpawnRun> {
  const objective = text?.trim() || args.spawnObjective?.trim()
  if (objective === undefined || objective.length === 0) {
    throw new Error("khala spawn requires an objective, for example: khala spawn --count 5 --objective \"audit this workspace\"")
  }
  const token = await resolveConfiguredAgentToken(args.token, env)
  return runKhalaSpawn({
    baseUrl: args.baseUrl,
    ...(args.spawnBranch === undefined ? {} : { branch: args.spawnBranch }),
    ...(args.spawnCommit === undefined ? {} : { commit: args.spawnCommit }),
    count: args.spawnCount ?? 1,
    cwd: process.cwd(),
    env,
    fixture: args.spawnFixture,
    ...(args.spawnMaxParallel === undefined ? {} : { maxParallel: args.spawnMaxParallel }),
    objective,
    onEvent: args.json ? undefined : event => writeSpawnEvent(event),
    ...(args.spawnPylonRef === undefined ? {} : { pylonRef: args.spawnPylonRef }),
    ...(args.spawnRepo === undefined ? {} : { repo: args.spawnRepo }),
    strategy: args.spawnStrategy,
    ...(args.spawnTimeoutMs === undefined ? {} : { timeoutMs: args.spawnTimeoutMs }),
    ...(token === undefined ? {} : { token }),
    ...(args.spawnVerify === undefined ? {} : { verify: args.spawnVerify }),
    workflow: args.spawnWorkflow,
  })
}

async function runFleetRunCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
): Promise<KhalaFleetRunResult> {
  if (args.spawnRepo === undefined || args.spawnRepo.trim().length === 0) {
    throw new Error("khala fleet run requires --repo <owner/repo>")
  }
  if (args.fleetIssues === undefined || args.fleetIssues.length === 0) {
    throw new Error("khala fleet run requires --issues <numbers>")
  }
  if (args.spawnVerify === undefined || args.spawnVerify.trim().length === 0) {
    throw new Error("khala fleet run requires --verify <command>")
  }
  const token = await resolveConfiguredAgentToken(args.token, env)
  return await runKhalaFleetSupervisor({
    baseUrl: args.baseUrl,
    ...(args.spawnBranch === undefined ? {} : { branch: args.spawnBranch }),
    ...(args.spawnCommit === undefined ? {} : { commit: args.spawnCommit }),
    dryRun: args.fleetDryRun,
    env,
    issues: args.fleetIssues,
    ...(args.spawnMaxParallel === undefined ? {} : { maxSlots: args.spawnMaxParallel }),
    once: args.fleetOnce,
    ...(args.fleetPerAccount === undefined ? {} : { perAccount: args.fleetPerAccount }),
    ...(args.spawnPylonRef === undefined ? {} : { pylonRef: args.spawnPylonRef }),
    repo: args.spawnRepo,
    ...(token === undefined ? {} : { token }),
    verify: args.spawnVerify,
  })
}

async function readSpawnRunCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  runRef: string,
): Promise<KhalaSpawnRun> {
  const run = await readKhalaSpawnRun(env, runRef)
  if (run.strategy !== "pylon_codex_assignments") return run
  try {
    return await refreshKhalaPylonSpawnRun({
      baseUrl: args.baseUrl,
      env,
      run,
      token: args.token,
    })
  } catch {
    return run
  }
}

function parseInteractiveSpawn(argument: string): { readonly count: number; readonly objective: string } {
  const trimmed = argument.trim()
  if (trimmed.length === 0) {
    throw new Error("Usage: /spawn <count> <objective>")
  }
  const [first = "", ...rest] = trimmed.split(/\s+/)
  const count = /^\d+$/.test(first) ? parsePositiveInteger(first, "/spawn count") : 1
  const objective = /^\d+$/.test(first) ? rest.join(" ").trim() : trimmed
  if (objective.length === 0) {
    throw new Error("Usage: /spawn <count> <objective>")
  }
  return { count, objective }
}

function shouldRunHeadless(args: ParsedArgs): boolean {
  return args.headless || args.prompt !== undefined || args.json || !process.stdin.isTTY
}

function parsePositiveInteger(value: string, flag: string): number {
  const parsed = Number.parseInt(value, 10)
  if (!/^\d+$/.test(value) || !Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} requires a positive integer`)
  }
  return parsed
}

function parseSpawnStrategy(value: string): KhalaSpawnStrategy {
  if (value === "auto" || value === "local" || value === "pylon") return value
  throw new Error("--strategy must be auto, local, or pylon")
}

function parseSpawnWorkflow(value: string): KhalaSpawnWorkflow {
  if (value === "claude_agent_task" || value === "cloud_coding_session" || value === "codex_agent_task") return value
  throw new Error("--workflow must be claude_agent_task, cloud_coding_session, or codex_agent_task")
}

function requireValue(argv: ReadonlyArray<string>, index: number, flag: string): string {
  const value = argv[index + 1]
  if (value === undefined || value.startsWith("-")) {
    throw new Error(`${flag} requires a value`)
  }
  return value
}

function printError(error: KhalaCliError): void {
  process.stderr.write(`khala: ${friendlyErrorMessage(error)}${formatTraceSuffix(error)}\n`)
}

function formatInteractiveError(error: KhalaCliError): string {
  return `${friendlyErrorMessage(error)}${formatTraceSuffix(error)}`
}

// One human-friendly message for every error surface (headless stderr and the
// interactive transcript both use it). New users should never see a raw
// "fetch failed" or "unauthorized"; map the common failures to plain language
// with a clear next step.
function friendlyErrorMessage(error: KhalaCliError): string {
  if (error.statusCode === 429) {
    return "Rate limited after retrying. Please wait a moment and try again."
  }
  if (error.statusCode === 401 || error.statusCode === 403) {
    return "Not authorized. Run `khala login` to sign in (or pass --token / set OPENAGENTS_AGENT_TOKEN)."
  }
  if (
    error.statusCode === 502 ||
    error.statusCode === 503 ||
    error.statusCode === 504 ||
    error.code === "inference_unavailable" ||
    error.reason === "inference_unavailable"
  ) {
    return "Inference is unavailable after retrying. Please try again in a moment."
  }
  if (isNetworkError(error)) {
    return "Could not reach Khala. Check your internet connection and try again."
  }
  return error.reason
}

function isNetworkError(error: KhalaCliError): boolean {
  if (error.code !== "request_failed" && error.code !== "runtime_error") return false
  return /fetch failed|network|ENOTFOUND|ECONNREFUSED|EAI_AGAIN|getaddrinfo|timed out|timeout|socket/i.test(error.reason)
}

// BYOK (BRING-YOUR-OWN-KEY) PROVIDER KEYS
//
// `khala key add/list/remove` and the interactive `/key` command let a user
// attach their own upstream provider API key so their usage runs on their own
// provider account (OpenRouter-style). The key is stored 0600 beside the agent
// token, never printed in full, and only sent over TLS in the BYOK header.

async function runKeyCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  command: Extract<ParsedCommand, { readonly kind: "key" }>,
): Promise<number> {
  if (command.action === "list") {
    const record = await readProviderKey(env)
    if (args.json) {
      process.stdout.write(`${JSON.stringify(formatKeyJson(record, env))}\n`)
    } else {
      process.stdout.write(`${formatKeyStatus(record)}\n`)
    }
    return 0
  }

  if (command.action === "remove") {
    const cleared = await clearProviderKey(env)
    process.stdout.write(
      cleared
        ? "Removed your stored provider key. Khala chats use the shared collective again.\n"
        : "No provider key was stored.\n",
    )
    return 0
  }

  // add
  const provider = normalizeProviderName(command.provider)
  if (provider === undefined) {
    throw new Error(
      `Usage: khala key add <provider> <api-key>\nKnown providers: ${BYOK_PROVIDERS.join(", ")} (aliases like claude, gemini, grok also work).`,
    )
  }
  const key = validateProviderKeyShape(command.key)
  const record = await writeProviderKey(env, { provider, key })
  process.stdout.write(`${formatKeyAdded(record, env)}\n`)
  return 0
}

function formatKeyAdded(record: StoredProviderKey, env: Record<string, string | undefined>): string {
  return [
    `Saved your ${record.provider} key (${redactProviderKey(record.key)}).`,
    terminalStyle.meta(`Stored locally at ${providerKeyPath(env)} (only you can read it).`),
    terminalStyle.meta("Khala now sends this key with your chats so usage can run on your own provider account."),
    terminalStyle.meta("Full upstream BYOK routing is rolling out; until then the server acknowledges the key and chats use the shared collective. Run `khala key remove` to stop sending it."),
  ].join("\n")
}

function formatKeyStatus(record: StoredProviderKey | undefined): string {
  if (record === undefined) {
    return [
      "No provider key configured. Khala chats use the shared collective (free, no key needed).",
      terminalStyle.meta("Add your own provider key with: khala key add <provider> <api-key>"),
      terminalStyle.meta(`Known providers: ${BYOK_PROVIDERS.join(", ")}`),
    ].join("\n")
  }
  return [
    `Provider key: ${describeStoredProviderKey(record)} (added ${record.addedAt}).`,
    terminalStyle.meta("Sent with your chats so usage can run on your own provider account."),
    terminalStyle.meta("Remove it any time with: khala key remove"),
  ].join("\n")
}

function formatKeyJson(
  record: StoredProviderKey | undefined,
  env: Record<string, string | undefined>,
): Record<string, unknown> {
  // NEVER include the raw key in JSON output: only the redacted tail.
  if (record === undefined) {
    return { configured: false, path: providerKeyPath(env) }
  }
  return {
    configured: true,
    provider: record.provider,
    keyRedacted: redactProviderKey(record.key),
    addedAt: record.addedAt,
    path: providerKeyPath(env),
  }
}

// Resolve the effective chat execution context. When the user has a BYOK
// provider key, route through the authenticated path with an ensured agent
// token so the key can be attached; otherwise use the parsed mode/token.
async function resolveChatExecution(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
): Promise<{
  readonly mode: ChatMode
  readonly token: string | undefined
  readonly providerName: string | undefined
  readonly providerKey: string | undefined
}> {
  const record = await readProviderKey(env)
  if (record === undefined) {
    return { mode: args.mode, token: args.token, providerName: undefined, providerKey: undefined }
  }
  // BYOK present: the key only attaches on the authenticated path, which needs
  // an agent token. Auto-mint/reuse the stored free key so this stays seamless.
  let token: string | undefined
  try {
    token = await ensureStoredAgentToken({ baseUrl: args.baseUrl, env, explicitToken: args.token })
  } catch {
    token = args.token
  }
  return { mode: "api", token, providerName: record.provider, providerKey: record.key }
}

// OPENAGENTS DEVICE-AUTH LOGIN (#6363, epic #6359)

// Shared login runner for `khala login`, `khala --headless login`, and the
// interactive `/login` slash command. Starts the standard OpenAgents
// device-auth flow, prints the verification URL + user code (and opens the
// browser when possible), polls until the browser sign-in links the token to
// the owner account, then re-stores the now-owner-linked token.
async function runLoginCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
): Promise<number> {
  let waitingDots: { readonly stop: () => void } | undefined
  try {
    const result = await runKhalaLogin({
      baseUrl: args.baseUrl,
      env,
      explicitToken: args.token,
      openBrowser: openVerificationUrl,
      onPrompt: prompt => {
        process.stdout.write(`${formatLoginPrompt(prompt)}\n`)
        waitingDots = startWaitingDots()
      },
    })
    waitingDots?.stop()
    process.stdout.write(`${formatLoginSuccess(result)}\n`)
    return 0
  } catch (error) {
    waitingDots?.stop()
    printError(toKhalaCliError(error, "Login failed."))
    return 1
  }
}

function formatLoginPrompt(prompt: {
  readonly userCode: string
  readonly verificationUrl: string
  readonly expiresAt: string | undefined
}): string {
  const lines = [
    "To sign in to OpenAgents, open this URL in your browser:",
    `  ${terminalStyle.assistant(prompt.verificationUrl)}`,
    `Then confirm this code: ${terminalStyle.assistant(prompt.userCode)}`,
  ]
  const expiry = formatLoginExpiry(prompt.expiresAt)
  if (expiry !== undefined) {
    lines.push(terminalStyle.meta(`The code expires ${expiry}.`))
  }
  lines.push(terminalStyle.meta("Waiting for you to finish signing in..."))
  return lines.join("\n")
}

function formatLoginExpiry(expiresAt: string | undefined): string | undefined {
  if (expiresAt === undefined) return undefined
  const parsed = Date.parse(expiresAt)
  if (Number.isNaN(parsed)) return undefined
  const minutes = Math.max(1, Math.round((parsed - Date.now()) / 60_000))
  return `in about ${minutes} minute${minutes === 1 ? "" : "s"}`
}

function formatLoginSuccess(result: KhalaLoginResult): string {
  // Identify by the USER's email only. Never fall back to a display name: a
  // service/agent token's display name (e.g. "Artanis") is NOT the human user,
  // and printing "Signed in as Artanis" wrongly conflates the operator agent you
  // talk to with you, the account owner. No email => name-less account language.
  const who =
    result.email !== undefined
      ? `Signed in as ${result.email}.`
      : "Signed in to your OpenAgents account."
  const linkNote = result.alreadyLinked
    ? "Your Khala token is linked to your OpenAgents account."
    : "Your Khala token is now linked to your OpenAgents account."
  const hint =
    "Artanis is the operator agent you talk to — run /artanis (owner only)."
  return `${who} ${linkNote}\n${hint}`
}

// ARTANIS OPERATOR CHANNEL (#6363, epic #6359)

function parseChannelSwitch(input: string): Channel | undefined {
  const [command = ""] = input.split(/\s+/)
  if (command === "/artanis") return "artanis"
  if (command === "/khala") return "khala"
  return undefined
}

function channelName(channel: Channel): string {
  return channel === "artanis" ? "Artanis operator" : "Khala"
}

function speakerLabel(channel: Channel): string {
  return channel === "artanis" ? "Artanis:" : "Khala:"
}

function artanisBanner(): string {
  return terminalStyle.meta(
    "Artanis operator channel (owner-only). You are talking to the real operator agent, not the public Khala collective. Type /khala to switch back.",
  )
}

function channelSwitchNotice(channel: Channel): string {
  if (channel === "artanis") {
    return `${terminalStyle.assistant("Artanis:")} ${artanisBanner()}`
  }
  return `${terminalStyle.assistant("Khala:")} ${terminalStyle.meta("Back on the public Khala channel.")}`
}

function formatArtanisError(error: KhalaCliError): string {
  if (error.statusCode === 401 || error.statusCode === 403) {
    return `This is the owner-only Artanis channel and your token is not authorized for it. Run \`khala login\` to sign in as the owner (or set OPENAGENTS_AGENT_TOKEN to the owner agent token), then try again.${formatTraceSuffix(error)}`
  }
  if (error.statusCode === 404 || error.code === "schema_mismatch") {
    return `The Artanis operator endpoint is not available yet. It ships with the core Artanis lane; until then, /khala chat still works.${formatTraceSuffix(error)}`
  }
  return formatInteractiveError(error)
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
  const combined = `${previousBuffer}${delta}`
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
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

function formatFleetConnect(result: KhalaFleetConnectResult): string {
  const verb = result.status === "already_connected" ? "Already connected" : "Connected"
  const who = result.email ? `: ${result.email}` : ""
  return terminalStyle.meta([
    `${terminalStyle.assistant("Khala fleet:")} ${verb} Codex account ${result.accountRef}${who}`,
    "Your Codex credentials stay on this machine; OpenAgents orchestrates, your local Pylon executes.",
    "",
    "Next:",
    "  khala login               Sign in to Khala/OpenAgents",
    "  khala fleet link          Link this Pylon to that Khala account",
    "  khala fleet status        See your fleet",
    "  khala fleet connect       Add another account for more throughput",
  ].join("\n"))
}

function formatFleetLink(result: KhalaFleetLinkResult): string {
  return terminalStyle.meta([
    `${terminalStyle.assistant("Khala fleet:")} Linked local Pylon ${result.pylonRef}`,
    `registration: ${result.registrationRef}`,
    `public key: ${result.publicKey.slice(0, 12)}…${result.publicKey.slice(-8)}`,
    "",
    "Next:",
    "  khala fleet status        See connected Codex accounts",
    "  khala spawn --strategy pylon --pylon-ref " + result.pylonRef + " --objective \"implement public issue #123\"",
  ].join("\n"))
}

function formatFleetStatus(status: KhalaFleetStatus): string {
  if (status.accounts.length === 0) {
    return terminalStyle.meta([
      `${terminalStyle.assistant("Khala fleet:")} No Codex accounts connected yet.`,
      "Connect one (paste-free device login):",
      "  khala fleet connect",
    ].join("\n"))
  }
  const rows = status.accounts.map(account => {
    const ready = account.readiness === "ready" ? "ready" : "credentials-missing"
    const email = account.email ?? "(email unavailable)"
    return `  ${account.accountRef.padEnd(12)} ${ready.padEnd(18)} ${email}`
  })
  return terminalStyle.meta([
    `${terminalStyle.assistant("Khala fleet:")} ${status.accounts.length} Codex account(s), ${status.readyCount} ready`,
    `  ${"ACCOUNT".padEnd(12)} ${"READINESS".padEnd(18)} EMAIL`,
    ...rows,
    "",
    "Add another account for more throughput:",
    "  khala fleet connect",
  ].join("\n"))
}

function formatFleetRunResult(result: KhalaFleetRunResult): string {
  const plan = result.plan
  const pylon = plan.pylonRef ?? "(auto-resolve when run starts)"
  const lines = [
    `${terminalStyle.assistant("Khala fleet run:")} ${result.status}`,
    `  repo: ${plan.repo}`,
    `  issues: ${plan.issues.map(issue => `#${issue}`).join(", ")}`,
    `  pylon: ${pylon}`,
    `  ready accounts: ${plan.readyAccounts.join(", ")}`,
    `  target slots: ${plan.targetSlots} (min(${plan.maxSlots}, ${plan.readyAccounts.length} accounts * ${plan.perAccount}))`,
    `  verify: ${plan.verify}`,
  ]
  if (result.rounds.length > 0) {
    lines.push("", "Planned/last round:")
    for (const round of result.rounds) {
      const target = round.issue === null ? round.workKind : `#${round.issue}`
      lines.push(`  slot ${round.slot}: ${round.accountRef} -> ${target} (${round.status})`)
    }
  }
  if (result.mode === "supervise") {
    lines.push("", "Supervisor mode keeps refilling slots until stopped.")
  }
  return terminalStyle.meta(lines.join("\n"))
}

async function runArtanisApprovalGateCommand(
  args: ParsedArgs,
  env: Record<string, string | undefined>,
  command: Extract<ParsedCommand, { readonly kind: "artanisApprovalGate" }>,
): Promise<unknown> {
  const token = await ensureStoredAgentToken({
    baseUrl: args.baseUrl,
    env,
    explicitToken: args.token,
  })
  if (command.action === "status") {
    return await Effect.runPromise(fetchArtanisConsole({
      baseUrl: args.baseUrl,
      token,
    }))
  }
  if (command.action === "arm-pylon-dispatch") {
    return await Effect.runPromise(armArtanisPylonDispatch({
      baseUrl: args.baseUrl,
      token,
      expiresInHours: args.artanisExpiresInHours,
    }))
  }
  const gateRef = command.gateRef?.trim()
  if (gateRef === undefined || gateRef.length === 0) {
    throw new Error(`khala artanis ${command.action} requires a gate ref`)
  }
  return await Effect.runPromise(decideArtanisApprovalGate({
    action: command.action,
    baseUrl: args.baseUrl,
    gateRef,
    token,
  }))
}

function formatArtanisApprovalGateResult(result: unknown): string {
  if (result !== null && typeof result === "object") {
    const record = result as {
      readonly armedGate?: {
        readonly expiresAtDisplay?: unknown
        readonly gateRef?: unknown
        readonly kind?: unknown
        readonly state?: unknown
      }
      readonly gateCount?: unknown
      readonly ledgerRef?: unknown
    }
    if (record.armedGate !== undefined) {
      const gate = record.armedGate
      return terminalStyle.meta([
        `${terminalStyle.assistant("Artanis:")} pylon dispatch gate armed`,
        `  gate: ${String(gate.gateRef ?? "(unknown)")}`,
        `  kind: ${String(gate.kind ?? "pylon_job_dispatch")}`,
        `  state: ${String(gate.state ?? "approved")}`,
        `  expires: ${String(gate.expiresAtDisplay ?? "(unknown)")}`,
      ].join("\n"))
    }
    if (typeof record.ledgerRef === "string" || typeof record.gateCount === "number") {
      return terminalStyle.meta([
        `${terminalStyle.assistant("Artanis:")} approval gates`,
        `  ledger: ${String(record.ledgerRef ?? "(unknown)")}`,
        `  gates: ${String(record.gateCount ?? "(unknown)")}`,
      ].join("\n"))
    }
  }
  return terminalStyle.meta(JSON.stringify(result, null, 2))
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

function writeSpawnEvent(event: KhalaSpawnLifecycleEvent): void {
  const subject = event.workerRef === undefined ? event.runRef : event.workerRef
  process.stdout.write(`${terminalStyle.meta(`Khala spawn: ${subject} ${event.state} - ${event.message}`)}\n`)
}

function formatSpawnRuns(runs: readonly KhalaSpawnRun[]): string {
  if (runs.length === 0) return terminalStyle.meta("No Khala spawn runs yet.")
  return terminalStyle.meta(runs.map(run => {
    const accepted = run.workers.filter(worker => worker.state === "accepted").length
    const active = run.workers.filter(worker => worker.state === "queued" || worker.state === "starting" || worker.state === "running").length
    return `${run.runRef} - ${run.state} - ${accepted}/${run.workers.length} accepted, ${active} active - ${run.objective}`
  }).join("\n"))
}

function formatSpawnWorker(worker: KhalaSpawnWorker): string {
  const lines = [
    `${worker.workerRef}`,
    `state: ${worker.state}`,
    `run: ${worker.runRef}`,
    `objective: ${worker.objective}`,
    `worktree: ${worker.localWorktree ?? "not assigned"}`,
    `session: ${worker.sessionRef ?? "not reported"}`,
    `commands: ${worker.commandCount ?? 0}`,
    `edited files: ${worker.editedFileCount ?? 0}`,
  ]
  if (worker.blockerRefs.length > 0) {
    lines.push(`blockers: ${worker.blockerRefs.join(", ")}`)
  }
  if (worker.error !== undefined) {
    lines.push(`error: ${worker.error}`)
  }
  if (worker.resultText !== undefined && worker.resultText.trim().length > 0) {
    lines.push(`result:\n${worker.resultText.trim()}`)
  }
  return terminalStyle.meta(lines.join("\n"))
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
    const explicitToken = input.args.token?.trim()
    const storedToken = explicitToken === undefined || explicitToken.length === 0
      ? await readStoredAgentToken(input.env)
      : undefined
    if ((explicitToken !== undefined && explicitToken.length > 0) || storedToken !== undefined) {
      lines.push(`traces: agent token configured for ${baseUrlFor(input.args.baseUrl)}/traces (token redacted; validity not checked)`)
      lines.push(`token store: ${explicitToken === undefined || explicitToken.length === 0 ? traceTokenPath(input.env) : "explicit token provided"}`)
    } else {
      lines.push(`traces: sign in with khala login to view owner-only traces at ${baseUrlFor(input.args.baseUrl)}/traces`)
      lines.push(`token store: not configured (${traceTokenPath(input.env)})`)
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lines.push(`traces: unavailable (${message})`)
  }
  return terminalStyle.meta(lines.join("\n"))
}

function resolveTraceUrl(baseUrl: string, info: ChatTurnMetadata | undefined): string | undefined {
  if (info?.traceUrl !== undefined) {
    const traceUrl = info.traceUrl.startsWith("http://") || info.traceUrl.startsWith("https://")
      ? info.traceUrl
      : `${baseUrlFor(baseUrl)}${info.traceUrl.startsWith("/") ? "" : "/"}${info.traceUrl}`
    return redactTraceTokenQuery(traceUrl)
  }
  if (info?.traceUuid !== undefined) {
    return `${baseUrlFor(baseUrl)}/trace/${encodeURIComponent(info.traceUuid)}`
  }
  return undefined
}

function redactTraceTokenQuery(traceUrl: string): string {
  try {
    const parsed = new URL(traceUrl)
    if (!parsed.searchParams.has("token")) return traceUrl
    parsed.searchParams.delete("token")
    return `${parsed.toString()} (token redacted)`
  } catch {
    if (!/[?&]token=/i.test(traceUrl)) return traceUrl
    const redacted = traceUrl
      .replace(/([?&])token=[^&#]*&?/i, "$1")
      .replace(/[?&]($|#)/, "$1")
      .replace(/[?&]$/, "")
    return `${redacted} (token redacted)`
  }
}

function baseUrlFor(baseUrl: string): string {
  return baseUrl.replace(/\/+$/, "")
}

function createCliSessionId(): string {
  return `khala_cli.${crypto.randomUUID()}`
}

// First-run welcome shown at the top of every interactive session. Kept short,
// clean, and inviting so a brand-new user immediately knows what Khala is and
// how to start.
function welcomeBanner(): string {
  return [
    `${terminalStyle.assistant(`Khala v${KHALA_CLI_VERSION}`)}${terminalStyle.meta(" — collective intelligence from OpenAgents")}`,
    "Ask anything and the answer streams back. No signup, no API key needed.",
    terminalStyle.meta("Type /help for commands, /exit to quit."),
  ].join("\n")
}

function interactiveHelp(): string {
  return `${terminalStyle.assistant("Khala")} is a collective intelligence you chat with from your terminal.
Just type a message and press Enter — answers stream back live. It's free to use.

${terminalStyle.meta("Slash commands:")}
/help              Show this command list
/key               Add or manage a bring-your-own provider API key (BYOK)
/login             Sign in as the owner (then you can talk to Artanis)
/logout            Clear the stored Khala token
/artanis           Switch to the owner-only Artanis operator channel
/khala             Switch back to the public Khala channel
/feedback <text>   Save product feedback with the last trace when available
/info              Show this CLI session and trace viewing link
/msginfo           Show metadata for the last Khala response
/codex status      Show local Codex connection status
/codex connect     Connect Codex to Khala with device auth
/codex <task>      Delegate a workspace task directly to Codex
/spawn 5 <task>    Spawn supervised Khala child workers
/workers           List local Khala spawn runs
/worker <ref>      Show one Khala worker
/join <runRef>     Show an aggregate Khala spawn run
/cancel <ref>      Cancel a Khala spawn run or worker
/tokens            Show global Khala tokens served
/changelog         Show recent Khala CLI changes
/version           Show the installed Khala CLI version
/exit, /quit       Quit

${terminalStyle.meta("Tell a friend:")} npm install -g @openagentsinc/khala
`
}

function usage(): string {
  return `Khala CLI — a collective intelligence you chat with from your terminal.
Free to use, streams answers live, no signup or API key required.

Get started:
  khala                       Start an interactive chat
  khala --prompt "hello"      Ask one question and print the answer
  npm install -g @openagentsinc/khala   Install / share

Usage:
  khala
  khala help
  khala login
  khala logout
  khala resume <sessionId>
  khala resume <sessionId> --prompt "continue"
  khala fork <sessionId>
  khala fork <sessionId> --prompt "try another path"
  khala info
  khala key add <provider> <api-key>
  khala key list
  khala key remove
  khala version
  khala changelog
  khala tokens
  khala fleet connect
  khala fleet connect --account codex-2
  khala fleet link
  khala fleet status
  khala fleet run --repo owner/repo --issues 123,124 --verify "bun test" --dry-run
  khala fleet status --live
  khala auth codex
  khala codex status
  khala codex "read README.md"
  khala spawn --count 5 --objective "audit this workspace" --strategy local
  khala spawn --strategy pylon --count 5 --objective "implement public issue #123" --repo OpenAgentsInc/openagents --commit <sha> --verify "bun test"
  khala workers
  khala worker <workerRef>
  khala join <runRef>
  khala cancel <runRef|workerRef>
  khala feedback "The transcript disappeared"
  khala --prompt "Say hello"
  khala --headless --json < prompt.txt
  khala --api --token "$OPENAGENTS_AGENT_TOKEN" --prompt "Hello"
  khala --artanis --prompt "What are you working on right now?"

Interactive commands:
  /help              Show slash commands
  /login             Sign in as the owner (then you can talk to Artanis)
  /logout            Clear the stored Khala token
  /artanis           Switch to the owner-only Artanis operator channel
  /khala             Switch back to the public Khala channel
  /feedback <text>   Save product feedback without sending it to inference
  /info              Show this CLI session and trace viewing link
  /msginfo           Show metadata for the last Khala response
  /codex status      Show local Codex connection status
  /codex connect     Connect Codex to Khala with device auth
  /codex <task>      Delegate a workspace task directly to Codex
  /spawn 5 <task>    Spawn supervised Khala child workers
  /workers           List local Khala spawn runs
  /worker <ref>      Show one Khala worker
  /join <runRef>     Show an aggregate Khala spawn run
  /cancel <ref>      Cancel a Khala spawn run or worker
  /tokens            Show global Khala tokens served
  /changelog         Show recent Khala CLI changes
  /version           Show the installed Khala CLI version
  /exit              Quit

Flags:
  --public             Use /api/khala/chat (default, no auth)
  --api                Use /api/v1/chat/completions with openagents/khala
  --artanis            Talk to the owner-only Artanis operator channel
  --session-dir <dir>  Store/read append-only JSONL sessions in this state dir
                       (POST /api/operator/artanis/chat, owner token required)
  --khala              Use the public Khala channel (default)
  --base-url <url>     Override ${DEFAULT_BASE_URL}
  --token <token>      Bearer token for --api (env: OPENAGENTS_AGENT_TOKEN)
  --prompt, -p <text>  Run one headless prompt
  --headless           Read one prompt from stdin when --prompt is absent
  --json, --no-stream  Print final JSON instead of streaming deltas
  --models             Print /api/v1/models JSON
  --mint-free-key      Call POST /api/keys/free and print the response once
  --count <n>          Worker count for khala spawn (default: 1; max: 10 local, 20 pylon)
  --max-parallel <n>   Max concurrent workers for khala spawn
  --objective <text>   Objective text for khala spawn
  --strategy <mode>    Spawn strategy: auto, local, or pylon
  --pylon-ref <ref>    Caller-owned target Pylon ref for --strategy pylon
  --fixture            Use the bounded public fixture for --strategy pylon
  --repo <owner/repo>  Public GitHub repo for --strategy pylon workspace work
  --branch <name>      Public repo branch for --strategy pylon (default: main)
  --commit <sha>       Pinned public repo commit for --strategy pylon
  --verify <command>   Bounded verification command for --strategy pylon
  --workflow <name>    Pylon workflow: claude_agent_task, codex_agent_task, or cloud_coding_session
  --timeout <seconds>  Per-worker timeout for khala spawn
  --account <ref>      Codex account ref for khala fleet connect (auto-assigned if omitted)
  --force              Re-run device login for khala fleet connect even if already linked
  --issues <list>      Public issue numbers for khala fleet run, comma or space separated
  --per-account <n>    Fleet run slots per ready Codex account (default: 1)
  --once               Fleet run one refill round, then exit
  --dry-run            Fleet run prints the resolved plan without dispatching
  --live               Poll /api/operator/fleet/state for khala fleet status
  --help, -h           Show this help
`
}
