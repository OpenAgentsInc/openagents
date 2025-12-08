/**
 * StepSummary: Compact representation of tool execution for context management.
 * Prevents context overflow by summarizing tool outputs intelligently.
 */

export interface StepSummary {
  step: number;
  tool: string;
  success: boolean;
  message: string; // Always <= MAX_MESSAGE_CHARS
}

const MAX_MESSAGE_CHARS = 100;
const MAX_SUMMARIES = 3;

/**
 * Create a compact summary of a tool execution result.
 * Tool-aware: produces readable summaries instead of truncated blobs.
 */
export function summarizeToolResult(
  step: number,
  tool: string,
  success: boolean,
  rawOutput: string,
  args?: Record<string, unknown>
): StepSummary {
  let message: string;

  switch (tool) {
    case "read_file": {
      const path = args?.path ?? "file";
      const lines = rawOutput.split("\n").length;
      const chars = rawOutput.length;
      message = success
        ? `Read ${path} (${lines} lines, ${chars} chars)`
        : `Failed to read ${path}: ${rawOutput.slice(0, 50)}`;
      break;
    }

    case "write_file": {
      const path = args?.path ?? "file";
      const bytes = typeof args?.content === "string" ? args.content.length : 0;
      message = success
        ? `Wrote ${bytes} bytes to ${path}`
        : `Failed to write ${path}: ${rawOutput.slice(0, 50)}`;
      break;
    }

    case "run_command": {
      const cmd = String(args?.command ?? "").slice(0, 40);
      const cmdDisplay = cmd.length < 40 ? cmd : cmd + "...";
      if (success) {
        const hasOutput = rawOutput.trim().length > 0;
        message = hasOutput
          ? `Ran: ${cmdDisplay} (ok, output)`
          : `Ran: ${cmdDisplay} (ok, no output)`;
      } else {
        const errorSnippet = rawOutput.slice(0, 30).replace(/\n/g, " ");
        message = `Ran: ${cmdDisplay} (failed: ${errorSnippet})`;
      }
      break;
    }

    case "edit_file": {
      const path = args?.path ?? "file";
      message = success
        ? `Edited ${path}`
        : `Failed to edit ${path}: ${rawOutput.slice(0, 50)}`;
      break;
    }

    case "task_complete": {
      message = "Signaled task complete";
      break;
    }

    case "verification": {
      message = success
        ? "Verification passed"
        : "Verification failed: output does not meet spec";
      break;
    }

    default: {
      // Fallback: truncate raw output
      message = rawOutput.slice(0, MAX_MESSAGE_CHARS);
      if (rawOutput.length > MAX_MESSAGE_CHARS) {
        message = message.slice(0, MAX_MESSAGE_CHARS - 3) + "...";
      }
    }
  }

  // Final safety cap
  if (message.length > MAX_MESSAGE_CHARS) {
    message = message.slice(0, MAX_MESSAGE_CHARS - 3) + "...";
  }

  return { step, tool, success, message };
}

/**
 * Build the Previous field from step history.
 * Keeps only the last MAX_SUMMARIES entries.
 */
export function buildPreviousField(history: StepSummary[]): string {
  if (history.length === 0) return "none";

  const recent = history.slice(-MAX_SUMMARIES);
  return recent
    .map(h => `Step ${h.step} (${h.tool}): ${h.message}`)
    .join("; ");
}

/**
 * Constants for external use
 */
export const STEP_SUMMARY_LIMITS = {
  maxMessageChars: MAX_MESSAGE_CHARS,
  maxSummaries: MAX_SUMMARIES,
} as const;
