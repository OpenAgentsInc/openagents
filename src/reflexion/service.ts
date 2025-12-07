/**
 * Reflexion Service
 *
 * Unified service for the reflexion system.
 * Manages failure tracking, reflection generation, and learning from reflections.
 */

import { Effect, Context, Layer } from "effect";
import { ReflectionGenerator, ReflectionGeneratorLive, type ReflectionGeneratorError } from "./generator.js";
import { MemoryService, makeMemoryServiceLive, type MemoryServiceError } from "../memory/service.js";
import { SkillService, makeSkillServiceLive, type SkillServiceError } from "../skills/service.js";
import { createSkill } from "../skills/schema.js";
import { makeFMServiceLayer, type FMServiceError } from "../fm/service.js";
import {
  type FailureContext,
  type Reflection,
  type ReflectionHistory,
  createFailureContext,
  formatReflectionsForPrompt,
} from "./schema.js";

// --- Error Types ---

export class ReflexionServiceError extends Error {
  readonly _tag = "ReflexionServiceError";
  constructor(
    readonly reason: string,
    override readonly message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "ReflexionServiceError";
  }

  static from(
    e: ReflectionGeneratorError | MemoryServiceError | SkillServiceError | FMServiceError,
  ): ReflexionServiceError {
    return new ReflexionServiceError(e._tag, e.message, e);
  }
}

// --- Service Interface ---

export interface IReflexionService {
  // --- Failure Tracking ---

  /** Record a failure for later reflection */
  readonly recordFailure: (
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
  ) => Effect.Effect<FailureContext, ReflexionServiceError>;

  /** Get failures for a task */
  readonly getFailures: (
    taskDescription: string,
  ) => Effect.Effect<FailureContext[], ReflexionServiceError>;

  // --- Reflection Generation ---

  /** Generate a reflection for a failure */
  readonly reflect: (
    failure: FailureContext,
  ) => Effect.Effect<Reflection, ReflexionServiceError>;

  /** Generate a quick heuristic reflection (no FM call) */
  readonly quickReflect: (
    failure: FailureContext,
  ) => Effect.Effect<Reflection, never>;

  /** Get all reflections for a task */
  readonly getReflections: (
    taskDescription: string,
  ) => Effect.Effect<Reflection[], ReflexionServiceError>;

  // --- Prompt Injection ---

  /** Get formatted reflections for prompt injection */
  readonly getReflectionPrompt: (
    taskDescription: string,
  ) => Effect.Effect<string, ReflexionServiceError>;

  /** Build a reflection-enhanced prompt for a retry attempt */
  readonly buildRetryPrompt: (
    taskDescription: string,
    basePrompt: string,
  ) => Effect.Effect<string, ReflexionServiceError>;

  // --- Learning ---

  /** Mark a reflection as successful (task completed after using it) */
  readonly markSuccess: (
    reflectionId: string,
  ) => Effect.Effect<void, ReflexionServiceError>;

  /** Convert a successful reflection into a skill */
  readonly learnSkill: (
    reflectionId: string,
  ) => Effect.Effect<string | null, ReflexionServiceError>;

  /** Get the full reflection history for a task */
  readonly getHistory: (
    taskDescription: string,
  ) => Effect.Effect<ReflectionHistory, ReflexionServiceError>;

  // --- Stats ---

  /** Get reflexion statistics */
  readonly getStats: () => Effect.Effect<
    {
      totalFailures: number;
      totalReflections: number;
      successfulReflections: number;
      skillsLearned: number;
    },
    ReflexionServiceError
  >;
}

// --- Service Tag ---

export class ReflexionService extends Context.Tag("ReflexionService")<
  ReflexionService,
  IReflexionService
>() {}

// --- In-Memory Storage ---

interface ReflexionStore {
  failures: Map<string, FailureContext[]>; // keyed by task description hash
  reflections: Map<string, Reflection[]>; // keyed by task description hash
  reflectionById: Map<string, Reflection>;
  failureById: Map<string, FailureContext>;
  skillsLearned: number;
}

const createStore = (): ReflexionStore => ({
  failures: new Map(),
  reflections: new Map(),
  reflectionById: new Map(),
  failureById: new Map(),
  skillsLearned: 0,
});

const hashTask = (taskDescription: string): string => {
  // Simple hash for grouping by task
  let hash = 0;
  for (let i = 0; i < taskDescription.length; i++) {
    const char = taskDescription.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash;
  }
  return `task-${Math.abs(hash).toString(36)}`;
};

