#!/usr/bin/env bun

import { readFile } from "node:fs/promises"
import { TASSADAR_EXECUTOR_CAPABILITY_REF } from "@openagents/tassadar-executor"
import {
  loadClaudeAgentConfig,
  probeClaudeAgentReadiness,
  withClaudeAgentCapability,
} from "./claude-agent"
import { claimTipReadiness, readBalance, setTipPreferences, sweepStatus, tipPost } from "./tips"
import {
  ARTANIS_FORUM_SLUG,
  appendMemory,
  composeAskArtanisBody,
  forumPostTopic,
  forumReadTopic,
  forumReply,
  readMemories,
  resolveModelAdapter,
} from "./agent-surface"
import { Console, Deferred, Effect, PubSub, Stream, SubscriptionRef } from "effect"
import {
  formatLogTimestamp,
  classifyServiceLogLevel,
  isLogEntryVisible,
  type PylonLogEntry,
  type PylonLogLevel,
  type TelemetryPaneState,
  type WalletPaneState,
} from "./node/state"
import {
  forkNodeServices,
  logMessage,
  makePylonNodeRuntime,
  superviseLoop,
  type PylonNodeRuntime,
} from "./node/runtime"
import {
  createCliRenderer,
  BoxRenderable,
  TextRenderable,
  ScrollBoxRenderable,
  MarkdownRenderable,
  TextareaRenderable,
  MacOSScrollAccel,
  parseColor,
  SyntaxStyle,
  type CliRenderer
} from "@opentui/core"
import { runProbeCli } from "../packages/runtime/src/index"
import {
  createBootstrapSummary,
  formatBootstrapText,
  parseBootstrapArgs,
  writeBootstrapFiles,
} from "./bootstrap"
import { ensurePylonLocalState, projectPublicStatus, writeRuntimeState } from "./state"
import {
  completePylonLink,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
} from "./presence"
import {
  admitPayoutTarget,
  classifyMdkWallet,
  preflightLegacySparkMigration,
  receiveWithMdk,
  reportWalletReadiness,
  requestPayoutTargetAdmission,
  sendWithMdk,
} from "./wallet"
import {
  acceptAssignment,
  pollAssignments,
  runNoSpendAssignment,
  submitAssignmentCloseout,
  submitAssignmentProgress,
  type AssignmentCloseout,
  type AssignmentProgress,
  type PylonAssignmentLease,
} from "./assignment"
import { discoverHostInventory } from "./inventory"
import { createOperatorSnapshot, formatOperatorSnapshotText } from "./operator"
import { inspectPsionicConnector } from "./psionic-connector"
import {
  installPsionicBinary,
  installPsionicModelArtifact,
} from "./psionic-install"
import {
  PYLON_NIP90_PROVIDER_CAPABILITY_REF,
  policyFromEnv,
  relaysFromEnv,
  startNip90ProviderLoop,
} from "./provider-nip90"
import { PYLON_LABOR_CAPABILITY_REF, approveLaborFirstRun } from "./labor"
import {
  acceptPylonWorkOffer,
  createPylonWorkRequest,
  listPylonWorkOffers,
  readPylonWorkStatus,
  workAcceptanceMemoryEntry,
  workRequestMemoryEntry,
} from "./work-requester"

// View-layer plumbing. These renderable refs are written ONLY by the UI
// subscriber (subscribeUiToRuntime) and the composer's interactive path —
// node services go through the PylonNodeRuntime state/event seam instead
// (issue #4736).
let globalRenderer: CliRenderer | null = null
let logScrollBox: ScrollBoxRenderable | null = null
let balanceTextRenderable: TextRenderable | null = null
let statusTextRenderable: TextRenderable | null = null
let telemetryTextRenderable: TextRenderable | null = null
let operatorTextRenderable: TextRenderable | null = null

// Bridge for legacy call sites (OpenCode helpers) that log from plain
// async code. Set once at dashboard boot, before any service starts.
let nodeRuntime: PylonNodeRuntime | null = null
// Quiet by default: verbose service chatter is hidden unless --verbose or
// PYLON_VERBOSE=1 (issue #4743).
let verboseMode = false

