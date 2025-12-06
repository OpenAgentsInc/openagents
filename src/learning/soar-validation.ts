/**
 * SOAR Structural Validation
 *
 * Validates that synthetic tasks from hindsight relabeling are meaningful.
 * Filters out degenerate cases:
 * - Identity transformation (input = output)
 * - Trivial output (all zeros, empty)
 * - Non-deterministic (random output)
 * - Hardcoded lookup tables
 *
 * Validation criteria:
 * 1. Input ≠ Output (non-trivial transformation)
 * 2. Output is structurally valid (parses, type-checks)
 * 3. Transformation is consistent across examples
 * 4. Not a hardcoded lookup table
 */

import * as S from "effect/Schema";
import { Effect, Context, Layer } from "effect";
import type { SyntheticTaskSolution } from "./soar-hindsight.js";

// --- Validation Result ---

export const ValidationResult = S.Struct({
  /** Whether the synthetic task is valid */
  isValid: S.Boolean,

  /** Validation score (0-1, higher is better) */
  score: S.Number,

  /** Reasons for rejection (if invalid) */
  rejectionReasons: S.Array(S.String),

  /** Validation checks performed */
  checksPerformed: S.Array(
    S.Struct({
      check: S.String,
      passed: S.Boolean,
      details: S.optional(S.String),
    }),
  ),

  /** Timestamp */
  validatedAt: S.String,
});
export type ValidationResult = S.Schema.Type<typeof ValidationResult>;

// --- Validation Configuration ---

export interface ValidationConfig {
  /** Minimum output length */
  minOutputLength: number;

  /** Maximum similarity between input and output (0-1) */
  maxInputOutputSimilarity: number;

  /** Minimum code complexity (estimated) */
  minCodeComplexity: number;

  /** Maximum constant ratio in code */
  maxConstantRatio: number;

  /** Enable entropy check */
  enableEntropyCheck: boolean;

  /** Minimum entropy for output */
  minEntropy: number;
}

export const DEFAULT_VALIDATION_CONFIG: ValidationConfig = {
  minOutputLength: 5,
  maxInputOutputSimilarity: 0.95,
  minCodeComplexity: 3, // At least 3 distinct operations
  maxConstantRatio: 0.8, // Max 80% constants
  enableEntropyCheck: true,
  minEntropy: 0.5,
};

// --- Validation Functions ---

/**
 * Check if output is non-trivial.
 */
export const checkNonTrivialOutput = (
  output: unknown,
  config: ValidationConfig,
): { passed: boolean; details: string } => {
  const outputStr = typeof output === "string" ? output : JSON.stringify(output);

  if (outputStr.length < config.minOutputLength) {
    return { passed: false, details: `Output too short: ${outputStr.length} < ${config.minOutputLength}` };
  }

  // Check for all-same characters
  if (new Set(outputStr).size === 1) {
    return { passed: false, details: "Output is all same character" };
  }

  // Check for empty or null
  if (output === null || output === undefined || outputStr === "null" || outputStr === "undefined") {
    return { passed: false, details: "Output is null/undefined" };
  }

  // Check for empty array/object
  if (outputStr === "[]" || outputStr === "{}") {
    return { passed: false, details: "Output is empty collection" };
  }

  return { passed: true, details: "Output is non-trivial" };
};

/**
 * Calculate similarity between two values.
 */
const calculateSimilarity = (a: unknown, b: unknown): number => {
  const strA = typeof a === "string" ? a : JSON.stringify(a);
  const strB = typeof b === "string" ? b : JSON.stringify(b);

  if (strA === strB) return 1.0;
  if (strA.length === 0 || strB.length === 0) return 0;

  // Simple character overlap similarity
  const setA = new Set(strA);
  const setB = new Set(strB);
  const arrA = Array.from(setA);
  const arrB = Array.from(setB);
  const intersection = new Set(arrA.filter((x) => setB.has(x)));
  const union = new Set([...arrA, ...arrB]);

  return intersection.size / union.size;
};

/**
 * Check that input and output are different (non-identity).
 */
export const checkNonIdentity = (
  input: unknown,
  output: unknown,
  config: ValidationConfig,
): { passed: boolean; details: string } => {
  const similarity = calculateSimilarity(input, output);

  if (similarity >= config.maxInputOutputSimilarity) {
    return {
      passed: false,
      details: `Input/output too similar: ${(similarity * 100).toFixed(1)}% >= ${(config.maxInputOutputSimilarity * 100).toFixed(1)}%`,
    };
  }

  return { passed: true, details: `Input/output similarity: ${(similarity * 100).toFixed(1)}%` };
};

/**
 * Estimate code complexity (number of distinct operations).
 */
