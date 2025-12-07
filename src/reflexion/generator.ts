/**
 * Reflection Generator
 *
 * Uses Foundation Models to generate self-critique reflections after failures.
 * Implements the verbal reinforcement aspect of Reflexion.
 */

import { Effect, Context, Layer } from "effect";
import { FMService, type FMServiceError, makeFMServiceLayer } from "../fm/service.js";
import {
  type FailureContext,
  type Reflection,
  type ExtractedSkillPattern,
  buildReflectionPrompt,
  buildSkillExtractionPrompt,
  createReflection,
} from "./schema.js";

// --- Error Types ---

export class ReflectionGeneratorError extends Error {
  readonly _tag = "ReflectionGeneratorError";
  constructor(
    readonly reason: "fm_error" | "parse_error" | "generation_failed",
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "ReflectionGeneratorError";
  }

  static fromFMError(e: FMServiceError): ReflectionGeneratorError {
    return new ReflectionGeneratorError("fm_error", e.message, e);
  }
}

// --- Generator Interface ---

export interface IReflectionGenerator {
  /**
   * Generate a reflection from a failure context.
   */
  readonly generateReflection: (
    failure: FailureContext,
  ) => Effect.Effect<Reflection, ReflectionGeneratorError>;

  /**
   * Generate multiple reflections for a sequence of failures.
   */
  readonly generateReflections: (
    failures: FailureContext[],
  ) => Effect.Effect<Reflection[], ReflectionGeneratorError>;

  /**
   * Extract a skill pattern from a successful reflection.
   */
  readonly extractSkillPattern: (
    reflection: Reflection,
    failure: FailureContext,
  ) => Effect.Effect<ExtractedSkillPattern | null, ReflectionGeneratorError>;

  /**
   * Generate a quick reflection without FM (heuristic-based).
   * Useful when FM is unavailable or for simple errors.
   */
  readonly generateQuickReflection: (
    failure: FailureContext,
  ) => Effect.Effect<Reflection, never>;
}

// --- Generator Tag ---

export class ReflectionGenerator extends Context.Tag("ReflectionGenerator")<
  ReflectionGenerator,
  IReflectionGenerator
>() {}

// --- Implementation ---

const makeReflectionGenerator = (): Effect.Effect<
  IReflectionGenerator,
  never,
  FMService
