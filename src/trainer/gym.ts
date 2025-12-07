/**
 * Training Gym
 *
 * The "gym" environment for running training tasks.
 * Provides isolated execution with skill/memory integration.
 */

import { Effect, Context, Layer } from "effect";
import { SkillService, makeSkillServiceLive, type SkillServiceError } from "../skills/service.js";
import type { SkillStoreError } from "../skills/store.js";
import { MemoryService, makeMemoryServiceLive, type MemoryServiceError } from "../memory/service.js";
import type { MemoryStoreError } from "../memory/store.js";
import {
  ReflexionService,
  makeReflexionServiceLive,
  type ReflexionServiceError,
} from "../reflexion/service.js";
import {
  ArchivistService,
  makeArchivistServiceLive,
  type ArchivistError,
} from "../archivist/service.js";
import type { TrajectoryStoreError } from "../archivist/store.js";
import { FMService, makeFMServiceLayer, type FMServiceError } from "../fm/service.js";
import type { PatternExtractorError } from "../archivist/extractor.js";
import type { TrainingTask, TaskResult, TrainingConfig } from "./schema.js";
import { DEFAULT_TRAINING_CONFIG, createTaskResult } from "./schema.js";
type TaskResultData = Parameters<typeof createTaskResult>[1];
import type { TrajectoryAction } from "../archivist/schema.js";

// --- Error Types ---

export class GymError extends Error {
  readonly _tag = "GymError";
  constructor(
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "GymError";
  }

  static fromSkill(e: SkillServiceError): GymError {
    return new GymError(e.message, e);
  }

  static fromMemory(e: MemoryServiceError): GymError {
    return new GymError(e.message, e);
  }

  static fromReflexion(e: ReflexionServiceError): GymError {
    return new GymError(e.message, e);
  }

  static fromArchivist(e: ArchivistError): GymError {
    return new GymError(e.message, e);
  }

  static fromFM(e: FMServiceError): GymError {
    return new GymError(e.message, e);
  }
}

// --- Gym Interface ---

export interface IGym {
  /** Execute a single task */
  readonly executeTask: (
    task: TrainingTask,
    config: TrainingConfig,
  ) => Effect.Effect<TaskResult, GymError>;

  /** Execute a task with retries and reflexion */
  readonly executeWithRetry: (
    task: TrainingTask,
    config: TrainingConfig,
  ) => Effect.Effect<TaskResult, GymError>;

  /** Get relevant skills for a task */
  readonly getRelevantSkills: (
    task: TrainingTask,
  ) => Effect.Effect<string[], GymError>;

  /** Get relevant memories for a task */
  readonly getRelevantMemories: (
    task: TrainingTask,
  ) => Effect.Effect<string[], GymError>;
}

// --- Service Tag ---

export class Gym extends Context.Tag("Gym")<Gym, IGym>() {}

// --- Implementation ---

const makeGym = (
  config: TrainingConfig,
): Effect.Effect<
  IGym,
  never,
  FMService | SkillService | MemoryService | ReflexionService | ArchivistService