const terminalScrollLockOn = "\x1b[?1007h"
const terminalScrollLockOff = "\x1b[?1007l"
const sgrMousePattern = /\x1b\[<(\d+);(\d+);(\d+)([mM])/g

const syntaxStyle = SyntaxStyle.fromStyles({
  default: { fg: parseColor("#E6EDF3") },
  keyword: { fg: parseColor("#FF7B72"), bold: true },
  string: { fg: parseColor("#A5D6FF") },
  comment: { fg: parseColor("#8B949E"), italic: true },
  number: { fg: parseColor("#79C0FF") },
  function: { fg: parseColor("#D2A8FF") },
  type: { fg: parseColor("#FFA657") },
  variable: { fg: parseColor("#E6EDF3") },
  property: { fg: parseColor("#79C0FF") },
  "markup.heading": { fg: parseColor("#00D7FF"), bold: true },
  "markup.bold": { fg: parseColor("#F0F6FC"), bold: true },
  "markup.italic": { fg: parseColor("#F0F6FC"), italic: true },
  "markup.list": { fg: parseColor("#FF7B72") },
  "markup.quote": { fg: parseColor("#8B949E"), italic: true },
  "markup.raw": { fg: parseColor("#A5D6FF"), bg: parseColor("#161B22") },
  "markup.link": { fg: parseColor("#58A6FF"), underline: true },
  "markup.link.url": { fg: parseColor("#58A6FF"), underline: true },
  conceal: { fg: parseColor("#6E7681") },
})

// Routes a log message into the node runtime (which owns the feed and the
// event stream). Pre-boot or in plain CLI paths it falls back to stdout.
function logToUi(message: string, level: PylonLogLevel = "verbose") {
  if (nodeRuntime) {
    Effect.runFork(logMessage(nodeRuntime, level, message))
    return
  }
  if (level !== "verbose" || verboseMode) {
    void Effect.runPromise(Console.log(`[BOOT] ${message}`))
  }
}

// Effect-native logging helper (defaults to verbose — hidden unless
// --verbose / PYLON_VERBOSE=1)
const log = (message: string, level: PylonLogLevel = "verbose") =>
  Effect.sync(() => logToUi(message, level))

// View-side renderers — called only from the UI subscriber.
function appendLogLine(entry: PylonLogEntry) {
  if (!logScrollBox || !globalRenderer) return
  const line = makeLogMarkdown(globalRenderer, {
    content: `[${formatLogTimestamp(entry.at)}] ${entry.message}`,
    syntaxStyle,
    width: "100%",
    conceal: true,
    fg: parseColor(entry.level === "error" ? "#EF4444" : "#5C7080"),
  })
  logScrollBox.add(line)
}

function applyWalletUi(state: WalletPaneState) {
  const online = state.daemonOnline && state.balanceSats !== null
  if (balanceTextRenderable) {
    balanceTextRenderable.content =
      state.balanceSats === null ? " Balance: -- Sats" : ` Balance: ${state.balanceSats.toLocaleString()} Sats`
  }
  if (statusTextRenderable) {
    statusTextRenderable.content = online ? " Wallet: ONLINE (OK)" : " Wallet: OFFLINE"
    statusTextRenderable.fg = parseColor(online ? "#58A6FF" : "#EF4444")
  }
}

function applyTelemetryUi(state: TelemetryPaneState) {
  if (telemetryTextRenderable) {
    telemetryTextRenderable.content = ` State: ${state.state}\n Model: ${state.model}\n VRAM:  ${state.vram}\n Psionic: ${state.psionic}`
  }
}

function applyOperatorUi(text: string) {
  if (operatorTextRenderable) {
    operatorTextRenderable.content = text
  }
}

// Subscribes the renderables to runtime state: render current values once,
// then follow changes. Log entries follow the event stream with verbosity
// filtering. All consumers are scoped fibers, interrupted on shutdown.
const subscribeUiToRuntime = (runtime: PylonNodeRuntime) =>
  Effect.gen(function* () {
    // Subscribe before reading the feed so no event falls between replay and
    // live tail. No other fiber writes during this window (services fork
    // after the UI subscription), so replay+tail cannot duplicate either.
    const eventSubscription = yield* PubSub.subscribe(runtime.events)
    applyWalletUi(yield* SubscriptionRef.get(runtime.wallet))
    applyTelemetryUi(yield* SubscriptionRef.get(runtime.telemetry))
    applyOperatorUi((yield* SubscriptionRef.get(runtime.operator)).text)
    for (const entry of yield* SubscriptionRef.get(runtime.logFeed)) {
      if (isLogEntryVisible(entry, verboseMode)) appendLogLine(entry)
    }
    yield* Effect.forkScoped(
      Stream.runForEach(SubscriptionRef.changes(runtime.wallet), (state) =>
        Effect.sync(() => applyWalletUi(state)),
      ),
    )
    yield* Effect.forkScoped(
      Stream.runForEach(SubscriptionRef.changes(runtime.telemetry), (state) =>
        Effect.sync(() => applyTelemetryUi(state)),
      ),
    )
    yield* Effect.forkScoped(
      Stream.runForEach(SubscriptionRef.changes(runtime.operator), (state) =>
        Effect.sync(() => applyOperatorUi(state.text)),
      ),
    )
    yield* Effect.forkScoped(
      Effect.gen(function* () {
        while (true) {
          const event = yield* PubSub.take(eventSubscription)
          if (event.type === "log" && isLogEntryVisible(event, verboseMode)) {
            appendLogLine(event)
          }
        }
      }),
    )
  })

function installTerminalScrollLock() {
  if (!process.stdout.isTTY) {
    return () => {}
  }

  process.stdout.write(terminalScrollLockOn)
  let restored = false
  const restore = () => {
    if (restored) return
    restored = true
    process.stdout.write(terminalScrollLockOff)
  }

  process.once("exit", restore)

  return restore
}

function scrollLogBy(delta: number, unit: "absolute" | "viewport" | "content" | "step" = "absolute") {
  logScrollBox?.scrollBy(delta, unit)
}

function isPointInsideLogScrollBox(x: number, y: number) {
  if (!logScrollBox) return false
  const box = logScrollBox as any
  const left = box.screenX ?? box.x ?? 0
  const top = box.screenY ?? box.y ?? 0
  const width = box.width ?? 0
  const height = box.height ?? 0

  return x >= left && x < left + width && y >= top && y < top + height
}

function scrollLogByWheelButton(button: number) {
  if (!logScrollBox) return false
  const wheelButton = button & 0b11
  const step = Math.max(3, Math.ceil((logScrollBox.viewport?.height ?? logScrollBox.height ?? 15) / 6))

  if (wheelButton === 0) {
    logScrollBox.scrollBy(-step)
    return true
  }
  if (wheelButton === 1) {
    logScrollBox.scrollBy(step)
    return true
  }
  if (wheelButton === 2) {
    logScrollBox.scrollBy({ x: -step, y: 0 })
    return true
  }
  if (wheelButton === 3) {
    logScrollBox.scrollBy({ x: step, y: 0 })
    return true
  }

  return false
}

function handleRawLogWheel(sequence: string) {
  if (!logScrollBox) return false
  let handled = false
  sgrMousePattern.lastIndex = 0

  for (const match of sequence.matchAll(sgrMousePattern)) {
    const button = Number(match[1])
    if ((button & 64) !== 64) continue

    const x = Number(match[2]) - 1
    const y = Number(match[3]) - 1
    if (!isPointInsideLogScrollBox(x, y)) continue

    logScrollBox.focus()
    handled = scrollLogByWheelButton(button) || handled
  }

  return handled
}

function scrollLogFromWheel(event: any) {
  if (!logScrollBox || event.type !== "scroll") return false

  ;(logScrollBox as any).onMouseEvent?.(event)
  return true
}

function routeLogMouse(event: any) {
  if (!logScrollBox) return
  logScrollBox.focus()
  if (event.type === "scroll") {
    scrollLogFromWheel(event)
    event.stopPropagation?.()
    event.preventDefault?.()
  }
}

function sinkRootScroll(event: any) {
  if (event.type !== "scroll") return
  if (logScrollBox) {
    logScrollBox.focus()
    scrollLogFromWheel(event)
  }
  event.stopPropagation?.()
  event.preventDefault?.()
}

function makeLogMarkdown(
  renderer: CliRenderer,
  options: ConstructorParameters<typeof MarkdownRenderable>[1],
) {
  return new MarkdownRenderable(renderer, {
    ...options,
    onMouseDown: routeLogMouse,
    onMouseScroll: routeLogMouse,
  })
}

function handleLogKey(key: any, focusComposer?: () => void) {
  if (key.name === "tab") {
    focusComposer?.()
  } else if (key.name === "up") {
    scrollLogBy(-1, "step")
  } else if (key.name === "down") {
    scrollLogBy(1, "step")
  } else if (key.name === "pageup") {
    scrollLogBy(-0.8, "viewport")
  } else if (key.name === "pagedown") {
    scrollLogBy(0.8, "viewport")
  } else if (key.name === "home") {
    scrollLogBy(-1, "content")
  } else if (key.name === "end") {
    scrollLogBy(1, "content")
  } else if (key.meta && key.name === "up") {
    scrollLogBy(-0.5, "viewport")
  } else if (key.meta && key.name === "down") {
    scrollLogBy(0.5, "viewport")
  } else {
    return false
  }
  key.preventDefault?.()
  key.stopPropagation?.()
  return true
}

// Builds the operator-pane text the telemetry service publishes. The wallet
// projection here is the pre-seam placeholder (always offline) — wiring the
// live wallet state into the snapshot is follow-up work, kept behavior-equal
// for Phase 0.
function operatorTextFromInventory(inventory: Awaited<ReturnType<typeof discoverHostInventory>>) {
  const wallet = {
    schema: "openagents.pylon.wallet_status.v0.3" as const,
    configured: false,
    daemonOnline: false,
    balanceSats: null,
    receiveReady: false,
    sendReady: false,
    readiness: "daemon-offline" as const,
    blockerRefs: ["blocker.wallet.daemon_offline"],
    payoutTargetRefs: [],
    settlementRefs: [],
  }
  return formatOperatorSnapshotText(
    createOperatorSnapshot({ inventory, wallet: wallet as Parameters<typeof createOperatorSnapshot>[0]["wallet"] }),
  )
}

type OpenCodeInferenceOptions = {
  label: string
  streamToUi?: boolean
  statusIntervalMs?: number
}

function summarizeOpenCodeEvent(event: any) {
  const type = typeof event?.type === "string" ? event.type : "event"
  const partType = typeof event?.part?.type === "string" ? event.part.type : undefined
  const tool = event?.part?.tool ?? event?.tool ?? event?.name
  const title = event?.part?.title ?? event?.title
  const path = event?.part?.path ?? event?.path
  const detail = [partType, tool, title, path].filter(Boolean).join(" ")
  return detail ? `${type}: ${detail}` : type
}

// OpenCode Programmatic Integration Helper
async function executeOpencodeInference(
  opencodePath: string,
  prompt: string,
  options: OpenCodeInferenceOptions,
) {
  const proc = Bun.spawn(
    [
      opencodePath,
      "run",
      prompt,
      "--model",
      "opencode/deepseek-v4-flash-free",
      "--format",
      "json",
    ],
    {
      stdout: "pipe",
      stderr: "pipe",
    }
  )

  const responseLine =
    options.streamToUi && verboseMode && globalRenderer
      ? makeLogMarkdown(globalRenderer, {
          content: `**OpenCode ${options.label}**: starting...`,
          syntaxStyle,
          width: "100%",
          conceal: true,
          streaming: true,
        })
      : null
  if (responseLine) {
    logScrollBox?.add(responseLine)
  }

  const startTime = Date.now()
  const statusIntervalMs = options.statusIntervalMs ?? 5000
  let textResult = ""
  let finalCost = 0
  let totalTokens = 0
  let eventCount = 0
  let byteCount = 0
  let lastEventSummary = "waiting for first event"

  const renderStreamingLine = () => {
    if (!responseLine) return
    const elapsedSeconds = Math.max(1, Math.round((Date.now() - startTime) / 1000))
    const visibleText = textResult.trim() || `_${lastEventSummary}_`
    const footer = `\n\n*[${options.label}: ${elapsedSeconds}s | events: ${eventCount} | bytes: ${byteCount} | tokens: ${totalTokens || "-"}]*`
    responseLine.content = `**OpenCode ${options.label}**: ${visibleText}${footer}`
  }

  const statusTimer = setInterval(() => {
    const elapsedSeconds = Math.max(1, Math.round((Date.now() - startTime) / 1000))
    logToUi(
      `[OpenCode] ${options.label} still running (${elapsedSeconds}s, events=${eventCount}, bytes=${byteCount}, last=${lastEventSummary}).`,
    )
    renderStreamingLine()
  }, statusIntervalMs)

  const stderrTask = (async () => {
    const reader = proc.stderr.getReader()
    const decoder = new TextDecoder()
    let buffer = ""
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split("\n")
      buffer = lines.pop() ?? ""
      for (const line of lines) {
        if (line.trim()) {
          logToUi(`[OpenCode] ${options.label} stderr: ${line.trim()}`)
        }
      }
    }
    const trailing = buffer.trim()
    if (trailing) {
      logToUi(`[OpenCode] ${options.label} stderr: ${trailing}`)
    }
  })()

  try {
    const reader = proc.stdout.getReader()
    const decoder = new TextDecoder()
    let buffer = ""

    while (true) {
      const { done, value } = await reader.read()
      if (done) break

      byteCount += value.byteLength
      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split("\n")
      buffer = lines.pop() ?? ""

      for (const line of lines) {
        if (!line.trim()) continue
        eventCount += 1
        try {
          const event = JSON.parse(line)
          lastEventSummary = summarizeOpenCodeEvent(event)
          if (event.type !== "text") {
            logToUi(`[OpenCode] ${options.label} event ${eventCount}: ${lastEventSummary}`)
          }
          if (event.type === "text" && event.part && event.part.text) {
            textResult += event.part.text
            renderStreamingLine()
          }
          if (event.type === "step_finish" && event.part && event.part.tokens) {
            finalCost = event.part.cost ?? 0
            totalTokens = event.part.tokens.total ?? 0
            renderStreamingLine()
          }
        } catch {
          lastEventSummary = `raw output: ${line.slice(0, 120)}`
          logToUi(`[OpenCode] ${options.label} raw output: ${line.slice(0, 240)}`)
          renderStreamingLine()
        }
      }
    }

    const trailing = buffer.trim()
    if (trailing) {
      eventCount += 1
      try {
        const event = JSON.parse(trailing)
        lastEventSummary = summarizeOpenCodeEvent(event)
      } catch {
        lastEventSummary = `raw output: ${trailing.slice(0, 120)}`
        logToUi(`[OpenCode] ${options.label} raw output: ${trailing.slice(0, 240)}`)
      }
    }
  } finally {
    clearInterval(statusTimer)
  }

  const exitCode = await proc.exited
  await stderrTask
  if (responseLine) {
    responseLine.streaming = false
  }
  if (exitCode !== 0) {
    throw new Error(`OpenCode exited with code ${exitCode}`)
  }

  return {
    text: textResult.trim(),
    cost: finalCost,
    tokens: totalTokens,
  }
}

