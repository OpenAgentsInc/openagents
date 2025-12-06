/**
 * Guardrails Schema
 *
 * Types for safety constraints in the learning system.
 * Implements boundaries to prevent harmful or wasteful behavior.
 */

// --- Rule Types ---

/**
 * A guardrail rule definition.
 */
export interface GuardrailRule {
  /** Rule ID */
  id: string;
  /** Rule name */
  name: string;
  /** Description */
  description: string;
  /** Rule category */
  category: RuleCategory;
  /** Severity if violated */
  severity: "warning" | "error" | "critical";
  /** Whether rule is enabled */
  enabled: boolean;
  /** Check function name (for dispatch) */
  checkFn: string;
  /** Rule parameters */
  params: Record<string, unknown>;
}

/**
 * Categories of guardrail rules.
 */
export type RuleCategory =
  | "resource"    // Resource usage limits
  | "safety"      // Safety constraints
  | "quality"     // Quality thresholds
  | "behavior"    // Behavioral limits
  | "access";     // Access controls

/**
 * Result of a guardrail check.
 */
export interface GuardrailResult {
  /** Rule that was checked */
  ruleId: string;
  /** Whether the check passed */
  passed: boolean;
  /** Severity if failed */
  severity?: "warning" | "error" | "critical";
  /** Message */
  message: string;
  /** Suggested action */
  action?: string;
  /** Context/details */
  context?: Record<string, unknown>;
  /** Timestamp */
  timestamp: string;
}

/**
 * Aggregated guardrail status.
 */
export interface GuardrailStatus {
  /** All checks passed */
  allPassed: boolean;
  /** Number of warnings */
  warnings: number;
  /** Number of errors */
  errors: number;
  /** Number of critical issues */
  critical: number;
  /** Individual results */
  results: GuardrailResult[];
  /** Should block operation */
  shouldBlock: boolean;
  /** Timestamp */
  timestamp: string;
}

// --- Config Types ---

/**
 * Guardrails configuration.
 */
export interface GuardrailsConfig {
  /** Maximum tokens per task */
  maxTokensPerTask: number;
  /** Maximum duration per task in ms */
  maxDurationPerTask: number;
  /** Maximum retries per task */
  maxRetriesPerTask: number;
  /** Maximum total tokens per run */
  maxTokensPerRun: number;
  /** Maximum duration per run in ms */
  maxDurationPerRun: number;
  /** Minimum success rate to continue */
  minSuccessRate: number;
  /** Maximum consecutive failures */
  maxConsecutiveFailures: number;
  /** Blocked file patterns */
  blockedPatterns: string[];
  /** Maximum skills to learn per run */
  maxSkillsPerRun: number;
  /** Maximum memory entries */
  maxMemoryEntries: number;
  /** Enable strict mode (block on any violation) */
  strictMode: boolean;
}

export const DEFAULT_GUARDRAILS_CONFIG: GuardrailsConfig = {
  maxTokensPerTask: 50000,
  maxDurationPerTask: 300000, // 5 minutes
  maxRetriesPerTask: 3,
  maxTokensPerRun: 1000000, // 1M tokens
  maxDurationPerRun: 3600000, // 1 hour
  minSuccessRate: 0.1,
  maxConsecutiveFailures: 10,
  blockedPatterns: [
    "*.env",
    "*.pem",
    "*.key",
    "*credentials*",
    "*secrets*",
    "*password*",
    "~/.ssh/*",
    "~/.aws/*",
  ],
  maxSkillsPerRun: 50,
  maxMemoryEntries: 10000,
  strictMode: false,
};

// --- Built-in Rules ---

/**
 * Built-in guardrail rules.
 */
