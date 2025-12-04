#!/usr/bin/env bun
/**
 * Terminal-Bench CLI Wrapper
 *
 * Entry point for Harbor to invoke MechaCoder for Terminal-Bench evaluation.
 * Executes a task using Claude Code subagent and outputs:
 * - events.jsonl: Streaming events during execution
 * - trajectory.json: ATIF v1.4 format trajectory
 * - metrics.json: Token usage, cost, timing, tool stats
 *
 * Usage:
 *   bun src/cli/tbench.ts \
 *     --instruction "Task description" \
 *     --model "anthropic/claude-sonnet-4-5" \
 *     --output-dir /logs/agent \
 *     --timeout 3600
 */

import { parseArgs } from "util";
import { join } from "path";
import { appendFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import { runClaudeCodeSubagent } from "../agent/orchestrator/claude-code-subagent.js";
import type { Subtask, OrchestratorEvent } from "../agent/orchestrator/types.js";
import {
  createClaudeCodeAgent,
  createEmptyTrajectory,
  type Trajectory,
  type Step,
  type Metrics,
  timestamp,
} from "../atif/index.js";

// ============================================================================
// CLI Argument Parsing
// ============================================================================

interface TBenchArgs {
  instruction: string;
  model: string | undefined;
  outputDir: string;
  timeout: number | undefined;
  cwd: string | undefined;
  help: boolean | undefined;
}

const parseCliArgs = (): TBenchArgs => {
  const { values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      instruction: { type: "string", short: "i" },
      model: { type: "string", short: "m" },
      "output-dir": { type: "string", short: "o" },
      timeout: { type: "string", short: "t" },
      cwd: { type: "string", short: "c" },
      help: { type: "boolean", short: "h" },
    },
  });

  if (values.help) {
    console.log(`
Terminal-Bench CLI Wrapper

Usage:
  bun src/cli/tbench.ts --instruction "Task" --output-dir /logs/agent [options]

Required:
  -i, --instruction   Task instruction/description to execute
  -o, --output-dir    Directory to write output files

Options:
  -m, --model         Model to use (default: anthropic/claude-sonnet-4-5)
  -t, --timeout       Timeout in seconds (default: 3600)
  -c, --cwd           Working directory (default: current directory)
  -h, --help          Show this help message

Output Files:
  events.jsonl        Streaming events during execution
  trajectory.json     ATIF v1.4 format trajectory
  metrics.json        Token usage, cost, timing, tool stats
`);
    process.exit(0);
  }

  if (!values.instruction) {
    console.error("Error: --instruction is required");
    process.exit(1);
  }

  if (!values["output-dir"]) {
    console.error("Error: --output-dir is required");
    process.exit(1);
  }

  return {
    instruction: values.instruction,
    model: values.model,
    outputDir: values["output-dir"],
    timeout: values.timeout ? parseInt(values.timeout, 10) : undefined,
    cwd: values.cwd,
    help: values.help,
  };
};

// ============================================================================
// Event Recording
// ============================================================================

interface TBenchEvent {
  timestamp: string;
  type: string;
  data: Record<string, unknown>;
}

class EventRecorder {
  private eventsPath: string;
  private events: TBenchEvent[] = [];

  constructor(outputDir: string) {
    this.eventsPath = join(outputDir, "events.jsonl");
    // Clear/create the file
    writeFileSync(this.eventsPath, "");
  }

  record(type: string, data: Record<string, unknown> = {}): void {
    const event: TBenchEvent = {
      timestamp: timestamp(),
      type,
      data,
    };
    this.events.push(event);
    appendFileSync(this.eventsPath, JSON.stringify(event) + "\n");
  }

  getEvents(): TBenchEvent[] {
    return this.events;
  }
}

// ============================================================================
// ATIF Trajectory Builder
// ============================================================================

class TrajectoryBuilder {
  private steps: Step[] = [];
  private stepId = 1;
  private startTime: string;
  private totalInputTokens = 0;
  private totalOutputTokens = 0;
  private totalCost = 0;

  constructor(
    private sessionId: string,
    private agent: ReturnType<typeof createClaudeCodeAgent>,
    private instruction: string
  ) {
    this.startTime = timestamp();

    // Record the initial user instruction as first step
    this.addStep("user", this.instruction);
  }