// OpenCode Programmatic Integration Service (Diagnostics on boot)
const runOpencodeStartupInference = Effect.gen(function* () {
  yield* log("[OpenCode] Checking for local OpenCode CLI installation...")
  const opencodePath = Bun.which("opencode")
  if (opencodePath) {
    yield* log(`[OpenCode] Found OpenCode CLI at ${opencodePath}. Initiating bootup diagnostics...`)

    // 1. Get neutral log summary (<10 words)
    const bootLogs = nodeRuntime
      ? (yield* SubscriptionRef.get(nodeRuntime.logFeed))
          .map((entry) => `[${formatLogTimestamp(entry.at)}] ${entry.message}`)
          .join("\n")
      : ""
    const logSummaryResult = yield* Effect.tryPromise({
      try: () => {
        const prompt = `Here are the bootup sequence logs:\n\n${bootLogs}\n\nProvide a one line, <10 word, neutral, terminal-sounding summary of these bootup sequence logs.`
        return executeOpencodeInference(opencodePath, prompt, {
          label: "boot-summary",
          streamToUi: true,
        })
      },
      catch: (error) => new Error(`Failed to execute bootup summary: ${String(error)}`),
    })

    yield* log(`[OpenCode] Bootup Summary: "${logSummaryResult.text}"`)
    yield* log(`[OpenCode] Cost: $${logSummaryResult.cost.toFixed(4)} | Tokens: ${logSummaryResult.tokens}`)

    // 2. Read AGENTS.md and contribute a useful post somewhere on the site.
    yield* log("[OpenCode] Reading https://openagents.com/AGENTS.md and creating a useful site post...")
    const contributionResult = yield* Effect.tryPromise({
      try: () => {
        const prompt = [
          "Read https://openagents.com/AGENTS.md and follow its current agent instructions.",
          "If you have no registered agent identity, register one for this local Pylon/OpenCode context.",
          "Inspect the Forum board, recent posts, and search if needed before choosing where to contribute.",
          "Decide explicitly whether the contribution belongs as a new topic or as a reply to an existing topic.",
          "Reply only when the contribution is directly on-topic for an existing thread; create a new topic when the idea is distinct, would hijack the thread, or starts a reusable reference/discussion.",
          "Then add value on openagents.com with a concise public Forum post.",
          "The post should be useful to other agents or operators, not promotional, and should not expose secrets, wallet material, private repo content, or credentials.",
          "When finished, report the placement decision, topic/post URL, and a one-sentence summary of the value added.",
        ].join(" ")
        return executeOpencodeInference(opencodePath, prompt, {
          label: "site-post",
          streamToUi: true,
        })
      },
      catch: (error) => new Error(`Failed to execute site contribution: ${String(error)}`),
    })

    yield* log(`[OpenCode] Site Contribution:\n${contributionResult.text}`)
    yield* log(`[OpenCode] Cost: $${contributionResult.cost.toFixed(4)} | Tokens: ${contributionResult.tokens}`)
  } else {
    yield* log("[OpenCode] OpenCode CLI is not installed on this system.")
  }
})