> =>
  Effect.gen(function* () {
    const fm = yield* FMService;
    const skills = yield* SkillService;
    const memory = yield* MemoryService;
    const reflexion = yield* ReflexionService;
    const archivist = yield* ArchivistService;

    const mapSkillError = Effect.mapError(GymError.fromSkill);
    const mapMemoryError = Effect.mapError(GymError.fromMemory);
    const mapReflexionError = Effect.mapError(GymError.fromReflexion);
    const mapArchivistError = Effect.mapError(GymError.fromArchivist);
    const mapFMError = Effect.mapError(GymError.fromFM);

    const getRelevantSkills = (task: TrainingTask): Effect.Effect<string[], GymError> =>
      Effect.gen(function* () {
        if (!config.useSkills) {
          return [];
        }

        const relevant = yield* skills
          .selectSkills(task.prompt, { topK: 5, minSimilarity: 0.3 })
          .pipe(mapSkillError);

        return relevant.map((s) => s.id);
      });

    const getRelevantMemories = (task: TrainingTask): Effect.Effect<string[], GymError> =>
      Effect.gen(function* () {
        if (!config.useMemory) {
          return [];
        }

        const relevant = yield* memory
          .getRelevantMemories(task.prompt, { limit: 5, minRelevance: 0.3 })
          .pipe(mapMemoryError);

        return relevant.map((m) => m.id);
      });

    const buildPrompt = (
      task: TrainingTask,
      skillIds: string[],
      memoryContext: string,
      reflexionContext: string,
    ): Effect.Effect<string, GymError> =>
      Effect.gen(function* () {
        const parts: string[] = [];

        // Base prompt
        parts.push("You are a coding assistant executing a benchmark task.");
        parts.push("");

        // Add reflexion context if available
        if (reflexionContext) {
          parts.push(reflexionContext);
          parts.push("");
        }

        // Add memory context if available
        if (memoryContext) {
          parts.push("## Relevant Context");
          parts.push(memoryContext);
          parts.push("");
        }

        // Add skill context if skills were retrieved
        if (skillIds.length > 0) {
          const skillDocs: string[] = [];
          for (const id of skillIds) {
            const skill = yield* skills.getSkill(id).pipe(mapSkillError);
            if (skill) {
              skillDocs.push(`### ${skill.name}`);
              skillDocs.push(skill.description);
              if (skill.code) {
                skillDocs.push("```");
                skillDocs.push(skill.code);
                skillDocs.push("```");
              }
            }
          }
          if (skillDocs.length > 0) {
            parts.push("## Available Skills");
            parts.push(...skillDocs);
            parts.push("");
          }
        }

        // Task prompt
        parts.push("## Task");
        parts.push(task.prompt);
        parts.push("");

        if (task.expectedBehavior) {
          parts.push("## Expected Behavior");
          parts.push(task.expectedBehavior);
          parts.push("");
        }

        parts.push("## Instructions");
        parts.push("Execute this task step by step. Use the available skills when applicable.");
        parts.push("Report your progress and final result clearly.");

        return parts.join("\n");
      });

    const executeTask = (
      task: TrainingTask,
      taskConfig: TrainingConfig,
    ): Effect.Effect<TaskResult, GymError> =>
      Effect.gen(function* () {
        const startTime = Date.now();
        const actions: TrajectoryAction[] = [];

        // Get relevant skills and memories
        const skillIds = yield* getRelevantSkills(task);
        const relevantMemories = yield* getRelevantMemories(task);

        // Get memory context
        let memoryContext = "";
        if (relevantMemories.length > 0) {
          for (const memId of relevantMemories.slice(0, 3)) {
            const mem = yield* memory.getMemory(memId).pipe(mapMemoryError);
            if (mem) {
              memoryContext += `- ${mem.content}\n`;
            }
          }
        }

        // Get reflexion context
        const reflexionContext = yield* reflexion
          .getReflectionPrompt(task.prompt)
          .pipe(mapReflexionError);

        // Build full prompt
        const fullPrompt = yield* buildPrompt(task, skillIds, memoryContext, reflexionContext);

        actions.push({
          type: "thinking",
          content: `Prepared prompt with ${skillIds.length} skills, ${relevantMemories.length} memories`,
          timestamp: new Date().toISOString(),
        });

        // Execute with FM
        let response: string;
        let inputTokens = 0;
        let outputTokens = 0;

        try {
          // Run with timeout using chat API
          const chatResult = yield* Effect.race(
            fm.chat({
              messages: [{ role: "user", content: fullPrompt }],
            }).pipe(mapFMError),
            Effect.sleep(task.timeoutMs).pipe(
              Effect.flatMap(() =>
                Effect.fail(new GymError(`Task timed out after ${task.timeoutMs}ms`)),
              ),
            ),
          );

          const choiceContent = chatResult.choices[0]?.message.content ?? "";
          response = typeof choiceContent === "string" ? choiceContent : "";
          inputTokens = chatResult.usage?.prompt_tokens ?? Math.ceil(fullPrompt.length / 4);
          outputTokens = chatResult.usage?.completion_tokens ?? Math.ceil(response.length / 4);

          actions.push({
            type: "output",
            content: response.slice(0, 500),
            success: true,
            durationMs: Date.now() - startTime,
            timestamp: new Date().toISOString(),
          });
        } catch (e) {
          const err = e as Error;
          actions.push({
            type: "error",
            content: err.message,
            success: false,
            timestamp: new Date().toISOString(),
          });

          // Record trajectory for failed task
          if (taskConfig.recordTrajectories) {
            yield* archivist
              .recordTrajectory(task.id, task.prompt, {
                actions,
                outcome: err.message.includes("timeout") ? "timeout" : "failure",
                errorMessage: err.message,
                skillsUsed: skillIds,
                totalDurationMs: Date.now() - startTime,
                model: taskConfig.model,
                tokens: { input: inputTokens, output: 0, total: inputTokens },
              })
              .pipe(mapArchivistError);
          }

          return createTaskResult(task.id, {
            outcome: err.message.includes("timeout") ? "timeout" : "failure",
            errorMessage: err.message,
            durationMs: Date.now() - startTime,
            model: taskConfig.model,
            tokens: { input: inputTokens, output: 0, total: inputTokens },
            skillsUsed: skillIds,
            usedReflexion: !!reflexionContext,
          });
        }

        // Determine outcome (simplified - would need actual validation)
        const outcome = determineOutcome(response, task);
        const score = calculateScore(response, task);

        // Record trajectory
        if (taskConfig.recordTrajectories) {
          yield* archivist
            .recordTrajectory(task.id, task.prompt, {
              actions,
              outcome,
              skillsUsed: skillIds,
              totalDurationMs: Date.now() - startTime,
              model: taskConfig.model,
              tokens: { input: inputTokens, output: outputTokens, total: inputTokens + outputTokens },
            })
            .pipe(mapArchivistError);
        }

        // Update skill usage
        for (const skillId of skillIds) {
          yield* skills.recordUsage(skillId, outcome === "success").pipe(mapSkillError);
        }

        const result = createTaskResult(task.id, {
          outcome,
          score,
          output: response,
          durationMs: Date.now() - startTime,
          model: taskConfig.model,
          tokens: { input: inputTokens, output: outputTokens, total: inputTokens + outputTokens },
          skillsUsed: skillIds,
          usedReflexion: !!reflexionContext,
        });

        return result;
      });

    const executeWithRetry = (
      task: TrainingTask,
      taskConfig: TrainingConfig,
    ): Effect.Effect<TaskResult, GymError> =>
      Effect.gen(function* () {
        let lastResult: TaskResult | null = null;

        for (let attempt = 1; attempt <= taskConfig.maxRetries + 1; attempt++) {
          const result = yield* executeTask(task, taskConfig);

          if (result.outcome === "success") {
            // Mark reflexion as successful if used
            if (result.usedReflexion && lastResult) {
              const reflections = yield* reflexion
                .getReflections(task.prompt)
                .pipe(mapReflexionError);
              for (const r of reflections) {
                yield* reflexion.markSuccess(r.id).pipe(mapReflexionError);
              }
            }
            const successData: TaskResultData = {
              outcome: result.outcome,
              durationMs: result.durationMs,
              model: result.model,
              tokens: result.tokens,
              skillsUsed: result.skillsUsed,
              usedReflexion: result.usedReflexion,
              attemptNumber: attempt,
            };
            if (result.score !== undefined) {
              successData.score = result.score;
            }
            if (result.output !== undefined) {
              successData.output = result.output;
            }
            if (result.errorMessage !== undefined) {
              successData.errorMessage = result.errorMessage;
            }
            const finalResult = createTaskResult(result.taskId, successData);
            return finalResult;
          }

          lastResult = result;

          // If we have more retries, generate a reflection
          if (attempt < taskConfig.maxRetries + 1 && taskConfig.useReflexion) {
            const failure = yield* reflexion
              .recordFailure(task.prompt, result.errorMessage ?? "Task failed", {
                attemptNumber: attempt,
                durationMs: result.durationMs,
                skillsUsed: result.skillsUsed,
              })
              .pipe(mapReflexionError);

            // Generate reflection for next attempt
            yield* reflexion.quickReflect(failure);
          }
        }

        // Return last result with attempt count
        const finalData: TaskResultData = {
          outcome: lastResult!.outcome,
          durationMs: lastResult!.durationMs,
          model: lastResult!.model,
          tokens: lastResult!.tokens,
          skillsUsed: lastResult!.skillsUsed,
          usedReflexion: lastResult!.usedReflexion,
          attemptNumber: taskConfig.maxRetries + 1,
        };
        if (lastResult!.score !== undefined) {
          finalData.score = lastResult!.score;
        }
        if (lastResult!.output !== undefined) {
          finalData.output = lastResult!.output;
        }
        if (lastResult!.errorMessage !== undefined) {
          finalData.errorMessage = lastResult!.errorMessage;
        }
        const finalResult = createTaskResult(lastResult!.taskId, finalData);
        return finalResult;
      });

    return {
      executeTask,
      executeWithRetry,
      getRelevantSkills,
      getRelevantMemories,
    };
  });

