import { Effect } from "effect";
import type { Tool } from "../../tools/schema.js";
import type { OpenRouterClient } from "../../llm/openrouter.js";
import { detectClaudeCode, type ClaudeCodeAvailability } from "./claude-code-detector.js";
import { runClaudeCodeSubagent } from "./claude-code-subagent.js";
import { runSubagent, createSubagentConfig } from "./subagent.js";
import type {
  SubagentResult,
  Subtask,
  ClaudeCodeSettings,
  SubagentConfig,
} from "./types.js";

const COMPLEX_KEYWORDS = ["refactor", "multi-file", "multi file", "search", "fetch", "investigate"];

type VerificationResult = { passed: boolean; outputs: string[] };
type VerificationFn = (commands: string[], cwd: string) => Promise<VerificationResult>;

export interface RunBestAvailableSubagentOptions<R> {
  subtask: Subtask;
  cwd: string;
  openagentsDir: string;
  tools: Tool<any, any, any, any>[];
  model?: string;
  signal?: AbortSignal;
  claudeCode?: ClaudeCodeSettings;
  verificationCommands?: string[];
  verifyFn?: VerificationFn;
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

const hasVerificationCommands = (commands?: string[]): commands is string[] =>
  !!commands && commands.length > 0;

const formatVerificationError = (outputs: string[]): string => {
  const firstOutput = outputs.find((output) => output && output.trim().length > 0);
  if (!firstOutput) return "Verification failed (typecheck/tests)";

  const summary = firstOutput
    .trim()
    .split("\n")
    .slice(0, 3)
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();

  return summary.length > 0
    ? `Verification failed (typecheck/tests): ${summary}`
    : "Verification failed (typecheck/tests)";
};

const mergeFilesModified = (...fileSets: Array<string[]>): string[] => {
  const merged = new Set<string>();
  for (const files of fileSets) {
    files.forEach((file) => merged.add(file));
  }
  return Array.from(merged);
};

/**
 * Route a subtask to the best available subagent.
 * Prefers Claude Code when enabled and available, falling back to the minimal subagent on failure.
 */
export const runBestAvailableSubagent = <R = OpenRouterClient>(
  options: RunBestAvailableSubagentOptions<R>
): Effect.Effect<SubagentResult, Error, R | OpenRouterClient> =>
  Effect.gen(function* () {
    const { subtask, claudeCode } = options;
    const tryClaude = shouldUseClaudeCode(subtask, claudeCode);
    const maxTurns = claudeCode?.maxTurnsPerSubtask ?? 30;
    const timeoutMs = claudeCode?.timeoutMsPerSubtask;
    const verificationCommands = options.verificationCommands;
    const resumeSessionId = subtask.claudeCode?.sessionId;
    const forkSession = subtask.claudeCode?.resumeStrategy === "fork";

    const runMinimal = () =>
      (options.runMinimalSubagent ?? runSubagent)(
        createSubagentConfig(subtask, options.cwd, options.tools, {
          ...(options.model ? { model: options.model } : {}),
          ...(options.signal ? { signal: options.signal } : {}),
          maxTurns,
        })
      );

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
        const ccRawResult = yield* Effect.tryPromise({
          try: () =>
            claudeRunner(subtask, {
              cwd: options.cwd,
              openagentsDir: options.openagentsDir ?? options.cwd,
              maxTurns,
              ...(timeoutMs ? { timeoutMs } : {}),
              ...(options.signal ? { signal: options.signal } : {}),
              ...(claudeCode?.permissionMode ? { permissionMode: claudeCode.permissionMode } : {}),
              ...(resumeSessionId ? { resumeSessionId } : {}),
              ...(forkSession ? { forkSession } : {}),
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

        const ccResult: SubagentResult = {
          ...ccRawResult,
          agent: ccRawResult.agent ?? "claude-code",
        };

        const shouldVerifyClaude =
          ccResult.success && options.verifyFn && hasVerificationCommands(verificationCommands);

        if (shouldVerifyClaude) {
          const verification = yield* Effect.tryPromise({
            try: () => options.verifyFn!(verificationCommands!, options.cwd),
            catch: (error: any) => error as Error,
          }).pipe(
            Effect.catchAll(() =>
              Effect.succeed<VerificationResult>({ passed: false, outputs: [] })
            )
          );

          if (!verification.passed) {
            if (defaultFallbackEnabled(claudeCode)) {
              const minimalResult = yield* runMinimal();
              const mergedFiles = mergeFilesModified(
                ccResult.filesModified,
                minimalResult.filesModified
              );

              if (!minimalResult.success) {
                return {
                  ...minimalResult,
                  agent: minimalResult.agent ?? "minimal",
                  filesModified: mergedFiles,
                };
              }

              if (options.verifyFn && hasVerificationCommands(verificationCommands)) {
                const fallbackVerification = yield* Effect.tryPromise({
                  try: () => options.verifyFn!(verificationCommands!, options.cwd),
                  catch: (error: any) => error as Error,
                }).pipe(
                  Effect.catchAll(() =>
                    Effect.succeed<VerificationResult>({ passed: false, outputs: [] })
                  )
                );

                if (!fallbackVerification.passed) {
                  return {
                    success: false,
                    subtaskId: subtask.id,
                    filesModified: mergedFiles,
                    turns: minimalResult.turns,
                    agent: minimalResult.agent ?? "minimal",
                    error: formatVerificationError(fallbackVerification.outputs),
                    verificationOutputs: fallbackVerification.outputs,
                  };
                }
              }

              return {
                ...minimalResult,
                agent: minimalResult.agent ?? "minimal",
                filesModified: mergedFiles,
              };
            }

            return {
              ...ccResult,
              success: false,
              error: formatVerificationError(verification.outputs),
              verificationOutputs: verification.outputs,
            };
          }

          return {
            ...ccResult,
            verificationOutputs: verification.outputs,
          };
        }

        if (ccResult.success || !defaultFallbackEnabled(claudeCode) || options.signal?.aborted) {
          return ccResult;
        }
      }
    }

    const minimalResult = yield* runMinimal();
    return {
      ...minimalResult,
      agent: minimalResult.agent ?? "minimal",
    };
  });