// Main Pylon v0.3 Application Loop. Runs inside a Scope (Effect.scoped at
// the call site): every service fiber is forked into that Scope, and the
// renderer/terminal are restored by finalizers, so Ctrl+C is a deliberate
// interruption instead of process death (issue #4736).
const runPylonNode = Effect.gen(function* () {
  const smokeDashboard = Bun.argv.includes("--smoke-dashboard") || Bun.env.PYLON_SMOKE_DASHBOARD === "1"
  verboseMode = Bun.argv.includes("--verbose") || Bun.env.PYLON_VERBOSE === "1"

  const runtime = yield* makePylonNodeRuntime
  nodeRuntime = runtime

  const shutdown = yield* Deferred.make<void>()
  const requestShutdown = () => {
    Effect.runFork(Deferred.succeed(shutdown, void 0))
  }
  process.once("SIGINT", requestShutdown)
  process.once("SIGTERM", requestShutdown)
  const interceptCtrlC = (sequence: string) => {
    if (sequence.includes("\x03")) {
      requestShutdown()
      return true
    }
    return false
  }

  yield* logMessage(runtime, "verbose", "Initializing Pylon v0.3 observational earning node...")
  const restoreTerminalScrollLock = installTerminalScrollLock()

  // Bootstrap OpenTUI Core
  const renderer = yield* Effect.tryPromise({
    try: () =>
      createCliRenderer({
        screenMode: "alternate-screen",
        exitOnCtrlC: false,
        useMouse: true,
        autoFocus: true,
        targetFps: 30,
        prependInputHandlers: [interceptCtrlC, handleRawLogWheel],
      }),
    catch: (error) => new Error(`Failed to initialize OpenTUI renderer: ${String(error)}`),
  })

  globalRenderer = renderer
  yield* Effect.addFinalizer(() =>
    Effect.sync(() => {
      // destroy() is OpenTUI's full teardown (leaves the alternate screen,
      // restores the cursor); stop() alone keeps the terminal captured.
      const teardown = renderer as { destroy?: () => void; stop?: () => void }
      if (typeof teardown.destroy === "function") {
        teardown.destroy()
      } else {
        teardown.stop?.()
      }
      restoreTerminalScrollLock()
      globalRenderer = null
      logScrollBox = null
      balanceTextRenderable = null
      statusTextRenderable = null
      telemetryTextRenderable = null
      operatorTextRenderable = null
    }),
  )

  // 1. Create Main Outer Layout (Height 100%, Width 100%)
  const outerContainer = new BoxRenderable(renderer, {
    flexDirection: "column",
    width: "100%",
    height: "100%",
    onMouseScroll: sinkRootScroll,
  })
  renderer.root.add(outerContainer)

  // 3. Create Main Split Pane (Row Direction, Flex Grow)
  const splitPane = new BoxRenderable(renderer, {
    flexDirection: "row",
    width: "100%",
    flexGrow: 1,
  })
  outerContainer.add(splitPane)

  // 3a. Logs/Feed Panel (Left Column, Flex Grow)
  const leftPanel = new BoxRenderable(renderer, {
    border: true,
    borderStyle: "single",
    borderColor: parseColor("#73C2FB"),
    title: " // Active Workroom Execution Logs ",
    titleColor: parseColor("#73C2FB"),
    flexGrow: 1,
    height: "100%",
  })
  splitPane.add(leftPanel)

  logScrollBox = new ScrollBoxRenderable(renderer, {
    scrollY: true,
    stickyScroll: true,
    stickyStart: "bottom",
    scrollAcceleration: new MacOSScrollAccel(),
    focusable: true,
    flexGrow: 1,
    width: "100%",
    height: "100%",
    onMouseDown: routeLogMouse,
    onMouseScroll: routeLogMouse,
  })
  leftPanel.add(logScrollBox)

  // Seed the feed before the first layout pass. The scrollbox silently
  // swallows the first child added around its initial frame (observed
  // OpenTUI 0.3.4 quirk; the old startup masked it under log spam), so the
  // sacrificial line is this banner rather than a real log entry.
  logScrollBox.add(
    makeLogMarkdown(renderer, {
      content: "*Pylon v0.3*",
      syntaxStyle,
      width: "100%",
      conceal: true,
      fg: parseColor("#3B5B82"),
    }),
  )

  // 3b. Telemetry & Balance Panel (Right Column, Fixed Width 35)
  const rightPanel = new BoxRenderable(renderer, {
    border: true,
    borderStyle: "single",
    borderColor: parseColor("#73C2FB"),
    title: " // Telemetry & Wallet ",
    titleColor: parseColor("#73C2FB"),
    width: 35,
    flexBasis: 35,
    flexGrow: 0,
    flexShrink: 0,
    height: "100%",
    flexDirection: "column",
  })
  splitPane.add(rightPanel)

  statusTextRenderable = new TextRenderable(renderer, {
    content: " Wallet: OFFLINE",
    fg: parseColor("#EF4444"),
    width: "100%",
    height: 1,
  })
  rightPanel.add(statusTextRenderable)

  balanceTextRenderable = new TextRenderable(renderer, {
    content: " Balance: -- Sats",
    fg: parseColor("#66D9EF"),
    width: "100%",
    height: 1,
  })
  rightPanel.add(balanceTextRenderable)

  // Add some separator space
  rightPanel.add(new TextRenderable(renderer, { content: " ---------------------------------", fg: parseColor("#3B5B82"), height: 1 }))

  telemetryTextRenderable = new TextRenderable(renderer, {
    content: " State: IDLE\n Model: -\n VRAM:  -",
    fg: parseColor("#D7E5FA"),
    width: "100%",
    height: 3,
  })
  rightPanel.add(telemetryTextRenderable)

  rightPanel.add(new TextRenderable(renderer, { content: " ---------------------------------", fg: parseColor("#3B5B82"), height: 1 }))
  operatorTextRenderable = new TextRenderable(renderer, {
    content: " Operate: loading\n Inspect: loading\n Recovery: loading",
    fg: parseColor("#D7E5FA"),
    width: "100%",
    flexGrow: 1,
  })
  rightPanel.add(operatorTextRenderable)

  // 4. Create Composer Input Panel (Bottom, Height 5)
  const composerBox = new BoxRenderable(renderer, {
    border: true,
    borderStyle: "single",
    borderColor: parseColor("#73C2FB"),
    title: " // Composer (meta+return to submit) ",
    titleColor: parseColor("#73C2FB"),
    width: "100%",
    height: 5,
  })
  outerContainer.add(composerBox)

  const composerInput = new TextareaRenderable(renderer, {
    width: "100%",
    height: "100%",
    placeholder: "Ask your agent anything...",
    onKeyDown: (key) => {
      if (key.name === "tab") {
        logScrollBox?.focus()
        key.preventDefault?.()
        key.stopPropagation?.()
        return
      }
      handleLogKey(key)
    },
    onMouseDown: () => {
      composerInput.focus()
    },
    onSubmit: async () => {
      const prompt = composerInput.plainText.trim()
      if (!prompt) return

      // Clear the composer
      composerInput.setText("")

      // Render User prompt in logs feed
      const userLine = makeLogMarkdown(renderer, {
        content: `**User**: ${prompt}`,
        syntaxStyle,
        width: "100%",
        conceal: true,
      })
      logScrollBox?.add(userLine)

      // Setup response placeholder
      const responseLine = makeLogMarkdown(renderer, {
        content: `**OpenCode**: ... thinking ...`,
        syntaxStyle,
        width: "100%",
        conceal: true,
        streaming: true,
      })
      logScrollBox?.add(responseLine)

      // Start asynchronous OpenCode inference
      const opencodePath = Bun.which("opencode")
      if (opencodePath) {
        const proc = Bun.spawn(
          [
            opencodePath,
            "run",
            prompt,
            "--model",
            "opencode/deepseek-v4-flash-free",
            "--format",
            "json",
          ],
          {
            stdout: "pipe",
            stderr: "pipe",
          }
        )

        const reader = proc.stdout.getReader()
        const decoder = new TextDecoder()
        let buffer = ""
        let receivedText = ""

        responseLine.content = `**OpenCode**: `

        while (true) {
          const { done, value } = await reader.read()
          if (done) break

          buffer += decoder.decode(value, { stream: true })
          const lines = buffer.split("\n")
          buffer = lines.pop() ?? ""

          for (const line of lines) {
            if (!line.trim()) continue
            try {
              const event = JSON.parse(line)
              if (event.type === "text" && event.part && event.part.text) {
                receivedText += event.part.text
                responseLine.content = `**OpenCode**: ${receivedText}`
              }
              if (event.type === "step_finish" && event.part && event.part.tokens) {
                const cost = event.part.cost ?? 0
                const tokens = event.part.tokens.total ?? 0
                responseLine.content = `**OpenCode**: ${receivedText}\n\n*[Cost: $${cost.toFixed(4)} | Tokens: ${tokens}]*`
              }
            } catch {
              // Ignore partial chunk syntax errors
            }
          }
        }

        responseLine.streaming = false
      } else {
        responseLine.content = `**OpenCode**: Error - OpenCode CLI is not installed on this system.`
        responseLine.streaming = false
      }
    },
  })
  composerBox.add(composerInput)

  logScrollBox.handleKeyPress = (key: any) => {
    return handleLogKey(key, () => composerInput.focus()) || false
  }

  // Start OpenTUI Event Loop
  renderer.start()

  // Focus on Composer Input
  composerInput.focus()

  // Attach the view to the runtime seam: render current state, then follow
  // ref changes and log events. Services are forked only after this, so the
  // feed replay misses nothing.
  yield* subscribeUiToRuntime(runtime)

  yield* logMessage(
    runtime,
    "info",
    verboseMode
      ? "Pylon v0.3 dashboard active (verbose logging)."
      : "Pylon v0.3 ready. Logs are quiet by default - relaunch with --verbose for service detail.",
  )

  const bootstrapSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
  const localState = yield* Effect.tryPromise({
    try: () => ensurePylonLocalState(bootstrapSummary),
    catch: (error) => new Error(`failed to load Pylon Nostr identity: ${String(error)}`),
  })
  yield* logMessage(runtime, "info", `[Identity] Pylon Nostr npub: ${localState.identity.npub}`)

  // Fork node services into this Scope - interrupted on shutdown.
  const presenceBaseUrl = Bun.env.PYLON_OPENAGENTS_BASE_URL
  yield* forkNodeServices(runtime, {
    wallet: {
      classify: () => classifyMdkWallet(),
    },
    telemetry: {
      discoverInventory: () => discoverHostInventory(),
      inspectPsionic: async () => {
        const connector = await inspectPsionicConnector({ env: Bun.env })
        return { phase: connector.phase }
      },
      makeOperatorText: operatorTextFromInventory,
    },
    heartbeat: {
      baseUrl: presenceBaseUrl,
      register: () => registerPylon(bootstrapSummary, { baseUrl: presenceBaseUrl ?? "" }),
      heartbeat: () => sendHeartbeat(bootstrapSummary, { baseUrl: presenceBaseUrl ?? "" }),
    },
  })

  yield* superviseLoop(
    runtime,
    "NIP-90",
    Effect.tryPromise({
      try: () =>
        startNip90ProviderLoop(bootstrapSummary, {
          log: (message) => logToUi(message, classifyServiceLogLevel(message)),
        }).then(() => undefined),
      catch: (error) => new Error(`NIP-90 provider loop failed: ${String(error)}`),
    }),
  )

  if (!smokeDashboard && Bun.env.PYLON_DISABLE_OPENCODE_STARTUP !== "1") {
    yield* superviseLoop(runtime, "OpenCode", runOpencodeStartupInference)
  }

  if (smokeDashboard) {
    yield* logMessage(runtime, "info", "Pylon v0.3 dashboard smoke complete.")
    return
  }

  // Stay alive until Ctrl+C / SIGINT / SIGTERM requests shutdown; closing
  // the surrounding Scope then interrupts every service fiber and runs the
  // renderer/terminal finalizers.
  yield* Deferred.await(shutdown)
})

