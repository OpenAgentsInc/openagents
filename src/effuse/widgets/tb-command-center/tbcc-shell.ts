/**
 * TB Command Center Shell Widget
 *
 * Main container widget that provides tab navigation and coordinates
 * between child widgets (Dashboard, Tasks, Runs, Settings).
 */

import { Effect, Stream } from "effect"
import { SocketServiceTag } from "../../services/socket.js"
import { html, joinTemplates } from "../../template/html.js"
import { TABS } from "./types.js"

import type { Widget } from "../../widget/types.js"
import type { TabId, CurrentRunInfo } from "./types.js"
// ============================================================================
// Types
// ============================================================================

export interface TBCCShellState {
  /** Currently active tab */
  activeTab: TabId
  /** Whether sidebar is collapsed */
  sidebarCollapsed: boolean
  /** Current run status (if any) */
  currentRun: CurrentRunInfo | null
  /** Connection status */
  connected: boolean
}

export type TBCCShellEvent =
  | { type: "changeTab"; tab: TabId }
  | { type: "toggleSidebar" }
  | { type: "runStarted"; runId: string; taskId: string; taskName: string }
  | { type: "runCompleted"; runId: string; outcome: string }
  | { type: "connectionChange"; connected: boolean }

// ============================================================================
// Widget Definition
// ============================================================================