  addStep(
    source: "user" | "agent" | "system",
    message: string,
    extra?: {
      toolCalls?: Array<{ tool_call_id: string; function_name: string; arguments: unknown }>;
      metrics?: Metrics;
      observation?: { results: Array<{ source_call_id?: string; content: unknown }> };
    }
  ): void {
    const step: Step = {
      step_id: this.stepId++,
      timestamp: timestamp(),
      source,
      message,
      ...(extra?.toolCalls && { tool_calls: extra.toolCalls }),
      ...(extra?.metrics && { metrics: extra.metrics }),
      ...(extra?.observation && { observation: extra.observation }),
    };
    this.steps.push(step);

    // Accumulate metrics
    if (extra?.metrics) {
      this.totalInputTokens += extra.metrics.prompt_tokens ?? 0;
      this.totalOutputTokens += extra.metrics.completion_tokens ?? 0;
      this.totalCost += extra.metrics.cost_usd ?? 0;
    }
  }

  addToolResult(toolCallId: string, content: unknown, isError = false): void {
    this.addStep("system", isError ? "Tool execution failed" : "Tool execution result", {
      observation: {
        results: [{ source_call_id: toolCallId, content }],
      },
    });
  }

  build(success: boolean): Trajectory {
    const endTime = timestamp();
    const base = createEmptyTrajectory(this.sessionId, this.agent);

    // Build new trajectory with all fields (avoiding readonly mutation)
    const trajectory: Trajectory = {
      ...base,
      steps: this.steps,
      final_metrics: {
        total_prompt_tokens: this.totalInputTokens,
        total_completion_tokens: this.totalOutputTokens,
        total_cost_usd: this.totalCost > 0 ? this.totalCost : undefined,
        total_steps: this.steps.length,
      },
      extra: {
        instruction: this.instruction,
        start_time: this.startTime,
        end_time: endTime,
        success,
      },
    };

    return trajectory;
  }
}

// ============================================================================
// Metrics Collection
// ============================================================================

interface TBenchMetrics {
  model: string;
  instruction: string;
  success: boolean;
  startTime: string;
  endTime: string;
  durationMs: number;
  turns: number;
  tokens: {
    input: number;
    output: number;
    cacheRead: number;
    cacheCreation: number;
    total: number;
  };
  cost: number | undefined;
  filesModified: string[];
  toolsUsed: Record<string, number>;
  error: string | undefined;
}

// ============================================================================
// Main Execution
// ============================================================================