// --- Helper Functions ---

/**
 * Determine outcome from response (simplified heuristic).
 */
const determineOutcome = (
  response: string,
  _task: TrainingTask,
): TaskResult["outcome"] => {
  const lower = response.toLowerCase();

  // Check for explicit failure indicators
  if (
    lower.includes("error:") ||
    lower.includes("failed to") ||
    lower.includes("cannot ") ||
    lower.includes("unable to")
  ) {
    return "failure";
  }

  // Check for explicit success indicators
  if (
    lower.includes("successfully") ||
    lower.includes("completed") ||
    lower.includes("done") ||
    lower.includes("finished")
  ) {
    return "success";
  }

  // Check for partial success
  if (lower.includes("partial") || lower.includes("incomplete")) {
    return "partial";
  }

  // Default to success if no clear indicators
  return "success";
};

/**
 * Calculate score from response (simplified heuristic).
 */
const calculateScore = (response: string, task: TrainingTask): number => {
  const outcome = determineOutcome(response, task);

  switch (outcome) {
    case "success":
      return 1.0;
    case "partial":
      return 0.5;
    case "failure":
      return 0.0;
    case "timeout":
      return 0.0;
    default:
      return 0.5;
  }
};

// --- Layer ---

export const GymLayer: Layer.Layer<
  Gym,
  never,
  FMService | SkillService | MemoryService | ReflexionService | ArchivistService
