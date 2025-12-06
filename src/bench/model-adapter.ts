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
import { createOllamaClient, checkOllamaHealth } from "../llm/ollama.js";
import { createFMClient, checkFMHealth, isMacOS } from "../llm/foundation-models.js";
import type { TerminalBenchTask } from "./terminal-bench.js";
import { Effect } from "effect";
import { SkillService, makeSkillServiceLive, type Skill } from "../skills/index.js";
import { MemoryService, makeMemoryServiceLive } from "../memory/service.js";
import { ReflexionService, makeReflexionServiceLive } from "../reflexion/service.js";

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

export interface FMModelConfig {
  type: "foundation-models";
  /** Server port (default: 11435) */
  port?: number;
  /** Enable skill injection for Voyager-style learning (default: true) */
  useSkills?: boolean;
  /** Project root for skill library (default: process.cwd()) */
  projectRoot?: string;
  /** Max skills to inject into prompt (default: 5) */
  maxSkills?: number;
  /** Min similarity threshold for skill retrieval (default: 0.3) */
  minSimilarity?: number;
  /** Enable memory retrieval and injection (default: false) */
  useMemory?: boolean;
  /** Max memories to inject into prompt (default: 5) */
  maxMemories?: number;
  /** Enable reflexion on failures (default: false) */
  useReflection?: boolean;
  /** Max reflection-based retries per task (default: 2) */
  maxReflectionRetries?: number;
}

export type ModelConfig = ClaudeCodeModelConfig | OllamaModelConfig | FMModelConfig;

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
    /** Skills injected into the system prompt (Voyager-style) */
    skillsUsed?: string[];
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
      const { resolve, dirname } = await import("path");
      const { readFileSync, writeFileSync, existsSync, mkdirSync } = await import("fs");

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

// --- Foundation Models Runner ---

/**
 * Apple FM context limits (determined empirically):
 * - Max per-request: ~1373 chars = ~347 tokens
 * - Safe limit: ~1100 chars = ~280 tokens
 * - Bridge can corrupt after repeated errors - needs restart
 */
const FM_MAX_CONTEXT_CHARS = 1100; // Safe limit with room for response
const FM_CONTEXT_EXCEEDED_ERROR = "Exceeded model context window size";

/**
 * System prompt for Foundation Models-based coding agents.
 * Uses text-based tool calling since FM may not support native JSON tool calling.
 * Keep this SHORT - Apple FM has limited context window.
 */
const FM_BASE_PROMPT = `You must use tools. Output ONLY a tool_call tag.

Example: To create hello.txt with "Hi":
<tool_call>{"name":"write_file","arguments":{"path":"hello.txt","content":"Hi"}}</tool_call>

Available: write_file, read_file, run_command

After the tool runs, say TASK_COMPLETE.`;

/**
 * Truncate messages to fit within FM context limit.
 * Strategy: Keep system prompt, trim middle history, keep last user message.
 */
const truncateMessagesForFM = (
  messages: Array<{ role: "system" | "user" | "assistant"; content: string }>,
  maxChars: number = FM_MAX_CONTEXT_CHARS,
): Array<{ role: "system" | "user" | "assistant"; content: string }> => {
  // Calculate total chars
  const totalChars = messages.reduce((sum, m) => sum + m.content.length + 20, 0); // +20 for role overhead

  if (totalChars <= maxChars) {
    return messages;
  }

  // Need to truncate
  // Strategy: Keep first (system) and last (user) messages, truncate middle
  const result: typeof messages = [];
  let usedChars = 0;
  const reserveForLast = 200; // Reserve space for last user message

  // Always keep system message (but truncate if too long)
  if (messages.length > 0 && messages[0].role === "system") {
    const systemMsg = messages[0];
    const maxSystemChars = Math.min(systemMsg.content.length, maxChars - reserveForLast - 100);
    result.push({
      role: "system",
      content: systemMsg.content.slice(0, maxSystemChars),
    });
    usedChars += maxSystemChars + 20;
  }

  // Always keep last message
  if (messages.length > 1) {
    const lastMsg = messages[messages.length - 1];
    const maxLastChars = Math.min(lastMsg.content.length, maxChars - usedChars - 50);
    // Truncate content if needed
    const truncatedContent = lastMsg.content.length > maxLastChars
      ? lastMsg.content.slice(0, maxLastChars) + "...[truncated]"
      : lastMsg.content;
    result.push({
      role: lastMsg.role,
      content: truncatedContent,
    });
  }

  return result;
};

