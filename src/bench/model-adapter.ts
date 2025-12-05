/**
 * Model adapter for TerminalBench execution.
 *
 * Provides a unified interface for running benchmark tasks with either:
 * - Claude Code (via SDK subagent)
 * - Ollama (via local inference with tool calling)
 *
 * Usage:
 *   const runner = createModelRunner({ type: "claude-code" });
 *   const result = await runner.runTask(task, { workspace, timeout });
 *
 *   const runner = createModelRunner({ type: "ollama", model: "codellama:34b" });
 *   const result = await runner.runTask(task, { workspace, timeout });
 */

import { runClaudeCodeSubagent } from "../agent/orchestrator/claude-code-subagent.js";
import type { Subtask } from "../agent/orchestrator/types.js";
import { createOllamaClient, checkOllamaHealth, type OllamaConfig, type OllamaError } from "../llm/ollama.js";
import type { TerminalBenchTask } from "./terminal-bench.js";
import { Effect } from "effect";

// --- Configuration Types ---

export interface ClaudeCodeModelConfig {
  type: "claude-code";
}

export interface OllamaModelConfig {
  type: "ollama";
  /** Model name (e.g., "codellama:34b", "deepseek-coder:33b") */
  model: string;
  /** Ollama endpoint (default: http://localhost:11434) */
  endpoint?: string;
}

export type ModelConfig = ClaudeCodeModelConfig | OllamaModelConfig;

// --- Result Types ---

export interface TaskRunResult {
  success: boolean;
  turns: number;
  tokens: number;
  durationMs: number;
  output: string;
  error?: string;
  model: string;
  sessionMetadata?: {
    toolsUsed?: Record<string, number>;
    usage?: {
      inputTokens?: number;
      outputTokens?: number;
    };
  };
}

export interface RunTaskOptions {
  /** Workspace directory for task execution */
  workspace: string;
  /** Task timeout in seconds */
  timeout: number;
  /** Max agent turns */
  maxTurns: number;
  /** Callback for output streaming */
  onOutput?: (text: string) => void;
  /** Run ID for HUD/ATIF integration */
  runId?: string | undefined;
  /** Callback fired when sessionId becomes available (for ATIF disk persistence) */
  onSessionId?: (sessionId: string) => void;
}

// --- Model Runner Interface ---

export interface ModelRunner {
  readonly config: ModelConfig;
  readonly modelName: string;
  runTask: (task: TerminalBenchTask, options: RunTaskOptions) => Promise<TaskRunResult>;
  checkHealth: () => Promise<{ available: boolean; error?: string }>;
}

// --- Claude Code Runner ---

const createClaudeCodeRunner = (): ModelRunner => {
  const config: ClaudeCodeModelConfig = { type: "claude-code" };

  return {
    config,
    modelName: "claude-code",

    async runTask(task: TerminalBenchTask, options: RunTaskOptions): Promise<TaskRunResult> {
      const startTime = Date.now();
      let outputText = "";

      const onOutput = (text: string): void => {
        outputText += text;
        options.onOutput?.(text);
      };

      // Create subtask for Claude Code
      const subtask: Subtask = {
        id: task.id,
        description: task.description,
        status: "in_progress",
        startedAt: new Date().toISOString(),
      };

      try {
        const result = await runClaudeCodeSubagent(subtask, {
          cwd: options.workspace,
          maxTurns: task.max_turns ?? options.maxTurns,
          permissionMode: "bypassPermissions",
          timeoutMs: (task.timeout_seconds ?? options.timeout) * 1000,
          ...(options.runId ? { runId: options.runId } : {}),
          ...(options.onSessionId ? { onSessionId: options.onSessionId } : {}),
          onOutput,
        });

        const durationMs = Date.now() - startTime;
        const usage = result.sessionMetadata?.usage;
        const tokens = (usage?.inputTokens ?? 0) + (usage?.outputTokens ?? 0);

        return {
          success: result.success,
          turns: result.turns,
          tokens,
          durationMs,
          output: outputText,
          error: result.error,
          model: "claude-code",
          sessionMetadata: result.sessionMetadata,
        };
      } catch (e) {
        const durationMs = Date.now() - startTime;
        const errorMsg = e instanceof Error ? e.message : String(e);
        return {
          success: false,
          turns: 0,
          tokens: 0,
          durationMs,
          output: outputText,
          error: errorMsg,
          model: "claude-code",
        };
      }
    },

    async checkHealth(): Promise<{ available: boolean; error?: string }> {
      // Claude Code availability is checked by the SDK
      // For now, assume it's available if we can import the module
      try {
        // Simple check - try to detect Claude Code CLI
        const proc = Bun.spawn(["which", "claude"], {
          stdout: "pipe",
          stderr: "pipe",
        });
        const exitCode = await proc.exited;
        return { available: exitCode === 0 };
      } catch {
        return { available: false, error: "Claude Code CLI not found" };
      }
    },
  };
};

