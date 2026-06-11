// Pylon dashboard view (issue #4737): the four-pane layout rebuilt as
// Solid components on @opentui/solid. Rules of this layer:
//   - No Effect imports. The bridge (`bridge.ts`) is the only module that
//     touches both worlds; components render from `store.ts` signals.
//   - No colors outside `theme.ts` tokens.
//   - Imperative renderable access only via refs for focus/scroll plumbing.
//
// This file is .tsx and is loaded dynamically after the Solid transform
// plugin is active (bunfig preload in dev/tests; --preload re-exec for the
// packaged bin — see ensureSolidRuntime in src/index.ts).

import {
  createCliRenderer,
  MacOSScrollAccel,
  type CliRenderer,
  type ScrollBoxRenderable,
  type TextareaRenderable,
} from "@opentui/core"
import { render, useTerminalDimensions } from "@opentui/solid"
import { ErrorBoundary, For, Show, type JSX } from "solid-js"
import { runOpencodeStream } from "../opencode-run"
import { theme } from "./theme"
import {
  appendChatFeedItem,
  feedItems,
  operatorText,
  telemetryState,
  walletState,
} from "./store"

export { appendChatFeedItem } from "./store"
export { attachRuntimeToView } from "./bridge"

// --- View-internal plumbing (focus, scroll, terminal modes) ---------------

let scrollRef: ScrollBoxRenderable | undefined
let composerRef: TextareaRenderable | undefined

const terminalScrollLockOn = "\x1b[?1007h"
const terminalScrollLockOff = "\x1b[?1007l"
const sgrMousePattern = /\x1b\[<(\d+);(\d+);(\d+)([mM])/g

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
  scrollRef?.scrollBy(delta, unit)
}

function isPointInsideLogScrollBox(x: number, y: number) {
  if (!scrollRef) return false
  const box = scrollRef as any
  const left = box.screenX ?? box.x ?? 0
  const top = box.screenY ?? box.y ?? 0
  const width = box.width ?? 0
  const height = box.height ?? 0
  return x >= left && x < left + width && y >= top && y < top + height
}

function scrollLogByWheelButton(button: number) {
  if (!scrollRef) return false
  const wheelButton = button & 0b11
  const step = Math.max(3, Math.ceil((scrollRef.viewport?.height ?? scrollRef.height ?? 15) / 6))
  if (wheelButton === 0) {
    scrollRef.scrollBy(-step)
    return true
  }
  if (wheelButton === 1) {
    scrollRef.scrollBy(step)
    return true
  }
  if (wheelButton === 2) {
    scrollRef.scrollBy({ x: -step, y: 0 })
    return true
  }
  if (wheelButton === 3) {
    scrollRef.scrollBy({ x: step, y: 0 })
    return true
  }
  return false
}

function handleRawLogWheel(sequence: string) {
  if (!scrollRef) return false
  let handled = false
  sgrMousePattern.lastIndex = 0
  for (const match of sequence.matchAll(sgrMousePattern)) {
    const button = Number(match[1])
    if ((button & 64) !== 64) continue
    const x = Number(match[2]) - 1
    const y = Number(match[3]) - 1
    if (!isPointInsideLogScrollBox(x, y)) continue
    scrollRef.focus()
    handled = scrollLogByWheelButton(button) || handled
  }
  return handled
}

function scrollLogFromWheel(event: any) {
  if (!scrollRef || event.type !== "scroll") return false
  ;(scrollRef as any).onMouseEvent?.(event)
  return true
}

function routeLogMouse(event: any) {
  if (!scrollRef) return
  scrollRef.focus()
  if (event.type === "scroll") {
    scrollLogFromWheel(event)
    event.stopPropagation?.()
    event.preventDefault?.()
  }
}

