/**
 * Reflexion Schema
 *
 * Types for MechaCoder's self-critique and reflection system.
 * Based on Reflexion research showing +11% improvement from verbal reinforcement.
 *
 * Key insight: After failures, generate verbal self-critique that gets injected
 * into subsequent attempts. This provides "verbal reinforcement" without
 * parameter updates.
 */

// --- Failure Context ---

/**
 * Context about a failure for reflection.
 */
export interface FailureContext {
  /** Unique ID for this failure */
  id: string;
  /** Task that was being attempted */
  taskDescription: string;
  /** What was tried */
  attemptDescription: string;
  /** The error or failure that occurred */
  errorMessage: string;
  /** Error type classification */
  errorType: ErrorType;
  /** Files that were involved */
  filesInvolved: string[];
  /** Code that was written (if any) */
  codeWritten?: string;
  /** Skills that were used (if any) */
  skillsUsed?: string[];
  /** Attempt number (1-indexed) */
  attemptNumber: number;
  /** Duration of the failed attempt */
  durationMs?: number;
  /** Timestamp */
  timestamp: string;
  /** Project context */
  projectId?: string;
}

/**
 * Classification of error types for targeted reflection.
 */
export type ErrorType =
  | "type_error"       // TypeScript type mismatch
  | "import_error"     // Missing or wrong import
  | "syntax_error"     // Syntax issues
  | "runtime_error"    // Runtime exceptions
  | "test_failure"     // Test assertion failed
  | "build_error"      // Build/compilation error
  | "timeout"          // Task timed out
  | "tool_error"       // Tool execution failed
  | "logic_error"      // Wrong behavior/output
  | "unknown";         // Unclassified

/**
 * Classify an error message into an error type.
 */
export const classifyError = (errorMessage: string): ErrorType => {
  const lower = errorMessage.toLowerCase();

  if (lower.includes("type") && (lower.includes("not assignable") || lower.includes("ts2"))) {
    return "type_error";
  }
  if (lower.includes("cannot find module") || lower.includes("import") || lower.includes("ts2307")) {
    return "import_error";
  }
  if (lower.includes("syntax") || lower.includes("unexpected token") || lower.includes("parsing")) {
    return "syntax_error";
  }
  if (lower.includes("timeout") || lower.includes("timed out")) {
    return "timeout";
  }
  if (
    (lower.includes("test") && lower.includes("fail")) ||
    (lower.includes("expect") && (lower.includes("fail") || lower.includes("tobe") || lower.includes("toequal")))
  ) {
    return "test_failure";
  }
  if (lower.includes("build") || lower.includes("compile") || lower.includes("bundle")) {
    return "build_error";
  }
  if (lower.includes("tool") || lower.includes("command failed")) {
    return "tool_error";
  }
  if (lower.includes("runtime") || lower.includes("exception") || lower.includes("error:")) {
    return "runtime_error";
  }

  return "unknown";
};

// --- Reflection ---

/**
 * A reflection generated after a failure.
 */
export interface Reflection {
  /** Unique ID for this reflection */
  id: string;
  /** Reference to the failure this reflects on */
  failureId: string;
  /** What went wrong (diagnosis) */
  whatWentWrong: string;
  /** Why it went wrong (root cause) */
  whyItWentWrong: string;
  /** What to try differently (action plan) */
  whatToTryNext: string;
  /** Specific fix suggestion (if applicable) */
  suggestedFix?: string;
  /** Lessons learned (for memory) */
  lessonsLearned: string[];
  /** Confidence in this reflection (0-1) */
  confidence: number;
  /** Whether this reflection led to success */
  ledToSuccess?: boolean;
  /** Timestamp */
  timestamp: string;
}

// --- Reflection Prompt ---

/**
 * Build a prompt for generating a reflection from a failure.
 */
export const buildReflectionPrompt = (context: FailureContext): string => {
  const parts = [
    "You are a coding assistant reflecting on a failed attempt. Analyze the failure and provide actionable insights.",
    "",
    "## Failed Task",
    context.taskDescription,
    "",
    "## What Was Tried",
    context.attemptDescription,
    "",
    "## Error",
    `Type: ${context.errorType}`,
    context.errorMessage,
    "",
    "## Files Involved",
    context.filesInvolved.join(", ") || "None specified",
  ];

  if (context.codeWritten) {
    parts.push("", "## Code Written", "```", context.codeWritten, "```");
  }

  if (context.skillsUsed?.length) {
    parts.push("", "## Skills Used", context.skillsUsed.join(", "));
  }

  parts.push(
    "",
    "## Your Reflection",
    "Provide a structured reflection with:",
    "1. **What went wrong**: Diagnose the specific issue",
    "2. **Why it went wrong**: Identify the root cause",
    "3. **What to try next**: Concrete action plan",
    "4. **Suggested fix**: If applicable, provide the exact fix",
    "5. **Lessons learned**: What to remember for future tasks",
    "",
    "Be specific and actionable. Focus on what can be done differently.",
  );

  return parts.join("\n");
};

// --- Reflection Injection ---

