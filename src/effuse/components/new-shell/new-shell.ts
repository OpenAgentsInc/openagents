/**
 * New Shell Component
 *
 * Tab container shell for switching between Gym and Commander screens.
 * Follows the same pattern as tbcc-shell.ts to preserve child component state.
 */

import { Effect } from "effect"
import type { Component } from "../../component/types.js"
import { html, joinTemplates } from "../../template/html.js"
import { TABS } from "./types.js"

import type { TabId, NewShellState, NewShellEvent } from "./types.js"

// ============================================================================
// Component Definition
// ============================================================================

export const NewShellComponent: Component<NewShellState, NewShellEvent> = {
  id: "new-shell",

  initialState: () => ({
    activeTab: "gym",
    sidebarCollapsed: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

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
      const statusDot = html`<span class="w-2 h-2 rounded-full bg-emerald-500"></span>`
      const statusText = html`<span class="text-xs text-zinc-500">Ready</span>`

      // Sidebar width
      const sidebarWidth = state.sidebarCollapsed ? "w-16" : "w-[260px]"

      return html`
        <div class="flex h-full">
          <!-- Sidebar -->
          <aside class="${sidebarWidth} flex-shrink-0 bg-zinc-950 border-r border-zinc-800/60 flex flex-col transition-all duration-200">
            <!-- Header -->
            <div class="px-4 py-4 border-b border-zinc-800/60">
              ${state.sidebarCollapsed
                ? html`<span class="text-lg font-bold font-mono text-zinc-100">OA</span>`
                : html`
                    <h1 class="text-lg font-bold font-mono text-zinc-100">OpenAgents</h1>
                    <span class="text-xs text-zinc-500">Gym</span>
                  `}
            </div>

            <!-- Navigation -->
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
          <main class="flex-1 overflow-hidden">
            <!-- Tab Content Containers -->
            <div id="new-tab-gym" class="${state.activeTab === "gym" ? "" : "hidden"} h-full"></div>
            <div id="new-tab-commander" class="${state.activeTab === "commander" ? "" : "hidden"} h-full bg-zinc-950"></div>
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
      switch (event.type) {
        case "changeTab": {
          // CRITICAL: Don't update state - it triggers re-render which wipes child widgets
          // Instead, update DOM directly
          const tabIds: TabId[] = ["gym", "commander"]

          // Update tab container visibility
          for (const tabId of tabIds) {
            const container = yield* ctx.dom.queryOption(`#new-tab-${tabId}`)
            if (container) {
              if (tabId === event.tab) {
                container.classList.remove("hidden")
              } else {
                container.classList.add("hidden")
              }
            }
          }

          // Update tab button active states
          const allButtons = ctx.container.querySelectorAll(`[data-action='changeTab']`)
          for (const btn of Array.from(allButtons)) {
            const btnTab = (btn as HTMLElement).dataset.tab as TabId
            if (btnTab === event.tab) {
              btn.classList.remove("text-zinc-400", "border-transparent", "hover:text-zinc-200", "hover:bg-zinc-900/40")
              btn.classList.add("bg-zinc-800/60", "text-zinc-100", "border-emerald-500")
            } else {
              btn.classList.remove("bg-zinc-800/60", "text-zinc-100", "border-emerald-500")
              btn.classList.add("text-zinc-400", "border-transparent", "hover:text-zinc-200", "hover:bg-zinc-900/40")
            }
          }

          break
        }

        case "toggleSidebar": {
          yield* ctx.state.update((s) => ({ ...s, sidebarCollapsed: !s.sidebarCollapsed }))
          break
        }
      }
    }),
}
