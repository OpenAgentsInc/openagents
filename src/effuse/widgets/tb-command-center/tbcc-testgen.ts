/**
 * TB Command Center - TestGen Widget
 *
 * Select a TB task and run environment-aware test generation,
 * streaming test results into the UI one at a time.
 */

import { Effect, Stream } from "effect";
import { html, joinTemplates } from "../../template/html.js";
import { SocketServiceTag } from "../../services/socket.js";
import type { Widget } from "../../widget/types.js";
import type {
  TestGenStartMessage,
  TestGenTestMessage,
  TestGenCompleteMessage,
  TestGenErrorMessage,
} from "../../../hud/protocol.js";

// ============================================================================
// Types
// ============================================================================

export interface TBTestGenState {
  /** Current generation status */
  status: "idle" | "loading_suite" | "generating" | "complete" | "error";

  /** Available task IDs from loaded suite */
  taskIds: string[];

  /** Selected task ID (null = random) */
  selectedTaskId: string | null;

  /** Session ID for current generation */
  sessionId: string | null;

  /** Task info */
  taskId: string | null;
  taskDescription: string | null;

  /** Environment context */
  environment: {
    platform: string;
    prohibitedTools: string[];
    languages: string[];
    fileCount: number;
    filePreviews: number;
  } | null;

  /** Generated tests (streamed in one at a time) */
  tests: Array<{
    id: string;
    category: string;
    input: string;
    expectedOutput: string | null;
    reasoning: string;
    confidence: number;
  }>;

  /** Completion summary */
  totalTests: number;
  durationMs: number;
  uncertainties: string[];

  /** Error message if generation failed */
  error: string | null;
}

export type TBTestGenEvent =
  | { type: "loadSuite" }
  | { type: "selectTask"; taskId: string | null }
  | { type: "generate" }
  | { type: "clear" };

// ============================================================================
// Widget Definition
// ============================================================================