export const TBCCShellWidget: Widget<TBCCShellState, TBCCShellEvent, SocketServiceTag> = {
  id: "tbcc-shell",

  initialState: () => ({
    activeTab: "dashboard",
    sidebarCollapsed: false,
    currentRun: null,
    connected: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      if ((window as any).bunLog) {
        (window as any).bunLog(`[TBCCShell] render called, activeTab=${state.activeTab}`);
      }

      // Tab navigation items
      const tabItems = joinTemplates(
        TABS.map((tab) => {
          const isActive = tab.id === state.activeTab
          const baseClasses = "flex items-center gap-3 px-4 py-3 text-sm font-mono transition-colors cursor-pointer"
          const activeClasses = isActive
            ? "bg-zinc-800/60 text-zinc-100 border-l-2 border-emerald-500"
            : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900/40 border-l-2 border-transparent"

          return html`
            <button class="${baseClasses} ${activeClasses}" data-action="changeTab" data-tab="${tab.id}">
              ${state.sidebarCollapsed ? "" : html`<span>${tab.label}</span>`}
            </button>
          `
        })
      )

      // Status indicator
      const statusDot = state.currentRun
        ? html`<span class="w-2 h-2 rounded-full bg-blue-500 animate-pulse"></span>`
        : state.connected
          ? html`<span class="w-2 h-2 rounded-full bg-emerald-500"></span>`
          : html`<span class="w-2 h-2 rounded-full bg-zinc-600"></span>`

      const statusText = state.currentRun
        ? html`<span class="text-xs text-blue-300 truncate">${state.currentRun.taskName}</span>`
        : state.connected
          ? html`<span class="text-xs text-zinc-500">Ready</span>`
          : html`<span class="text-xs text-zinc-600">Disconnected</span>`

      // Sidebar width
      const sidebarWidth = state.sidebarCollapsed ? "w-16" : "w-[260px]"

      return html`
        <div class="flex h-full">
          <!-- Sidebar -->
          <aside class="${sidebarWidth} flex-shrink-0 bg-zinc-950 border-r border-zinc-800/60 flex flex-col transition-all duration-200">
            <!-- Header -->
            <div class="px-4 py-4 border-b border-zinc-800/60">
              ${state.sidebarCollapsed
          ? html`<span class="text-lg font-bold font-mono text-zinc-100">TB</span>`
          : html`
                    <h1 class="text-lg font-bold font-mono text-zinc-100">TerminalBench</h1>
                    <span class="text-xs text-zinc-500">Command Center</span>
                  `}
            </div>

            <!-- Tab Navigation -->
            <nav class="flex-1 py-2 flex flex-col">${tabItems}</nav>

            <!-- Collapse Toggle -->
            <button
              class="px-4 py-2 text-xs text-zinc-500 hover:text-zinc-300 border-t border-zinc-800/60 transition-colors"
              data-action="toggleSidebar"
            >
              ${state.sidebarCollapsed ? "→" : "← Collapse"}
            </button>

            <!-- Status Bar -->
            <div class="px-4 py-3 border-t border-zinc-800/60 flex items-center gap-2">
              ${statusDot} ${state.sidebarCollapsed ? "" : statusText}
            </div>
          </aside>

          <!-- Main Content Area -->
          <main class="flex-1 bg-zinc-950 overflow-hidden">
            <!-- Tab Content Containers -->
            <div id="tbcc-tab-dashboard" class="${state.activeTab === "dashboard" ? "" : "hidden"} h-full"></div>
            <div id="tbcc-tab-tasks" class="${state.activeTab === "tasks" ? "" : "hidden"} h-full"></div>
            <div id="tbcc-tab-runs" class="${state.activeTab === "runs" ? "" : "hidden"} h-full"></div>
            <div id="tbcc-tab-testgen" class="${state.activeTab === "testgen" ? "" : "hidden"} h-full"></div>
            <div id="tbcc-tab-settings" class="${state.activeTab === "settings" ? "" : "hidden"} h-full"></div>
          </main>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Tab change clicks
      yield* ctx.dom.delegate(ctx.container, "[data-action='changeTab']", "click", (_e, target) => {
        const tab = (target as HTMLElement).dataset.tab as TabId
        if (tab) {
          Effect.runFork(ctx.emit({ type: "changeTab", tab }))
        }
      })

      // Sidebar toggle
      yield* ctx.dom.delegate(ctx.container, "[data-action='toggleSidebar']", "click", () => {
        Effect.runFork(ctx.emit({ type: "toggleSidebar" }))
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      if ((window as any).bunLog) {
        (window as any).bunLog(`[TBCCShell] handleEvent: ${event.type}`);
      }
      switch (event.type) {
        case "changeTab": {
          if ((window as any).bunLog) {
            (window as any).bunLog(`[TBCCShell] Changing tab to: ${event.tab}`);
          }

          // CRITICAL: Don't update state - it triggers re-render which wipes child widgets
          // Instead, update DOM directly. State will be out of sync, but UI will work.
          // If shell re-renders for other reasons, we'll sync state then.
          const TABS: TabId[] = ["dashboard", "tasks", "runs", "testgen", "settings"]

          // Update tab container visibility
          for (const tabId of TABS) {
            const container = yield* ctx.dom.queryOption(`#tbcc-tab-${tabId}`)
            if (container) {
              if (tabId === event.tab) {
                container.classList.remove("hidden")
                if ((window as any).bunLog) {
                  (window as any).bunLog(`[TBCCShell] Showing tab: ${tabId}, container.innerHTML.length=${container.innerHTML.length}`);
                }
              } else {
                container.classList.add("hidden")
              }
            } else {
              if ((window as any).bunLog) {
                (window as any).bunLog(`[TBCCShell] WARNING: Tab container #tbcc-tab-${tabId} not found!`);
              }
            }
          }

          // Update sidebar button active states
          const allButtons = ctx.container.querySelectorAll(`[data-action='changeTab']`)
          for (const btn of Array.from(allButtons)) {
            const btnTab = (btn as HTMLElement).dataset.tab as TabId
            if (btnTab === event.tab) {
              btn.classList.remove("text-zinc-400", "hover:text-zinc-200", "hover:bg-zinc-900/40", "border-transparent")
              btn.classList.add("bg-zinc-800/60", "text-zinc-100", "border-emerald-500")
            } else {
              btn.classList.remove("bg-zinc-800/60", "text-zinc-100", "border-emerald-500")
              btn.classList.add("text-zinc-400", "hover:text-zinc-200", "hover:bg-zinc-900/40", "border-transparent")
            }
          }

          // Update state silently (without triggering re-render) by reading current tab from DOM
          // This keeps state in sync for other code that reads it, but doesn't trigger re-render
          // Actually, we can't update state without triggering re-render, so we'll leave it out of sync
          // If other code needs activeTab, it should read from DOM or we'll handle it differently

          break
        }

        case "toggleSidebar": {
          yield* ctx.state.update((s) => ({ ...s, sidebarCollapsed: !s.sidebarCollapsed }))
          break
        }

        case "runStarted": {
          yield* ctx.state.update((s) => ({
            ...s,
            currentRun: {
              runId: event.runId,
              taskId: event.taskId,
              taskName: event.taskName,
              attempt: 1,
              maxAttempts: 5,
              status: "running",
              startedAt: Date.now(),
              currentStep: 0,
              totalSteps: null,
            },
          }))
          break
        }

        case "runCompleted": {
          yield* ctx.state.update((s) => ({
            ...s,
            currentRun: null,
          }))
          break
        }

        case "connectionChange": {
          yield* ctx.state.update((s) => ({ ...s, connected: event.connected }))
          break
        }
      }
    }),

  subscriptions: (ctx) => {
    // Subscribe to socket connection status and run events
    const socketSub = Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      // Mark as connected initially
      yield* ctx.emit({ type: "connectionChange", connected: true })

      // Subscribe to run-related HUD messages
      yield* Stream.runForEach(socket.getMessages(), (msg) =>
        Effect.gen(function* () {
          if (msg.type === "tb_run_start") {
            const data = msg as { type: "tb_run_start"; runId: string; taskIds: string[] }
            yield* ctx.emit({
              type: "runStarted",
              runId: data.runId,
              taskId: data.taskIds[0] ?? "unknown",
              taskName: data.taskIds[0] ?? "Task",
            })
          } else if (msg.type === "tb_run_complete") {
            const data = msg as { type: "tb_run_complete"; runId: string; passRate: number }
            yield* ctx.emit({
              type: "runCompleted",
              runId: data.runId,
              outcome: data.passRate >= 0.5 ? "success" : "failure",
            })
          }
        })
      )
    })

    return [Stream.make(socketSub)]
  },
}
