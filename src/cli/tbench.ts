#!/usr/bin/env bun
/**
 * Terminal-Bench CLI Wrapper
 *
 * Entry point for Harbor to invoke MechaCoder for Terminal-Bench evaluation.
 * Executes a task using Claude Code CLI in headless mode and outputs:
 * - events.jsonl: Streaming events during execution
 * - trajectory.json: ATIF v1.4 format trajectory
 * - metrics.json: Token usage, cost, timing, tool stats
 *
 * Usage:
 *   bun src/cli/tbench.ts \
 *     --instruction "Task description" \
 *     --output-dir /logs/agent \
 *     --timeout 3600
 */

import { parseArgs } from "util";
import { join } from "path";
import { appendFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import {
  createClaudeCodeAgent,
  createEmptyTrajectory,
  type Trajectory,
  type Step,
  timestamp,
} from "../atif/index.js";
import { spawn } from "child_process";

// ============================================================================
// CLI Argument Parsing
// ============================================================================

interface TBenchArgs {
  instruction: string;
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
}

// ============================================================================
// ATIF Trajectory Builder
// ============================================================================

class TrajectoryBuilder {
  private steps: Step[] = [];
  private stepId = 1;
  private startTime: string;

  constructor(
    private sessionId: string,
    private agent: ReturnType<typeof createClaudeCodeAgent>,
    private instruction: string
  ) {
    this.startTime = timestamp();
    this.addStep("user", this.instruction);
  }

  addStep(source: "user" | "agent" | "system", message: string): void {
    const step: Step = {
      step_id: this.stepId++,
      timestamp: timestamp(),
      source,
      message,
    };
    this.steps.push(step);
  }

  build(success: boolean, metrics?: { inputTokens: number; outputTokens: number; costUsd?: number }): Trajectory {
    const endTime = timestamp();
    const base = createEmptyTrajectory(this.sessionId, this.agent);

    return {
      ...base,
      steps: this.steps,
      final_metrics: {
        total_prompt_tokens: metrics?.inputTokens ?? 0,
        total_completion_tokens: metrics?.outputTokens ?? 0,
        total_cost_usd: metrics?.costUsd,
        total_steps: this.steps.length,
      },
      extra: {
        instruction: this.instruction,
        start_time: this.startTime,
        end_time: endTime,
        success,
      },
    };
  }
}

// ============================================================================
// Claude CLI Runner
// ============================================================================

interface ClaudeResult {
  success: boolean;
  output: string;
  sessionId: string | undefined;
  turns: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  costUsd: number | undefined;
  error: string | undefined;
}

async function runClaudeCli(
  instruction: string,
  options: {
    cwd: string;
    timeout: number;
    onOutput?: (text: string) => void;
  }
): Promise<ClaudeResult> {
  return new Promise((resolve) => {
    const args = [
      "--print",
      "--dangerously-skip-permissions",
      "--output-format", "json",
      "--max-turns", "300",
      "-p", instruction,
    ];

    console.log(`Running: claude ${args.join(" ").slice(0, 100)}...`);

    const proc = spawn("claude", args, {
      cwd: options.cwd,
      env: process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data) => {
      const text = data.toString();
      stdout += text;
      options.onOutput?.(text);
    });

    proc.stderr.on("data", (data) => {
      stderr += data.toString();
    });

    const timeoutId = setTimeout(() => {
      proc.kill("SIGTERM");
      resolve({
        success: false,
        output: stdout,
        sessionId: undefined,
        turns: 0,
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
        costUsd: undefined,
        error: `Timeout after ${options.timeout}s`,
      });
    }, options.timeout * 1000);

    proc.on("close", (code) => {
      clearTimeout(timeoutId);

      // Try to parse JSON result
      let result: ClaudeResult = {
        success: code === 0,
        output: stdout,
        sessionId: undefined,
        turns: 0,
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
        costUsd: undefined,
        error: code !== 0 ? `Claude CLI exited with code ${code}: ${stderr}` : undefined,
      };

      // Parse JSON output if available
      try {
        const jsonResult = JSON.parse(stdout);
        result.sessionId = jsonResult.session_id;
        result.turns = jsonResult.num_turns ?? 0;
        result.costUsd = jsonResult.total_cost_usd;

        if (jsonResult.usage) {
          result.inputTokens = jsonResult.usage.input_tokens ?? 0;
          result.outputTokens = jsonResult.usage.output_tokens ?? 0;
          result.cacheReadTokens = jsonResult.usage.cache_read_input_tokens ?? 0;
          result.cacheCreationTokens = jsonResult.usage.cache_creation_input_tokens ?? 0;
        }

        // Check result type
        if (jsonResult.type === "result") {
          result.success = jsonResult.subtype === "success";
          if (!result.success && jsonResult.subtype) {
            result.error = `Claude finished with: ${jsonResult.subtype}`;
          }
        }
      } catch {
        // Not JSON, use raw output
        if (code === 0) {
          result.success = true;
        }
      }

      resolve(result);
    });

    proc.on("error", (err) => {
      clearTimeout(timeoutId);
      resolve({
        success: false,
        output: stdout,
        sessionId: undefined,
        turns: 0,
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
        costUsd: undefined,
        error: `Failed to spawn claude: ${err.message}`,
      });
    });
  });
}

// ============================================================================
// Metrics Collection
// ============================================================================

interface TBenchMetrics {
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
    cwd: args.cwd || process.cwd(),
    timeout: args.timeout,
  });

  // Create trajectory builder
  const sessionId = `tbench-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const agent = createClaudeCodeAgent("claude-code", "2.0.58");
  const trajectoryBuilder = new TrajectoryBuilder(sessionId, agent, args.instruction);

  console.log(`\n=== Terminal-Bench Run ===`);
  console.log(`Instruction: ${args.instruction.slice(0, 100)}...`);
  console.log(`Output: ${args.outputDir}`);
  console.log(`CWD: ${args.cwd || process.cwd()}`);
  console.log(`Timeout: ${args.timeout || 3600}s`);
  console.log(`ANTHROPIC_API_KEY: ${process.env.ANTHROPIC_API_KEY ? "set" : "not set"}`);
  console.log(`ANTHROPIC_OAUTH_TOKEN: ${process.env.ANTHROPIC_OAUTH_TOKEN ? "set (" + process.env.ANTHROPIC_OAUTH_TOKEN.slice(0, 20) + "...)" : "not set"}`);
  console.log(`===========================\n`);

  // Output handler
  const onOutput = (text: string): void => {
    process.stdout.write(text);
  };

  // Run Claude CLI
  const result = await runClaudeCli(args.instruction, {
    cwd: args.cwd || process.cwd(),
    timeout: args.timeout || 3600,
    onOutput,
  });

  eventRecorder.record("run_complete", {
    success: result.success,
    turns: result.turns,
    error: result.error,
  });

  // Add final response to trajectory
  if (result.success) {
    trajectoryBuilder.addStep("agent", "Task completed successfully.");
  } else {
    trajectoryBuilder.addStep("system", `Task failed: ${result.error || "Unknown error"}`);
  }

  const endTime = Date.now();
  const endTimeIso = timestamp();
  const durationMs = endTime - startTime;

  // Build trajectory
  const trajectory = trajectoryBuilder.build(result.success, {
    inputTokens: result.inputTokens,
    outputTokens: result.outputTokens,
    ...(result.costUsd !== undefined ? { costUsd: result.costUsd } : {}),
  });

  // Build metrics
  const metrics: TBenchMetrics = {
    instruction: args.instruction,
    success: result.success,
    startTime: startTimeIso,
    endTime: endTimeIso,
    durationMs,
    turns: result.turns,
    tokens: {
      input: result.inputTokens,
      output: result.outputTokens,
      cacheRead: result.cacheReadTokens,
      cacheCreation: result.cacheCreationTokens,
      total: result.inputTokens + result.outputTokens,
    },
    cost: result.costUsd,
    error: result.error,
  };

  // Write output files
  writeFileSync(join(args.outputDir, "trajectory.json"), JSON.stringify(trajectory, null, 2));
  writeFileSync(join(args.outputDir, "metrics.json"), JSON.stringify(metrics, null, 2));

  console.log(`\n=== Run Complete ===`);
  console.log(`Success: ${result.success}`);
  console.log(`Turns: ${result.turns}`);
  console.log(`Duration: ${(durationMs / 1000).toFixed(1)}s`);
  console.log(`Tokens: ${result.inputTokens} in / ${result.outputTokens} out`);
  if (metrics.cost) console.log(`Cost: $${metrics.cost.toFixed(4)}`);
  console.log(`Output: ${args.outputDir}`);
  if (result.error) console.log(`Error: ${result.error}`);
  console.log(`====================\n`);

  // Exit with appropriate code
  process.exit(result.success ? 0 : 1);
};

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(2);
});
