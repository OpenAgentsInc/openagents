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
  type CliRenderer,
  type Renderable,
  type TextareaRenderable,
} from "@opentui/core"
import { render, useRenderer, useTerminalDimensions } from "@opentui/solid"
import { ErrorBoundary, For, Show, createMemo, createSignal, onMount, type JSX } from "solid-js"
import { theme } from "./theme"
import {
  activeRoute,
  appendChatFeedItem,
  assignmentRows,
  assignmentsStatus,
  balanceHistory,
  estimateMarkdownRows,
  feedLineCount,
  feedScrollOffset,
  operatorText,
  registerFeedViewport,
  scrollFeedBy,
  setActiveRoute,
  setAssignmentRows,
  setAssignmentsStatus,
  setVerboseMode,
  streamingTails,
  telemetryState,
  verboseMode,
  visibleFeedLines,
  walletState,
} from "./store"
import { DialogHost, registerDialogFocusHooks } from "./dialogs"
import { mountNetworkOverlay } from "./network-scene"
import {
  footerHints,
  installPylonKeymap,
  registerComposerFocusLayer,
  registerLogsScrollLayer,
  type AssignmentActions,
  type PylonKeymap,
  type WalletActions,
} from "./commands"

export { appendChatFeedItem, setVerboseMode } from "./store"
export { attachRuntimeToView } from "./bridge"
export type { AssignmentActions, WalletActions } from "./commands"

// --- View-internal plumbing (focus, scroll, terminal modes) ---------------

let scrollRef: Renderable | undefined
let composerRef: TextareaRenderable | undefined
let keymapRef: PylonKeymap | undefined

// Composer history/stash (issue #4741): cycled with ctrl+p / ctrl+n while
// composing; persisted via the node-side composer store between sessions.
let composerHistory: string[] = []
let composerHistoryIndex: number | null = null
let persistComposer: ((state: { history: string[]; stash: string }) => void) | null = null
let restoredStash = ""
let composerBackend: ComposerBackend | null = null
// 3D network scene (sidebar). Soft-fails to a text placeholder; disabled in
// the test harness and via PYLON_DISABLE_3D=1.
let enable3dFlag = false
let networkSceneDispose: (() => void) | null = null

function recordComposerSubmission(prompt: string): void {
  const trimmed = prompt.trim()
  if (!trimmed) return
  composerHistory = [...composerHistory.filter((entry) => entry !== trimmed), trimmed].slice(-50)
  composerHistoryIndex = null
  persistComposer?.({ history: composerHistory, stash: "" })
}

function cycleComposerHistory(direction: -1 | 1): void {
  if (composerHistory.length === 0 || !composerRef) return
  if (composerHistoryIndex === null) {
    composerHistoryIndex = direction === -1 ? composerHistory.length - 1 : null
  } else {
    const next = composerHistoryIndex + direction
    composerHistoryIndex = next < 0 ? 0 : next >= composerHistory.length ? null : next
  }
  composerRef.setText(composerHistoryIndex === null ? "" : composerHistory[composerHistoryIndex] ?? "")
}

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

