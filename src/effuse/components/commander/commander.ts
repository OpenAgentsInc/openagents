/**
 * Commander Component
 *
 * Main MechaCoder control interface with prompt input and ATIF event feed.
 */

import { Effect, Stream } from "effect"
import type { Component } from "../../component/types.js"
import { SocketServiceTag } from "../../services/socket.js"
import { html } from "../../template/html.js"
import { renderThreadContainer, type ThreadItem } from "../atif-thread.js"
import type { CommanderState, CommanderEvent, TestItem } from "./types.js"
import {
  isTestGenStart,
  isTestGenTest,
  isTestGenProgress,
  isTestGenReflection,
  isTestGenComplete,
  isTestGenError,
  isTestGenMessage,
  type TestGenStartMessage,
  type TestGenTestMessage,
  type TestGenProgressMessage,
  type TestGenReflectionMessage,
  type TestGenCompleteMessage,
  type TestGenErrorMessage,
} from "../../../hud/protocol.js"

// ============================================================================
// Component Definition
// ============================================================================

export const CommanderComponent: Component<CommanderState, CommanderEvent, SocketServiceTag> = {
  id: "commander",

  initialState: () => ({
    promptInput: "",
    isGenerating: false,
    sessionId: null,
    threadItems: [],
    expandedItemId: null,
    statusMessage: null,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Render thread items
      const threadContent = state.threadItems.length > 0
        ? renderThreadContainer(state.threadItems, {
            expandedItemId: state.expandedItemId,
          })
        : html`
            <div class="flex items-center justify-center h-full text-zinc-600 font-mono text-sm">
              Enter a task description and click Generate to see tests
            </div>
          `

      return html`
        <div class="flex flex-col h-full p-6 gap-4">
          <!-- Header -->
          <div class="flex-shrink-0">
            <h2 class="text-xl font-bold font-mono text-zinc-100 mb-1">Commander</h2>
            <p class="text-sm text-zinc-500">Generate tests from task descriptions</p>
          </div>

          <!-- Prompt Input Section -->
          <div class="flex-shrink-0">
            <div class="flex gap-3">
              <div class="flex-1 relative">
                <textarea
                  id="commander-prompt-input"
                  class="w-full h-24 px-4 py-3 bg-zinc-900/60 border border-zinc-700/60 rounded-lg text-zinc-200 font-mono text-sm placeholder-zinc-600 resize-none focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/30"
                  placeholder="Describe a task... e.g., 'Extract the last IP address and date from log files using grep'"
                  data-input="prompt"
                >${state.promptInput}</textarea>
              </div>
              <div class="flex flex-col gap-2">
                <button
                  class="px-5 py-3 bg-emerald-600 hover:bg-emerald-500 disabled:bg-zinc-700 disabled:text-zinc-500 text-white font-mono text-sm rounded-lg transition-colors"
                  data-action="submit"
                  ${state.isGenerating ? "disabled" : ""}
                >
                  ${state.isGenerating ? "Generating..." : "Generate"}
                </button>
                <button
                  class="px-5 py-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-400 font-mono text-xs rounded-lg transition-colors"
                  data-action="clear"
                >
                  Clear
                </button>
              </div>
            </div>
            ${state.statusMessage
              ? html`<div class="mt-2 text-xs font-mono text-zinc-500">${state.statusMessage}</div>`
              : ""}
          </div>

          <!-- ATIF Event Feed -->
          <div class="flex-1 overflow-y-auto bg-zinc-900/30 border border-zinc-800/60 rounded-lg p-4">
            <div class="mb-3 pb-2 border-b border-zinc-800/60 flex items-center justify-between">
              <h3 class="text-sm font-mono text-zinc-400">Test Generation Feed</h3>
              <span class="text-xs font-mono text-zinc-600">${state.threadItems.length} items</span>
            </div>
            ${threadContent}
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Submit button - use mousedown due to webview-bun click bug
      yield* ctx.dom.delegate(ctx.container, "[data-action='submit']", "mousedown", () => {
        Effect.runFork(ctx.emit({ type: "submitPrompt" }))
      })

      // Clear button
      yield* ctx.dom.delegate(ctx.container, "[data-action='clear']", "mousedown", () => {
        Effect.runFork(ctx.emit({ type: "clearItems" }))
      })

      // Thread item toggle
      yield* ctx.dom.delegate(ctx.container, "[data-action='toggleItem']", "mousedown", (_e, target) => {
        const itemId = (target as HTMLElement).dataset.itemId
        if (itemId) {
          Effect.runFork(ctx.emit({ type: "toggleItem", itemId }))
        }
      })

      // Text input change - use input event for real-time updates
      const textarea = ctx.container.querySelector("#commander-prompt-input") as HTMLTextAreaElement | null
      if (textarea) {
        textarea.addEventListener("input", (e) => {
          const value = (e.target as HTMLTextAreaElement).value
          Effect.runFork(ctx.emit({ type: "promptChanged", value }))
        })
      }
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "promptChanged": {
          yield* ctx.state.update((s) => ({ ...s, promptInput: event.value }))
          break
        }

        case "submitPrompt": {
          const state = yield* ctx.state.get
          if (state.isGenerating || !state.promptInput.trim()) {
            break
          }

          // Start generation
          yield* ctx.state.update((s) => ({
            ...s,
            isGenerating: true,
            threadItems: [],
            statusMessage: "Starting test generation...",
          }))

          // Trigger the test generation via socket
          const socket = yield* SocketServiceTag
          const sessionId = yield* Effect.sync(() => `tg-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`)

          yield* ctx.state.update((s) => ({ ...s, sessionId }))

          // Call the custom testgen method
          yield* Effect.promise(() =>
            socket.startCustomTestGen(state.promptInput.trim(), sessionId, "local")
          )

          break
        }

        case "clearItems": {
          yield* ctx.state.update((s) => ({
            ...s,
            threadItems: [],
            sessionId: null,
            statusMessage: null,
            isGenerating: false,
          }))
          break
        }

        case "toggleItem": {
          yield* ctx.state.update((s) => ({
            ...s,
            expandedItemId: s.expandedItemId === event.itemId ? null : event.itemId,
          }))
          break
        }

        case "testgenStarted": {
          yield* ctx.state.update((s) => ({
            ...s,
            statusMessage: `Generating tests for: ${event.taskDescription.slice(0, 50)}...`,
          }))
          break
        }

        case "testgenProgress": {
          const item: ThreadItem = {
            type: "progress",
            timestamp: Date.now(),
            data: {
              phase: event.phase,
              category: event.category,
              round: event.round,
              status: event.status,
            },
          }
          yield* ctx.state.update((s) => ({
            ...s,
            threadItems: [...s.threadItems, item],
          }))
          break
        }

        case "testgenReflection": {
          const item: ThreadItem = {
            type: "reflection",
            timestamp: Date.now(),
            data: {
              category: event.category,
              text: event.text,
              action: "assessing",
            },
          }
          yield* ctx.state.update((s) => ({
            ...s,
            threadItems: [...s.threadItems, item],
          }))
          break
        }

        case "testgenTest": {
          const item: ThreadItem = {
            type: "test",
            timestamp: Date.now(),
            data: event.test,
          }
          yield* ctx.state.update((s) => ({
            ...s,
            threadItems: [...s.threadItems, item],
          }))
          break
        }

        case "testgenComplete": {
          const item: ThreadItem = {
            type: "complete",
            timestamp: Date.now(),
            data: {
              totalTests: event.totalTests,
              totalRounds: event.totalRounds,
              comprehensivenessScore: event.comprehensivenessScore,
              totalTokensUsed: event.totalTokensUsed,
              durationMs: event.durationMs,
              uncertainties: [],
            },
          }
          yield* ctx.state.update((s) => ({
            ...s,
            threadItems: [...s.threadItems, item],
            isGenerating: false,
            statusMessage: `Complete: ${event.totalTests} tests generated`,
          }))
          break
        }

        case "testgenError": {
          const item: ThreadItem = {
            type: "error",
            timestamp: Date.now(),
            data: {
              error: event.error,
            },
          }
          yield* ctx.state.update((s) => ({
            ...s,
            threadItems: [...s.threadItems, item],
            isGenerating: false,
            statusMessage: `Error: ${event.error}`,
          }))
          break
        }
      }
    }),

  subscriptions: (ctx) => {
    // Subscribe to testgen-related HUD messages
    const testgenSub = Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      yield* Stream.runForEach(socket.getMessages(), (msg) =>
        Effect.gen(function* () {
          // Only process testgen messages
          if (!isTestGenMessage(msg)) return

          // Get current session ID to filter messages
          const state = yield* ctx.state.get
          const sessionId = state.sessionId

          // Process based on message type
          if (isTestGenStart(msg)) {
            const m = msg as TestGenStartMessage
            // Only process if no session or matching session
            if (!sessionId || m.sessionId === sessionId) {
              yield* ctx.emit({
                type: "testgenStarted",
                sessionId: m.sessionId,
                taskDescription: m.taskDescription,
              })
            }
          } else if (isTestGenProgress(msg)) {
            const m = msg as TestGenProgressMessage
            if (sessionId && m.sessionId === sessionId) {
              yield* ctx.emit({
                type: "testgenProgress",
                phase: m.phase,
                category: m.currentCategory ?? null,
                round: m.round,
                status: m.status,
              })
            }
          } else if (isTestGenReflection(msg)) {
            const m = msg as TestGenReflectionMessage
            if (sessionId && m.sessionId === sessionId) {
              yield* ctx.emit({
                type: "testgenReflection",
                category: m.category ?? null,
                text: m.reflectionText,
              })
            }
          } else if (isTestGenTest(msg)) {
            const m = msg as TestGenTestMessage
            if (sessionId && m.sessionId === sessionId) {
              yield* ctx.emit({
                type: "testgenTest",
                test: {
                  id: m.test.id,
                  category: m.test.category,
                  input: m.test.input,
                  expectedOutput: m.test.expectedOutput,
                  reasoning: m.test.reasoning,
                  confidence: m.test.confidence,
                },
              })
            }
          } else if (isTestGenComplete(msg)) {
            const m = msg as TestGenCompleteMessage
            if (sessionId && m.sessionId === sessionId) {
              yield* ctx.emit({
                type: "testgenComplete",
                totalTests: m.totalTests,
                totalRounds: m.totalRounds,
                comprehensivenessScore: m.comprehensivenessScore,
                totalTokensUsed: m.totalTokensUsed,
                durationMs: m.durationMs,
              })
            }
          } else if (isTestGenError(msg)) {
            const m = msg as TestGenErrorMessage
            if (sessionId && m.sessionId === sessionId) {
              yield* ctx.emit({
                type: "testgenError",
                error: m.error,
              })
            }
          }
        })
      )
    })

    return [Stream.make(testgenSub)]
  },
}
