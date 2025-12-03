import { Effect } from "effect";
import type { Tool } from "../../tools/schema.js";
import type { OpenRouterClient } from "../../llm/openrouter.js";
import { detectClaudeCode, type ClaudeCodeAvailability } from "./claude-code-detector.js";
import { runClaudeCodeSubagent } from "./claude-code-subagent.js";
import { runSubagent, createSubagentConfig } from "./subagent.js";
import type { SubagentResult, Subtask, ClaudeCodeSettings, SubagentConfig } from "./types.js";

const COMPLEX_KEYWORDS = ["refactor", "multi-file", "multi file", "search", "fetch", "investigate"];

export interface RunBestAvailableSubagentOptions<R> {
  subtask: Subtask;
  cwd: string;
  openagentsDir: string;
  tools: Tool<any, any, any, any>[];
  model?: string;
  signal?: AbortSignal;
  claudeCode?: ClaudeCodeSettings;
  detectClaudeCodeFn?: () => Promise<ClaudeCodeAvailability>;
  runClaudeCodeFn?: typeof runClaudeCodeSubagent;
  runMinimalSubagent?: (config: SubagentConfig) => Effect.Effect<SubagentResult, Error, R>;
}

const shouldEnableClaudeCode = (settings?: ClaudeCodeSettings): boolean =>
  settings?.enabled !== false;

export const shouldUseClaudeCode = (subtask: Subtask, settings?: ClaudeCodeSettings): boolean => {
  if (!shouldEnableClaudeCode(settings)) return false;

  const preferComplex = settings?.preferForComplexTasks ?? true;
  if (!preferComplex) return true;

  const description = subtask.description.toLowerCase();
  if (description.length > 300) return true;
  return COMPLEX_KEYWORDS.some((keyword) => description.includes(keyword));
};

const defaultFallbackEnabled = (settings?: ClaudeCodeSettings): boolean =>
  settings?.fallbackToMinimal ?? true;

/**
 * Route a subtask to the best available subagent.
 * Prefers Claude Code when enabled and available, falling back to the minimal subagent on failure.
 */
export const runBestAvailableSubagent = <R = OpenRouterClient>(
  options: RunBestAvailableSubagentOptions<R>
): Effect.Effect<SubagentResult, Error, R> =>
  Effect.gen(function* () {
    const { subtask, claudeCode } = options;
    const tryClaude = shouldUseClaudeCode(subtask, claudeCode);
    const maxTurns = claudeCode?.maxTurnsPerSubtask ?? 30;

    if (tryClaude) {
      const detectFn = options.detectClaudeCodeFn ?? detectClaudeCode;
      const availability = yield* Effect.tryPromise({
        try: () => detectFn(),
        catch: (error) => error as Error,
      }).pipe(
        Effect.catchAll(() => Effect.succeed<ClaudeCodeAvailability>({ available: false }))
      );

      if (availability.available) {
        const claudeRunner = options.runClaudeCodeFn ?? runClaudeCodeSubagent;
        const ccResult = yield* Effect.tryPromise({
          try: () =>
            claudeRunner(subtask, {
              cwd: options.cwd,
              openagentsDir: options.openagentsDir ?? options.cwd,
              maxTurns,
            }),
          catch: (error: any) => error as Error,
        }).pipe(
          Effect.catchAll((error: any) =>
            Effect.succeed<SubagentResult>({
              success: false,
              subtaskId: subtask.id,
              filesModified: [],
              turns: 0,
              error: error?.message || String(error),
            })
          )
        );

        if (ccResult.success || !defaultFallbackEnabled(claudeCode)) {
          return ccResult;
        }
      }
    }

    const minimalRunner = options.runMinimalSubagent ?? runSubagent;
    const configOptions =
      options.model || options.signal
        ? {
            ...(options.model ? { model: options.model } : {}),
            ...(options.signal ? { signal: options.signal } : {}),
            maxTurns,
          }
        : { maxTurns };

    return yield* minimalRunner(
      createSubagentConfig(subtask, options.cwd, options.tools, configOptions)
    );
  });
