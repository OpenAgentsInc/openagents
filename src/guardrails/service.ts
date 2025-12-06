/**
 * Guardrails Service
 *
 * Validates operations against safety constraints.
 * Enforces resource limits and behavioral boundaries.
 */

import { Effect, Context, Layer } from "effect";
import type {
  GuardrailRule,
  GuardrailResult,
  GuardrailStatus,
  GuardrailsConfig,
} from "./schema.js";
import {
  DEFAULT_GUARDRAILS_CONFIG,
  BUILTIN_RULES,
  createResult,
  aggregateResults,
  matchesBlockedPattern,
  getEnabledRules,
} from "./schema.js";

// --- Error Types ---

export class GuardrailsError extends Error {
  readonly _tag = "GuardrailsError";
  constructor(
    readonly reason: string,
    message: string,
    readonly violations: GuardrailResult[],
  ) {
    super(message);
    this.name = "GuardrailsError";
  }
}

// --- Validation Context ---

/**
 * Context for validation checks.
 */
export interface ValidationContext {
  /** Current task tokens */
  taskTokens?: number;
  /** Current task duration */
  taskDurationMs?: number;
  /** Current retry count */
  retryCount?: number;
  /** Total run tokens */
  runTokens?: number;
  /** Total run duration */
  runDurationMs?: number;
  /** Current success rate */
  successRate?: number;
  /** Consecutive failures */
  consecutiveFailures?: number;
  /** Files being accessed */
  filePaths?: string[];
  /** Skills learned this run */
  skillsLearned?: number;
  /** Total memory entries */
  memoryEntries?: number;
  /** Network operations */
  networkOperations?: string[];
}

// --- Service Interface ---

export interface IGuardrailsService {
  /** Validate a context against all rules */
  readonly validate: (context: ValidationContext) => Effect.Effect<GuardrailStatus, never>;

  /** Validate and throw if blocked */
  readonly validateOrFail: (
    context: ValidationContext,
  ) => Effect.Effect<GuardrailStatus, GuardrailsError>;

  /** Check a single rule */
  readonly checkRule: (
    rule: GuardrailRule,
    context: ValidationContext,
  ) => Effect.Effect<GuardrailResult, never>;

  /** Check if file access is allowed */
  readonly checkFileAccess: (filePath: string) => Effect.Effect<GuardrailResult, never>;

  /** Check if tokens are within limit */
  readonly checkTokens: (
    current: number,
    limit: number,
    scope: "task" | "run",
  ) => Effect.Effect<GuardrailResult, never>;

  /** Check if duration is within limit */
  readonly checkDuration: (
    current: number,
    limit: number,
    scope: "task" | "run",
  ) => Effect.Effect<GuardrailResult, never>;

  /** Get all rules */
  readonly getRules: () => Effect.Effect<GuardrailRule[], never>;

  /** Enable/disable a rule */
  readonly setRuleEnabled: (ruleId: string, enabled: boolean) => Effect.Effect<void, never>;

  /** Update config */
  readonly updateConfig: (updates: Partial<GuardrailsConfig>) => Effect.Effect<void, never>;

  /** Get current config */
  readonly getConfig: () => Effect.Effect<GuardrailsConfig, never>;
}

// --- Service Tag ---

export class GuardrailsService extends Context.Tag("GuardrailsService")<
  GuardrailsService,
  IGuardrailsService
>() {}

// --- Implementation ---

