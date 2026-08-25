/**
 * A tool call in one line, for the row that names it.
 *
 * Formats tool calls and userFacingNames closely mirroring Claude Code's
 * UI conventions (e.g. `Bash(command)`, `Read(file_path)`, `Edit(file_path)`, `delegate(description)`).
 */

/** Fields worth showing whole, in the order a tool would mean them. */
const SUBJECTS = ["command", "path", "file_path", "file", "pattern", "query", "name", "description"] as const;

/**
 * Format a tool call with CC-aligned semantics:
 * If toolName is provided, produces `ToolName(summary)` or `ToolName` if empty.
 * If called with 1 argument for backwards compatibility, produces `summary`.
 */
export function formatToolUseHeader(toolName: string, args: string): string {
  const summary = summarizeToolCall(args, toolName);
  if (!summary) return toolName;
  return `${toolName}(${summary})`;
}

export const summarizeToolCall = (args: string, toolName?: string): string => {
  const trimmed = args.trim();
  if (trimmed.length === 0) return "";

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    // Arguments still streaming in, or a tool that never sent JSON. The raw
    // text is better than nothing and the caller clips it.
    return trimmed;
  }

  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return trimmed;
  const record = parsed as Record<string, unknown>;

  // Special case: openagents / delegate / bash / file tools
  if (toolName === "delegate" || record["prompt"] !== undefined) {
    if (typeof record["description"] === "string" && record["description"].length > 0) {
      return record["description"];
    }
  }

  // An argument vector, which is the case this exists for: the `openagents`
  // tool takes the command line as a list, and joining it back is the whole
  // translation.
  const vector = record["args"];
  if (Array.isArray(vector) && vector.every((part) => typeof part === "string")) {
    return (vector as ReadonlyArray<string>).map(quote).join(" ");
  }

  for (const key of SUBJECTS) {
    const value = record[key];
    if (typeof value === "string" && value.length > 0) return value;
  }

  // Nothing named. Keep every field, without the JSON punctuation that made
  // the row unreadable.
  const pairs = Object.entries(record).filter(([, value]) => value !== undefined);
  if (pairs.length === 0) return "";

  return pairs.map(([key, value]) => `${key}=${scalar(value)}`).join(" ");
};

/**
 * A shell-style quote, so a line with a space in it still reads as one
 * argument rather than as two.
 */
const quote = (part: string): string =>
  part.length === 0 || /[\s"']/.test(part) ? JSON.stringify(part) : part;

const scalar = (value: unknown): string => {
  if (typeof value === "string") return quote(value);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
};
