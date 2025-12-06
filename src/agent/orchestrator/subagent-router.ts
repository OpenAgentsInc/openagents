import { Effect } from "effect";
import type { Tool } from "../../tools/schema.js";
import type { OpenRouterClient } from "../../llm/openrouter.js";
import type { HookCallback } from "@anthropic-ai/claude-agent-sdk";
import { detectClaudeCode, type ClaudeCodeAvailability } from "./claude-code-detector.js";
import { runClaudeCodeSubagent } from "./claude-code-subagent.js";
import { runSubagent, createSubagentConfig } from "./subagent.js";
import { checkFMHealth, isMacOS, createFMClient } from "../../fm/index.js";
import type {
  SubagentResult,
  Subtask,
  ClaudeCodeSettings,
  SubagentConfig,
} from "./types.js";

// ============================================================================
// FM (Foundation Models) Settings & Types
// ============================================================================

export interface FMSettings {
  enabled?: boolean;
  /** Port for FM bridge (default: 11435) */
  port?: number;
  /** Enable Voyager-style skill injection (default: true) */
  useSkills?: boolean;
  /** Enable Generative Agents-style memory injection (default: false) */
  useMemory?: boolean;
  /** Enable Reflexion pattern (default: false) */
  useReflection?: boolean;
  /** Max reflection-based retries (default: 2) */
  maxReflectionRetries?: number;
  /** Project root for loading skills/memories (default: cwd) */
  projectRoot?: string;
  /** Max skills to inject (default: 5) */
  maxSkills?: number;
  /** Max memories to inject (default: 5) */
  maxMemories?: number;
  /** Minimum similarity for skill/memory matching (default: 0.3) */
  minSimilarity?: number;
}

export interface FMAvailability {
  available: boolean;
  error?: string;
}

/**
 * Detect if FM (Apple Foundation Models) is available.
 * Checks: macOS platform + bridge health.
 */
export const detectFMAvailability = async (port = 11435): Promise<FMAvailability> => {
  // Platform check
  if (!isMacOS()) {
    return { available: false, error: "FM requires macOS" };
  }

  // Bridge health check
  try {
    const result = await Effect.runPromise(
      checkFMHealth(port).pipe(
        Effect.catchAll((e) =>
          Effect.succeed({
            available: false,
            serverRunning: false,
            modelAvailable: false,
            error: e instanceof Error ? e.message : String(e),
          }),
        ),
      ),
    );

    if (!result.available || !result.serverRunning) {
      return {
        available: false,
        error: result.error ?? "FM bridge not running",
      };
    }

    return { available: true };
  } catch (e) {
    return {
      available: false,
      error: e instanceof Error ? e.message : String(e),
    };
  }
};

/**
 * Run a subtask using Apple Foundation Models.
 * Uses a simplified tool execution loop similar to the TB model adapter.
 */
