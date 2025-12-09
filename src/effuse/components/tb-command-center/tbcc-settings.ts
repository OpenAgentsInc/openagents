/**
 * TB Command Center Settings Component
 *
 * Configuration for TB execution and logging.
 */

import { Effect } from "effect"
import type { Component } from "../../component/types.js"
import { html } from "../../template/html.js"
import type { ExecutionSettings, LoggingSettings } from "./types.js"
import { DEFAULT_EXECUTION_SETTINGS, DEFAULT_LOGGING_SETTINGS } from "./types.js"

// ============================================================================
// Types
// ============================================================================

export interface TBCCSettingsState {
  execution: ExecutionSettings
  logging: LoggingSettings
  saved: boolean
}

export type TBCCSettingsEvent =
  | { type: "updateExecution"; key: keyof ExecutionSettings; value: any }
  | { type: "updateLogging"; key: keyof LoggingSettings; value: any }
  | { type: "save" }
  | { type: "reset" }

// ============================================================================
// Helpers
// ============================================================================

const STORAGE_KEY = "tbcc_settings"

const loadSettings = (): { execution: ExecutionSettings; logging: LoggingSettings } => {
  if (typeof localStorage === "undefined") {
    return { execution: DEFAULT_EXECUTION_SETTINGS, logging: DEFAULT_LOGGING_SETTINGS }
  }
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved) {
      return JSON.parse(saved)
    }
  } catch (e) {
    console.error("Failed to load settings", e)
  }
  return { execution: DEFAULT_EXECUTION_SETTINGS, logging: DEFAULT_LOGGING_SETTINGS }
}

const saveSettings = (execution: ExecutionSettings, logging: LoggingSettings) => {
  if (typeof localStorage === "undefined") return
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ execution, logging }))
  } catch (e) {
    console.error("Failed to save settings", e)
  }
}

// ============================================================================
// Component Definition
// ============================================================================