const makeGuardrailsService = (initialConfig: GuardrailsConfig): IGuardrailsService => {
  let config = { ...initialConfig };
  let rules = [...BUILTIN_RULES];

  const checkRule = (
    rule: GuardrailRule,
    context: ValidationContext,
  ): Effect.Effect<GuardrailResult, never> =>
    Effect.gen(function* () {
      if (!rule.enabled) {
        return createResult(rule.id, true, "Rule disabled");
      }

      switch (rule.checkFn) {
        case "checkTokensPerTask": {
          const limit = (rule.params.maxTokens as number) ?? config.maxTokensPerTask;
          const current = context.taskTokens ?? 0;
          if (current > limit) {
            return createResult(rule.id, false, `Task tokens (${current}) exceed limit (${limit})`, {
              severity: rule.severity,
              action: "Reduce prompt size or split task",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Task tokens within limit");
        }

        case "checkDurationPerTask": {
          const limit = (rule.params.maxDurationMs as number) ?? config.maxDurationPerTask;
          const current = context.taskDurationMs ?? 0;
          if (current > limit) {
            return createResult(rule.id, false, `Task duration (${current}ms) exceeds limit (${limit}ms)`, {
              severity: rule.severity,
              action: "Increase timeout or simplify task",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Task duration within limit");
        }

        case "checkTokensPerRun": {
          const limit = (rule.params.maxTokens as number) ?? config.maxTokensPerRun;
          const current = context.runTokens ?? 0;
          if (current > limit) {
            return createResult(rule.id, false, `Run tokens (${current}) exceed limit (${limit})`, {
              severity: rule.severity,
              action: "End run to avoid excessive token usage",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Run tokens within limit");
        }

        case "checkDurationPerRun": {
          const limit = (rule.params.maxDurationMs as number) ?? config.maxDurationPerRun;
          const current = context.runDurationMs ?? 0;
          if (current > limit) {
            return createResult(rule.id, false, `Run duration (${current}ms) exceeds limit (${limit}ms)`, {
              severity: rule.severity,
              action: "End run to avoid timeout",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Run duration within limit");
        }

        case "checkBlockedFiles": {
          const patterns = (rule.params.patterns as string[]) ?? config.blockedPatterns;
          const files = context.filePaths ?? [];
          for (const file of files) {
            if (matchesBlockedPattern(file, patterns)) {
              return createResult(rule.id, false, `Access to blocked file: ${file}`, {
                severity: rule.severity,
                action: "Remove file from operation",
                context: { file, patterns },
              });
            }
          }
          return createResult(rule.id, true, "No blocked files accessed");
        }

        case "checkNetworkAccess": {
          const operations = context.networkOperations ?? [];
          if (operations.length > 0) {
            return createResult(rule.id, false, `Network access not allowed: ${operations.join(", ")}`, {
              severity: rule.severity,
              action: "Disable network operations",
              context: { operations },
            });
          }
          return createResult(rule.id, true, "No network access");
        }

        case "checkSuccessRate": {
          const minRate = (rule.params.minRate as number) ?? config.minSuccessRate;
          const current = context.successRate ?? 1;
          if (current < minRate) {
            return createResult(rule.id, false, `Success rate (${(current * 100).toFixed(1)}%) below minimum (${(minRate * 100).toFixed(1)}%)`, {
              severity: rule.severity,
              action: "Review failing tasks and adjust approach",
              context: { current, minRate },
            });
          }
          return createResult(rule.id, true, "Success rate acceptable");
        }

        case "checkConsecutiveFailures": {
          const limit = (rule.params.maxFailures as number) ?? config.maxConsecutiveFailures;
          const current = context.consecutiveFailures ?? 0;
          if (current >= limit) {
            return createResult(rule.id, false, `Too many consecutive failures (${current})`, {
              severity: rule.severity,
              action: "Pause and review approach",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Consecutive failures within limit");
        }

        case "checkRetries": {
          const limit = (rule.params.maxRetries as number) ?? config.maxRetriesPerTask;
          const current = context.retryCount ?? 0;
          if (current > limit) {
            return createResult(rule.id, false, `Retries (${current}) exceed limit (${limit})`, {
              severity: rule.severity,
              action: "Skip task and move to next",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Retries within limit");
        }

        case "checkSkillsPerRun": {
          const limit = (rule.params.maxSkills as number) ?? config.maxSkillsPerRun;
          const current = context.skillsLearned ?? 0;
          if (current > limit) {
            return createResult(rule.id, false, `Skills learned (${current}) exceed limit (${limit})`, {
              severity: rule.severity,
              action: "Review skill quality before adding more",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Skills within limit");
        }

        case "checkMemoryEntries": {
          const limit = (rule.params.maxEntries as number) ?? config.maxMemoryEntries;
          const current = context.memoryEntries ?? 0;
          if (current > limit) {
            return createResult(rule.id, false, `Memory entries (${current}) exceed limit (${limit})`, {
              severity: rule.severity,
              action: "Prune old or low-value memories",
              context: { current, limit },
            });
          }
          return createResult(rule.id, true, "Memory entries within limit");
        }

        default:
          return createResult(rule.id, true, `Unknown check function: ${rule.checkFn}`);
      }
    });

  const validate = (context: ValidationContext): Effect.Effect<GuardrailStatus, never> =>
    Effect.gen(function* () {
      const enabledRules = getEnabledRules(rules);
      const results: GuardrailResult[] = [];

      for (const rule of enabledRules) {
        const result = yield* checkRule(rule, context);
        results.push(result);
      }

      return aggregateResults(results);
    });

  const validateOrFail = (
    context: ValidationContext,
  ): Effect.Effect<GuardrailStatus, GuardrailsError> =>
    Effect.gen(function* () {
      const status = yield* validate(context);

      if (config.strictMode && !status.allPassed) {
        const violations = status.results.filter((r) => !r.passed);
        throw new GuardrailsError(
          "validation_failed",
          `Guardrail violations: ${violations.map((v) => v.message).join("; ")}`,
          violations,
        );
      }

      if (status.shouldBlock) {
        const violations = status.results.filter(
          (r) => !r.passed && (r.severity === "error" || r.severity === "critical"),
        );
        throw new GuardrailsError(
          "blocked",
          `Critical guardrail violations: ${violations.map((v) => v.message).join("; ")}`,
          violations,
        );
      }

      return status;
    });

  const checkFileAccess = (filePath: string): Effect.Effect<GuardrailResult, never> =>
    Effect.gen(function* () {
      if (matchesBlockedPattern(filePath, config.blockedPatterns)) {
        return createResult("blocked-files", false, `Access to blocked file: ${filePath}`, {
          severity: "critical",
          action: "Remove file from operation",
          context: { filePath },
        });
      }
      return createResult("blocked-files", true, "File access allowed");
    });

  const checkTokens = (
    current: number,
    limit: number,
    scope: "task" | "run",
  ): Effect.Effect<GuardrailResult, never> =>
    Effect.gen(function* () {
      const ruleId = scope === "task" ? "max-tokens-task" : "max-tokens-run";
      if (current > limit) {
        return createResult(ruleId, false, `${scope} tokens (${current}) exceed limit (${limit})`, {
          severity: scope === "task" ? "warning" : "error",
          context: { current, limit },
        });
      }
      return createResult(ruleId, true, `${scope} tokens within limit`);
    });

  const checkDuration = (
    current: number,
    limit: number,
    scope: "task" | "run",
  ): Effect.Effect<GuardrailResult, never> =>
    Effect.gen(function* () {
      const ruleId = scope === "task" ? "max-duration-task" : "max-duration-run";
      if (current > limit) {
        return createResult(ruleId, false, `${scope} duration (${current}ms) exceeds limit (${limit}ms)`, {
          severity: "error",
          context: { current, limit },
        });
      }
      return createResult(ruleId, true, `${scope} duration within limit`);
    });

  const getRules = (): Effect.Effect<GuardrailRule[], never> => Effect.succeed([...rules]);

  const setRuleEnabled = (ruleId: string, enabled: boolean): Effect.Effect<void, never> =>
    Effect.sync(() => {
      const rule = rules.find((r) => r.id === ruleId);
      if (rule) {
        rule.enabled = enabled;
      }
    });

  const updateConfig = (updates: Partial<GuardrailsConfig>): Effect.Effect<void, never> =>
    Effect.sync(() => {
      config = { ...config, ...updates };
    });

  const getConfig = (): Effect.Effect<GuardrailsConfig, never> => Effect.succeed({ ...config });

  return {
    validate,
    validateOrFail,
    checkRule,
    checkFileAccess,
    checkTokens,
    checkDuration,
    getRules,
    setRuleEnabled,
    updateConfig,
    getConfig,
  };
};

// --- Layer ---

export const makeGuardrailsServiceLive = (
  config: Partial<GuardrailsConfig> = {},
): Layer.Layer<GuardrailsService, never, never> =>
  Layer.succeed(
    GuardrailsService,
    makeGuardrailsService({ ...DEFAULT_GUARDRAILS_CONFIG, ...config }),
  );

export const GuardrailsServiceLive: Layer.Layer<GuardrailsService, never, never> =
  makeGuardrailsServiceLive();