export const BUILTIN_RULES: GuardrailRule[] = [
  // Resource limits
  {
    id: "max-tokens-task",
    name: "Max Tokens Per Task",
    description: "Limit token usage per individual task",
    category: "resource",
    severity: "warning",
    enabled: true,
    checkFn: "checkTokensPerTask",
    params: { maxTokens: 50000 },
  },
  {
    id: "max-duration-task",
    name: "Max Duration Per Task",
    description: "Limit execution time per task",
    category: "resource",
    severity: "error",
    enabled: true,
    checkFn: "checkDurationPerTask",
    params: { maxDurationMs: 300000 },
  },
  {
    id: "max-tokens-run",
    name: "Max Tokens Per Run",
    description: "Limit total token usage per training run",
    category: "resource",
    severity: "error",
    enabled: true,
    checkFn: "checkTokensPerRun",
    params: { maxTokens: 1000000 },
  },
  {
    id: "max-duration-run",
    name: "Max Duration Per Run",
    description: "Limit total execution time per run",
    category: "resource",
    severity: "error",
    enabled: true,
    checkFn: "checkDurationPerRun",
    params: { maxDurationMs: 3600000 },
  },

  // Safety constraints
  {
    id: "blocked-files",
    name: "Blocked File Patterns",
    description: "Prevent access to sensitive files",
    category: "safety",
    severity: "critical",
    enabled: true,
    checkFn: "checkBlockedFiles",
    params: { patterns: DEFAULT_GUARDRAILS_CONFIG.blockedPatterns },
  },
  {
    id: "no-network-access",
    name: "No Network Access",
    description: "Prevent unauthorized network operations",
    category: "safety",
    severity: "critical",
    enabled: false, // Disabled by default
    checkFn: "checkNetworkAccess",
    params: {},
  },

  // Quality thresholds
  {
    id: "min-success-rate",
    name: "Minimum Success Rate",
    description: "Require minimum success rate to continue",
    category: "quality",
    severity: "warning",
    enabled: true,
    checkFn: "checkSuccessRate",
    params: { minRate: 0.1 },
  },
  {
    id: "max-consecutive-failures",
    name: "Max Consecutive Failures",
    description: "Stop after too many consecutive failures",
    category: "quality",
    severity: "error",
    enabled: true,
    checkFn: "checkConsecutiveFailures",
    params: { maxFailures: 10 },
  },

  // Behavioral limits
  {
    id: "max-retries",
    name: "Max Retries Per Task",
    description: "Limit retry attempts per task",
    category: "behavior",
    severity: "warning",
    enabled: true,
    checkFn: "checkRetries",
    params: { maxRetries: 3 },
  },
  {
    id: "max-skills-run",
    name: "Max Skills Per Run",
    description: "Limit skills learned per run",
    category: "behavior",
    severity: "warning",
    enabled: true,
    checkFn: "checkSkillsPerRun",
    params: { maxSkills: 50 },
  },
  {
    id: "max-memory-entries",
    name: "Max Memory Entries",
    description: "Limit total memory entries",
    category: "behavior",
    severity: "warning",
    enabled: true,
    checkFn: "checkMemoryEntries",
    params: { maxEntries: 10000 },
  },
];

// --- Helper Functions ---

/**
 * Create a guardrail result.
 */
export const createResult = (
  ruleId: string,
  passed: boolean,
  message: string,
  options?: {
    severity?: "warning" | "error" | "critical";
    action?: string;
    context?: Record<string, unknown>;
  },
): GuardrailResult => ({
  ruleId,
  passed,
  severity: options?.severity,
  message,
  action: options?.action,
  context: options?.context,
  timestamp: new Date().toISOString(),
});

/**
 * Aggregate multiple results into a status.
 */
export const aggregateResults = (results: GuardrailResult[]): GuardrailStatus => {
  const warnings = results.filter((r) => !r.passed && r.severity === "warning").length;
  const errors = results.filter((r) => !r.passed && r.severity === "error").length;
  const critical = results.filter((r) => !r.passed && r.severity === "critical").length;

  return {
    allPassed: results.every((r) => r.passed),
    warnings,
    errors,
    critical,
    results,
    shouldBlock: errors > 0 || critical > 0,
    timestamp: new Date().toISOString(),
  };
};

/**
 * Check if a file path matches blocked patterns.
 */
export const matchesBlockedPattern = (
  filePath: string,
  patterns: string[],
): boolean => {
  const normalizedPath = filePath.toLowerCase();

  for (const pattern of patterns) {
    const normalizedPattern = pattern.toLowerCase();

    // Simple glob matching
    // First check for patterns with wildcards on both ends (e.g., *credentials*)
    if (normalizedPattern.startsWith("*") && normalizedPattern.endsWith("*") && normalizedPattern.length > 2) {
      const middle = normalizedPattern.slice(1, -1);
      if (normalizedPath.includes(middle)) {
        return true;
      }
    } else if (normalizedPattern.startsWith("*")) {
      const suffix = normalizedPattern.slice(1);
      if (normalizedPath.endsWith(suffix) || normalizedPath.includes(suffix)) {
        return true;
      }
    } else if (normalizedPattern.endsWith("*")) {
      const prefix = normalizedPattern.slice(0, -1);
      if (normalizedPath.startsWith(prefix) || normalizedPath.includes(prefix)) {
        return true;
      }
    } else if (normalizedPattern.includes("*")) {
      // Contains wildcard in middle
      const parts = normalizedPattern.split("*");
      if (parts.every((part) => normalizedPath.includes(part))) {
        return true;
      }
    } else {
      // Exact match
      if (normalizedPath.includes(normalizedPattern)) {
        return true;
      }
    }
  }

  return false;
};

/**
 * Get rules by category.
 */
export const getRulesByCategory = (
  rules: GuardrailRule[],
  category: RuleCategory,
): GuardrailRule[] => rules.filter((r) => r.category === category);

/**
 * Get enabled rules.
 */
export const getEnabledRules = (rules: GuardrailRule[]): GuardrailRule[] =>
  rules.filter((r) => r.enabled);