/**
 * Build system prompt with injected skills and memories (Voyager + Generative Agents style).
 * Skills and memories are retrieved based on semantic similarity to the task.
 */
const buildFMSystemPrompt = (options?: {
  skills?: Skill[];
  memories?: string;
  reflections?: string;
}): string => {
  const { skills, memories, reflections } = options ?? {};
  let prompt = FM_BASE_PROMPT;

  // Add skills section
  if (skills && skills.length > 0) {
    const skillsSection = skills.map(skill => {
      const params = skill.parameters
        .map(p => `${p.name}: ${p.type}${p.required ? " (required)" : ""}`)
        .join(", ");
      const successInfo = skill.successRate !== undefined
        ? ` [${(skill.successRate * 100).toFixed(0)}% success rate]`
        : "";
      return `### ${skill.name}${successInfo}
${skill.description}
${params ? `Parameters: ${params}` : ""}
\`\`\`typescript
${skill.code}
\`\`\``;
    }).join("\n\n");

    prompt += `

## Relevant Skills (use these patterns when applicable)

The following code patterns have been proven effective for similar tasks.
Adapt them to your specific situation:

${skillsSection}

Remember: These are reference patterns, not exact solutions. Modify as needed.`;
  }

  // Add memories section
  if (memories && memories.trim()) {
    prompt += `

## Relevant Memories

Previous experiences that may be helpful:

${memories}`;
  }

  // Add reflections section (from previous failures)
  if (reflections && reflections.trim()) {
    prompt += `

## Lessons from Previous Attempts

${reflections}`;
  }

  return prompt;
};

/**
 * Parse tool calls from model output.
 * Handles multiple formats:
 * - <tool_call>{"name":"...", "arguments":{...}}</tool_call>
 * - ```json {"tool_call":{"name":"...", "arguments":{...}}} ```
 * - ```json {"name":"...", "arguments":{...}} ```
 * - "Using write_file tool with arguments: path=hello.txt, content=Hello World"
 * - {"response":"Using write_file tool with arguments: path=X, content=Y"}
 */
