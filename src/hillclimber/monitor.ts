/**
 * HillClimber Monitor Module
 *
 * Validates actions BEFORE execution to catch obvious mistakes.
 * Rule-based, no LLM needed for fast validation.
 *
 * Part of the MAP-inspired architecture for 10x better HillClimber.
 */

// ============================================================================
// Types
// ============================================================================

export interface MonitorDecision {
  /** Whether the action is allowed */
  allowed: boolean;
  /** Reason for rejection (if not allowed) */
  reason?: string;
  /** Warning message (if allowed but risky) */
  warning?: string;
  /** Suggested alternative action */
  suggestion?: string;
}

export interface ActionContext {
  /** Tool being called */
  toolName: string;
  /** Tool arguments */
  args: Record<string, unknown>;
  /** Current workspace path */
  workspace: string;
  /** Task ID being worked on */
  taskId: string;
  /** Files that have been modified in this session */
  modifiedFiles: string[];
  /** Turn number in the execution */
  turnNumber: number;
  /** Previous actions taken */
  previousActions: string[];
}

// ============================================================================
// Validation Rules
// ============================================================================

/**
 * Rule: Prevent writing outside workspace
 */
function checkWorkspaceBounds(ctx: ActionContext): MonitorDecision | null {
  if (ctx.toolName === "write_file" || ctx.toolName === "edit_file") {
    const path = ctx.args.path as string || ctx.args.file_path as string;
    if (path) {
      // Absolute path outside workspace
      if (path.startsWith("/") && !path.startsWith(ctx.workspace) && !path.startsWith("/app")) {
        return {
          allowed: false,
          reason: `Cannot write outside workspace: ${path}`,
          suggestion: `Use relative path or /app/ prefix`,
        };
      }
      // Parent directory traversal
      if (path.includes("..")) {
        return {
          allowed: false,
          reason: `Path traversal not allowed: ${path}`,
          suggestion: `Use direct path without ..`,
        };
      }
    }
  }
  return null;
}

/**
 * Rule: Prevent dangerous shell commands
 */
function checkDangerousCommands(ctx: ActionContext): MonitorDecision | null {
  if (ctx.toolName === "run_command") {
    const cmd = ctx.args.command as string || "";
    const dangerous = [
      { pattern: /rm\s+-rf?\s+\/(?!app)/, reason: "Cannot delete system directories" },
      { pattern: /rm\s+-rf?\s+\*/, reason: "Cannot delete all files" },
      { pattern: /chmod\s+777/, reason: "Overly permissive chmod" },
      { pattern: /curl.*\|\s*(?:ba)?sh/, reason: "Cannot pipe curl to shell" },
      { pattern: /wget.*\|\s*(?:ba)?sh/, reason: "Cannot pipe wget to shell" },
      { pattern: /sudo/, reason: "Cannot use sudo" },
      { pattern: /\bdd\s+.*of=\/dev/, reason: "Cannot write to block devices" },
    ];

    for (const { pattern, reason } of dangerous) {
      if (pattern.test(cmd)) {
        return {
          allowed: false,
          reason,
          suggestion: "Use a safer alternative",
        };
      }
    }
  }
  return null;
}

/**
 * Rule: Warn about overwriting solution files without testing first
 *
 * NOTE: Task-specific solution file mappings have been removed.
 * This rule now uses a generic heuristic: warn if overwriting any file
 * that was previously modified without testing.
 */
function checkTestBeforeSubmit(ctx: ActionContext): MonitorDecision | null {
  if (ctx.toolName === "write_file") {
    const path = ctx.args.path as string || ctx.args.file_path as string || "";

    // Generic check: warn if overwriting a previously modified file without testing
    if (ctx.turnNumber > 1 && ctx.modifiedFiles.includes(path)) {
      // Check if they've tested before rewriting
      const hasTestedRecently = ctx.previousActions.some(
        (a) => a.includes("verify_progress") || a.includes("test") || a.includes("pytest")
      );

      if (!hasTestedRecently) {
        return {
          allowed: true, // Allow but warn
          warning: `Overwriting ${path} without testing. Consider using verify_progress first.`,
        };
      }
    }
  }
  return null;
}

/**
 * Rule: Prevent infinite loops (same action repeated)
 */
function checkRepetition(ctx: ActionContext): MonitorDecision | null {
  if (ctx.previousActions.length >= 3) {
    const last3 = ctx.previousActions.slice(-3);
    const actionSignature = `${ctx.toolName}:${JSON.stringify(ctx.args)}`;

    // Check if this exact action was done recently
    const repeats = last3.filter((a) => a === actionSignature).length;
    if (repeats >= 2) {
      return {
        allowed: false,
        reason: `Action repeated ${repeats + 1} times: ${ctx.toolName}`,
        suggestion: "Try a different approach or use verify_progress to see current state",
      };
    }
  }
  return null;
}

// ============================================================================
// GUARDRAIL: NO TASK-SPECIFIC HARDCODING
//
// All task-specific validation rules have been removed:
// - checkRegexLogSpecific
// - checkPathTracingSpecific
// - checkDnaAssemblySpecific
//
// This file must NEVER contain:
// - Task IDs (e.g., "regex-log", "path-tracing")
// - Task-specific patterns (e.g., IPv4 format, date format)
// - Task-specific hints (e.g., "use lookahead for IPv4")
// - Task-specific file paths (e.g., "/app/regex.txt")
//
// All knowledge must come from:
// 1. The task description (passed as parameter)
// 2. General process knowledge (TDD, iteration)
//
// If you're tempted to add task-specific code, you're defeating the thesis:
// "Architecture beats model size"
// ============================================================================

// ============================================================================
// Main Monitor
// ============================================================================

/**
 * All validation rules in priority order
 *
 * NOTE: All task-specific validation rules have been removed.
 */
const VALIDATION_RULES = [
  checkWorkspaceBounds,
  checkDangerousCommands,
  checkRepetition,
  checkTestBeforeSubmit,
];

/**
 * Monitor an action before execution.
 *
 * @param ctx Action context with tool, args, and execution state
 * @returns Decision on whether to allow the action
 */
export function monitorAction(ctx: ActionContext): MonitorDecision {
  const warnings: string[] = [];

  for (const rule of VALIDATION_RULES) {
    const decision = rule(ctx);

    if (decision) {
      if (!decision.allowed) {
        // Immediate rejection
        return decision;
      }

      if (decision.warning) {
        warnings.push(decision.warning);
      }
    }
  }

  // All rules passed
  const result: MonitorDecision = { allowed: true };
  if (warnings.length > 0) {
    result.warning = warnings.join("; ");
  }
  return result;
}

/**
 * Create an action signature for tracking
 */
export function createActionSignature(toolName: string, args: Record<string, unknown>): string {
  // Create a normalized signature for comparison
  const normalizedArgs: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(args)) {
    if (key === "content" || key === "new_string") {
      // For content, just use a hash/length to detect same writes
      normalizedArgs[key] = `content:${String(value).length}`;
    } else {
      normalizedArgs[key] = value;
    }
  }

  return `${toolName}:${JSON.stringify(normalizedArgs)}`;
}

/**
 * Check if an action is the same as a previous one
 */
export function isSameAction(
  action1: { toolName: string; args: Record<string, unknown> },
  action2: { toolName: string; args: Record<string, unknown> }
): boolean {
  return createActionSignature(action1.toolName, action1.args) ===
    createActionSignature(action2.toolName, action2.args);
}