export const runFMSubagent = async (
  subtask: Subtask,
  options: {
    cwd: string;
    settings?: FMSettings;
    maxTurns?: number;
    signal?: AbortSignal;
    onOutput?: (text: string) => void;
  },
): Promise<SubagentResult> => {
  const port = options.settings?.port ?? 11435;
  const client = createFMClient({ port, autoStart: true });
  const maxTurns = options.maxTurns ?? 300;
  const startTime = Date.now();
  let turns = 0;

  const log = (text: string): void => {
    options.onOutput?.(text + "\n");
  };

  log(`[FM] Starting subtask: ${subtask.description.slice(0, 100)}...`);

  // Build system prompt
  const systemPrompt = `You are an expert coding assistant. Complete the subtask below.

Tools available:
- read_file(path): Read a file
- write_file(path, content): Write a file
- edit_file(path, old_text, new_text): Edit a file
- run_command(command): Run a shell command

To use a tool, output:
<tool_call>{"name": "tool_name", "arguments": {"arg1": "value1"}}</tool_call>

When you have completed the subtask, output: SUBTASK_COMPLETE`;

  const userPrompt = `## Subtask

${subtask.description}

Working directory: ${options.cwd}

Complete this subtask. When finished, output SUBTASK_COMPLETE on its own line.`;

  let messages: Array<{ role: "system" | "user" | "assistant"; content: string }> = [
    { role: "system", content: systemPrompt },
    { role: "user", content: userPrompt },
  ];

  const filesModified: Set<string> = new Set();

  // Tool execution helper
  const executeTool = async (
    name: string,
    args: Record<string, unknown>,
  ): Promise<{ success: boolean; output: string }> => {
    try {
      const { resolve, dirname } = await import("path");
      const { readFileSync, writeFileSync, existsSync, mkdirSync } = await import("fs");

      switch (name) {
        case "read_file": {
          const path = resolve(options.cwd, args.path as string);
          if (!existsSync(path)) {
            return { success: false, output: `File not found: ${path}` };
          }
          const content = readFileSync(path, "utf-8");
          return { success: true, output: content };
        }

        case "write_file": {
          const path = resolve(options.cwd, args.path as string);
          const dir = dirname(path);
          if (!existsSync(dir)) {
            mkdirSync(dir, { recursive: true });
          }
          writeFileSync(path, args.content as string);
          filesModified.add(path);
          return { success: true, output: `Wrote ${(args.content as string).length} bytes to ${path}` };
        }

        case "edit_file": {
          const path = resolve(options.cwd, args.path as string);
          if (!existsSync(path)) {
            return { success: false, output: `File not found: ${path}` };
          }
          let content = readFileSync(path, "utf-8");
          const oldText = args.old_text as string;
          if (!content.includes(oldText)) {
            return { success: false, output: `Text not found in file: ${oldText.slice(0, 50)}...` };
          }
          content = content.replace(oldText, args.new_text as string);
          writeFileSync(path, content);
          filesModified.add(path);
          return { success: true, output: `Edited ${path}` };
        }

        case "run_command": {
          const proc = Bun.spawn(["sh", "-c", args.command as string], {
            cwd: options.cwd,
            stdout: "pipe",
            stderr: "pipe",
          });
          const exitCode = await proc.exited;
          const stdout = await new Response(proc.stdout).text();
          const stderr = await new Response(proc.stderr).text();
          const output = stdout + (stderr ? `\nSTDERR:\n${stderr}` : "");
          return {
            success: exitCode === 0,
            output: `Exit code: ${exitCode}\n${output}`,
          };
        }

        default:
          return { success: false, output: `Unknown tool: ${name}` };
      }
    } catch (e) {
      return {
        success: false,
        output: `Tool error: ${e instanceof Error ? e.message : String(e)}`,
      };
    }
  };

  // Parse tool calls from model output
  const parseToolCalls = (text: string): Array<{ name: string; arguments: Record<string, unknown> }> => {
    const regex = /<tool_call>(.*?)<\/tool_call>/gs;
    const calls: Array<{ name: string; arguments: Record<string, unknown> }> = [];
    let match;
    while ((match = regex.exec(text)) !== null) {
      try {
        const parsed = JSON.parse(match[1]);
        if (parsed.name && typeof parsed.name === "string") {
          calls.push({
            name: parsed.name,
            arguments: parsed.arguments ?? {},
          });
        }
      } catch {
        // Skip malformed tool calls
      }
    }
    return calls;
  };

  try {
    while (turns < maxTurns) {
      // Check abort signal
      if (options.signal?.aborted) {
        return {
          success: false,
          subtaskId: subtask.id,
          filesModified: Array.from(filesModified),
          turns,
          agent: "fm",
          error: "Aborted",
        };
      }

      turns++;
      log(`\n--- FM Turn ${turns} ---`);

      // Call Foundation Models
      const response = await Effect.runPromise(client.chat({ messages }));

      const choice = response.choices[0];
      const assistantContent = choice?.message.content ?? "";

      log(`FM: ${assistantContent.slice(0, 500)}${assistantContent.length > 500 ? "..." : ""}`);

      // Check for completion
      if (assistantContent.includes("SUBTASK_COMPLETE")) {
        log(`[FM] Subtask completed in ${turns} turns`);
        return {
          success: true,
          subtaskId: subtask.id,
          filesModified: Array.from(filesModified),
          turns,
          agent: "fm",
        };
      }

      // Parse and execute tool calls
      const toolCalls = parseToolCalls(assistantContent);

      if (toolCalls.length > 0) {
        messages.push({ role: "assistant", content: assistantContent });

        const toolResults: string[] = [];
        for (const toolCall of toolCalls) {
          log(`Tool: ${toolCall.name}(${JSON.stringify(toolCall.arguments).slice(0, 100)}...)`);
          const result = await executeTool(toolCall.name, toolCall.arguments);
          log(`Result: ${result.success ? "success" : "failure"}`);
          toolResults.push(`${toolCall.name} result: ${result.output}`);
        }

        messages.push({
          role: "user",
          content: "Tool results:\n" + toolResults.join("\n\n"),
        });
      } else {
        messages.push({ role: "assistant", content: assistantContent });

        if (!assistantContent.trim()) {
          log("[FM] Warning: Empty response");
          break;
        }

        messages.push({
          role: "user",
          content: "Please continue with the subtask. Use tools to make changes, and say SUBTASK_COMPLETE when done.",
        });
      }
    }

    // Reached max turns
    return {
      success: false,
      subtaskId: subtask.id,
      filesModified: Array.from(filesModified),
      turns,
      agent: "fm",
      error: `Reached max turns (${maxTurns})`,
    };
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    log(`[FM] Error: ${errorMsg}`);
    return {
      success: false,
      subtaskId: subtask.id,
      filesModified: Array.from(filesModified),
      turns,
      agent: "fm",
      error: errorMsg,
    };
  }
};

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
  /** FM (Apple Foundation Models) settings - enabled when available on macOS */
  fm?: FMSettings;
  verificationCommands?: string[];
  verifyFn?: VerificationFn;
  detectClaudeCodeFn?: () => Promise<ClaudeCodeAvailability>;
  runClaudeCodeFn?: typeof runClaudeCodeSubagent;
  /** FM availability detector (for testing) */
  detectFMFn?: (port?: number) => Promise<FMAvailability>;
  /** FM subagent runner (for testing) */
  runFMFn?: typeof runFMSubagent;
  runMinimalSubagent?: (config: SubagentConfig) => Effect.Effect<SubagentResult, Error, R>;
  /** Callback for streaming text output from Claude Code */
  onOutput?: (text: string) => void;
  /** Additional context (e.g., AGENTS.md content) to prepend to subagent prompts */
  additionalContext?: string;
  /** Formatted reflections from previous failures (Reflexion pattern) */
  reflections?: string;
  /** PreToolUse guard hook to enforce worktree boundaries */
  worktreeGuardHook?: HookCallback;
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
 *
 * Routing order:
 * 1. Claude Code (when enabled and appropriate)
 * 2. FM - Apple Foundation Models (when enabled and available on macOS)
 * 3. Minimal subagent (OpenRouter/Grok fallback)
 *
 * Remote-context call: Claude Code can be resumed via sessionId, unlike local-context tools.
 */