const parseToolCalls = (text: string): Array<{ name: string; arguments: Record<string, unknown> }> => {
  const calls: Array<{ name: string; arguments: Record<string, unknown> }> = [];

  // Try <tool_call>...</tool_call> format
  const tagRegex = /<tool_call>(.*?)<\/tool_call>/gs;
  let match;
  while ((match = tagRegex.exec(text)) !== null) {
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

  // Try ```json ... ``` format (FM sometimes uses this)
  if (calls.length === 0) {
    const codeBlockRegex = /```(?:json)?\s*(\{[\s\S]*?\})\s*```/g;
    while ((match = codeBlockRegex.exec(text)) !== null) {
      try {
        const parsed = JSON.parse(match[1]);
        // Handle {"tool_call": {...}} wrapper
        const toolData = parsed.tool_call ?? parsed;
        if (toolData.name && typeof toolData.name === "string") {
          calls.push({
            name: toolData.name,
            arguments: toolData.arguments ?? {},
          });
        }
        // Handle {"response": "Using write_file tool with arguments: ..."} format
        if (parsed.response && typeof parsed.response === "string") {
          const descriptiveCall = parseDescriptiveToolCall(parsed.response);
          if (descriptiveCall) {
            calls.push(descriptiveCall);
          }
        }
      } catch {
        // Skip malformed tool calls
      }
    }
  }

  // Try descriptive format as last resort: "Using {tool} tool with arguments: ..."
  if (calls.length === 0) {
    const descriptiveCall = parseDescriptiveToolCall(text);
    if (descriptiveCall) {
      calls.push(descriptiveCall);
    }
  }

  return calls;
};

/**
 * Parse descriptive tool call format from FM.
 * e.g., "Using write_file tool with arguments: path=hello.txt, content=Hello, world!"
 */
const parseDescriptiveToolCall = (text: string): { name: string; arguments: Record<string, unknown> } | null => {
  // Match "Using {tool_name} tool with arguments: {args}"
  const descriptiveRegex = /Using\s+(\w+)\s+tool\s+with\s+arguments?:\s*(.+)/i;
  const match = descriptiveRegex.exec(text);
  if (!match) return null;

  const toolName = match[1];
  const argsStr = match[2];

  // Parse arguments: "path=hello.txt, content=Hello, world!"
  // Handle both comma-separated and key=value format
  const args: Record<string, unknown> = {};

  // For write_file, expect path= and content=
  if (toolName === "write_file") {
    const pathMatch = argsStr.match(/path\s*=\s*([^,]+?)(?:,|$)/);
    const contentMatch = argsStr.match(/content\s*=\s*(.+)/);
    if (pathMatch) {
      args.path = pathMatch[1].trim();
    }
    if (contentMatch) {
      args.content = contentMatch[1].trim();
    }
    if (args.path) {
      return { name: toolName, arguments: args };
    }
  }

  // For read_file, expect path=
  if (toolName === "read_file") {
    const pathMatch = argsStr.match(/path\s*=\s*(.+)/);
    if (pathMatch) {
      args.path = pathMatch[1].trim();
      return { name: toolName, arguments: args };
    }
  }

  // For run_command, expect command=
  if (toolName === "run_command") {
    const commandMatch = argsStr.match(/command\s*=\s*(.+)/);
    if (commandMatch) {
      args.command = commandMatch[1].trim();
      return { name: toolName, arguments: args };
    }
  }

  // Generic fallback: try to parse key=value pairs
  const pairRegex = /(\w+)\s*=\s*([^,]+)(?:,|$)/g;
  let pairMatch;
  while ((pairMatch = pairRegex.exec(argsStr)) !== null) {
    args[pairMatch[1]] = pairMatch[2].trim();
  }

  if (Object.keys(args).length > 0) {
    return { name: toolName, arguments: args };
  }

  return null;
};

const createFMRunner = (fmConfig: FMModelConfig): ModelRunner => {
  const port = fmConfig.port ?? 11435;
  const client = createFMClient({ port, autoStart: true });
  const useSkills = fmConfig.useSkills ?? true;
  const useMemory = fmConfig.useMemory ?? false;
  const useReflection = fmConfig.useReflection ?? false;
  const maxReflectionRetries = fmConfig.maxReflectionRetries ?? 2;
  const projectRoot = fmConfig.projectRoot ?? process.cwd();
  const maxSkills = fmConfig.maxSkills ?? 5;
  const maxMemories = fmConfig.maxMemories ?? 5;
  const minSimilarity = fmConfig.minSimilarity ?? 0.3;

  // Create service layers for this project
  const skillLayer = makeSkillServiceLive(projectRoot);
  const memoryLayer = useMemory ? makeMemoryServiceLive(projectRoot) : null;
  const reflexionLayer = useReflection ? makeReflexionServiceLive(projectRoot) : null;

  // Skill retrieval helper (returns empty array on failure)
  const getRelevantSkills = async (taskDescription: string): Promise<{ skills: Skill[]; ids: string[] }> => {
    if (!useSkills) {
      return { skills: [], ids: [] };
    }

    try {
      const program = Effect.gen(function* () {
        const service = yield* SkillService;
        const skills = yield* service.selectSkills(taskDescription, {
          topK: maxSkills,
          minSimilarity,
        });
        return skills;
      });

      const skills = await Effect.runPromise(
        program.pipe(
          Effect.provide(skillLayer),
          Effect.catchAll(() => Effect.succeed([] as Skill[])),
        ),
      );

      return {
        skills,
        ids: skills.map(s => s.id),
      };
    } catch {
      // Skill retrieval failed - continue without skills
      return { skills: [], ids: [] };
    }
  };

  // Record skill usage after task completion
  const recordSkillUsage = async (skillIds: string[], success: boolean): Promise<void> => {
    if (!useSkills || skillIds.length === 0) {
      return;
    }

    try {
      const program = Effect.gen(function* () {
        const service = yield* SkillService;
        for (const id of skillIds) {
          yield* service.recordUsage(id, success);
        }
      });

      await Effect.runPromise(
        program.pipe(
          Effect.provide(skillLayer),
          Effect.catchAll(() => Effect.succeed(undefined)),
        ),
      );
    } catch {
      // Ignore skill usage tracking errors
    }
  };

  // Memory retrieval helper (returns empty string on failure)
  const getRelevantMemories = async (taskDescription: string): Promise<string> => {
    if (!useMemory || !memoryLayer) {
      return "";
    }

    try {
      const program = Effect.gen(function* () {
        const service = yield* MemoryService;
        const formatted = yield* service.formatForPrompt(taskDescription, {
          limit: maxMemories,
          minRelevance: minSimilarity,
        });
        return formatted;
      });

      return await Effect.runPromise(
        program.pipe(
          Effect.provide(memoryLayer),
          Effect.catchAll(() => Effect.succeed("")),
        ),
      );
    } catch {
      return "";
    }
  };

  // Record task outcome in memory
  const recordTaskMemory = async (
    taskDescription: string,
    outcome: "success" | "failure",
    options?: { errorMessage?: string; skillsUsed?: string[]; durationMs?: number },
  ): Promise<void> => {
    if (!useMemory || !memoryLayer) {
      return;
    }

    try {
      const program = Effect.gen(function* () {
        const service = yield* MemoryService;
        yield* service.recordTask(taskDescription, outcome, options);
      });

      await Effect.runPromise(
        program.pipe(
          Effect.provide(memoryLayer),
          Effect.catchAll(() => Effect.succeed(undefined)),
        ),
      );
    } catch {
      // Ignore memory recording errors
    }
  };

  // Reflexion helper: generate reflection from failure
  const generateReflection = async (
    taskDescription: string,
    errorMessage: string,
    options?: { filesInvolved?: string[]; skillsUsed?: string[]; attemptNumber?: number },
  ): Promise<string> => {
    if (!useReflection || !reflexionLayer) {
      return "";
    }

    try {
      const program = Effect.gen(function* () {
        const service = yield* ReflexionService;
        const failure = yield* service.recordFailure(taskDescription, errorMessage, options);
        const reflection = yield* service.reflect(failure);
        // Format reflection for prompt injection
        return `**What went wrong:** ${reflection.whatWentWrong}\n**Root cause:** ${reflection.whyItWentWrong}\n**Try next:** ${reflection.whatToTryNext}${reflection.suggestedFix ? `\n**Suggested fix:** ${reflection.suggestedFix}` : ""}`;
      });

      return await Effect.runPromise(
        program.pipe(
          Effect.provide(reflexionLayer),
          Effect.catchAll(() => Effect.succeed("")),
        ),
      );
    } catch {
      return "";
    }
  };

  // Get existing reflections for a task (for retry prompts)
  const getReflections = async (taskDescription: string): Promise<string> => {
    if (!useReflection || !reflexionLayer) {
      return "";
    }

    try {
      const program = Effect.gen(function* () {
        const service = yield* ReflexionService;
        const prompt = yield* service.getReflectionPrompt(taskDescription);
        return prompt;
      });

      return await Effect.runPromise(
        program.pipe(
          Effect.provide(reflexionLayer),
          Effect.catchAll(() => Effect.succeed("")),
        ),
      );
    } catch {
      return "";
    }
  };

  // Execute a tool call (reuses the same logic as Ollama runner)
  const executeTool = async (
    name: string,
    args: Record<string, unknown>,
    workspace: string,
  ): Promise<{ success: boolean; output: string }> => {
    try {
      const { resolve, dirname, basename, isAbsolute } = await import("path");
      const { readFileSync, writeFileSync, existsSync, mkdirSync } = await import("fs");

      // Helper to normalize paths - reject absolute paths and resolve relative ones
      const normalizePath = (inputPath: string): string => {
        if (isAbsolute(inputPath)) {
          // If it's an absolute path, just use the basename to make it relative
          return resolve(workspace, basename(inputPath));
        }
        return resolve(workspace, inputPath);
      };

      switch (name) {
        case "read_file": {
          const path = normalizePath(args.path as string);
          if (!existsSync(path)) {
            return { success: false, output: `File not found: ${path}` };
          }
          const content = readFileSync(path, "utf-8");
          return { success: true, output: content };
        }

        case "write_file": {
          const path = normalizePath(args.path as string);
          const dir = dirname(path);
          if (!existsSync(dir)) {
            mkdirSync(dir, { recursive: true });
          }
          writeFileSync(path, args.content as string);
          return { success: true, output: `Wrote ${(args.content as string).length} bytes to ${path}` };
        }

        case "edit_file": {
          const path = normalizePath(args.path as string);
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
    config: fmConfig,
    modelName: "fm:default",

    async runTask(task: TerminalBenchTask, options: RunTaskOptions): Promise<TaskRunResult> {
      const startTime = Date.now();
      let outputText = "";
      let totalTokens = 0;
      let turns = 0;
      let attemptNumber = 0;

      const log = (text: string): void => {
        outputText += text + "\n";
        options.onOutput?.(text + "\n");
      };

      // Retrieve relevant skills for this task (Voyager-style)
      const { skills, ids: skillIds } = await getRelevantSkills(task.description);
      if (skills.length > 0) {
        log(`[Skills] Injected ${skills.length} relevant skills: ${skills.map(s => s.name).join(", ")}`);
      }

      // Retrieve relevant memories (Generative Agents style)
      const memories = await getRelevantMemories(task.description);
      if (memories) {
        log(`[Memory] Injected relevant memories`);
      }

      // Get any existing reflections from previous attempts
      let reflections = await getReflections(task.description);
      if (reflections) {
        log(`[Reflexion] Loaded reflections from previous attempts`);
      }

      // Build system prompt with injected skills, memories, and reflections
      const buildPrompt = () => buildFMSystemPrompt({ skills, memories, reflections });
      let systemPrompt = buildPrompt();

      // Build initial messages with workspace context (keep short for FM context limits)
      const userMessage = `${task.description}`;

      let messages: Array<{ role: "system" | "user" | "assistant"; content: string }> = [
        { role: "system", content: systemPrompt },
        { role: "user", content: userMessage },
      ];

      const maxTurns = task.max_turns ?? options.maxTurns;
      const timeoutMs = (task.timeout_seconds ?? options.timeout) * 1000;

      // Helper to finalize with skill tracking and memory recording
      const finalize = async (result: TaskRunResult): Promise<TaskRunResult> => {
        await recordSkillUsage(skillIds, result.success);
        await recordTaskMemory(
          task.description,
          result.success ? "success" : "failure",
          {
            ...(result.error ? { errorMessage: result.error } : {}),
            skillsUsed: skillIds,
            durationMs: result.durationMs,
          },
        );
        return {
          ...result,
          sessionMetadata: {
            ...result.sessionMetadata,
            skillsUsed: skills.map(s => s.id),
          },
        };
      };

      // Retry loop with reflexion on failure
      while (attemptNumber <= maxReflectionRetries) {
        attemptNumber++;
        if (attemptNumber > 1) {
          log(`\n[Reflexion] Retry attempt ${attemptNumber}/${maxReflectionRetries + 1}`);
          // Reset messages with updated reflections
          systemPrompt = buildPrompt();
          messages = [
            { role: "system", content: systemPrompt },
            { role: "user", content: task.description },
          ];
          turns = 0; // Reset turn count for new attempt
        }

      try {
        while (turns < maxTurns) {
          // Check timeout
          if (Date.now() - startTime > timeoutMs) {
            return await finalize({
              success: false,
              turns,
              tokens: totalTokens,
              durationMs: Date.now() - startTime,
              output: outputText,
              error: "Task timed out",
              model: "fm:default",
            });
          }

          turns++;
          log(`\n--- Turn ${turns} ---`);

          // Truncate messages to fit FM context limit
          const truncatedMessages = truncateMessagesForFM(messages);
          if (truncatedMessages.length < messages.length) {
            log(`[Context] Truncated ${messages.length} messages to ${truncatedMessages.length} for context limit`);
          }

          // Call Foundation Models with retry on context error
          let response;
          try {
            response = await Effect.runPromise(
              client.chat({ messages: truncatedMessages }),
            );
          } catch (e) {
            const errorMsg = e instanceof Error ? e.message : String(e);
            // If context exceeded, try with even shorter context
            if (errorMsg.includes(FM_CONTEXT_EXCEEDED_ERROR)) {
              log(`[Context] Hit context limit, retrying with minimal context`);
              const minimalMessages = truncateMessagesForFM(messages, FM_MAX_CONTEXT_CHARS / 2);
              try {
                response = await Effect.runPromise(
                  client.chat({ messages: minimalMessages }),
                );
              } catch (e2) {
                // If still failing, throw original error
                throw e;
              }
            } else {
              throw e;
            }
          }

          // Track tokens
          if (response.usage) {
            totalTokens += response.usage.total_tokens ?? 0;
          }

          const choice = response.choices[0];
          const assistantContent = choice?.message.content ?? "";

          // Log assistant response
          log(`Assistant: ${assistantContent}`);

          // Parse and execute tool calls FIRST (before checking TASK_COMPLETE)
          const toolCalls = parseToolCalls(assistantContent);

          if (toolCalls.length > 0) {
            // Add assistant message
            messages.push({ role: "assistant", content: assistantContent });

            // Execute each tool
            const toolResults: string[] = [];
            for (const toolCall of toolCalls) {
              log(`Tool call: ${toolCall.name}(${JSON.stringify(toolCall.arguments)})`);

              const result = await executeTool(toolCall.name, toolCall.arguments, options.workspace);
              log(`Tool result: ${result.success ? "success" : "failure"}`);
              if (result.output.length < 500) {
                log(result.output);
              } else {
                log(result.output.slice(0, 500) + "...[truncated]");
              }

              toolResults.push(`${toolCall.name} result: ${result.output}`);
            }

            // Add tool results as user message
            messages.push({
              role: "user",
              content: "Tool results:\n" + toolResults.join("\n\n"),
            });

            // Check for completion AFTER executing tools
            if (assistantContent.includes("TASK_COMPLETE")) {
              return await finalize({
                success: true,
                turns,
                tokens: totalTokens,
                durationMs: Date.now() - startTime,
                output: outputText,
                model: "fm:default",
              });
            }
          } else {
            // No tool calls - add response and continue
            messages.push({ role: "assistant", content: assistantContent });

            // Check for completion (no tools needed)
            if (assistantContent.includes("TASK_COMPLETE")) {
              return await finalize({
                success: true,
                turns,
                tokens: totalTokens,
                durationMs: Date.now() - startTime,
                output: outputText,
                model: "fm:default",
              });
            }

            // If no tool calls and no completion, prompt for action
            if (!assistantContent.trim()) {
              log("Warning: Empty response from model");
              break;
            }

            // Add a nudge to continue
            messages.push({
              role: "user",
              content: "Please continue with the task. Use tools to make changes, and say TASK_COMPLETE when done.",
            });
          }
        }

        // Reached max turns - generate reflection and retry if enabled
        const error = `Reached max turns (${maxTurns})`;
        if (useReflection && attemptNumber <= maxReflectionRetries) {
          log(`\n[Reflexion] Generating reflection for failure...`);
          const newReflection = await generateReflection(task.description, error, {
            skillsUsed: skillIds,
            attemptNumber,
          });
          if (newReflection) {
            reflections = reflections ? `${reflections}\n\n${newReflection}` : newReflection;
            log(`[Reflexion] Generated new insight, will retry with updated context`);
            continue; // Retry with new reflection
          }
        }
        return await finalize({
          success: false,
          turns,
          tokens: totalTokens,
          durationMs: Date.now() - startTime,
          output: outputText,
          error,
          model: "fm:default",
        });
      } catch (e) {
        const durationMs = Date.now() - startTime;
        const errorMsg = e instanceof Error ? e.message : String(e);
        log(`Error: ${errorMsg}`);

        // Generate reflection and retry if enabled
        if (useReflection && attemptNumber <= maxReflectionRetries) {
          log(`\n[Reflexion] Generating reflection for error...`);
          const newReflection = await generateReflection(task.description, errorMsg, {
            skillsUsed: skillIds,
            attemptNumber,
          });
          if (newReflection) {
            reflections = reflections ? `${reflections}\n\n${newReflection}` : newReflection;
            log(`[Reflexion] Generated new insight, will retry with updated context`);
            continue; // Retry with new reflection
          }
        }

        return await finalize({
          success: false,
          turns,
          tokens: totalTokens,
          durationMs,
          output: outputText,
          error: errorMsg,
          model: "fm:default",
        });
      }
      } // End of retry loop

      // Should not reach here, but return failure if we do
      return await finalize({
        success: false,
        turns,
        tokens: totalTokens,
        durationMs: Date.now() - startTime,
        output: outputText,
        error: "Exhausted all retry attempts",
        model: "fm:default",
      });
    },

    async checkHealth(): Promise<{ available: boolean; error?: string }> {
      // Platform check
      if (!isMacOS()) {
        return { available: false, error: "Foundation Models requires macOS" };
      }

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

        if (!result.serverRunning) {
          return {
            available: false,
            error: result.error ?? "Foundation Models server not running. Build with: cd swift/foundation-bridge && ./build.sh",
          };
        }

        if (!result.modelAvailable) {
          return {
            available: false,
            error: result.error ?? "Apple Intelligence not available on this device",
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
 *
 * // Foundation Models (Apple Silicon, macOS 26+)
 * const runner = createModelRunner({ type: "foundation-models" });
 */
export const createModelRunner = (config: ModelConfig): ModelRunner => {
  switch (config.type) {
    case "claude-code":
      return createClaudeCodeRunner();
    case "ollama":
      return createOllamaRunner(config);
    case "foundation-models":
      return createFMRunner(config);
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
 * parseModelString("fm") // { type: "foundation-models" }
 * parseModelString("foundation-models") // { type: "foundation-models" }
 */
export const parseModelString = (modelStr: string): ModelConfig => {
  if (modelStr === "claude-code" || modelStr === "claude") {
    return { type: "claude-code" };
  }

  if (modelStr.startsWith("ollama:")) {
    const model = modelStr.slice("ollama:".length);
    return { type: "ollama", model };
  }

  // Foundation Models (Apple Silicon, macOS 26+)
  if (modelStr === "fm" || modelStr === "foundation-models" || modelStr === "apple") {
    return { type: "foundation-models" };
  }

  if (modelStr.startsWith("fm:")) {
    // fm:11435 would use a custom port (future extension)
    return { type: "foundation-models" };
  }

  // Default to Claude Code for backwards compatibility
  return { type: "claude-code" };
};