> =>
  Effect.gen(function* () {
    const fm = yield* FMService;

    /**
     * Parse a reflection from FM response text.
     */
    const parseReflectionResponse = (
      failureId: string,
      response: string,
    ): Reflection => {
      // Try to extract structured sections from the response
      const whatWentWrongMatch = response.match(
        /\*?\*?what went wrong\*?\*?:?\s*(.+?)(?=\*?\*?why|$)/is,
      );
      const whyMatch = response.match(
        /\*?\*?why(?:\s+it went wrong)?\*?\*?:?\s*(.+?)(?=\*?\*?what to try|$)/is,
      );
      const whatToTryMatch = response.match(
        /\*?\*?what to try(?:\s+next)?\*?\*?:?\s*(.+?)(?=\*?\*?suggested|lessons|$)/is,
      );
      const suggestedFixMatch = response.match(
        /\*?\*?suggested fix\*?\*?:?\s*(.+?)(?=\*?\*?lessons|$)/is,
      );
      const lessonsMatch = response.match(
        /\*?\*?lessons(?:\s+learned)?\*?\*?:?\s*(.+?)$/is,
      );

      // Extract or use defaults
      const whatWentWrong = whatWentWrongMatch?.[1]?.trim() ||
        "The attempted solution did not work as expected.";
      const whyItWentWrong = whyMatch?.[1]?.trim() ||
        "The approach may have been incorrect or incomplete.";
      const whatToTryNext = whatToTryMatch?.[1]?.trim() ||
        "Try a different approach based on the error message.";
      const suggestedFix = suggestedFixMatch?.[1]?.trim();

      // Parse lessons (comma or semicolon separated, or bullet points)
      const lessonsText = lessonsMatch?.[1]?.trim() || "";
      const lessons = lessonsText
        .split(/[;,\n]|(?:^|\n)\s*[-*•]/)
        .map((l) => l.trim())
        .filter((l) => l.length > 0);

      return createReflection(failureId, {
        whatWentWrong,
        whyItWentWrong,
        whatToTryNext,
        ...(suggestedFix ? { suggestedFix } : {}),
        lessonsLearned: lessons,
        confidence: 0.8,
      });
    };

    /**
     * Generate heuristic-based reflection for common error types.
     */
    const generateHeuristicReflection = (failure: FailureContext): Reflection => {
      let whatWentWrong: string;
      let whyItWentWrong: string;
      let whatToTryNext: string;
      let suggestedFix: string | undefined;
      const lessons: string[] = [];

      switch (failure.errorType) {
        case "import_error":
          whatWentWrong = "Failed to import a required module or symbol.";
          whyItWentWrong = "The import path may be wrong, or the symbol isn't exported.";
          whatToTryNext = "Search for the correct export location and fix the import.";
          suggestedFix = "Use grep to find where the symbol is exported, then update the import.";
          lessons.push("Always verify import paths exist before using them.");
          break;

        case "type_error":
          whatWentWrong = "TypeScript type mismatch between expected and actual types.";
          whyItWentWrong = "The types don't align - could be missing properties, wrong types, or inference issues.";
          whatToTryNext = "Check the type definition and ensure the value matches exactly.";
          suggestedFix = "Add explicit type annotations or use type assertions where appropriate.";
          lessons.push("Check type definitions before using values.");
          break;

        case "syntax_error":
          whatWentWrong = "Code has a syntax error that prevents parsing.";
          whyItWentWrong = "Missing or extra brackets, quotes, or invalid syntax.";
          whatToTryNext = "Review the code around the error location for syntax issues.";
          lessons.push("Use an editor with syntax highlighting to catch errors early.");
          break;

        case "test_failure":
          whatWentWrong = "A test assertion failed - expected value doesn't match actual.";
          whyItWentWrong = "Either the test expectation is wrong or the implementation is incorrect.";
          whatToTryNext = "Check both the test and the implementation to find the mismatch.";
          lessons.push("Review test expectations carefully before implementing.");
          break;

        case "timeout":
          whatWentWrong = "The operation took too long and timed out.";
          whyItWentWrong = "Could be an infinite loop, waiting for unavailable resource, or genuinely slow operation.";
          whatToTryNext = "Add logging to identify where time is spent, or increase timeout if legitimate.";
          lessons.push("Add timeout handling and progress logging for long operations.");
          break;

        case "runtime_error":
          whatWentWrong = "A runtime error occurred during execution.";
          whyItWentWrong = "Null/undefined access, invalid operation, or unhandled edge case.";
          whatToTryNext = "Add null checks and defensive coding around the error location.";
          lessons.push("Handle edge cases and add null checks proactively.");
          break;

        default:
          whatWentWrong = `The task failed with: ${failure.errorMessage.slice(0, 100)}`;
          whyItWentWrong = "The exact cause is unclear from the error message.";
          whatToTryNext = "Read the error message carefully and search for similar issues.";
          lessons.push("When unclear, search for the error message online.");
      }

      return createReflection(failure.id, {
        whatWentWrong,
        whyItWentWrong,
        whatToTryNext,
        ...(suggestedFix ? { suggestedFix } : {}),
        lessonsLearned: lessons,
        confidence: 0.6, // Lower confidence for heuristic
      });
    };

    const generateReflection = (
      failure: FailureContext,
    ): Effect.Effect<Reflection, ReflectionGeneratorError> =>
      Effect.gen(function* () {
        const prompt = buildReflectionPrompt(failure);

        const response = yield* fm
          .chat({
            messages: [
              {
                role: "system",
                content: "You are a helpful coding assistant that reflects on failures to improve.",
              },
              { role: "user", content: prompt },
            ],
            temperature: 0.3, // Lower temperature for more focused analysis
          })
          .pipe(Effect.mapError(ReflectionGeneratorError.fromFMError));

        const content = response.choices[0]?.message?.content;
        if (!content) {
          // Fall back to heuristic if FM returns empty
          return generateHeuristicReflection(failure);
        }

        try {
          return parseReflectionResponse(failure.id, content);
        } catch (e) {
          throw new ReflectionGeneratorError(
            "parse_error",
            `Failed to parse reflection response: ${e}`,
            e instanceof Error ? e : undefined,
          );
        }
      });

    const generateReflections = (
      failures: FailureContext[],
    ): Effect.Effect<Reflection[], ReflectionGeneratorError> =>
      Effect.gen(function* () {
        const reflections: Reflection[] = [];

        for (const failure of failures) {
          const reflection = yield* generateReflection(failure);
          reflections.push(reflection);
        }

        return reflections;
      });

    const extractSkillPattern = (
      reflection: Reflection,
      failure: FailureContext,
    ): Effect.Effect<ExtractedSkillPattern | null, ReflectionGeneratorError> =>
      Effect.gen(function* () {
        // Only extract from high-confidence successful reflections
        if (!reflection.ledToSuccess || reflection.confidence < 0.7) {
          return null;
        }

        const prompt = buildSkillExtractionPrompt(reflection, failure);

        const response = yield* fm
          .chat({
            messages: [
              {
                role: "system",
                content: "You are a skill extraction system. Output only valid JSON.",
              },
              { role: "user", content: prompt },
            ],
            temperature: 0.2,
          })
          .pipe(Effect.mapError(ReflectionGeneratorError.fromFMError));

        const content = response.choices[0]?.message?.content;
        if (!content) {
          return null;
        }

        try {
          // Extract JSON from response (may be wrapped in markdown)
          const jsonMatch = content.match(/\{[\s\S]*\}/);
          if (!jsonMatch) {
            return null;
          }

          const parsed = JSON.parse(jsonMatch[0]);
          return {
            name: parsed.name || "Unnamed Skill",
            description: parsed.description || "",
            errorPatterns: parsed.errorPatterns || [],
            solution: parsed.solution || "",
            category: parsed.category || "debugging",
            sourceReflectionId: reflection.id,
          };
        } catch {
          return null;
        }
      });

    const generateQuickReflection = (
      failure: FailureContext,
    ): Effect.Effect<Reflection, never> =>
      Effect.succeed(generateHeuristicReflection(failure));

    return {
      generateReflection,
      generateReflections,
      extractSkillPattern,
      generateQuickReflection,
    };
  });

// --- Layer ---

export const ReflectionGeneratorLive: Layer.Layer<
  ReflectionGenerator,
  never,
  FMService
> = Layer.effect(ReflectionGenerator, makeReflectionGenerator());

/**
 * Create a complete ReflectionGenerator layer with FM dependency.
 */
export const makeReflectionGeneratorLive = (): Layer.Layer<
  ReflectionGenerator,
  never,
  never
> => {
  const fmLayer = makeFMServiceLayer({ autoStart: false, enableLogging: false });
  return Layer.provide(ReflectionGeneratorLive, fmLayer);
};
