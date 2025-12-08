/**
 * Suite-aware hint system for FM micro-tasks.
 * Hints are disabled for TB2 to avoid fm-mini heuristics polluting real benchmarks.
 */

export type SuiteMode = "fm-mini" | "tb2" | "unknown";

/**
 * Determine suite mode from suite path or name.
 */
export function getSuiteMode(suitePath: string | undefined): SuiteMode {
  if (!suitePath) return "unknown";

  const lower = suitePath.toLowerCase();
  if (lower.includes("terminal-bench-mini") || lower.includes("fm-mini")) {
    return "fm-mini";
  }
  if (lower.includes("terminal-bench-2") || lower.includes("tb2")) {
    return "tb2";
  }
  return "unknown";
}

/**
 * Build a hint for the current task context.
 * Returns undefined if no hint should be shown.
 *
 * IMPORTANT: TB2 gets NO hints by default. Only fm-mini uses hints.
 */
export function buildHint(
  taskDescription: string,
  previousActions: string[],
  mode: SuiteMode
): string | undefined {
  // TB2: No hints until we add task-specific ones
  if (mode === "tb2") {
    return undefined;
  }

  // Unknown mode: be conservative, no hints
  if (mode === "unknown") {
    return undefined;
  }

  // fm-mini: Keep existing hint logic
  const descLower = taskDescription.toLowerCase();
  const prevJoined = previousActions.join(" ").toLowerCase();

  // Hint: read before write (only for fm-mini)
  if (
    (descLower.includes("read") || descLower.includes("copy") || descLower.includes("duplicate")) &&
    !prevJoined.includes("read_file")
  ) {
    return "Hint: This task requires reading a file first. Use read_file before writing.";
  }

  // Hint: after reading, write exactly (only for fm-mini)
  if (prevJoined.includes("read_file") && !prevJoined.includes("write_file")) {
    return "Hint: You just read file content. Write it EXACTLY to the target file using write_file.";
  }

  // Hint: word count (only for fm-mini)
  if (descLower.includes("count") && descLower.includes("word")) {
    return "Hint: Use shell tools like 'wc -w' for counting words.";
  }

  return undefined;
}