/**
 * Format reflections for injection into the system prompt.
 */
export const formatReflectionsForPrompt = (reflections: Reflection[]): string => {
  if (reflections.length === 0) {
    return "";
  }

  const formatted = reflections
    .map((r, i) => {
      const lines = [
        `### Reflection ${i + 1}`,
        `**What went wrong**: ${r.whatWentWrong}`,
        `**Why**: ${r.whyItWentWrong}`,
        `**What to try**: ${r.whatToTryNext}`,
      ];

      if (r.suggestedFix) {
        lines.push(`**Suggested fix**: ${r.suggestedFix}`);
      }

      if (r.lessonsLearned.length > 0) {
        lines.push(`**Lessons**: ${r.lessonsLearned.join("; ")}`);
      }

      return lines.join("\n");
    })
    .join("\n\n");

  return [
    "## Previous Attempt Reflections",
    "",
    "You previously attempted this task and encountered issues. Here are reflections to guide your next attempt:",
    "",
    formatted,
    "",
    "Use these insights to avoid repeating the same mistakes.",
  ].join("\n");
};

// --- Reflection History ---

/**
 * History of reflections for a task.
 */
export interface ReflectionHistory {
  /** Task being worked on */
  taskDescription: string;
  /** All failures encountered */
  failures: FailureContext[];
  /** All reflections generated */
  reflections: Reflection[];
  /** Whether the task was eventually successful */
  succeeded: boolean;
  /** Total attempts */
  totalAttempts: number;
  /** Successful reflection (if task succeeded) */
  successfulReflectionId?: string;
}

// --- Skill Extraction ---

/**
 * Extract a skill pattern from a successful reflection.
 * Used to convert effective reflections into reusable skills.
 */
export interface ExtractedSkillPattern {
  /** Suggested skill name */
  name: string;
  /** Description of what the skill does */
  description: string;
  /** Error patterns this skill addresses */
  errorPatterns: string[];
  /** The fix/solution */
  solution: string;
  /** Category for the skill */
  category: string;
  /** Source reflection ID */
  sourceReflectionId: string;
}

/**
 * Prompt for extracting a skill from a successful reflection.
 */
export const buildSkillExtractionPrompt = (
  reflection: Reflection,
  failure: FailureContext,
): string => {
  return [
    "You are extracting a reusable skill from a successful debugging reflection.",
    "",
    "## Original Error",
    `Type: ${failure.errorType}`,
    failure.errorMessage,
    "",
    "## Successful Reflection",
    `What went wrong: ${reflection.whatWentWrong}`,
    `Why: ${reflection.whyItWentWrong}`,
    `Solution: ${reflection.whatToTryNext}`,
    reflection.suggestedFix ? `Fix: ${reflection.suggestedFix}` : "",
    "",
    "## Extract Skill",
    "Create a reusable skill pattern with:",
    "1. **name**: Short descriptive name (e.g., 'Fix Missing Import')",
    "2. **description**: What the skill does",
    "3. **errorPatterns**: List of error message patterns that trigger this skill",
    "4. **solution**: The fix procedure as code or steps",
    "5. **category**: One of: debugging, testing, refactoring, git, build",
    "",
    "Output as JSON.",
  ].join("\n");
};

// --- Helper Functions ---

/**
 * Generate a unique failure ID.
 */
export const generateFailureId = (): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `fail-${timestamp}-${random}`;
};

/**
 * Generate a unique reflection ID.
 */
export const generateReflectionId = (): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `refl-${timestamp}-${random}`;
};

/**
 * Create a failure context from task execution result.
 */
export const createFailureContext = (
  taskDescription: string,
  errorMessage: string,
  options?: {
    attemptDescription?: string;
    filesInvolved?: string[];
    codeWritten?: string;
    skillsUsed?: string[];
    attemptNumber?: number;
    durationMs?: number;
    projectId?: string;
  },
): FailureContext => ({
  id: generateFailureId(),
  taskDescription,
  attemptDescription: options?.attemptDescription ?? taskDescription,
  errorMessage,
  errorType: classifyError(errorMessage),
  filesInvolved: options?.filesInvolved ?? [],
  codeWritten: options?.codeWritten,
  skillsUsed: options?.skillsUsed,
  attemptNumber: options?.attemptNumber ?? 1,
  durationMs: options?.durationMs,
  timestamp: new Date().toISOString(),
  projectId: options?.projectId,
});

/**
 * Create a reflection from structured data.
 */
export const createReflection = (
  failureId: string,
  data: {
    whatWentWrong: string;
    whyItWentWrong: string;
    whatToTryNext: string;
    suggestedFix?: string;
    lessonsLearned?: string[];
    confidence?: number;
  },
): Reflection => ({
  id: generateReflectionId(),
  failureId,
  whatWentWrong: data.whatWentWrong,
  whyItWentWrong: data.whyItWentWrong,
  whatToTryNext: data.whatToTryNext,
  suggestedFix: data.suggestedFix,
  lessonsLearned: data.lessonsLearned ?? [],
  confidence: data.confidence ?? 0.7,
  timestamp: new Date().toISOString(),
});