// --- Implementation ---

const makeReflexionService = (): Effect.Effect<
  IReflexionService,
  never,
  ReflectionGenerator | MemoryService | SkillService
> =>
  Effect.gen(function* () {
    const generator = yield* ReflectionGenerator;
    const memory = yield* MemoryService;
    const skills = yield* SkillService;

    const store = createStore();

    const mapGeneratorError = Effect.mapError(ReflexionServiceError.from);
    const mapMemoryError = Effect.mapError(ReflexionServiceError.from);
    const mapSkillError = Effect.mapError(ReflexionServiceError.from);

    const recordFailure = (
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
    ): Effect.Effect<FailureContext, ReflexionServiceError> =>
      Effect.gen(function* () {
        const taskKey = hashTask(taskDescription);
        const existingFailures = store.failures.get(taskKey) ?? [];
        const attemptNumber = options?.attemptNumber ?? existingFailures.length + 1;

        const failure = createFailureContext(taskDescription, errorMessage, {
          ...options,
          attemptNumber,
        });

        // Store in memory
        existingFailures.push(failure);
        store.failures.set(taskKey, existingFailures);
        store.failureById.set(failure.id, failure);

        // Also record as episodic memory
        yield* memory
          .recordTask(taskDescription, "failure", {
            errorMessage: errorMessage,
            ...(options?.filesInvolved ? { filesModified: options.filesInvolved } : {}),
            ...(options?.skillsUsed ? { skillsUsed: options.skillsUsed } : {}),
            ...(options?.durationMs ? { durationMs: options.durationMs } : {}),
            ...(options?.projectId ? { projectId: options.projectId } : {}),
            importance: "high",
            tags: ["failure", failure.errorType],
          })
          .pipe(mapMemoryError);

        return failure;
      });

    const getFailures = (
      taskDescription: string,
    ): Effect.Effect<FailureContext[], ReflexionServiceError> =>
      Effect.succeed(store.failures.get(hashTask(taskDescription)) ?? []);

    const reflect = (
      failure: FailureContext,
    ): Effect.Effect<Reflection, ReflexionServiceError> =>
      Effect.gen(function* () {
        const reflection = yield* generator.generateReflection(failure).pipe(mapGeneratorError);

        // Store reflection
        const taskKey = hashTask(failure.taskDescription);
        const existingReflections = store.reflections.get(taskKey) ?? [];
        existingReflections.push(reflection);
        store.reflections.set(taskKey, existingReflections);
        store.reflectionById.set(reflection.id, reflection);

        // Record lessons as semantic memories
        for (const lesson of reflection.lessonsLearned) {
          yield* memory
            .recordKnowledge("pattern", lesson, {
              ...(failure.projectId ? { context: `Learned from failure: ${failure.errorType}` } : {}),
              ...(failure.projectId ? { projectId: failure.projectId } : {}),
              importance: "medium",
              tags: ["lesson", "reflection", failure.errorType],
            })
            .pipe(mapMemoryError);
        }

        return reflection;
      });

    const quickReflect = (
      failure: FailureContext,
    ): Effect.Effect<Reflection, never> =>
      Effect.gen(function* () {
        const reflection = yield* generator.generateQuickReflection(failure);

        // Store reflection
        const taskKey = hashTask(failure.taskDescription);
        const existingReflections = store.reflections.get(taskKey) ?? [];
        existingReflections.push(reflection);
        store.reflections.set(taskKey, existingReflections);
        store.reflectionById.set(reflection.id, reflection);

        return reflection;
      });

    const getReflections = (
      taskDescription: string,
    ): Effect.Effect<Reflection[], ReflexionServiceError> =>
      Effect.succeed(store.reflections.get(hashTask(taskDescription)) ?? []);

    const getReflectionPrompt = (
      taskDescription: string,
    ): Effect.Effect<string, ReflexionServiceError> =>
      Effect.gen(function* () {
        const reflections = yield* getReflections(taskDescription);
        return formatReflectionsForPrompt(reflections);
      });

    const buildRetryPrompt = (
      taskDescription: string,
      basePrompt: string,
    ): Effect.Effect<string, ReflexionServiceError> =>
      Effect.gen(function* () {
        const reflectionPrompt = yield* getReflectionPrompt(taskDescription);

        if (!reflectionPrompt) {
          return basePrompt;
        }

        return `${basePrompt}\n\n${reflectionPrompt}`;
      });

    const markSuccess = (reflectionId: string): Effect.Effect<void, ReflexionServiceError> =>
      Effect.gen(function* () {
        const reflection = store.reflectionById.get(reflectionId);
        if (reflection) {
          reflection.ledToSuccess = true;
          store.reflectionById.set(reflectionId, reflection);
        }
      });

    const learnSkill = (
      reflectionId: string,
    ): Effect.Effect<string | null, ReflexionServiceError> =>
      Effect.gen(function* () {
        const reflection = store.reflectionById.get(reflectionId);
        if (!reflection || !reflection.ledToSuccess) {
          return null;
        }

        const failure = store.failureById.get(reflection.failureId);
        if (!failure) {
          return null;
        }

        // Extract skill pattern from reflection
        const pattern = yield* generator
          .extractSkillPattern(reflection, failure)
          .pipe(mapGeneratorError);

        if (!pattern) {
          return null;
        }

        // Create and register the skill
        const skill = createSkill({
          name: pattern.name,
          description: pattern.description,
          code: pattern.solution,
          category: pattern.category as any,
          source: "learned",
          tags: ["learned", "reflexion", failure.errorType],
        });

        yield* skills.registerSkill(skill).pipe(mapSkillError);

        // Link skill to memory
        yield* memory
          .linkSkill(skill.id, pattern.errorPatterns, {
            ...(failure.projectId ? { projectId: failure.projectId } : {}),
            importance: "high",
            tags: ["learned-skill"],
          })
          .pipe(mapMemoryError);

        store.skillsLearned++;
        return skill.id;
      });

    const getHistory = (
      taskDescription: string,
    ): Effect.Effect<ReflectionHistory, ReflexionServiceError> =>
      Effect.gen(function* () {
        const taskKey = hashTask(taskDescription);
        const failures = store.failures.get(taskKey) ?? [];
        const reflections = store.reflections.get(taskKey) ?? [];

        const successfulReflection = reflections.find((r) => r.ledToSuccess);

        const baseHistory = {
          taskDescription,
          failures,
          reflections,
          succeeded: !!successfulReflection,
          totalAttempts: failures.length,
        };

        return successfulReflection
          ? { ...baseHistory, successfulReflectionId: successfulReflection.id }
          : baseHistory;
      });

    const getStats = (): Effect.Effect<
      {
        totalFailures: number;
        totalReflections: number;
        successfulReflections: number;
        skillsLearned: number;
      },
      ReflexionServiceError
    > =>
      Effect.gen(function* () {
        let totalFailures = 0;
        let totalReflections = 0;
        let successfulReflections = 0;

        for (const failures of store.failures.values()) {
          totalFailures += failures.length;
        }

        for (const reflections of store.reflections.values()) {
          totalReflections += reflections.length;
          successfulReflections += reflections.filter((r) => r.ledToSuccess).length;
        }

        return {
          totalFailures,
          totalReflections,
          successfulReflections,
          skillsLearned: store.skillsLearned,
        };
      });

    return {
      recordFailure,
      getFailures,
      reflect,
      quickReflect,
      getReflections,
      getReflectionPrompt,
      buildRetryPrompt,
      markSuccess,
      learnSkill,
      getHistory,
      getStats,
    };
  });

// --- Layer ---

export const ReflexionServiceLayer: Layer.Layer<
  ReflexionService,
  never,
  ReflectionGenerator | MemoryService | SkillService
> = Layer.effect(ReflexionService, makeReflexionService());

/**
 * Create a complete ReflexionService layer with all dependencies.
 */
export const makeReflexionServiceLive = (
  projectRoot: string = process.cwd(),
): Layer.Layer<ReflexionService, never, never> => {
  const fmLayer = makeFMServiceLayer({ autoStart: false, enableLogging: false });
  const generatorLayer = Layer.provide(ReflectionGeneratorLive, fmLayer);
  const memoryLayer = makeMemoryServiceLive(projectRoot);
  const skillLayer = makeSkillServiceLive(projectRoot);

  return Layer.provide(
    ReflexionServiceLayer,
    Layer.mergeAll(generatorLayer, memoryLayer, skillLayer),
  );
};

/**
 * Default ReflexionService layer.
 */
export const ReflexionServiceLive: Layer.Layer<ReflexionService, never, never> =
  makeReflexionServiceLive();