export const TBCCSettingsComponent: Component<TBCCSettingsState, TBCCSettingsEvent> = {
  id: "tbcc-settings",

  initialState: () => {
    const { execution, logging } = loadSettings()
    return {
      execution,
      logging,
      saved: false,
    }
  },

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      const inputClass = "w-full bg-zinc-900 border border-zinc-700 rounded px-3 py-2 text-sm text-zinc-200 focus:border-emerald-500 focus:outline-none"
      const checkboxClass = "w-4 h-4 rounded border-zinc-700 bg-zinc-900 text-emerald-500 focus:ring-emerald-500 focus:ring-offset-zinc-900"
      const labelClass = "block text-xs font-medium text-zinc-400 mb-1"

      return html`
        <div class="h-full overflow-y-auto p-8">
          <div class="max-w-2xl mx-auto">
            <div class="flex items-center justify-between mb-8">
              <h2 class="text-xl font-bold font-mono text-zinc-100">Settings</h2>
              ${state.saved
          ? html`<span class="text-sm text-emerald-400 animate-fade-out">Settings saved!</span>`
          : ""}
            </div>

            <!-- Model Selection -->
            <div class="bg-zinc-900/40 border border-zinc-800/60 rounded-lg p-6 mb-6">
              <h3 class="text-sm font-bold text-zinc-200 mb-4 flex items-center gap-2">
                <span class="text-lg">🤖</span> Model
              </h3>
              <div class="flex gap-4">
                <label class="flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors
                  ${state.execution.model === "fm"
                    ? "bg-emerald-900/30 border-emerald-600/50 text-emerald-200"
                    : "border-zinc-700/50 text-zinc-400 hover:border-zinc-600/60"}">
                  <input
                    type="radio"
                    name="model"
                    value="fm"
                    ${state.execution.model === "fm" ? "checked" : ""}
                    data-action="updateExecution"
                    data-key="model"
                    class="hidden"
                  />
                  <div>
                    <div class="font-medium">Foundation Model</div>
                    <div class="text-xs opacity-70">Apple on-device (default)</div>
                  </div>
                </label>
                <label class="flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors
                  ${state.execution.model === "claude-code"
                    ? "bg-blue-900/30 border-blue-600/50 text-blue-200"
                    : "border-zinc-700/50 text-zinc-400 hover:border-zinc-600/60"}">
                  <input
                    type="radio"
                    name="model"
                    value="claude-code"
                    ${state.execution.model === "claude-code" ? "checked" : ""}
                    data-action="updateExecution"
                    data-key="model"
                    class="hidden"
                  />
                  <div>
                    <div class="font-medium">Claude Code</div>
                    <div class="text-xs opacity-70">Cloud-based</div>
                  </div>
                </label>
              </div>
            </div>

            <!-- Execution Settings -->
            <div class="bg-zinc-900/40 border border-zinc-800/60 rounded-lg p-6 mb-6">
              <h3 class="text-sm font-bold text-zinc-200 mb-4 flex items-center gap-2">
                <span class="text-lg">⚡</span> Execution
              </h3>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div>
                  <label class="${labelClass}">Max Attempts</label>
                  <input
                    type="number"
                    class="${inputClass}"
                    value="${state.execution.maxAttempts}"
                    data-action="updateExecution"
                    data-key="maxAttempts"
                  />
                  <p class="text-[10px] text-zinc-500 mt-1">Retries per task</p>
                </div>

                <div>
                  <label class="${labelClass}">Max Steps per Run</label>
                  <input
                    type="number"
                    class="${inputClass}"
                    value="${state.execution.maxStepsPerRun}"
                    data-action="updateExecution"
                    data-key="maxStepsPerRun"
                  />
                  <p class="text-[10px] text-zinc-500 mt-1">Limit to prevent infinite loops</p>
                </div>

                <div>
                  <label class="${labelClass}">Timeout (seconds)</label>
                  <input
                    type="number"
                    class="${inputClass}"
                    value="${state.execution.timeoutSeconds}"
                    data-action="updateExecution"
                    data-key="timeoutSeconds"
                  />
                </div>

                <div>
                  <label class="${labelClass}">Recursion Limit</label>
                  <input
                    type="number"
                    class="${inputClass}"
                    value="${state.execution.recursionLimitN}"
                    data-action="updateExecution"
                    data-key="recursionLimitN"
                  />
                </div>
              </div>

              <div class="mt-6 space-y-3">
                <label class="flex items-center gap-3">
                  <input
                    type="checkbox"
                    class="${checkboxClass}"
                    ${state.execution.deepComputeEnabled ? "checked" : ""}
                    data-action="updateExecution"
                    data-key="deepComputeEnabled"
                  />
                  <span class="text-sm text-zinc-300">Enable Deep Compute (Tree Search)</span>
                </label>

                <label class="flex items-center gap-3">
                  <input
                    type="checkbox"
                    class="${checkboxClass}"
                    ${state.execution.earlyStopOnHighConfidence ? "checked" : ""}
                    data-action="updateExecution"
                    data-key="earlyStopOnHighConfidence"
                  />
                  <span class="text-sm text-zinc-300">Early stop on high confidence</span>
                </label>
              </div>
            </div>

            <!-- Logging Settings -->
            <div class="bg-zinc-900/40 border border-zinc-800/60 rounded-lg p-6 mb-8">
              <h3 class="text-sm font-bold text-zinc-200 mb-4 flex items-center gap-2">
                <span class="text-lg">📝</span> Logging & Storage
              </h3>

              <div class="space-y-3">
                <label class="flex items-center gap-3">
                  <input
                    type="checkbox"
                    class="${checkboxClass}"
                    ${state.logging.saveTrajectories ? "checked" : ""}
                    data-action="updateLogging"
                    data-key="saveTrajectories"
                  />
                  <span class="text-sm text-zinc-300">Save full trajectories (JSON)</span>
                </label>

                <label class="flex items-center gap-3">
                  <input
                    type="checkbox"
                    class="${checkboxClass}"
                    ${state.logging.saveTerminalOutput ? "checked" : ""}
                    data-action="updateLogging"
                    data-key="saveTerminalOutput"
                  />
                  <span class="text-sm text-zinc-300">Save terminal output logs</span>
                </label>

                <label class="flex items-center gap-3">
                  <input
                    type="checkbox"
                    class="${checkboxClass}"
                    ${state.logging.saveAtifTraces ? "checked" : ""}
                    data-action="updateLogging"
                    data-key="saveAtifTraces"
                  />
                  <span class="text-sm text-zinc-300">Save ATIF traces</span>
                </label>
              </div>

              <div class="mt-6">
                <label class="${labelClass}">Auto-prune logs older than (days)</label>
                <input
                  type="number"
                  class="${inputClass} w-32"
                  value="${state.logging.autoPruneDays ?? ""}"
                  placeholder="Never"
                  data-action="updateLogging"
                  data-key="autoPruneDays"
                />
              </div>
            </div>

            <!-- Actions -->
            <div class="flex items-center justify-end gap-4">
              <button
                class="px-4 py-2 text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
                data-action="reset"
              >
                Reset to Defaults
              </button>
              <button
                class="px-6 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-sm font-medium rounded transition-colors shadow-lg shadow-emerald-900/20"
                data-action="save"
              >
                Save Changes
              </button>
            </div>
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Inputs (text, number, checkbox)
      yield* ctx.dom.delegate(ctx.container, "input", "change", (_e, target) => {
        const el = target as HTMLInputElement
        const action = el.dataset.action
        const key = el.dataset.key

        if (!action || !key) return

        let value: any
        if (el.type === "checkbox") {
          value = el.checked
        } else if (el.type === "number") {
          value = el.value === "" ? null : Number(el.value)
        } else if (el.type === "radio") {
          value = el.value
        } else {
          value = el.value
        }

        if (action === "updateExecution") {
          Effect.runFork(ctx.emit({ type: "updateExecution", key: key as keyof ExecutionSettings, value }))
        } else if (action === "updateLogging") {
          Effect.runFork(ctx.emit({ type: "updateLogging", key: key as keyof LoggingSettings, value }))
        }
      })

      // Model selection labels (click to select radio)
      yield* ctx.dom.delegate(ctx.container, "label:has(input[name='model'])", "click", (_e, target) => {
        const label = target as HTMLLabelElement
        const radio = label.querySelector("input[type='radio']") as HTMLInputElement | null
        if (radio && !radio.checked) {
          Effect.runFork(ctx.emit({
            type: "updateExecution",
            key: "model" as keyof ExecutionSettings,
            value: radio.value
          }))
        }
      })

      // Buttons
      yield* ctx.dom.delegate(ctx.container, "button[data-action='save']", "click", () => {
        Effect.runFork(ctx.emit({ type: "save" }))
      })

      yield* ctx.dom.delegate(ctx.container, "button[data-action='reset']", "click", () => {
        Effect.runFork(ctx.emit({ type: "reset" }))
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "updateExecution": {
          yield* ctx.state.update((s) => ({
            ...s,
            execution: { ...s.execution, [event.key]: event.value },
            saved: false,
          }))
          break
        }

        case "updateLogging": {
          yield* ctx.state.update((s) => ({
            ...s,
            logging: { ...s.logging, [event.key]: event.value },
            saved: false,
          }))
          break
        }

        case "save": {
          const state = yield* ctx.state.get
          saveSettings(state.execution, state.logging)
          yield* ctx.state.update((s) => ({ ...s, saved: true }))

          // Reset saved flag after 2s
          yield* Effect.sleep("2 seconds")
          yield* ctx.state.update((s) => ({ ...s, saved: false }))
          break
        }

        case "reset": {
          yield* ctx.state.update((s) => ({
            ...s,
            execution: DEFAULT_EXECUTION_SETTINGS,
            logging: DEFAULT_LOGGING_SETTINGS,
            saved: false,
          }))
          break
        }
      }
    }),
}