export const runBestAvailableSubagent = <R = OpenRouterClient>(
  options: RunBestAvailableSubagentOptions<R>
): Effect.Effect<SubagentResult, Error, R | OpenRouterClient> =>
  Effect.gen(function* () {
    const { subtask, claudeCode, fm } = options;
    const tryClaude = shouldUseClaudeCode(subtask, claudeCode);
    const tryFM = fm?.enabled !== false; // FM enabled by default when settings provided
    const maxTurns = claudeCode?.maxTurnsPerSubtask ?? 300;
    const timeoutMs = claudeCode?.timeoutMsPerSubtask;
    const verificationCommands = options.verificationCommands;
    const resumeSessionId = subtask.claudeCode?.sessionId;
    const forkSession = subtask.claudeCode?.resumeStrategy === "fork";

    // Log session resumption for debugging
    if (resumeSessionId) {
      console.log(`[Claude Code] Resuming session: ${resumeSessionId}`);
    }

    const runMinimal = () =>
      (options.runMinimalSubagent ?? runSubagent)(
        createSubagentConfig(subtask, options.cwd, options.tools, {
          ...(options.model ? { model: options.model } : {}),
          ...(options.signal ? { signal: options.signal } : {}),
          ...(options.onOutput ? { onOutput: options.onOutput } : {}),
          maxTurns,
        })
      );

    // Helper to run FM subagent
    const runFM = async (): Promise<SubagentResult> => {
      const fmRunner = options.runFMFn ?? runFMSubagent;
      return fmRunner(subtask, {
        cwd: options.cwd,
        settings: fm,
        maxTurns,
        signal: options.signal,
        onOutput: options.onOutput,
      });
    };

    // --- Try Claude Code first ---
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
              ...(options.worktreeGuardHook ? { worktreeGuardHook: options.worktreeGuardHook } : {}),
              ...(options.onOutput ? { onOutput: options.onOutput } : {}),
              ...(options.additionalContext ? { additionalContext: options.additionalContext } : {}),
              ...(options.reflections ? { reflections: options.reflections } : {}),
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

    // --- Try FM (Apple Foundation Models) as fallback ---
    if (tryFM && fm) {
      const detectFMFn = options.detectFMFn ?? detectFMAvailability;
      const fmAvailability = yield* Effect.tryPromise({
        try: () => detectFMFn(fm.port),
        catch: (error) => error as Error,
      }).pipe(
        Effect.catchAll(() => Effect.succeed<FMAvailability>({ available: false }))
      );

      if (fmAvailability.available) {
        console.log(`[FM] Using Apple Foundation Models for subtask`);
        const fmResult = yield* Effect.tryPromise({
          try: () => runFM(),
          catch: (error: any) => error as Error,
        }).pipe(
          Effect.catchAll((error: any) =>
            Effect.succeed<SubagentResult>({
              success: false,
              subtaskId: subtask.id,
              filesModified: [],
              turns: 0,
              agent: "fm",
              error: error?.message || String(error),
            })
          )
        );

        // If FM succeeded or we're aborted, return the result
        if (fmResult.success || options.signal?.aborted) {
          return fmResult;
        }
        // Otherwise fall through to minimal
        console.log(`[FM] Failed, falling back to minimal subagent`);
      }
    }

    // --- Fall back to minimal subagent ---
    const minimalResult = yield* runMinimal();
    return {
      ...minimalResult,
      agent: minimalResult.agent ?? "minimal",
    };
  });