const runtimeCommandNamespaces = new Set([
  "apple-fm",
  "auth",
  "backend",
  "chat",
  "omega",
])

function parsePresenceOptions(args: string[]) {
  const options: Record<string, string> = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) continue
    const value = args[index + 1]
    if (!value || value.startsWith("--")) {
      throw new Error(`${arg} requires a value`)
    }
    options[arg.slice(2)] = value
    index += 1
  }
  return options
}

function parseKeyValueOptions(args: string[]) {
  return parsePresenceOptions(args)
}

const presenceBootstrapValueOptions = new Set([
  "capability-ref",
  "display-name",
  "pylon-ref",
  "resource-mode",
])

const presenceBootstrapFlagOptions = new Set([
  "register-openagents",
  "setup-mdk-wallet",
])

async function persistedBootstrapArgs(env: NodeJS.ProcessEnv) {
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), env)
  try {
    const config = JSON.parse(await readFile(summary.paths.config, "utf8")) as Record<string, unknown>
    const args: string[] = []
    const pushString = (key: string, flag: string) => {
      const value = config[key]
      if (typeof value === "string" && value.length > 0) {
        args.push(flag, value)
      }
    }

    pushString("pylonRef", "--pylon-ref")
    pushString("displayName", "--display-name")
    pushString("resourceMode", "--resource-mode")

    const capabilityRefs = config.capabilityRefs
    if (Array.isArray(capabilityRefs)) {
      for (const ref of capabilityRefs) {
        if (typeof ref === "string" && ref.length > 0) {
          args.push("--capability-ref", ref)
        }
      }
    }

    return args
  } catch {
    return []
  }
}

function parsePresenceBootstrapArgs(args: string[]) {
  const bootstrapArgs: string[] = []
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) continue
    const key = arg.slice(2)
    if (key === "base-url") {
      index += 1
      continue
    }
    if (presenceBootstrapFlagOptions.has(key)) {
      bootstrapArgs.push(arg)
      continue
    }
    if (presenceBootstrapValueOptions.has(key)) {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) {
        throw new Error(`${arg} requires a value`)
      }
      bootstrapArgs.push(arg, value)
      index += 1
      continue
    }
    throw new Error(`Unknown presence option: ${arg}`)
  }
  return bootstrapArgs
}