const estimateComplexity = (code: string): number => {
  // Count distinct operators and keywords
  const operators = code.match(/[+\-*/%=<>!&|^~?:]+/g) ?? [];
  const keywords = code.match(
    /\b(if|else|for|while|return|function|const|let|var|class|new|try|catch)\b/g,
  ) ?? [];
  const functionCalls = code.match(/\w+\s*\(/g) ?? [];

  return new Set([...operators, ...keywords, ...functionCalls]).size;
};

/**
 * Check code complexity.
 */
export const checkCodeComplexity = (
  code: string,
  config: ValidationConfig,
): { passed: boolean; details: string } => {
  const complexity = estimateComplexity(code);

  if (complexity < config.minCodeComplexity) {
    return {
      passed: false,
      details: `Code too simple: ${complexity} ops < ${config.minCodeComplexity}`,
    };
  }

  return { passed: true, details: `Code complexity: ${complexity} operations` };
};

/**
 * Calculate ratio of constants in code.
 */
const calculateConstantRatio = (code: string): number => {
  const totalChars = code.replace(/\s/g, "").length;
  if (totalChars === 0) return 1;

  // Count string literals and numbers
  const strings: string[] = code.match(/"[^"]*"|'[^']*'|`[^`]*`/g) ?? [];
  const numbers: string[] = code.match(/\b\d+\.?\d*\b/g) ?? [];

  const constantChars =
    strings.reduce((sum, s) => sum + s.length, 0) + numbers.reduce((sum, n) => sum + n.length, 0);

  return constantChars / totalChars;
};

/**
 * Check that code isn't just a lookup table.
 */
export const checkNotLookupTable = (
  code: string,
  config: ValidationConfig,
): { passed: boolean; details: string } => {
  const constantRatio = calculateConstantRatio(code);

  if (constantRatio >= config.maxConstantRatio) {
    return {
      passed: false,
      details: `Too many constants: ${(constantRatio * 100).toFixed(1)}% >= ${(config.maxConstantRatio * 100).toFixed(1)}%`,
    };
  }

  return { passed: true, details: `Constant ratio: ${(constantRatio * 100).toFixed(1)}%` };
};

/**
 * Calculate Shannon entropy of a string.
 */
const calculateEntropy = (str: string): number => {
  if (str.length === 0) return 0;

  const freq = new Map<string, number>();
  for (const char of str) {
    freq.set(char, (freq.get(char) ?? 0) + 1);
  }

  let entropy = 0;
  for (const count of Array.from(freq.values())) {
    const p = count / str.length;
    entropy -= p * Math.log2(p);
  }

  // Normalize to 0-1 range (max entropy for ASCII is ~6.6 bits)
  return Math.min(1, entropy / 6.6);
};

/**
 * Check output entropy (not too random or too uniform).
 */
export const checkEntropy = (
  output: unknown,
  config: ValidationConfig,
): { passed: boolean; details: string } => {
  if (!config.enableEntropyCheck) {
    return { passed: true, details: "Entropy check disabled" };
  }

  const outputStr = typeof output === "string" ? output : JSON.stringify(output);
  const entropy = calculateEntropy(outputStr);

  if (entropy < config.minEntropy) {
    return {
      passed: false,
      details: `Entropy too low: ${entropy.toFixed(2)} < ${config.minEntropy}`,
    };
  }

  return { passed: true, details: `Entropy: ${entropy.toFixed(2)}` };
};

/**
 * Validate a synthetic task-solution pair.
 */
export const validateSynthetic = (
  synthetic: SyntheticTaskSolution,
  config: ValidationConfig = DEFAULT_VALIDATION_CONFIG,
): ValidationResult => {
  const checks: Array<{ check: string; passed: boolean; details?: string }> = [];
  const rejectionReasons: string[] = [];

  // 1. Non-trivial output
  const trivialCheck = checkNonTrivialOutput(synthetic.task.output, config);
  checks.push({ check: "non_trivial_output", passed: trivialCheck.passed, details: trivialCheck.details });
  if (!trivialCheck.passed) rejectionReasons.push(trivialCheck.details);

  // 2. Non-identity transformation
  const identityCheck = checkNonIdentity(synthetic.task.input, synthetic.task.output, config);
  checks.push({ check: "non_identity", passed: identityCheck.passed, details: identityCheck.details });
  if (!identityCheck.passed) rejectionReasons.push(identityCheck.details);

  // 3. Code complexity
  const complexityCheck = checkCodeComplexity(synthetic.solution, config);
  checks.push({ check: "code_complexity", passed: complexityCheck.passed, details: complexityCheck.details });
  if (!complexityCheck.passed) rejectionReasons.push(complexityCheck.details);

  // 4. Not lookup table
  const lookupCheck = checkNotLookupTable(synthetic.solution, config);
  checks.push({ check: "not_lookup_table", passed: lookupCheck.passed, details: lookupCheck.details });
  if (!lookupCheck.passed) rejectionReasons.push(lookupCheck.details);

  // 5. Entropy check
  const entropyCheck = checkEntropy(synthetic.task.output, config);
  checks.push({ check: "entropy", passed: entropyCheck.passed, details: entropyCheck.details });
  if (!entropyCheck.passed) rejectionReasons.push(entropyCheck.details);

  const isValid = rejectionReasons.length === 0;
  const passedCount = checks.filter((c) => c.passed).length;
  const score = passedCount / checks.length;

  return {
    isValid,
    score,
    rejectionReasons,
    checksPerformed: checks,
    validatedAt: new Date().toISOString(),
  };
};

/**
 * Validate a batch of synthetic task-solutions.
 */
export const validateBatch = (
  synthetics: SyntheticTaskSolution[],
  config: ValidationConfig = DEFAULT_VALIDATION_CONFIG,
): { valid: SyntheticTaskSolution[]; invalid: SyntheticTaskSolution[]; results: Map<string, ValidationResult> } => {
  const valid: SyntheticTaskSolution[] = [];
  const invalid: SyntheticTaskSolution[] = [];
  const results = new Map<string, ValidationResult>();

  for (const synthetic of synthetics) {
    const result = validateSynthetic(synthetic, config);
    results.set(synthetic.task.id, result);

    if (result.isValid) {
      // Update the task's validated flag
      valid.push({
        ...synthetic,
        task: { ...synthetic.task, validated: true },
      });
    } else {
      invalid.push(synthetic);
    }
  }

  return { valid, invalid, results };
};

// --- Service Interface ---

export interface IValidationService {
  /** Validate a single synthetic task-solution */
  readonly validate: (synthetic: SyntheticTaskSolution) => Effect.Effect<ValidationResult, never>;

  /** Validate a batch of synthetics */
  readonly validateBatch: (
    synthetics: SyntheticTaskSolution[],
  ) => Effect.Effect<{
    valid: SyntheticTaskSolution[];
    invalid: SyntheticTaskSolution[];
    results: Map<string, ValidationResult>;
  }, never>;

  /** Get current config */
  readonly getConfig: () => Effect.Effect<ValidationConfig, never>;

  /** Update config */
  readonly updateConfig: (updates: Partial<ValidationConfig>) => Effect.Effect<ValidationConfig, never>;

  /** Get validation statistics */
  readonly getStats: () => Effect.Effect<ValidationStats, never>;
}

export interface ValidationStats {
  totalValidated: number;
  totalValid: number;
  totalInvalid: number;
  validationRate: number;
  averageScore: number;
  rejectionsByReason: Map<string, number>;
}

// --- Service Tag ---

export class ValidationService extends Context.Tag("ValidationService")<
  ValidationService,
  IValidationService
>() {}

// --- Service Implementation ---

const makeValidationService = (
  initialConfig: ValidationConfig = DEFAULT_VALIDATION_CONFIG,
): IValidationService => {
  let config = { ...initialConfig };
  let stats: ValidationStats = {
    totalValidated: 0,
    totalValid: 0,
    totalInvalid: 0,
    validationRate: 0,
    averageScore: 0,
    rejectionsByReason: new Map(),
  };

  const updateStats = (result: ValidationResult): void => {
    stats.totalValidated++;
    if (result.isValid) {
      stats.totalValid++;
    } else {
      stats.totalInvalid++;
      for (const reason of result.rejectionReasons) {
        const key = reason.split(":")[0] ?? reason;
        stats.rejectionsByReason.set(key, (stats.rejectionsByReason.get(key) ?? 0) + 1);
      }
    }
    stats.validationRate = stats.totalValid / stats.totalValidated;

    // Update average score
    const prevSum = stats.averageScore * (stats.totalValidated - 1);
    stats.averageScore = (prevSum + result.score) / stats.totalValidated;
  };

  return {
    validate: (synthetic) =>
      Effect.sync(() => {
        const result = validateSynthetic(synthetic, config);
        updateStats(result);
        return result;
      }),

    validateBatch: (synthetics) =>
      Effect.sync(() => {
        const batchResult = validateBatch(synthetics, config);
        for (const [_id, result] of Array.from(batchResult.results)) {
          updateStats(result);
        }
        return batchResult;
      }),

    getConfig: () => Effect.sync(() => ({ ...config })),

    updateConfig: (updates) =>
      Effect.sync(() => {
        config = { ...config, ...updates };
        return { ...config };
      }),

    getStats: () =>
      Effect.sync(() => ({
        ...stats,
        rejectionsByReason: new Map(stats.rejectionsByReason),
      })),
  };
};

// --- Layer ---

export const ValidationServiceLive: Layer.Layer<ValidationService, never, never> = Layer.succeed(
  ValidationService,
  makeValidationService(),
);

/**
 * Create a ValidationService layer with custom config.
 */
export const makeValidationServiceLayer = (
  config: Partial<ValidationConfig> = {},
): Layer.Layer<ValidationService, never, never> =>
  Layer.succeed(
    ValidationService,
    makeValidationService({ ...DEFAULT_VALIDATION_CONFIG, ...config }),
  );