export const TBTestGenWidget: Widget<TBTestGenState, TBTestGenEvent, SocketServiceTag> = {
  id: "tbcc-testgen",

  initialState: () => {
    console.log("[TBTestGen] Creating initial state")
    return {
      status: "idle",
      taskIds: [],
      selectedTaskId: null,
      sessionId: null,
      taskId: null,
      taskDescription: null,
      environment: null,
      tests: [],
      totalTests: 0,
      durationMs: 0,
      uncertainties: [],
      error: null,
    }
  },

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get;

      if ((window as any).bunLog) {
        (window as any).bunLog(`[TBTestGen] render called, status=${state.status}`);
      }

      // Header
      const header = html`
        <div class="border-b border-zinc-800/60 p-4">
          <h2 class="text-xl font-bold font-mono text-zinc-100">Test Generation</h2>
          <p class="text-sm text-zinc-500 mt-1">Environment-aware test generation for TB tasks</p>
        </div>
      `;

      // Controls: task selector and generate button
      const controls = html`
        <div class="p-4 border-b border-zinc-800/60 space-y-4">
          <div class="flex items-center gap-4">
            <label class="text-sm font-mono text-zinc-400 flex-shrink-0">Task:</label>
            <select
              class="flex-1 px-3 py-2 bg-zinc-900 border border-zinc-700 rounded text-sm font-mono text-zinc-200 focus:outline-none focus:border-emerald-500"
              data-action="selectTask"
              ${state.status !== "idle" && state.status !== "complete" && state.status !== "error" ? "disabled" : ""}
            >
              <option value="">Random task</option>
              ${state.taskIds.length > 0
          ? joinTemplates(
            state.taskIds.map(
              (id) => html`
                        <option value="${id}" ${state.selectedTaskId === id ? "selected" : ""}>${id}</option>
                      `
            )
          )
          : html`<option disabled>No tasks loaded</option>`}
            </select>
            <button
              class="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 text-white text-sm font-mono rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              data-action="generate"
              ${state.status !== "idle" && state.status !== "complete" && state.status !== "error" ? "disabled" : ""}
            >
              ${state.status === "generating" ? "Generating..." : "▶ Generate"}
            </button>
            ${state.status === "complete" || state.status === "error"
          ? html`
                  <button
                    class="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 text-white text-sm font-mono rounded transition-colors"
                    data-action="clear"
                  >
                    Clear
                  </button>
                `
          : ""}
          </div>
        </div>
      `;

      // Environment panel (shown after generation starts)
      const environmentPanel =
        state.environment
          ? html`
              <div class="p-4 bg-zinc-900/40 border-b border-zinc-800/60">
                <div class="flex items-start gap-6">
                  <div>
                    <span class="text-xs text-zinc-500 font-mono">Platform:</span>
                    <span class="ml-2 text-sm text-zinc-200 font-mono">${state.environment.platform}</span>
                  </div>
                  <div>
                    <span class="text-xs text-zinc-500 font-mono">Files:</span>
                    <span class="ml-2 text-sm text-zinc-200 font-mono">${state.environment.fileCount} (${state.environment.filePreviews} previews)</span>
                  </div>
                </div>
                ${state.environment.prohibitedTools.length > 0
              ? html`
                      <div class="mt-2">
                        <span class="text-xs text-zinc-500 font-mono">Prohibited Tools:</span>
                        <div class="mt-1 flex flex-wrap gap-2">
                          ${joinTemplates(
                state.environment.prohibitedTools.map(
                  (tool) => html`
                                  <span class="px-2 py-1 bg-red-900/30 border border-red-700/50 rounded text-xs text-red-300 font-mono">
                                    ${tool}
                                  </span>
                                `
                )
              )}
                        </div>
                      </div>
                    `
              : ""}
                ${state.environment.languages.length > 0
              ? html`
                      <div class="mt-2">
                        <span class="text-xs text-zinc-500 font-mono">Languages:</span>
                        <div class="mt-1 flex flex-wrap gap-2">
                          ${joinTemplates(
                state.environment.languages.map(
                  (lang) => html`
                                  <span class="px-2 py-1 bg-blue-900/30 border border-blue-700/50 rounded text-xs text-blue-300 font-mono">
                                    ${lang}
                                  </span>
                                `
                )
              )}
                        </div>
                      </div>
                    `
              : ""}
              </div>
            `
          : "";

      // Task description (shown after generation starts)
      const taskDescPanel =
        state.taskDescription
          ? html`
              <div class="p-4 bg-zinc-900/40 border-b border-zinc-800/60">
                <span class="text-xs text-zinc-500 font-mono">Task: ${state.taskId ?? ""}</span>
                <p class="mt-1 text-sm text-zinc-300 font-mono leading-relaxed">${state.taskDescription.slice(0, 300)}${state.taskDescription.length > 300 ? "..." : ""}</p>
              </div>
            `
          : "";

      // Test cards (streaming in one at a time)
      const testCards =
        state.tests.length > 0
          ? html`
              <div class="p-4 space-y-3 overflow-y-auto flex-1">
                <div class="flex items-center justify-between mb-2">
                  <h3 class="text-sm font-mono text-zinc-400">Generated Tests (${state.tests.length})</h3>
                  ${state.status === "complete"
              ? html`<span class="text-xs text-emerald-400 font-mono">${(state.durationMs / 1000).toFixed(1)}s</span>`
              : ""}
                </div>
                ${joinTemplates(
            state.tests.map((test) => {
              const categoryColors: Record<string, { bg: string; text: string; border: string }> = {
                anti_cheat: { bg: "bg-red-900/30", text: "text-red-300", border: "border-red-700/50" },
                existence: { bg: "bg-blue-900/30", text: "text-blue-300", border: "border-blue-700/50" },
                correctness: { bg: "bg-emerald-900/30", text: "text-emerald-300", border: "border-emerald-700/50" },
                boundary: { bg: "bg-yellow-900/30", text: "text-yellow-300", border: "border-yellow-700/50" },
                integration: { bg: "bg-purple-900/30", text: "text-purple-300", border: "border-purple-700/50" },
              };
              const colors = categoryColors[test.category] ?? { bg: "bg-zinc-800/30", text: "text-zinc-300", border: "border-zinc-700/50" };

              return html`
                        <div class="p-3 bg-zinc-900 border ${colors.border} rounded">
                          <div class="flex items-start justify-between mb-2">
                            <span class="px-2 py-1 ${colors.bg} ${colors.text} text-xs font-mono rounded uppercase">${test.category}</span>
                            <div class="flex items-center gap-2">
                              <span class="text-xs text-zinc-500 font-mono">${(test.confidence * 100).toFixed(0)}%</span>
                              <div class="w-16 h-1.5 bg-zinc-800 rounded overflow-hidden">
                                <div class="h-full ${colors.bg}" style="width: ${test.confidence * 100}%"></div>
                              </div>
                            </div>
                          </div>
                          <div class="space-y-2">
                            <div>
                              <span class="text-xs text-zinc-500 font-mono">Input:</span>
                              <pre class="mt-1 px-2 py-1 bg-zinc-950 border border-zinc-800 rounded text-xs text-zinc-300 font-mono overflow-x-auto">${test.input}</pre>
                            </div>
                            ${test.expectedOutput
                  ? html`
                                  <div>
                                    <span class="text-xs text-zinc-500 font-mono">Expected:</span>
                                    <pre class="mt-1 px-2 py-1 bg-zinc-950 border border-zinc-800 rounded text-xs text-zinc-300 font-mono overflow-x-auto">${test.expectedOutput}</pre>
                                  </div>
                                `
                  : ""}
                            <div>
                              <span class="text-xs text-zinc-500 font-mono">Reasoning:</span>
                              <p class="mt-1 text-xs text-zinc-400 leading-relaxed">${test.reasoning}</p>
                            </div>
                          </div>
                        </div>
                      `;
            })
          )}
              </div>
            `
          : "";

      // Completion summary
      const completionSummary =
        state.status === "complete" && state.uncertainties.length > 0
          ? html`
              <div class="p-4 bg-yellow-900/20 border-t border-yellow-700/50">
                <h4 class="text-sm font-mono text-yellow-300 mb-2">Uncertainties:</h4>
                <ul class="space-y-1">
                  ${joinTemplates(
            state.uncertainties.map(
              (u) => html`
                        <li class="text-xs text-yellow-200 font-mono">• ${u}</li>
                      `
            )
          )}
                </ul>
              </div>
            `
          : "";

      // Error message
      const errorPanel = state.error
        ? html`
            <div class="p-4 bg-red-900/20 border border-red-700/50 rounded m-4">
              <h4 class="text-sm font-mono text-red-300 mb-1">Error:</h4>
              <p class="text-xs text-red-200 font-mono">${state.error}</p>
            </div>
          `
        : "";

      // Empty state
      const emptyState =
        state.status === "idle" && state.tests.length === 0
          ? html`
              <div class="flex-1 flex items-center justify-center text-center p-8">
                <div>
                  <div class="text-6xl mb-4">🧪</div>
                  <h3 class="text-lg font-mono text-zinc-400 mb-2">No tests generated yet</h3>
                  <p class="text-sm text-zinc-600">Select a task and click Generate to start</p>
                </div>
              </div>
            `
          : "";

      // Loading state
      const loadingState =
        state.status === "generating" && state.tests.length === 0
          ? html`
              <div class="flex-1 flex items-center justify-center text-center p-8">
                <div>
                  <div class="animate-spin text-4xl mb-4">⚙️</div>
                  <h3 class="text-lg font-mono text-zinc-400">Generating tests...</h3>
                  <p class="text-sm text-zinc-600 mt-2">This may take 10-30 seconds</p>
                </div>
              </div>
            `
          : "";

      const result = html`
        <div class="h-full flex flex-col bg-zinc-950">
          ${header} ${controls} ${environmentPanel} ${taskDescPanel} ${errorPanel} ${emptyState} ${loadingState} ${testCards} ${completionSummary}
        </div>
      `;

      if ((window as any).bunLog) {
        const emptyStateStr = emptyState.toString();
        const controlsStr = controls.toString();
        (window as any).bunLog(`[TBTestGen] render output length=${result.toString().length}, hasEmptyState=${emptyStateStr !== ""}, hasControls=${controlsStr !== ""}`);
      }

      return result;
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Select task change
      yield* ctx.dom.delegate(ctx.container, "[data-action='selectTask']", "change", (_e, target) => {
        const value = (target as HTMLSelectElement).value;
        Effect.runFork(ctx.emit({ type: "selectTask", taskId: value || null }));
      });

      // Generate button
      yield* ctx.dom.delegate(ctx.container, "[data-action='generate']", "click", () => {
        Effect.runFork(ctx.emit({ type: "generate" }));
      });

      // Clear button
      yield* ctx.dom.delegate(ctx.container, "[data-action='clear']", "click", () => {
        Effect.runFork(ctx.emit({ type: "clear" }));
      });
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag;

      switch (event.type) {
        case "loadSuite": {
          // Load the suite to get task IDs
          yield* ctx.state.update((s) => ({ ...s, status: "loading_suite" }));

          const suiteInfo = yield* socket.loadTBSuite("tasks/terminal-bench-2.json").pipe(
            Effect.catchAll((error) =>
              Effect.gen(function* () {
                yield* ctx.state.update((s) => ({
                  ...s,
                  status: "error",
                  error: error.message,
                }));
                return null;
              })
            )
          );

          if (suiteInfo) {
            const taskIds = suiteInfo.tasks.map((t) => t.id);
            yield* ctx.state.update((s) => ({
              ...s,
              status: "idle",
              taskIds,
            }));
          }
          break;
        }

        case "selectTask": {
          yield* ctx.state.update((s) => ({ ...s, selectedTaskId: event.taskId }));
          break;
        }

        case "generate": {
          const state = yield* ctx.state.get;

          // Clear previous results
          yield* ctx.state.update((s) => ({
            ...s,
            status: "generating",
            sessionId: null,
            taskId: null,
            taskDescription: null,
            environment: null,
            tests: [],
            totalTests: 0,
            durationMs: 0,
            uncertainties: [],
            error: null,
          }));

          // Send request to start test generation
          const result = yield* socket
            .startTestGen("tasks/terminal-bench-2.json", state.selectedTaskId ?? undefined, "local")
            .pipe(
              Effect.catchAll((error) =>
                Effect.gen(function* () {
                  yield* ctx.state.update((s) => ({
                    ...s,
                    status: "error",
                    error: error.message,
                  }));
                  return null;
                })
              )
            );

          if (result) {
            yield* ctx.state.update((s) => ({ ...s, sessionId: result.sessionId }));
          }
          break;
        }

        case "clear": {
          yield* ctx.state.update((s) => ({
            ...s,
            status: "idle",
            sessionId: null,
            taskId: null,
            taskDescription: null,
            environment: null,
            tests: [],
            totalTests: 0,
            durationMs: 0,
            uncertainties: [],
            error: null,
          }));
          break;
        }
      }
    }),

  subscriptions: (ctx) => {
    // Subscribe to testgen HUD messages
    const testgenSub = Effect.gen(function* () {
      const socket = yield* SocketServiceTag;

      // Load suite on mount
      yield* ctx.emit({ type: "loadSuite" });

      // Subscribe to testgen messages
      yield* Stream.runForEach(socket.getMessages(), (msg) =>
        Effect.gen(function* () {
          const state = yield* ctx.state.get;

          // Only handle messages for our current session
          if ("sessionId" in msg && msg.sessionId !== state.sessionId) {
            return;
          }

          if (msg.type === "testgen_start") {
            const data = msg as TestGenStartMessage;
            yield* ctx.state.update((s) => ({
              ...s,
              taskId: data.taskId,
              taskDescription: data.taskDescription,
              environment: data.environment,
            }));
          } else if (msg.type === "testgen_test") {
            const data = msg as TestGenTestMessage;
            yield* ctx.state.update((s) => ({
              ...s,
              tests: [...s.tests, data.test],
            }));
          } else if (msg.type === "testgen_complete") {
            const data = msg as TestGenCompleteMessage;
            yield* ctx.state.update((s) => ({
              ...s,
              status: "complete",
              totalTests: data.totalTests,
              durationMs: data.durationMs,
              uncertainties: data.uncertainties,
            }));
          } else if (msg.type === "testgen_error") {
            const data = msg as TestGenErrorMessage;
            yield* ctx.state.update((s) => ({
              ...s,
              status: "error",
              error: data.error,
            }));
          }
        })
      );
    });

    return [Stream.make(testgenSub)];
  },
};
