#!/usr/bin/env bun

import { Effect, Console } from "effect"
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
import { ensurePylonLocalState, projectPublicStatus } from "./state"
import {
  completePylonLink,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
} from "./presence"
import {
  admitPayoutTarget,
  classifyMdkWallet,
  receiveWithMdk,
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
import {
  installPsionicBinary,
  installPsionicModelArtifact,
} from "./psionic-install"

// Global UI references for log aggregation and balance updates
let globalRenderer: CliRenderer | null = null
let logScrollBox: ScrollBoxRenderable | null = null
let balanceTextRenderable: TextRenderable | null = null
let statusTextRenderable: TextRenderable | null = null
let telemetryTextRenderable: TextRenderable | null = null
let operatorTextRenderable: TextRenderable | null = null

const logHistory: string[] = []
const maxLogHistoryEntries = 1000
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

function logToUi(message: string) {
  const now = new Date()
  const timestamp = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}:${String(now.getSeconds()).padStart(2, "0")}`
  logHistory.push(`[${timestamp}] ${message}`)
  if (logHistory.length > maxLogHistoryEntries) {
    logHistory.splice(0, logHistory.length - maxLogHistoryEntries)
  }

  if (logScrollBox && globalRenderer) {
    const line = makeLogMarkdown(globalRenderer, {
      content: `[${timestamp}] ${message}`,
      syntaxStyle,
      width: "100%",
      conceal: true,
      fg: parseColor("#5C7080"),
    })
    logScrollBox.add(line)
  } else {
    // Silent pre-boot buffering or console logging
    Console.log(`[BOOT] ${message}`)
  }
}

// Effect-native logging helper
const log = (message: string) => Effect.sync(() => logToUi(message))

function runBackgroundEffect(name: string, effect: Effect.Effect<void, unknown>) {
  void Effect.runPromise(effect).catch((error) => {
    logToUi(`[${name}] Background service stopped with error: ${String(error)}`)
  })
}

function updateMdkBalance(balance: number | null, suffix = "Sats") {
  if (balanceTextRenderable) {
    balanceTextRenderable.content =
      balance === null ? ` Balance: -- ${suffix}` : ` Balance: ${balance.toLocaleString()} ${suffix}`
  }
}

function updateMdkStatus(status: string, color = "#22C55E") {
  if (statusTextRenderable) {
    statusTextRenderable.content = ` Wallet: ${status}`
    statusTextRenderable.fg = parseColor(color)
  }
}

function updateTelemetryState(state: string, model: string, vram: string) {
  if (telemetryTextRenderable) {
    telemetryTextRenderable.content = ` State: ${state}\n Model: ${model}\n VRAM:  ${vram}`
  }
}

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

// Hardware Resource & Telemetry Discovery Service
const startHardwareTelemetryLoop = Effect.gen(function* () {
  yield* log("[Telemetry] Platform discovery initialized.")
  while (true) {
    const inventory = yield* Effect.tryPromise({
      try: () => discoverHostInventory(),
      catch: (error) => new Error(`inventory discovery failed: ${String(error)}`),
    }).pipe(
      Effect.catch((error) =>
        Effect.sync(() => {
          logToUi(`[Telemetry] Inventory unavailable: ${error.message}`)
          return null
        }),
      ),
    )
    yield* Effect.sync(() => {
      if (!inventory) {
        updateTelemetryState("UNAVAILABLE", "inventory unavailable", "--")
        return
      }
      const readyBackends = inventory.backendHealth.filter((backend) => backend.state === "ready" || backend.state === "configured")
      const model = readyBackends[0]?.modelRef ?? "None"
      const vram = inventory.accelerator.vramGb === null ? "--" : `${inventory.accelerator.vramGb.toFixed(1)} GB`
      const state = inventory.eligibleInventoryCount > 0 ? "INVENTORY FRESH" : "INVENTORY BLOCKED"
      updateTelemetryState(state, model, vram)
      if (operatorTextRenderable) {
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
        operatorTextRenderable.content = formatOperatorSnapshotText(createOperatorSnapshot({ inventory, wallet }))
      }
    })
    yield* Effect.sleep("10 seconds")
  }
})

// Money Dev Kit (MDK) Wallet Sidecar Service
const startMdkWalletService = Effect.gen(function* () {
  yield* log("[Wallet] Connecting to local MDK agent-wallet daemon...")
  let loggedOffline = false
  let loggedOnline = false
  while (true) {
    const status = yield* Effect.promise(async () => {
      try {
        return await classifyMdkWallet()
      } catch (error) {
        logToUi(`[Wallet] MDK status unavailable: ${String(error)}`)
        return null
      }
    })

    yield* Effect.sync(() => {
      if (status?.daemonOnline && status.balanceSats !== null) {
        updateMdkBalance(status.balanceSats)
        updateMdkStatus("ONLINE (OK)", "#58A6FF")
        if (!loggedOnline) {
          logToUi(`[Wallet] MDK agent-wallet daemon connected. Readiness: ${status.readiness}.`)
          loggedOnline = true
          loggedOffline = false
        }
      } else {
        updateMdkBalance(null)
        updateMdkStatus("OFFLINE", "#EF4444")
        if (!loggedOffline) {
          logToUi("[Wallet] Local MDK wallet balance is unavailable. Operating in OFFLINE mode.")
          loggedOffline = true
          loggedOnline = false
        }
      }
    })
    yield* Effect.sleep("10 seconds")
  }
})

// Nostr Continuous Presence Heartbeat Loop
const startPresenceHeartbeatLoop = Effect.gen(function* () {
  yield* log("[Heartbeat] Presence service initialized.")
  const baseUrl = Bun.env.PYLON_OPENAGENTS_BASE_URL
  if (!baseUrl) {
    yield* log("[Heartbeat] No OpenAgents base URL configured. Presence remains unregistered.")
    return
  }

  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
  yield* Effect.tryPromise({
    try: () => registerPylon(summary, { baseUrl }),
    catch: (error) => new Error(`presence registration failed: ${String(error)}`),
  }).pipe(
    Effect.catch((error) =>
      log(`[Heartbeat] Registration blocked: ${error.message}`)
    )
  )

  while (true) {
    yield* Effect.tryPromise({
      try: () => sendHeartbeat(summary, { baseUrl }),
      catch: (error) => new Error(`heartbeat failed: ${String(error)}`),
    }).pipe(
      Effect.catch((error) =>
        log(`[Heartbeat] Heartbeat blocked: ${error.message}`)
      )
    )
    yield* Effect.sleep("30 seconds")
  }
})

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
    options.streamToUi && globalRenderer
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
    const logSummaryResult = yield* Effect.tryPromise({
      try: () => {
        const prompt = `Here are the bootup sequence logs:\n\n${logHistory.join("\n")}\n\nProvide a one line, <10 word, neutral, terminal-sounding summary of these bootup sequence logs.`
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

// Main Pylon v0.3 Application Loop
const runPylonNode = Effect.gen(function* () {
  const smokeDashboard = Bun.argv.includes("--smoke-dashboard") || Bun.env.PYLON_SMOKE_DASHBOARD === "1"
  yield* log("Initializing Pylon v0.3 observational earning node...")
  const restoreTerminalScrollLock = installTerminalScrollLock()

  // Bootstrap OpenTUI Core
  const renderer = yield* Effect.tryPromise({
    try: () =>
      createCliRenderer({
        screenMode: "alternate-screen",
        exitOnCtrlC: true,
        useMouse: true,
        autoFocus: true,
        targetFps: 30,
        prependInputHandlers: [handleRawLogWheel],
      }),
    catch: (error) => new Error(`Failed to initialize OpenTUI renderer: ${String(error)}`),
  })

  globalRenderer = renderer

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

  const bootstrapSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
  const localState = yield* Effect.tryPromise({
    try: () => ensurePylonLocalState(bootstrapSummary),
    catch: (error) => new Error(`failed to load Pylon Nostr identity: ${String(error)}`),
  })
  yield* log(`[Identity] Pylon Nostr npub: ${localState.identity.npub}`)

  // Start Background Services as Concurrent Fibers
  yield* Effect.sync(() => {
    runBackgroundEffect("Telemetry", startHardwareTelemetryLoop)
    runBackgroundEffect("Wallet", startMdkWalletService)
    runBackgroundEffect("Heartbeat", startPresenceHeartbeatLoop)
    if (!smokeDashboard && Bun.env.PYLON_DISABLE_OPENCODE_STARTUP !== "1") {
      runBackgroundEffect("OpenCode", runOpencodeStartupInference)
    }
  })

  yield* log("Pylon v0.3 observational dashboard active.")

  if (smokeDashboard) {
    yield* log("Pylon v0.3 dashboard smoke complete.")
    renderer.stop?.()
    restoreTerminalScrollLock()
    yield* Effect.sync(() => process.exit(0))
    return
  }

  // Enter the persistent execution block
  yield* Effect.never
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
    process.stdout.write(`${JSON.stringify(projectPublicStatus(state, inventory), null, 2)}\n`)
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
      const options = parsePresenceOptions(args.slice(2))
      const baseUrl = options["base-url"] ?? Bun.env.PYLON_OPENAGENTS_BASE_URL
      if (!baseUrl) {
        throw new Error("presence commands require --base-url or PYLON_OPENAGENTS_BASE_URL")
      }
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const clientOptions = { baseUrl }
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

  if (args[0] === "wallet") {
    try {
      const command = args[1]
      const options = parseKeyValueOptions(args.slice(2))
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
      const state = await ensurePylonLocalState(summary)
      if (command === "status") {
        const status = await classifyMdkWallet()
        process.stdout.write(`${JSON.stringify(status, null, 2)}\n`)
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

  if (args[0] === "runtime" || runtimeCommandNamespaces.has(args[0] ?? "")) {
    const runtimeArgs = args[0] === "runtime" ? args.slice(1) : args
    const result = await Effect.runPromise(runProbeCli(runtimeArgs, { env: Bun.env }))
    if (result.stdout) process.stdout.write(result.stdout)
    if (result.stderr) process.stderr.write(result.stderr)
    process.exitCode = result.exitCode
    return
  }

  await Effect.runPromise(
    runPylonNode.pipe(
      Effect.catch((error) =>
        Console.error(`Pylon v0.3 crashed on startup: ${error.message}`)
      )
    )
  )
}

await main()