const main = async (): Promise<void> => {
  const args = parseCliArgs();
  const startTime = Date.now();
  const startTimeIso = timestamp();

  // Ensure output directory exists
  if (!existsSync(args.outputDir)) {
    mkdirSync(args.outputDir, { recursive: true });
  }

  const eventRecorder = new EventRecorder(args.outputDir);
  eventRecorder.record("run_start", {
    instruction: args.instruction,
    model: args.model,
    cwd: args.cwd || process.cwd(),
    timeout: args.timeout,
  });

  // Parse model name (provider/model format for Harbor compatibility)
  const modelName = args.model || "anthropic/claude-sonnet-4-5";
  // Model format: "provider/model" - used by Harbor for routing
  void modelName.split("/"); // Validate format but don't need separate parts

  // Create trajectory builder
  const sessionId = `tbench-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const agent = createClaudeCodeAgent(modelName, "1.0.0");
  const trajectoryBuilder = new TrajectoryBuilder(sessionId, agent, args.instruction);

  // Create subtask for Claude Code
  const subtask: Subtask = {
    id: sessionId,
    description: args.instruction,
    status: "in_progress",
    startedAt: startTimeIso,
  };

  // Track tools used
  const toolsUsed: Record<string, number> = {};

  // Event handler for orchestrator events
  const onEvent = (event: OrchestratorEvent): void => {
    eventRecorder.record(event.type, event as unknown as Record<string, unknown>);

    // Track tool usage from events (subtask_tool_use is emitted as custom event)
    const eventAny = event as Record<string, unknown>;
    if (eventAny.type === "subtask_tool_use" && typeof eventAny.toolName === "string") {
      const toolName = eventAny.toolName;
      toolsUsed[toolName] = (toolsUsed[toolName] || 0) + 1;
    }
  };

  // Output handler for streaming
  const onOutput = (text: string): void => {
    process.stdout.write(text);

    // Parse tool use/result events for trajectory
    if (text.startsWith("[TOOL_USE]")) {
      try {
        const json = JSON.parse(text.replace("[TOOL_USE] ", "").trim());
        trajectoryBuilder.addStep("agent", `Using tool: ${json.tool}`, {
          toolCalls: [
            {
              tool_call_id: json.id || `tool-${Date.now()}`,
              function_name: json.tool,
              arguments: json.input,
            },
          ],
        });
      } catch {
        // Ignore parse errors
      }
    } else if (text.startsWith("[TOOL_RESULT]")) {
      try {
        const json = JSON.parse(text.replace("[TOOL_RESULT] ", "").trim());
        trajectoryBuilder.addToolResult(
          json.tool_result,
          json.content,
          json.is_error
        );
      } catch {
        // Ignore parse errors
      }
    } else if (text.startsWith("[RESULT]")) {
      try {
        const json = JSON.parse(text.replace("[RESULT] ", "").trim());
        if (json.usage) {
          trajectoryBuilder.addStep("system", "Session result", {
            metrics: {
              prompt_tokens: json.usage.input_tokens,
              completion_tokens: json.usage.output_tokens,
              cost_usd: json.cost_usd,
            },
          });
        }
      } catch {
        // Ignore parse errors
      }
    }
  };

  console.log(`\n=== Terminal-Bench Run ===`);
  console.log(`Instruction: ${args.instruction.slice(0, 100)}...`);
  console.log(`Model: ${modelName}`);
  console.log(`Output: ${args.outputDir}`);
  console.log(`CWD: ${args.cwd || process.cwd()}`);
  console.log(`ANTHROPIC_API_KEY: ${process.env.ANTHROPIC_API_KEY ? "set" : "not set"}`);
  console.log(`ANTHROPIC_OAUTH_TOKEN: ${process.env.ANTHROPIC_OAUTH_TOKEN ? "set (" + process.env.ANTHROPIC_OAUTH_TOKEN.slice(0, 20) + "...)" : "not set"}`);
  console.log(`===========================\n`);

  let result;
  try {
    result = await runClaudeCodeSubagent(subtask, {
      cwd: args.cwd || process.cwd(),
      maxTurns: 300,
      permissionMode: "bypassPermissions",
      timeoutMs: (args.timeout || 3600) * 1000,
      onEvent,
      onOutput,
    });

    eventRecorder.record("run_complete", {
      success: result.success,
      turns: result.turns,
      filesModified: result.filesModified,
      error: result.error,
    });

    // Add final agent response to trajectory
    if (result.success) {
      trajectoryBuilder.addStep(
        "agent",
        `Task completed successfully. Modified ${result.filesModified.length} files.`
      );
    } else {
      trajectoryBuilder.addStep("system", `Task failed: ${result.error || "Unknown error"}`);
    }
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    eventRecorder.record("run_error", { error: errorMessage });
    trajectoryBuilder.addStep("system", `Execution error: ${errorMessage}`);

    result = {
      success: false,
      subtaskId: sessionId,
      filesModified: [],
      error: errorMessage,
      turns: 0,
      agent: "claude-code" as const,
    };
  }

  const endTime = Date.now();
  const endTimeIso = timestamp();
  const durationMs = endTime - startTime;

  // Build trajectory
  const trajectory = trajectoryBuilder.build(result.success);

  // Extract token usage from Claude Code session metadata
  const usage = result.sessionMetadata?.usage;
  const inputTokens = usage?.inputTokens || 0;
  const outputTokens = usage?.outputTokens || 0;
  const cacheReadTokens = usage?.cacheReadInputTokens || 0;
  const cacheCreationTokens = usage?.cacheCreationInputTokens || 0;

  // Build metrics
  const metrics: TBenchMetrics = {
    model: modelName,
    instruction: args.instruction,
    success: result.success,
    startTime: startTimeIso,
    endTime: endTimeIso,
    durationMs,
    turns: result.turns,
    tokens: {
      input: inputTokens,
      output: outputTokens,
      cacheRead: cacheReadTokens,
      cacheCreation: cacheCreationTokens,
      total: inputTokens + outputTokens,
    },
    cost: result.sessionMetadata?.totalCostUsd,
    filesModified: result.filesModified,
    toolsUsed: result.sessionMetadata?.toolsUsed || toolsUsed,
    error: result.error,
  };

  // Write output files
  writeFileSync(
    join(args.outputDir, "trajectory.json"),
    JSON.stringify(trajectory, null, 2)
  );
  writeFileSync(
    join(args.outputDir, "metrics.json"),
    JSON.stringify(metrics, null, 2)
  );

  console.log(`\n=== Run Complete ===`);
  console.log(`Success: ${result.success}`);
  console.log(`Turns: ${result.turns}`);
  console.log(`Duration: ${(durationMs / 1000).toFixed(1)}s`);
  console.log(`Tokens: ${inputTokens} in / ${outputTokens} out${cacheReadTokens ? ` (${cacheReadTokens} cache read)` : ""}`);
  if (metrics.cost) console.log(`Cost: $${metrics.cost.toFixed(4)}`);
  console.log(`Files Modified: ${result.filesModified.length}`);
  console.log(`Output: ${args.outputDir}`);
  console.log(`====================\n`);

  // Exit with appropriate code
  process.exit(result.success ? 0 : 1);
};

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(2);
});