function sinkRootScroll(event: any) {
  if (event.type !== "scroll") return
  if (scrollRef) {
    scrollRef.focus()
    scrollLogFromWheel(event)
  }
  event.stopPropagation?.()
  event.preventDefault?.()
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

// --- Composer -> OpenCode interaction --------------------------------------

async function submitPrompt(prompt: string) {
  appendChatFeedItem(`**User**: ${prompt}`)
  const response = appendChatFeedItem("**OpenCode**: ... thinking ...", { streaming: true })
  const opencodePath = Bun.which("opencode")
  if (!opencodePath) {
    response.update("**OpenCode**: Error - OpenCode CLI is not installed on this system.")
    response.finish()
    return
  }
  let lastText = ""
  try {
    const result = await runOpencodeStream(opencodePath, prompt, {
      onText: (text) => {
        lastText = text
        response.update(`**OpenCode**: ${text}`)
      },
      onUsage: ({ cost, tokens }) => {
        response.update(`**OpenCode**: ${lastText}\n\n*[Cost: $${cost.toFixed(4)} | Tokens: ${tokens}]*`)
      },
    })
    response.update(`**OpenCode**: ${result.text}\n\n*[Cost: $${result.cost.toFixed(4)} | Tokens: ${result.tokens}]*`)
  } catch (error) {
    response.update(`**OpenCode**: Error - ${error instanceof Error ? error.message : String(error)}`)
  } finally {
    response.finish()
  }
}

// --- Components -------------------------------------------------------------

function PaneFallback(props: { name: string; error: unknown }) {
  return (
    <box border borderStyle="single" borderColor={theme.colors.error} flexGrow={1}>
      <text fg={theme.colors.error}>
        {` ${props.name} pane crashed: ${props.error instanceof Error ? props.error.message : String(props.error)}`}
      </text>
    </box>
  )
}

function Pane(props: { name: string; children: JSX.Element }) {
  return (
    <ErrorBoundary fallback={(error) => <PaneFallback name={props.name} error={error} />}>
      {props.children}
    </ErrorBoundary>
  )
}

function LogFeed() {
  return (
    <box
      border
      borderStyle="single"
      borderColor={theme.colors.border}
      title=" // Active Workroom Execution Logs "
      titleColor={theme.colors.title}
      flexGrow={1}
      height="100%"
    >
      <scrollbox
        ref={(renderable: ScrollBoxRenderable) => {
          scrollRef = renderable
          renderable.handleKeyPress = (key: any) => handleLogKey(key, () => composerRef?.focus()) || false
        }}
        scrollY
        stickyScroll
        stickyStart="bottom"
        scrollAcceleration={new MacOSScrollAccel()}
        focusable
        flexGrow={1}
        width="100%"
        height="100%"
        onMouseDown={routeLogMouse}
        onMouseScroll={routeLogMouse}
      >
        <markdown
          content="*Pylon v0.3*"
          syntaxStyle={theme.syntaxStyle}
          width="100%"
          conceal
          fg={theme.colors.banner}
          onMouseDown={routeLogMouse}
          onMouseScroll={routeLogMouse}
        />
        <For each={feedItems}>
          {(item) => (
            <markdown
              content={item.markdown}
              syntaxStyle={theme.syntaxStyle}
              width="100%"
              conceal
              fg={item.tone === "log" ? theme.colors.logText : item.tone === "logError" ? theme.colors.logError : undefined}
              streaming={item.streaming}
              onMouseDown={routeLogMouse}
              onMouseScroll={routeLogMouse}
            />
          )}
        </For>
      </scrollbox>
    </box>
  )
}

function Separator() {
  return <text fg={theme.colors.separator} height={1}>{" ---------------------------------"}</text>
}

function WalletPane() {
  const online = () => walletState().daemonOnline && walletState().balanceSats !== null
  const balance = () => {
    const sats = walletState().balanceSats
    return sats === null ? " Balance: -- Sats" : ` Balance: ${sats.toLocaleString()} Sats`
  }
  return (
    <>
      <text fg={online() ? theme.colors.online : theme.colors.error} width="100%" height={1}>
        {online() ? " Wallet: ONLINE (OK)" : " Wallet: OFFLINE"}
      </text>
      <text fg={theme.colors.accent} width="100%" height={1}>
        {balance()}
      </text>
    </>
  )
}

function TelemetryPane() {
  const content = () => {
    const state = telemetryState()
    return ` State: ${state.state}\n Model: ${state.model}\n VRAM:  ${state.vram}\n Psionic: ${state.psionic}`
  }
  return (
    <text fg={theme.colors.text} width="100%" height={4}>
      {content()}
    </text>
  )
}

function OperatorPane() {
  return (
    <text fg={theme.colors.text} width="100%" flexGrow={1}>
      {operatorText()}
    </text>
  )
}

function Sidebar() {
  return (
    <box
      border
      borderStyle="single"
      borderColor={theme.colors.border}
      title=" // Telemetry & Wallet "
      titleColor={theme.colors.title}
      width={35}
      flexBasis={35}
      flexGrow={0}
      flexShrink={0}
      height="100%"
      flexDirection="column"
    >
      <WalletPane />
      <Separator />
      <TelemetryPane />
      <Separator />
      <OperatorPane />
    </box>
  )
}

function Composer() {
  return (
    <box
      border
      borderStyle="single"
      borderColor={theme.colors.border}
      title=" // Composer (meta+return to submit) "
      titleColor={theme.colors.title}
      width="100%"
      height={5}
    >
      <textarea
        ref={(renderable: TextareaRenderable) => {
          composerRef = renderable
          queueMicrotask(() => renderable.focus())
        }}
        width="100%"
        height="100%"
        placeholder="Ask your agent anything..."
        onKeyDown={(key: any) => {
          if (key.name === "tab") {
            scrollRef?.focus()
            key.preventDefault?.()
            key.stopPropagation?.()
            return
          }
          handleLogKey(key)
        }}
        onMouseDown={() => composerRef?.focus()}
        onSubmit={() => {
          const prompt = composerRef?.plainText.trim()
          if (!prompt) return
          composerRef?.setText("")
          void submitPrompt(prompt)
        }}
      />
    </box>
  )
}

export function Dashboard() {
  // Reactive resize: flexbox handles proportional resizing; the hook
  // additionally drops the fixed-width sidebar on terminals too narrow to
  // fit both panes usefully.
  const dimensions = useTerminalDimensions()
  return (
    <box flexDirection="column" width="100%" height="100%" onMouseScroll={sinkRootScroll}>
      <box flexDirection="row" width="100%" flexGrow={1}>
        <Pane name="logs">
          <LogFeed />
        </Pane>
        <Show when={dimensions().width >= 60}>
          <Pane name="telemetry">
            <Sidebar />
          </Pane>
        </Show>
      </box>
      <Pane name="composer">
        <Composer />
      </Pane>
    </box>
  )
}

// --- Mount ------------------------------------------------------------------

export interface StartDashboardOptions {
  onRequestShutdown: () => void
}

export interface DashboardHandle {
  renderer: CliRenderer
  destroy: () => void
}

export async function startDashboard(options: StartDashboardOptions): Promise<DashboardHandle> {
  const restoreScrollLock = installTerminalScrollLock()
  const interceptCtrlC = (sequence: string) => {
    if (sequence.includes("\x03")) {
      options.onRequestShutdown()
      return true
    }
    return false
  }

  const renderer = await createCliRenderer({
    screenMode: "alternate-screen",
    exitOnCtrlC: false,
    useMouse: true,
    autoFocus: true,
    targetFps: 30,
    prependInputHandlers: [interceptCtrlC, handleRawLogWheel],
  })

  await render(
    () => (
      <ErrorBoundary fallback={(error) => <PaneFallback name="dashboard" error={error} />}>
        <Dashboard />
      </ErrorBoundary>
    ),
    renderer,
  )

  return {
    renderer,
    destroy: () => {
      scrollRef = undefined
      composerRef = undefined
      try {
        renderer.destroy()
      } catch {
        // renderer teardown is best-effort during shutdown
      }
      restoreScrollLock()
    },
  }
}