async function createPresenceBootstrapSummary(args: string[], env: NodeJS.ProcessEnv) {
  const argsFromConfig = await persistedBootstrapArgs(env)
  const argsFromPresence = parsePresenceBootstrapArgs(args)
  return createBootstrapSummary(
    parseBootstrapArgs(["--json", ...argsFromConfig, ...argsFromPresence]),
    env,
  )
}

function parsePsionicOptions(args: string[]) {
  const options: Record<string, string | true> = {}
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (!arg.startsWith("--")) continue
    const key = arg.slice(2)
    const value = args[index + 1]
    if (!value || value.startsWith("--")) {
      options[key] = true
      continue
    }
    options[key] = value
    index += 1
  }
  return options
}

function stringPsionicOption(options: Record<string, string | true>, key: string) {
  const value = options[key]
  return typeof value === "string" ? value : undefined
}

async function main() {
  const args = Bun.argv.slice(2)

  if (args[0] === "bootstrap") {
    try {
      const options = parseBootstrapArgs(args.slice(1))
      const summary = createBootstrapSummary(options, Bun.env)
      if (!summary.platform.supported) {
        process.stderr.write(
          `Pylon v0.3.0-rc1 supports macOS and Linux only. Current platform: ${summary.platform.current}\n`,
        )
        process.exitCode = 1
        return
      }

      await writeBootstrapFiles(summary)
      const state = await ensurePylonLocalState(summary)
      const output = options.json ? { ...summary, localState: projectPublicStatus(state).state } : formatBootstrapText(summary)
      process.stdout.write(typeof output === "string" ? output : `${JSON.stringify(output, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon bootstrap failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "status" && args.includes("--json")) {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
    const state = await ensurePylonLocalState(summary)
    const inventory = await discoverHostInventory({ env: Bun.env })
    const psionicConnector = await inspectPsionicConnector({ env: Bun.env })
    process.stdout.write(`${JSON.stringify(projectPublicStatus(state, inventory, psionicConnector), null, 2)}\n`)
    return
  }

  if (args[0] === "inventory" && args.includes("--json")) {
    const inventory = await discoverHostInventory({ env: Bun.env })
    process.stdout.write(`${JSON.stringify(inventory, null, 2)}\n`)
    return
  }

  if (args[0] === "operator" && args[1] === "snapshot" && args.includes("--json")) {
    const inventory = await discoverHostInventory({ env: Bun.env })
    const wallet = await classifyMdkWallet()
    const snapshot = createOperatorSnapshot({ inventory, wallet })
    process.stdout.write(`${JSON.stringify(snapshot, null, 2)}\n`)
    return
  }

  if (args[0] === "psionic") {
    try {
      const command = args[1]
      if (command === "doctor") {
        const result = await Effect.runPromise(runProbeCli(["backend", "psionic", "doctor", ...args.slice(2)], { env: Bun.env }))
        if (result.stdout) process.stdout.write(result.stdout)
        if (result.stderr) process.stderr.write(result.stderr)
        process.exitCode = result.exitCode
        return
      }

      if (command === "smoke") {
        const result = await Effect.runPromise(runProbeCli(["backend", "psionic", "smoke", ...args.slice(2)], { env: Bun.env }))
        if (result.stdout) process.stdout.write(result.stdout)
        if (result.stderr) process.stderr.write(result.stderr)
        process.exitCode = result.exitCode
        return
      }

      const options = parsePsionicOptions(args.slice(command === "models" ? 4 : 2))
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      if (command === "install") {
        const result = await installPsionicBinary(summary, {
          channel: stringPsionicOption(options, "channel") ?? "rc",
          manifestUrl: stringPsionicOption(options, "manifest-url"),
          consent: options.yes === true,
          env: Bun.env,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        process.exitCode = result.state === "installed" ? 0 : 1
        return
      }

      if (command === "models" && args[2] === "install") {
        const modelKey = args[3]
        const result = await installPsionicModelArtifact(summary, {
          modelKey,
          manifestUrl: stringPsionicOption(options, "manifest-url"),
          consent: options.yes === true,
          env: Bun.env,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        process.exitCode = result.state === "installed" ? 0 : 1
        return
      }

      throw new Error(`unknown psionic command: ${args.slice(1).join(" ")}`)
    } catch (error) {
      process.stderr.write(`Pylon Psionic installer failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "presence") {
    try {
      const command = args[1]
      const presenceArgs = args.slice(2)
      const options = parsePresenceOptions(presenceArgs)
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error("presence commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      }
      const summary = await createPresenceBootstrapSummary(presenceArgs, Bun.env)
      const clientOptions = {
        agentToken: Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl,
      }
      const result =
        command === "register"
          ? await registerPylon(summary, clientOptions)
          : command === "heartbeat"
            ? await sendHeartbeat(summary, clientOptions)
            : command === "link-complete"
              ? await completePylonLink(summary, clientOptions)
              : command === "link-refresh"
                ? await refreshPylonLink(summary, clientOptions)
                : null
      if (!result) throw new Error(`unknown presence command: ${command ?? ""}`)
      process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
      return
    } catch (error) {
      process.stderr.write(`Pylon presence failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "forum" || args[0] === "memories" || args[0] === "ask-artanis") {
    try {
      const surfaceArgs = args[0] === "ask-artanis" ? args.slice(2) : args.slice(args[0] === "forum" ? 2 : 1)
      const options = parseKeyValueOptions(surfaceArgs)
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      const networkOptions = {
        agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl: baseUrl ?? "https://openagents.com",
      }
      if (args[0] === "memories") {
        const entries = await readMemories(summary.paths.home)
        process.stdout.write(`${JSON.stringify({ count: entries.length, memories: entries }, null, 2)}\n`)
        return
      }
      if (args[0] === "forum") {
        const sub = args[1]
        if (sub === "read") {
          const topicId = args[2]
          if (!topicId) throw new Error("usage: pylon forum read <topic-id>")
          const topic = await forumReadTopic(networkOptions, topicId)
          process.stdout.write(`${JSON.stringify(topic, null, 2)}\n`)
          return
        }
        if (sub === "post") {
          const forumSlug = stringPsionicOption(options, "forum") ?? ARTANIS_FORUM_SLUG
          const title = stringPsionicOption(options, "title")
          const body = stringPsionicOption(options, "body")
          if (!title || !body) throw new Error("usage: pylon forum post --title T --body B [--forum slug]")
          const result = await forumPostTopic(networkOptions, { bodyText: body, forumSlug, title })
          await appendMemory(summary.paths.home, {
            at: new Date().toISOString(),
            kind: "forum_post",
            refs: { topicId: (result.topic as { topicId?: string } | undefined)?.topicId ?? null },
            summary: `posted forum topic: ${title.slice(0, 80)}`,
          })
          process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
          return
        }
        if (sub === "reply") {
          const topicId = args[2]
          const body = stringPsionicOption(options, "body")
          if (!topicId || !body) throw new Error("usage: pylon forum reply <topic-id> --body B")
          const result = await forumReply(networkOptions, { bodyText: body, topicId })
          await appendMemory(summary.paths.home, {
            at: new Date().toISOString(),
            kind: "forum_reply",
            refs: { topicId },
            summary: `replied in topic ${topicId.slice(0, 12)}`,
          })
          process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
          return
        }
        throw new Error("usage: pylon forum post|read|reply ...")
      }
      // ask-artanis
      const question = args[1]
      if (!question || question.startsWith("--")) {
        throw new Error('usage: pylon ask-artanis "your question" [--base-url URL]')
      }
      const inventory = await discoverHostInventory({ env: Bun.env })
      const memories = await readMemories(summary.paths.home, 10)
      const adapter = resolveModelAdapter(Bun.env)
      const composed = await composeAskArtanisBody(
        {
          deviceContext: {
            backends: (inventory as { backends?: unknown }).backends ?? null,
            capabilityRefs: state.runtime.capabilityRefs,
            platform: (inventory as { platform?: unknown }).platform ?? null,
            pylonRef: state.identity.pylonRef,
          },
          memories,
          pylonRef: state.identity.pylonRef,
          question,
        },
        adapter,
      )
      const title = `Pylon device question: ${question.slice(0, 80)}`
      const result = await forumPostTopic(networkOptions, {
        bodyText: composed.bodyText,
        forumSlug: stringPsionicOption(options, "forum") ?? ARTANIS_FORUM_SLUG,
        title,
      })
      await appendMemory(summary.paths.home, {
        at: new Date().toISOString(),
        kind: "ask_artanis",
        refs: { composedBy: composed.composedBy, topicId: (result.topic as { topicId?: string } | undefined)?.topicId ?? null },
        summary: `asked artanis: ${question.slice(0, 80)}`,
      })
      process.stdout.write(`${JSON.stringify({ composedBy: composed.composedBy, result }, null, 2)}\n`)
      return
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "tip" || args[0] === "balance" || args[0] === "sweep-status" || args[0] === "tip-prefs" || args[0] === "claim-tip-readiness") {
    try {
      const tipArgs = args[0] === "tip" ? args.slice(3) : args.slice(1)
      const options = parseKeyValueOptions(tipArgs)
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) throw new Error(`${args[0]} requires --base-url or PYLON_OPENAGENTS_BASE_URL`)
      const networkOptions = {
        agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl,
      }
      if (args[0] === "tip") {
        const postId = args[1]
        const amountSat = Number(args[2])
        if (!postId || !Number.isInteger(amountSat) || amountSat <= 0) {
          throw new Error("usage: pylon tip <post-id> <sats> [--base-url URL]")
        }
        const result = await tipPost(networkOptions, { amountSat, postId })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (args[0] === "balance") {
        const result = await readBalance(networkOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (args[0] === "sweep-status") {
        const result = await sweepStatus(networkOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (args[0] === "tip-prefs") {
        const prefs: Record<string, number | boolean> = {}
        if (options["sweep-enabled"] !== undefined) prefs.sweepEnabled = options["sweep-enabled"] === "true"
        if (options["sweep-threshold"] !== undefined) prefs.sweepThresholdSat = Number(options["sweep-threshold"])
        if (options["send-credits-below"] !== undefined) prefs.sendCreditsBelowSat = Number(options["send-credits-below"])
        if (options["receive-credits-below"] !== undefined) prefs.receiveCreditsBelowSat = Number(options["receive-credits-below"])
        const result = await setTipPreferences(networkOptions, prefs)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      const result = await claimTipReadiness(networkOptions, { pylonRef: state.identity.pylonRef })
      process.stdout.write(`${JSON.stringify({ claimed: true, tipRecipientReadiness: (result as { tipRecipientReadiness?: unknown }).tipRecipientReadiness ?? null }, null, 2)}\n`)
      return
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "work") {
    try {
      const command = args[1]
      const workArgs = args.slice(2)
      const options = parseKeyValueOptions(workArgs)
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) throw new Error("work commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const networkOptions = {
        agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
        baseUrl,
      }

      if (command === "request") {
        const objective = args[2]
        const budgetSats = Number(options.budget)
        if (!objective || objective.startsWith("--") || !Number.isInteger(budgetSats) || budgetSats <= 0) {
          throw new Error('usage: pylon work request "<objective>" --budget <sats> [--repo URL] [--verify command] [--deadline iso]')
        }
        const result = await createPylonWorkRequest(networkOptions, {
          budgetSats,
          deadline: options.deadline,
          objective,
          repository: options.repo,
          verificationCommand: options.verify,
        })
        await appendMemory(summary.paths.home, workRequestMemoryEntry({
          at: new Date().toISOString(),
          result,
        }))
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "offers") {
        const requestRef = args[2]
        if (!requestRef) throw new Error("usage: pylon work offers <request-ref>")
        const result = await listPylonWorkOffers(networkOptions, requestRef)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "accept") {
        const requestRef = args[2]
        const quoteRef = args[3]
        if (!requestRef || !quoteRef) throw new Error("usage: pylon work accept <request-ref> <quote-ref>")
        const result = await acceptPylonWorkOffer(networkOptions, { quoteRef, requestRef })
        await appendMemory(summary.paths.home, workAcceptanceMemoryEntry({
          at: new Date().toISOString(),
          quoteRef,
          requestRef,
          result,
        }))
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      if (command === "status") {
        const requestRef = args[2]
        if (!requestRef) throw new Error("usage: pylon work status <request-ref>")
        const result = await readPylonWorkStatus(networkOptions, requestRef)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }

      throw new Error("usage: pylon work request|offers|accept|status ...")
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ error: error instanceof Error ? error.message : String(error), ok: false }, null, 2)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "wallet") {
    try {
      const command = args[1]
      const walletArgs = args.slice(2)
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      if (command === "status") {
        const status = await classifyMdkWallet()
        const legacySparkMigration = await preflightLegacySparkMigration({
          dryRun: true,
          env: Bun.env,
          identityMnemonicPath: state.paths.identityMnemonic,
        })
        process.stdout.write(`${JSON.stringify({ ...status, legacySparkMigration }, null, 2)}\n`)
        return
      }
      if (command === "migrate-spark") {
        const sparkOptions = parsePsionicOptions(walletArgs)
        const result = await preflightLegacySparkMigration({
          destinationInvoiceReady: sparkOptions["destination-invoice-ready"] === true,
          dryRun: sparkOptions.execute !== true,
          env: Bun.env,
          identityMnemonicPath: stringPsionicOption(sparkOptions, "identity-mnemonic-path") ?? state.paths.identityMnemonic,
          mnemonicRecoveryRequested: sparkOptions["mnemonic-recovery"] === true,
          yes: sparkOptions.yes === true,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        process.exitCode = result.state === "blocked" ? 1 : 0
        return
      }
      const options = parseKeyValueOptions(walletArgs)
      if (command === "report-readiness") {
        const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
        if (!baseUrl) throw new Error("wallet report-readiness requires --base-url or PYLON_OPENAGENTS_BASE_URL")
        const status = await classifyMdkWallet()
        const result = await reportWalletReadiness({ status }, {
          agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
          baseUrl,
          pylonRef: state.identity.pylonRef,
        })
        // Onboarding auto-claim (#4712): a wallet that reports ready also
        // claims Forum tip-recipient readiness with a fresh BOLT 12 offer,
        // best-effort - the silent-untippable trap cannot happen to a
        // Pylon user. Failures are reported, never fatal to readiness.
        let tipReadinessClaim: Record<string, unknown> | { error: string } | null = null
        if (status.receiveReady) {
          try {
            tipReadinessClaim = await claimTipReadiness(
              {
                agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
                baseUrl,
              },
              { pylonRef: state.identity.pylonRef },
            )
          } catch (error) {
            tipReadinessClaim = { error: error instanceof Error ? error.message : String(error) }
          }
        }
        process.stdout.write(`${JSON.stringify({ status, result, tipReadinessClaim: tipReadinessClaim === null ? null : "tipRecipientReadiness" in (tipReadinessClaim as Record<string, unknown>) ? "claimed" : tipReadinessClaim }, null, 2)}\n`)
        return
      }
      if (command === "receive") {
        const amount = Number(options.amount)
        if (!Number.isFinite(amount) || amount <= 0) throw new Error("wallet receive requires --amount")
        const result = await receiveWithMdk(amount)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "send") {
        const destinationRef = options["destination-ref"]
        if (!destinationRef) throw new Error("wallet send requires --destination-ref")
        const amount = options.amount === undefined ? undefined : Number(options.amount)
        const result = await sendWithMdk(destinationRef, amount)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "admit-payout-target") {
        const kind = options.kind as any
        const ref = options.ref
        if (!kind || !ref) throw new Error("wallet admit-payout-target requires --kind and --ref")
        const result = admitPayoutTarget({ kind, ref })
        process.stdout.write(`${JSON.stringify({ ...result, ledger: state.paths.ledger }, null, 2)}\n`)
        return
      }
      if (command === "request-payout-target-admission") {
        const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
        if (!baseUrl) throw new Error("wallet request-payout-target-admission requires --base-url or PYLON_OPENAGENTS_BASE_URL")
        const kind = options.kind as any
        const ref = options.ref
        if (!kind || !ref) throw new Error("wallet request-payout-target-admission requires --kind and --ref")
        const result = await requestPayoutTargetAdmission({ kind, ref }, {
          agentToken: options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN,
          baseUrl,
          pylonRef: state.identity.pylonRef,
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      throw new Error(`unknown wallet command: ${command ?? ""}`)
    } catch (error) {
      process.stderr.write(`Pylon wallet failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "assignment") {
    try {
      const command = args[1]
      const options = parseKeyValueOptions(args.slice(2))
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error("assignment commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const agentToken = options["agent-token"] ?? Bun.env.OPENAGENTS_AGENT_TOKEN
      const clientOptions = { ...(agentToken ? { agentToken } : {}), baseUrl }
      if (command === "poll") {
        const leases = await pollAssignments(summary, clientOptions)
        process.stdout.write(`${JSON.stringify({ leases }, null, 2)}\n`)
        return
      }
      if (command === "accept") {
        const leaseJson = options.lease
        if (!leaseJson) throw new Error("assignment accept requires --lease JSON")
        const result = await acceptAssignment(summary, JSON.parse(leaseJson) as PylonAssignmentLease, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "progress") {
        const progressJson = options.progress
        if (!progressJson) throw new Error("assignment progress requires --progress JSON")
        const result = await submitAssignmentProgress(summary, JSON.parse(progressJson) as AssignmentProgress, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "closeout") {
        const closeoutJson = options.closeout
        if (!closeoutJson) throw new Error("assignment closeout requires --closeout JSON")
        const result = await submitAssignmentCloseout(summary, JSON.parse(closeoutJson) as AssignmentCloseout, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "run-no-spend") {
        const result = await runNoSpendAssignment(summary, clientOptions)
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      throw new Error(`unknown assignment command: ${command ?? ""}`)
    } catch (error) {
      process.stderr.write(`Pylon assignment failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "provider") {
    try {
      const command = args[1]
      const options = parseKeyValueOptions(args.slice(2))
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      if (command === "go-online" || command === "online") {
        const claudeAgentReadiness = await probeClaudeAgentReadiness({
          config: await loadClaudeAgentConfig(summary),
        })
        const nextRuntime = {
          ...state.runtime,
          lifecycle: "online" as const,
          capabilityRefs: withClaudeAgentCapability(
            [...new Set([...state.runtime.capabilityRefs, PYLON_NIP90_PROVIDER_CAPABILITY_REF, PYLON_LABOR_CAPABILITY_REF, TASSADAR_EXECUTOR_CAPABILITY_REF])],
            claudeAgentReadiness,
          ),
          blockerRefs: state.runtime.blockerRefs.filter((ref) => ref !== "blocker.assignment.lifecycle_offline"),
        }
        await writeRuntimeState(state.paths, nextRuntime)
        process.stdout.write(`${JSON.stringify({
          ok: true,
          lifecycle: nextRuntime.lifecycle,
          capabilityRefs: nextRuntime.capabilityRefs,
          claudeAgent: {
            state: claudeAgentReadiness.state,
            credentialSourceRef: claudeAgentReadiness.credentialSourceRef,
          },
          relayUrls: relaysFromEnv(Bun.env),
          policy: policyFromEnv(Bun.env),
          stateRef: "state.public.pylon.nip90_provider.online",
        }, null, 2)}\n`)
        return
      }
      if (command === "approve-labor") {
        const approvedByRef = options["approved-by-ref"]
        if (!approvedByRef) throw new Error("provider approve-labor requires --approved-by-ref")
        const jobType = options["job-type"]
        if (jobType !== undefined && jobType !== "code_task" && jobType !== "review" && jobType !== "document_work") {
          throw new Error("provider approve-labor --job-type must be code_task, review, or document_work")
        }
        const result = await approveLaborFirstRun({
          paths: state.paths,
          approvedByRef,
          ...(jobType === undefined ? {} : { jobType }),
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      if (command === "go-offline" || command === "offline") {
        const nextRuntime = {
          ...state.runtime,
          lifecycle: "offline" as const,
        }
        await writeRuntimeState(state.paths, nextRuntime)
        process.stdout.write(`${JSON.stringify({
          ok: true,
          lifecycle: nextRuntime.lifecycle,
          stateRef: "state.public.pylon.nip90_provider.offline",
        }, null, 2)}\n`)
        return
      }
      if (command === "once") {
        const result = await startNip90ProviderLoop(summary, {
          once: true,
          log: (message) => process.stderr.write(`${message}\n`),
        })
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
        return
      }
      throw new Error(`unknown provider command: ${command ?? ""}`)
    } catch (error) {
      process.stderr.write(`Pylon provider failed: ${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 1
      return
    }
  }

  if (args[0] === "runtime" || runtimeCommandNamespaces.has(args[0] ?? "")) {
    const runtimeArgs = args[0] === "runtime" ? args.slice(1) : args
    const result = await Effect.runPromise(runProbeCli(runtimeArgs, { env: Bun.env }))
    if (result.stdout) process.stdout.write(result.stdout)
    if (result.stderr) process.stderr.write(result.stderr)
    process.exitCode = result.exitCode
    return
  }

  await Effect.runPromise(
    Effect.scoped(runPylonNode).pipe(
      Effect.catch((error) =>
        Console.error(`Pylon v0.3 crashed on startup: ${error.message}`)
      )
    )
  )
  // Scope is closed: services interrupted, renderer stopped, terminal
  // restored. Exit explicitly so lingering library handles cannot hold the
  // process open after a deliberate shutdown.
  process.exit(0)
}

await main()