function scrollLogBy(delta: number, unit: "viewport" | "content" | "step" = "step") {
  scrollFeedBy(delta, unit)
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
  const wheelButton = button & 0b11
  const step = 3
  if (wheelButton === 0) {
    scrollFeedBy(-step)
    return true
  }
  if (wheelButton === 1) {
    scrollFeedBy(step)
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
  if (event.type !== "scroll") return false
  const direction = event.scroll?.direction
  if (direction === "up") scrollFeedBy(-3)
  else if (direction === "down") scrollFeedBy(3)
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

// --- Composer -> agent backend interaction ---------------------------------

export interface ComposerBackendCallbacks {
  onText?: (fullText: string) => void
  onEvent?: (summary: string, eventCount: number) => void
  onUsage?: (usage: { label: string; value: string }) => void
}

export interface ComposerBackendResult {
  text: string
  footer?: string
}

export interface ComposerBackend {
  label: string
  statusLine?: string
  submit: (prompt: string, callbacks: ComposerBackendCallbacks) => Promise<ComposerBackendResult>
}

export function submitComposer(): void {
  const prompt = composerRef?.plainText.trim()
  if (!prompt) return
  composerRef?.setText("")
  recordComposerSubmission(prompt)
  void submitPrompt(prompt)
}

async function submitPrompt(prompt: string) {
  appendChatFeedItem(`**User**: ${prompt}`)
  const backend = composerBackend
  if (!backend) {
    const response = appendChatFeedItem("**Composer**: Error - no composer backend is configured.", { streaming: true })
    response.finish()
    return
  }
  const response = appendChatFeedItem(
    `**${backend.label}**: ... thinking ...${backend.statusLine ? `\n\n*${backend.statusLine}*` : ""}`,
    { streaming: true },
  )
  let lastText = ""
  let lastEvent = "waiting for first event"
  let lastUsage: { label: string; value: string } | null = null
  const render = () => {
    const visibleText = lastText.trim() || `_${lastEvent}_`
    const usage = lastUsage ? ` | ${lastUsage.label}: ${lastUsage.value}` : ""
    const status = backend.statusLine ? `${backend.statusLine} | ` : ""
    response.update(`**${backend.label}**: ${visibleText}\n\n*[${status}events: ${lastEvent}${usage}]*`)
  }
  try {
    const result = await backend.submit(prompt, {
      onText: (text) => {
        lastText = text
        render()
      },
      onEvent: (summary, count) => {
        lastEvent = `${count} ${summary}`
        render()
      },
      onUsage: (usage) => {
        lastUsage = usage
        render()
      },
    })
    response.update(`**${backend.label}**: ${result.text}${result.footer ? `\n\n*${result.footer}*` : ""}`)
  } catch (error) {
    response.update(`**${backend.label}**: Error - ${error instanceof Error ? error.message : String(error)}`)
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
  const dimensions = useTerminalDimensions()
  // Inner rows available for the virtual window: terminal height minus
  // composer (5), footer (1), feed borders (2), banner line (1), and the
  // estimated height of any live streaming tails.
  const tailRows = createMemo(() =>
    streamingTails.reduce(
      (sum, tail) => sum + Math.min(12, estimateMarkdownRows(tail.markdown, dimensions().width - 4)),
      0,
    ),
  )
  const viewportRows = createMemo(() => Math.max(3, dimensions().height - 9 - tailRows()))
  registerFeedViewport(() => viewportRows())
  const visible = createMemo(() => visibleFeedLines(feedScrollOffset(), viewportRows()))
  const scrolledUp = createMemo(() => feedScrollOffset() > 0)
  return (
    <box
      border
      borderStyle="single"
      borderColor={theme.colors.border}
      title=" // Active Workroom Execution Logs "
      titleColor={theme.colors.title}
      flexGrow={1}
      height="100%"
      flexDirection="column"
      focusable
      ref={(renderable: Renderable) => {
        scrollRef = renderable
        if (keymapRef) registerLogsScrollLayer(keymapRef, renderable)
      }}
      onMouseDown={routeLogMouse}
      onMouseScroll={routeLogMouse}
    >
      <text height={1} width="100%" fg={theme.colors.banner}>
        {scrolledUp()
          ? ` Pylon v0.3 - scrolled up ${feedScrollOffset()} lines (end: bottom)`
          : ` Pylon v0.3 - ${feedLineCount()} log lines`}
      </text>
      <For each={visible()}>
        {(line) => (
          <text
            height={1}
            width="100%"
            fg={line.tone === "log" ? theme.colors.logText : line.tone === "logError" ? theme.colors.logError : theme.colors.text}
          >
            {line.text || " "}
          </text>
        )}
      </For>
      <For each={streamingTails}>
        {(tail) => (
          <markdown
            content={tail.markdown}
            syntaxStyle={theme.syntaxStyle}
            width="100%"
            conceal
            streaming
            onMouseDown={routeLogMouse}
            onMouseScroll={routeLogMouse}
          />
        )}
      </For>
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

// Minimum terminal height for the 3D pane: below this the sidebar's
// operator snapshot would be evicted, which matters more than the visual.
export const networkPaneMinRows = 32

function NetworkPane() {
  const renderer = useRenderer()
  const dimensions = useTerminalDimensions()
  const roomy = () => dimensions().height >= networkPaneMinRows
  const [sceneState, setSceneState] = createSignal<"off" | "loading" | "active" | "unavailable">(
    enable3dFlag ? "loading" : "off",
  )
  onMount(() => {
    if (!enable3dFlag) return
    // Live state feed: wallet status colors the scene, new feed lines pulse
    // satellites, balance increases fire a payment burst.
    const readState = () => ({
      online: walletState().daemonOnline && walletState().balanceSats !== null,
      balanceSats: walletState().balanceSats,
      activityCount: feedLineCount(),
    })
    void mountNetworkOverlay(renderer as never, readState).then((handle) => {
      if (!handle) {
        setSceneState("unavailable")
        return
      }
      networkSceneDispose = handle.dispose
      setSceneState("active")
    })
  })
  // Reserves the sidebar cells the absolutely-positioned overlay paints.
  // The overlay itself also hides below the height threshold (see
  // mountNetworkOverlay), covering live resizes.
  return (
    <Show when={roomy()}>
      <Separator />
      <box width="100%" height={10} flexGrow={0} flexShrink={0}>
        <Show when={sceneState() !== "active"}>
          <text fg={theme.colors.textMuted} width="100%">
            {sceneState() === "loading"
              ? " network view: starting..."
              : sceneState() === "unavailable"
                ? " network view: gpu unavailable"
                : " network view: off"}
          </text>
        </Show>
      </box>
    </Show>
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
      <NetworkPane />
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
          if (keymapRef) registerComposerFocusLayer(keymapRef, renderable)
          queueMicrotask(() => {
            renderable.focus()
            if (restoredStash) {
              renderable.setText(restoredStash)
              restoredStash = ""
            }
          })
        }}
        width="100%"
        height="100%"
        placeholder="Ask your agent anything..."
        onMouseDown={() => composerRef?.focus()}
        onSubmit={submitComposer}
      />
    </box>
  )
}

function AssignmentsSurface() {
  return (
    <box
      border
      borderStyle="single"
      borderColor={theme.colors.border}
      title=" // Assignments "
      titleColor={theme.colors.title}
      flexGrow={1}
      height="100%"
      flexDirection="column"
    >
      <text height={1} width="100%" fg={theme.colors.textMuted}>
        {` ${assignmentsStatus()}  ·  ctrl+k -> "Assignments: refresh" / "accept a lease"  ·  f3 dashboard`}
      </text>
      <text height={1} width="100%" fg={theme.colors.accent}>
        {" LEASE                    PAYMENT     EXPIRES               GOAL"}
      </text>
      <For each={assignmentRows()}>
        {(row) => (
          <text height={1} width="100%" fg={theme.colors.text}>
            {` ${row.leaseRef.slice(0, 24).padEnd(24)} ${row.paymentMode.padEnd(11)} ${row.expiresAt.slice(0, 19).padEnd(21)} ${row.goal.slice(0, 60)}`}
          </text>
        )}
      </For>
      <Show when={assignmentRows().length === 0}>
        <text height={1} width="100%" fg={theme.colors.textMuted}>{" (no leases)"}</text>
      </Show>
    </box>
  )
}

function WalletSurface() {
  const online = () => walletState().daemonOnline && walletState().balanceSats !== null
  return (
    <box
      border
      borderStyle="single"
      borderColor={theme.colors.border}
      title=" // Wallet "
      titleColor={theme.colors.title}
      flexGrow={1}
      height="100%"
      flexDirection="column"
    >
      <text height={1} width="100%" fg={online() ? theme.colors.online : theme.colors.error}>
        {online() ? " Status: ONLINE" : " Status: OFFLINE"}
      </text>
      <text height={1} width="100%" fg={theme.colors.accent}>
        {` Balance: ${walletState().balanceSats === null ? "--" : walletState().balanceSats?.toLocaleString()} sats`}
      </text>
      <text height={1} width="100%" fg={theme.colors.text}>
        {` Readiness: ${walletState().readiness}`}
      </text>
      <text height={1} width="100%" fg={theme.colors.separator}>{" ---"}</text>
      <text height={1} width="100%" fg={theme.colors.textMuted}>
        {" Balance history (this session):"}
      </text>
      <For each={balanceHistory.slice(-12)}>
        {(point) => (
          <text height={1} width="100%" fg={theme.colors.text}>
            {` ${point.at.slice(11, 19)}  ${point.sats === null ? "--" : point.sats.toLocaleString()} sats`}
          </text>
        )}
      </For>
      <text width="100%" fg={theme.colors.textMuted} flexGrow={1}>
        {" Wallet operations (send / receive / admit payout target) run from the\n command palette (ctrl+k) and always require an explicit confirmation."}
      </text>
    </box>
  )
}

function Footer() {
  return (
    <text height={1} width="100%" fg={theme.colors.textMuted}>
      {`${footerHints()}  ·  meta+return send`}
    </text>
  )
}

export function Dashboard() {
  // Reactive resize: flexbox handles proportional resizing; the hook
  // additionally drops the fixed-width sidebar on terminals too narrow to
  // fit both panes usefully.
  const dimensions = useTerminalDimensions()
  return (
    <box flexDirection="column" width="100%" height="100%" onMouseScroll={sinkRootScroll}>
      <Show when={activeRoute() === "dashboard"}>
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
      </Show>
      <Show when={activeRoute() === "assignments"}>
        <Pane name="assignments">
          <AssignmentsSurface />
        </Pane>
      </Show>
      <Show when={activeRoute() === "wallet"}>
        <Pane name="wallet">
          <WalletSurface />
        </Pane>
      </Show>
      <Pane name="composer">
        <Composer />
      </Pane>
      <Footer />
      <DialogHost />
    </box>
  )
}

// --- Mount ------------------------------------------------------------------

// Shared chrome setup used by startDashboard (real terminal) and the test
// harness (issue #4742): installs the command registry + keymap on the given
// renderer and wires dialog focus hooks. Must run before <Dashboard/> mounts
// so component ref callbacks can attach their focus-scoped layers.
export function installDashboardChrome(
  renderer: CliRenderer,
  options: Pick<
    StartDashboardOptions,
    "walletActions" | "onRequestShutdown" | "onVerboseChange" | "keybindOverrides"
  >,
  assignmentActions: AssignmentActions | null,
): void {
  keymapRef = installPylonKeymap(
    renderer,
    {
      walletActions: options.walletActions,
      assignmentActions,
      setRoute: (route) => setActiveRoute(route),
      refreshAssignments: () => refreshAssignmentsInto(assignmentActions),
      currentAssignments: () => assignmentRows().map((row) => ({ leaseRef: row.leaseRef, goal: row.goal })),
      cycleComposerHistory,
      focusLogs: () => scrollRef?.focus(),
      focusComposer: () => composerRef?.focus(),
      focusedPane: () => ((scrollRef as { focused?: boolean } | undefined)?.focused ? "logs" : "composer"),
      scrollLogs: (delta, unit) => scrollFeedBy(delta, unit ?? "step"),
      submitComposer,
      toggleVerbose: () => {
        const next = !verboseMode()
        setVerboseMode(next)
        options.onVerboseChange?.(next)
        return next
      },
      requestShutdown: options.onRequestShutdown,
      log: (message) => {
        appendChatFeedItem(message)
      },
    },
    { overrides: options.keybindOverrides },
  )

  registerDialogFocusHooks({
    capture: () => {
      const wasLogs = (scrollRef as { focused?: boolean } | undefined)?.focused === true
      return () => {
        if (wasLogs) scrollRef?.focus()
        else composerRef?.focus()
      }
    },
  })
}

export interface StartDashboardOptions {
  onRequestShutdown: () => void
  verbose: boolean
  enable3d?: boolean
  walletActions: WalletActions
  assignmentActions?: AssignmentActions | null
  composerState?: { history: string[]; stash: string }
  onComposerPersist?: (state: { history: string[]; stash: string }) => void
  composerBackend?: ComposerBackend | null
  keybindOverrides?: Record<string, string>
  onVerboseChange?: (verbose: boolean) => void
}

export interface DashboardHandle {
  renderer: CliRenderer
  destroy: () => void
}

async function refreshAssignmentsInto(actions: AssignmentActions | null): Promise<void> {
  if (!actions) {
    setAssignmentsStatus("PYLON_OPENAGENTS_BASE_URL not configured")
    return
  }
  setAssignmentsStatus("refreshing...")
  try {
    const leases = await actions.poll()
    setAssignmentRows(
      leases.map((lease) => ({
        assignmentRef: lease.assignmentRef,
        leaseRef: lease.leaseRef,
        goal: lease.goal,
        paymentMode: lease.paymentMode,
        expiresAt: lease.expiresAt,
      })),
    )
    setAssignmentsStatus(`${leases.length} lease(s) at ${new Date().toISOString().slice(11, 19)}`)
  } catch (error) {
    setAssignmentsStatus(`refresh failed: ${error instanceof Error ? error.message : String(error)}`)
  }
}

export async function startDashboard(options: StartDashboardOptions): Promise<DashboardHandle> {
  setVerboseMode(options.verbose)
  composerHistory = options.composerState?.history ?? []
  restoredStash = options.composerState?.stash ?? ""
  enable3dFlag = options.enable3d ?? true
  composerHistoryIndex = null
  persistComposer = options.onComposerPersist ?? null
  composerBackend = options.composerBackend ?? null
  const assignmentActions = options.assignmentActions ?? null
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

  installDashboardChrome(renderer, options, assignmentActions)

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
      networkSceneDispose?.()
      networkSceneDispose = null
      const draft = composerRef?.plainText ?? ""
      persistComposer?.({ history: composerHistory, stash: draft })
      scrollRef = undefined
      composerRef = undefined
      keymapRef = undefined
      composerBackend = null
      try {
        renderer.destroy()
      } catch {
        // renderer teardown is best-effort during shutdown
      }
      restoreScrollLock()
    },
  }
}