> = Layer.effect(Gym, makeGym(DEFAULT_TRAINING_CONFIG));

/**
 * Create a Gym layer with custom config.
 */
export const makeGymLayer = (
  config: Partial<TrainingConfig> = {},
): Layer.Layer<
  Gym,
  never,
  FMService | SkillService | MemoryService | ReflexionService | ArchivistService
> => Layer.effect(Gym, makeGym({ ...DEFAULT_TRAINING_CONFIG, ...config }));

/**
 * Create a complete Gym layer with all dependencies.
 */
export const makeGymLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<
  Gym,
  SkillStoreError | MemoryStoreError | TrajectoryStoreError | PatternExtractorError,
  never
> => {
  const fmLayer = makeFMServiceLayer({ autoStart: false, enableLogging: false });
  const skillLayer = makeSkillServiceLive(projectRoot);
  const memoryLayer = makeMemoryServiceLive(projectRoot);
  const reflexionLayer = makeReflexionServiceLive(projectRoot);
  const archivistLayer = makeArchivistServiceLive(projectRoot);

  return Layer.provide(
    GymLayer,
    Layer.mergeAll(fmLayer, skillLayer, memoryLayer, reflexionLayer, archivistLayer),
  );
};

export const GymLive: Layer.Layer<
  Gym,
  SkillStoreError | MemoryStoreError | TrajectoryStoreError | PatternExtractorError,
  never
> = makeGymLive();
