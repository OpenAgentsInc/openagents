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
 */
function checkTestBeforeSubmit(ctx: ActionContext): MonitorDecision | null {
  if (ctx.toolName === "write_file") {
    const path = ctx.args.path as string || ctx.args.file_path as string || "";
    const solutionFiles: Record<string, string[]> = {
      "regex-log": ["regex.txt"],
      "path-tracing": ["image.c"],
      "model-extraction-relu-logits": ["steal.py", "stolen_A1.npy"],
      "video-processing": ["jump_analyzer.py"],
      "dna-assembly": ["primers.fasta"],
    };

    const taskSolutionFiles = solutionFiles[ctx.taskId] || [];
    const isSolutionFile = taskSolutionFiles.some((f) => path.endsWith(f));

    if (isSolutionFile && ctx.turnNumber > 1) {
      // Check if they've tested before rewriting
      const hasTestedRecently = ctx.previousActions.some(
        (a) => a.includes("verify_progress") || a.includes("test") || a.includes("pytest")
      );

      if (!hasTestedRecently && ctx.modifiedFiles.includes(path)) {
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

/**
 * Rule: Task-specific validation for regex-log
 */
function checkRegexLogSpecific(ctx: ActionContext): MonitorDecision | null {
  if (ctx.taskId !== "regex-log") return null;

  if (ctx.toolName === "write_file") {
    const path = ctx.args.path as string || ctx.args.file_path as string || "";
    const content = ctx.args.content as string || "";

    if (path.endsWith("regex.txt")) {
      // Check for common regex mistakes
      if (content.includes("\\d{3}-\\d{3}-\\d{4}")) {
        return {
          allowed: true,
          warning: "Pattern looks like phone number, not date. Date format is YYYY-MM-DD.",
        };
      }

      // Check if regex is too simple
      if (!content.includes("(?") && content.length < 50) {
        return {
          allowed: true,
          warning: "Regex might be too simple. Need lookahead (?=) for IPv4 constraint and boundary assertions.",
        };
      }

      // Check for balanced parentheses
      const openParens = (content.match(/\(/g) || []).length;
      const closeParens = (content.match(/\)/g) || []).length;
      if (openParens !== closeParens) {
        return {
          allowed: false,
          reason: `Unbalanced parentheses: ${openParens} open, ${closeParens} close`,
          suggestion: "Check regex syntax for balanced parentheses",
        };
      }
    }
  }
  return null;
}

/**
 * Rule: Task-specific validation for path-tracing
 */
function checkPathTracingSpecific(ctx: ActionContext): MonitorDecision | null {
  if (ctx.taskId !== "path-tracing") return null;

  if (ctx.toolName === "write_file") {
    const path = ctx.args.path as string || ctx.args.file_path as string || "";
    const content = ctx.args.content as string || "";

    if (path.endsWith("image.c")) {
      // Check for PPM format basics
      if (!content.includes("P6") && !content.includes("P3") && !content.includes("ppm")) {
        return {
          allowed: true,
          warning: "C code should output PPM format (P6 binary or P3 ASCII). Include PPM header.",
        };
      }

      // Check for image dimensions
      if (!content.includes("320") || !content.includes("200")) {
        return {
          allowed: true,
          warning: "Reference image is 320x200 pixels. Ensure dimensions match.",
        };
      }

      // Check code size (must be < 2KB compressed)
      if (content.length > 10000) {
        return {
          allowed: true,
          warning: "Code is very long. Must be < 2KB when gzipped. Consider more concise implementation.",
        };
      }
    }
  }
  return null;
}

/**
 * Rule: Task-specific validation for dna-assembly
 */
function checkDnaAssemblySpecific(ctx: ActionContext): MonitorDecision | null {
  if (ctx.taskId !== "dna-assembly") return null;

  if (ctx.toolName === "write_file") {
    const path = ctx.args.path as string || ctx.args.file_path as string || "";
    const content = ctx.args.content as string || "";

    if (path.endsWith("primers.fasta")) {
      // Count FASTA headers
      const headers = (content.match(/^>/gm) || []).length;
      if (headers !== 8) {
        return {
          allowed: true,
          warning: `FASTA file should have exactly 8 primers (4 pairs). Found ${headers} headers.`,
        };
      }

      // Check for BsaI recognition site
      if (!content.toLowerCase().includes("ggtctc")) {
        return {
          allowed: true,
          warning: "Primers should contain BsaI recognition site 'ggtctc'.",
        };
      }
    }
  }
  return null;
}

// ============================================================================
// Main Monitor
// ============================================================================

/**
 * All validation rules in priority order
 */
const VALIDATION_RULES = [
  checkWorkspaceBounds,
  checkDangerousCommands,
  checkRepetition,
  checkRegexLogSpecific,
  checkPathTracingSpecific,
  checkDnaAssemblySpecific,
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