// --- Ollama Runner ---

/**
 * System prompt for Ollama-based coding agents.
 * Kept minimal following the "pi-mono" insight that RL-trained models don't need extensive instructions.
 */
const OLLAMA_SYSTEM_PROMPT = `You are a coding assistant. Complete the task by writing code and using tools.

Available tools:
- read_file(path): Read a file's contents
- write_file(path, content): Write content to a file
- edit_file(path, old_text, new_text): Replace text in a file
- run_command(command): Execute a shell command

When done, say "TASK_COMPLETE" to indicate you've finished.`;

const createOllamaRunner = (ollamaConfig: OllamaModelConfig): ModelRunner => {
  const endpoint = ollamaConfig.endpoint ?? "http://localhost:11434";
  const client = createOllamaClient({
    endpoint,
    model: ollamaConfig.model,
  });

  // Define tools for the Ollama agent
  const tools = [
    {
      name: "read_file",
      description: "Read the contents of a file",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string", description: "Path to the file to read" },
        },
        required: ["path"],
      },
    },
    {
      name: "write_file",
      description: "Write content to a file",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string", description: "Path to the file to write" },
          content: { type: "string", description: "Content to write to the file" },
        },
        required: ["path", "content"],
      },
    },
    {
      name: "edit_file",
      description: "Replace text in a file",
      parameters: {
        type: "object",
        properties: {
          path: { type: "string", description: "Path to the file to edit" },
          old_text: { type: "string", description: "Text to find and replace" },
          new_text: { type: "string", description: "Replacement text" },
        },
        required: ["path", "old_text", "new_text"],
      },
    },
    {
      name: "run_command",
      description: "Execute a shell command",
      parameters: {
        type: "object",
        properties: {
          command: { type: "string", description: "Shell command to execute" },
        },
        required: ["command"],
      },
    },
  ];

  // Execute a tool call
  const executeTool = async (
    name: string,
    args: Record<string, unknown>,
    workspace: string,
  ): Promise<{ success: boolean; output: string }> => {
    try {
      const { join, resolve } = await import("path");
      const { readFileSync, writeFileSync, existsSync, mkdirSync } = await import("fs");
      const { dirname } = await import("path");

      switch (name) {
        case "read_file": {
          const path = resolve(workspace, args.path as string);
          if (!existsSync(path)) {
            return { success: false, output: `File not found: ${path}` };
          }
          const content = readFileSync(path, "utf-8");
          return { success: true, output: content };
        }

        case "write_file": {
          const path = resolve(workspace, args.path as string);
          const dir = dirname(path);
          if (!existsSync(dir)) {
            mkdirSync(dir, { recursive: true });
          }
          writeFileSync(path, args.content as string);
          return { success: true, output: `Wrote ${(args.content as string).length} bytes to ${path}` };
        }

        case "edit_file": {
          const path = resolve(workspace, args.path as string);
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
          return { success: true, output: `Edited ${path}` };
        }

        case "run_command": {
          const proc = Bun.spawn(["sh", "-c", args.command as string], {
            cwd: workspace,
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

  return {
    config: ollamaConfig,
    modelName: ollamaConfig.model,

    async runTask(task: TerminalBenchTask, options: RunTaskOptions): Promise<TaskRunResult> {
      const startTime = Date.now();
      let outputText = "";
      let totalTokens = 0;
      let turns = 0;

      const log = (text: string): void => {
        outputText += text + "\n";
        options.onOutput?.(text + "\n");
      };

      // Build initial messages
      const messages: Array<{ role: "system" | "user" | "assistant" | "tool"; content: string; tool_call_id?: string }> = [
        { role: "system", content: OLLAMA_SYSTEM_PROMPT },
        { role: "user", content: task.description },
      ];

      const maxTurns = task.max_turns ?? options.maxTurns;
      const timeoutMs = (task.timeout_seconds ?? options.timeout) * 1000;

      try {
        while (turns < maxTurns) {
          // Check timeout
          if (Date.now() - startTime > timeoutMs) {
            return {
              success: false,
              turns,
              tokens: totalTokens,
              durationMs: Date.now() - startTime,
              output: outputText,
              error: "Task timed out",
              model: ollamaConfig.model,
            };
          }

          turns++;
          log(`\n--- Turn ${turns} ---`);

          // Call Ollama
          const response = await Effect.runPromise(
            client.chat({
              messages,
              tools: tools as any,
              toolChoice: "auto",
            }),
          );

          // Track tokens
          if (response.usage) {
            totalTokens += response.usage.total_tokens ?? 0;
          }

          const choice = response.choices[0];
          const assistantContent = choice?.message.content;
          const toolCalls = choice?.message.tool_calls;

          // Log assistant response
          if (assistantContent) {
            log(`Assistant: ${assistantContent}`);
            messages.push({ role: "assistant", content: assistantContent });

            // Check for completion
            if (assistantContent.includes("TASK_COMPLETE")) {
              return {
                success: true,
                turns,
                tokens: totalTokens,
                durationMs: Date.now() - startTime,
                output: outputText,
                model: ollamaConfig.model,
              };
            }
          }

          // Handle tool calls
          if (toolCalls?.length) {
            // Add assistant message with tool calls
            messages.push({
              role: "assistant",
              content: assistantContent ?? "",
            });

            for (const toolCall of toolCalls) {
              log(`Tool call: ${toolCall.name}(${toolCall.arguments})`);

              let args: Record<string, unknown>;
              try {
                args = JSON.parse(toolCall.arguments);
              } catch {
                args = {};
              }

              const result = await executeTool(toolCall.name, args, options.workspace);
              log(`Tool result: ${result.success ? "success" : "failure"}`);
              if (result.output.length < 500) {
                log(result.output);
              } else {
                log(result.output.slice(0, 500) + "...[truncated]");
              }

              messages.push({
                role: "tool",
                content: result.output,
                tool_call_id: toolCall.id,
              });
            }
          } else if (!assistantContent) {
            // No content and no tool calls - something went wrong
            log("Warning: Empty response from model");
            break;
          }
        }

        // Reached max turns
        return {
          success: false,
          turns,
          tokens: totalTokens,
          durationMs: Date.now() - startTime,
          output: outputText,
          error: `Reached max turns (${maxTurns})`,
          model: ollamaConfig.model,
        };
      } catch (e) {
        const durationMs = Date.now() - startTime;
        const errorMsg = e instanceof Error ? e.message : String(e);
        log(`Error: ${errorMsg}`);
        return {
          success: false,
          turns,
          tokens: totalTokens,
          durationMs,
          output: outputText,
          error: errorMsg,
          model: ollamaConfig.model,
        };
      }
    },

    async checkHealth(): Promise<{ available: boolean; error?: string }> {
      try {
        const result = await Effect.runPromise(checkOllamaHealth(endpoint));
        if (!result.available) {
          return { available: false, error: "Ollama is not running" };
        }
        // Check if the specific model is available
        const modelAvailable = result.models.some(m =>
          m === ollamaConfig.model || m.startsWith(ollamaConfig.model.split(":")[0] + ":"),
        );
        if (!modelAvailable) {
          return {
            available: false,
            error: `Model ${ollamaConfig.model} not found. Available: ${result.models.join(", ")}`,
          };
        }
        return { available: true };
      } catch (e) {
        return {
          available: false,
          error: e instanceof Error ? e.message : String(e),
        };
      }
    },
  };
};

// --- Factory Function ---

/**
 * Create a model runner from configuration.
 *
 * @example
 * // Claude Code
 * const runner = createModelRunner({ type: "claude-code" });
 *
 * // Ollama with specific model
 * const runner = createModelRunner({ type: "ollama", model: "codellama:34b" });
 *
 * // Ollama with custom endpoint
 * const runner = createModelRunner({
 *   type: "ollama",
 *   model: "deepseek-coder:33b",
 *   endpoint: "http://gpu-server:11434"
 * });
 */
export const createModelRunner = (config: ModelConfig): ModelRunner => {
  switch (config.type) {
    case "claude-code":
      return createClaudeCodeRunner();
    case "ollama":
      return createOllamaRunner(config);
    default:
      throw new Error(`Unknown model type: ${(config as any).type}`);
  }
};

/**
 * Parse a model string into a ModelConfig.
 *
 * @example
 * parseModelString("claude-code") // { type: "claude-code" }
 * parseModelString("ollama:codellama:34b") // { type: "ollama", model: "codellama:34b" }
 */
export const parseModelString = (modelStr: string): ModelConfig => {
  if (modelStr === "claude-code" || modelStr === "claude") {
    return { type: "claude-code" };
  }

  if (modelStr.startsWith("ollama:")) {
    const model = modelStr.slice("ollama:".length);
    return { type: "ollama", model };
  }

  // Default to Claude Code for backwards compatibility
  return { type: "claude-code" };
};
