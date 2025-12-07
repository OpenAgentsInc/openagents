/**
 * Trainer Evolution
 *
 * Evolutionary profile system for the Trainer.
 * Supports profile mutation (prompt variants, config changes) and A/B comparison.
 *
 * Used for:
 * 1. Automatic prompt optimization through mutation
 * 2. Config hyperparameter tuning
 * 3. A/B testing different training strategies
 */

import { Effect } from "effect";
import type { TrainingConfig, TrainingStats, TaskResult } from "./schema.js";
import { DEFAULT_TRAINING_CONFIG } from "./schema.js";

// --- Profile Types ---

/**
 * An evolutionary profile for training.
 * Combines configuration with prompt variants and fitness tracking.
 */
export interface EvolutionProfile {
  /** Unique profile ID */
  id: string;
  /** Human-readable name */
  name: string;
  /** Training configuration */
  config: TrainingConfig;
  /** System prompt variant (if any) */
  systemPrompt?: string;
  /** Task prompt prefix/suffix modifications */
  promptModifiers?: PromptModifiers;
  /** Generation number (0 = seed) */
  generation: number;
  /** Parent profile ID (if mutated) */
  parentId?: string;
  /** Fitness score from evaluation */
  fitness?: number;
  /** Evaluation stats */
  stats?: TrainingStats;
  /** Created timestamp */
  createdAt: string;
}

/**
 * Modifications to apply to task prompts.
 */
export interface PromptModifiers {
  /** Prefix to add before task prompt */
  prefix?: string;
  /** Suffix to add after task prompt */
  suffix?: string;
  /** Additional context to inject */
  context?: string;
  /** Thinking style hints */
  thinkingStyle?: "step-by-step" | "holistic" | "minimal" | "verbose";
}

/**
 * Mutation configuration.
 */
export interface MutationConfig {
  /** Probability of mutating each config field (0-1) */
  configMutationRate: number;
  /** Probability of mutating prompt modifiers (0-1) */
  promptMutationRate: number;
  /** Maximum config delta (as percentage of original value) */
  maxConfigDelta: number;
  /** Prompt variant templates */
  promptVariants: PromptModifiers[];
}

export const DEFAULT_MUTATION_CONFIG: MutationConfig = {
  configMutationRate: 0.3,
  promptMutationRate: 0.5,
  maxConfigDelta: 0.2, // 20% change max
  promptVariants: [
    { thinkingStyle: "step-by-step" },
    { thinkingStyle: "holistic" },
    { prefix: "Think carefully about this task:" },
    { suffix: "Verify your solution before submitting." },
    { context: "This is a coding task. Focus on correctness first, then efficiency." },
  ],
};

/**
 * A/B comparison result.
 */
export interface ABComparisonResult {
  /** Profile A ID */
  profileAId: string;
  /** Profile B ID */
  profileBId: string;
  /** Winner (A, B, or tie) */
  winner: "A" | "B" | "tie";
  /** Statistical significance (p-value) */
  pValue: number;
  /** Effect size (Cohen's d) */
  effectSize: number;
  /** Profile A stats */
  statsA: TrainingStats;
  /** Profile B stats */
  statsB: TrainingStats;
  /** Comparison timestamp */
  timestamp: string;
}

// --- Profile Operations ---

/**
 * Generate a unique profile ID.
 */
export const generateProfileId = (): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 6);
  return `prof-${timestamp}-${random}`;
};

/**
 * Create a seed profile (generation 0).
 */
export const createSeedProfile = (
  name: string,
  config?: Partial<TrainingConfig>,
  promptModifiers?: PromptModifiers,
): EvolutionProfile => {
  const profile: EvolutionProfile = {
    id: generateProfileId(),
    name,
    config: { ...DEFAULT_TRAINING_CONFIG, ...config },
    generation: 0,
    createdAt: new Date().toISOString(),
  };
  if (promptModifiers) {
    profile.promptModifiers = promptModifiers;
  }
  return profile;
};

/**
 * Mutate a config value by a random delta.
 */
const mutateNumber = (value: number, maxDelta: number): number => {
  const delta = (Math.random() * 2 - 1) * maxDelta * value;
  return Math.max(0, Math.round(value + delta));
};

/**
 * Mutate a boolean value.
 */
const mutateBoolean = (value: boolean): boolean => !value;

/**
 * Pick a random item from an array.
 */
const pickRandom = <T>(items: T[]): T => items[Math.floor(Math.random() * items.length)];

/**
 * Mutate a profile's configuration.
 */
export const mutateConfig = (
  config: TrainingConfig,
  mutationConfig: MutationConfig,
): TrainingConfig => {
  const mutated = { ...config };

  // Mutate numeric fields
  if (Math.random() < mutationConfig.configMutationRate) {
    mutated.maxTasks = mutateNumber(config.maxTasks, mutationConfig.maxConfigDelta);
    mutated.maxTasks = Math.max(1, Math.min(100, mutated.maxTasks));
  }

  if (Math.random() < mutationConfig.configMutationRate) {
    mutated.maxRetries = mutateNumber(config.maxRetries, mutationConfig.maxConfigDelta);
    mutated.maxRetries = Math.max(0, Math.min(5, mutated.maxRetries));
  }

  if (Math.random() < mutationConfig.configMutationRate) {
    mutated.taskTimeoutMs = mutateNumber(config.taskTimeoutMs, mutationConfig.maxConfigDelta);
    mutated.taskTimeoutMs = Math.max(30000, Math.min(600000, mutated.taskTimeoutMs));
  }

  // Mutate boolean fields
  if (Math.random() < mutationConfig.configMutationRate * 0.5) {
    mutated.useSkills = mutateBoolean(config.useSkills);
  }

  if (Math.random() < mutationConfig.configMutationRate * 0.5) {
    mutated.useMemory = mutateBoolean(config.useMemory);
  }

  if (Math.random() < mutationConfig.configMutationRate * 0.5) {
    mutated.useReflexion = mutateBoolean(config.useReflexion);
  }

  return mutated;
};

/**
 * Mutate a profile's prompt modifiers.
 */
export const mutatePromptModifiers = (
  current: PromptModifiers | undefined,
  mutationConfig: MutationConfig,
): PromptModifiers | undefined => {
  if (Math.random() >= mutationConfig.promptMutationRate) {
    return current;
  }

  // Pick a random variant or modify existing
  if (!current || Math.random() < 0.5) {
    return pickRandom(mutationConfig.promptVariants);
  }

  // Modify existing
  const modified = { ...current };
  const variant = pickRandom(mutationConfig.promptVariants);

  // Merge one property from the variant
  if (variant.prefix && Math.random() < 0.3) modified.prefix = variant.prefix;
  if (variant.suffix && Math.random() < 0.3) modified.suffix = variant.suffix;
  if (variant.context && Math.random() < 0.3) modified.context = variant.context;
  if (variant.thinkingStyle && Math.random() < 0.3) modified.thinkingStyle = variant.thinkingStyle;

  return modified;
};

/**
 * Create a mutated child profile from a parent.
 */
export const mutateProfile = (
  parent: EvolutionProfile,
  mutationConfig: MutationConfig = DEFAULT_MUTATION_CONFIG,
): EvolutionProfile => {
  const mutatedConfig = mutateConfig(parent.config, mutationConfig);
  const mutatedPrompt = mutatePromptModifiers(parent.promptModifiers, mutationConfig);

  const profile: EvolutionProfile = {
    id: generateProfileId(),
    name: `${parent.name}-gen${parent.generation + 1}-${Math.random().toString(36).slice(2, 5)}`,
    config: mutatedConfig,
    generation: parent.generation + 1,
    parentId: parent.id,
    createdAt: new Date().toISOString(),
  };
  if (parent.systemPrompt) {
    profile.systemPrompt = parent.systemPrompt;
  }
  if (mutatedPrompt) {
    profile.promptModifiers = mutatedPrompt;
  }
  return profile;
};

/**
 * Crossover two profiles to create a child.
 */
export const crossoverProfiles = (
  parentA: EvolutionProfile,
  parentB: EvolutionProfile,
): EvolutionProfile => {
  // Mix configs
  const mixedConfig: TrainingConfig = {
    ...DEFAULT_TRAINING_CONFIG,
    maxTasks: Math.random() < 0.5 ? parentA.config.maxTasks : parentB.config.maxTasks,
    maxRetries: Math.random() < 0.5 ? parentA.config.maxRetries : parentB.config.maxRetries,
    useSkills: Math.random() < 0.5 ? parentA.config.useSkills : parentB.config.useSkills,
    useMemory: Math.random() < 0.5 ? parentA.config.useMemory : parentB.config.useMemory,
    useReflexion: Math.random() < 0.5 ? parentA.config.useReflexion : parentB.config.useReflexion,
    taskTimeoutMs: Math.random() < 0.5 ? parentA.config.taskTimeoutMs : parentB.config.taskTimeoutMs,
    projectRoot: parentA.config.projectRoot,
    recordTrajectories: parentA.config.recordTrajectories,
    model: parentA.config.model,
  };

  // Pick one parent's prompt modifiers
  const promptModifiers = Math.random() < 0.5 ? parentA.promptModifiers : parentB.promptModifiers;

  const profile: EvolutionProfile = {
    id: generateProfileId(),
    name: `cross-${parentA.name.slice(0, 8)}-${parentB.name.slice(0, 8)}`,
    config: mixedConfig,
    generation: Math.max(parentA.generation, parentB.generation) + 1,
    parentId: parentA.id, // Track primary parent
    createdAt: new Date().toISOString(),
  };
  if (promptModifiers) {
    profile.promptModifiers = promptModifiers;
  }
  return profile;
};

// --- Fitness Evaluation ---

/**
 * Calculate fitness score from training stats.
 * Higher is better. Combines success rate, speed, and efficiency.
 */
export const calculateFitness = (stats: TrainingStats): number => {
  if (stats.totalTasks === 0) return 0;

  // Weights for different metrics
  const successWeight = 0.6;
  const speedWeight = 0.2;
  const efficiencyWeight = 0.2;

  // Success component (0-1)
  const successScore = stats.successRate;

  // Speed component (0-1, faster is better)
  // Normalize: assume 60s is "good", 300s is "slow"
  const avgDurationSec = stats.averageDurationMs / 1000;
  const speedScore = Math.max(0, 1 - (avgDurationSec - 60) / 240);

  // Efficiency component (0-1, fewer tokens is better)
  // Normalize: assume 1000 tokens/task is "good", 5000 is "expensive"
  const tokensPerTask = stats.totalTokens / stats.totalTasks;
  const efficiencyScore = Math.max(0, 1 - (tokensPerTask - 1000) / 4000);

  return (
    successWeight * successScore +
    speedWeight * speedScore +
    efficiencyWeight * efficiencyScore
  );
};

/**
 * Update a profile with evaluation results.
 */
export const updateProfileFitness = (
  profile: EvolutionProfile,
  stats: TrainingStats,
): EvolutionProfile => ({
  ...profile,
  stats,
  fitness: calculateFitness(stats),
});

// --- A/B Comparison ---

/**
 * Calculate Cohen's d effect size.
 */
const calculateEffectSize = (
  meanA: number,
  stdA: number,
  meanB: number,
  stdB: number,
): number => {
  const pooledStd = Math.sqrt((stdA * stdA + stdB * stdB) / 2);
  if (pooledStd === 0) return 0;
  return (meanA - meanB) / pooledStd;
};

/**
 * Simple two-sample t-test p-value approximation.
 */
const approximatePValue = (
  meanA: number,
  stdA: number,
  nA: number,
  meanB: number,
  stdB: number,
  nB: number,
): number => {
  const se = Math.sqrt((stdA * stdA) / nA + (stdB * stdB) / nB);
  if (se === 0) return 1;
  const t = Math.abs(meanA - meanB) / se;
  // Approximate p-value using normal distribution tail
  return Math.exp(-0.5 * t * t);
};

/**
 * Calculate standard deviation from task results.
 */
const calculateStdDev = (results: TaskResult[]): number => {
  if (results.length < 2) return 0;
  const scores = results.map((r) => (r.outcome === "success" ? 1 : 0));
  const mean = scores.reduce((a: number, b: number) => a + b, 0) / scores.length;
  const variance = scores.reduce((sum: number, s: number) => sum + (s - mean) ** 2, 0) / (scores.length - 1);
  return Math.sqrt(variance);
};

/**
 * Compare two profiles using A/B testing methodology.
 */
export const compareProfiles = (
  profileA: EvolutionProfile,
  resultsA: TaskResult[],
  profileB: EvolutionProfile,
  resultsB: TaskResult[],
): ABComparisonResult => {
  const statsA = profileA.stats!;
  const statsB = profileB.stats!;

  // Calculate standard deviations
  const stdA = calculateStdDev(resultsA);
  const stdB = calculateStdDev(resultsB);

  // Effect size
  const effectSize = calculateEffectSize(
    statsA.successRate,
    stdA,
    statsB.successRate,
    stdB,
  );

  // P-value
  const pValue = approximatePValue(
    statsA.successRate,
    stdA,
    resultsA.length,
    statsB.successRate,
    stdB,
    resultsB.length,
  );

  // Determine winner
  let winner: "A" | "B" | "tie" = "tie";
  const significanceThreshold = 0.05;
  const effectThreshold = 0.2; // Small effect size

  if (pValue < significanceThreshold && Math.abs(effectSize) > effectThreshold) {
    winner = statsA.successRate > statsB.successRate ? "A" : "B";
  }

  return {
    profileAId: profileA.id,
    profileBId: profileB.id,
    winner,
    pValue,
    effectSize,
    statsA,
    statsB,
    timestamp: new Date().toISOString(),
  };
};

// --- Evolution Population ---

/**
 * Evolution population management.
 */
export interface EvolutionPopulation {
  /** All profiles in the population */
  profiles: EvolutionProfile[];
  /** Current generation */
  currentGeneration: number;
  /** Best profile so far */
  bestProfile?: EvolutionProfile;
  /** A/B comparison history */
  comparisons: ABComparisonResult[];
}

/**
 * Create a new population from seed profiles.
 */
export const createPopulation = (seeds: EvolutionProfile[]): EvolutionPopulation => ({
  profiles: seeds,
  currentGeneration: 0,
  comparisons: [],
});

/**
 * Select top profiles by fitness (tournament selection).
 */
export const selectTopProfiles = (
  population: EvolutionPopulation,
  count: number,
): EvolutionProfile[] => {
  const evaluated = population.profiles.filter((p) => p.fitness !== undefined);
  return evaluated
    .sort((a, b) => (b.fitness ?? 0) - (a.fitness ?? 0))
    .slice(0, count);
};

/**
 * Evolve the population to the next generation.
 */
export const evolvePopulation = (
  population: EvolutionPopulation,
  mutationConfig: MutationConfig = DEFAULT_MUTATION_CONFIG,
  eliteCount: number = 2,
  populationSize: number = 10,
): EvolutionPopulation => {
  // Select elite profiles
  const elites = selectTopProfiles(population, eliteCount);

  // Update best profile
  const currentBest = elites[0];
  const bestProfile =
    !population.bestProfile ||
    (currentBest && (currentBest.fitness ?? 0) > (population.bestProfile.fitness ?? 0))
      ? currentBest
      : population.bestProfile;

  // Generate new profiles through mutation and crossover
  const newProfiles: EvolutionProfile[] = [...elites];

  while (newProfiles.length < populationSize) {
    if (elites.length >= 2 && Math.random() < 0.3) {
      // Crossover
      const parentA = pickRandom(elites);
      const parentB = pickRandom(elites.filter((e) => e.id !== parentA.id));
      newProfiles.push(crossoverProfiles(parentA, parentB));
    } else {
      // Mutation
      const parent = pickRandom(elites);
      newProfiles.push(mutateProfile(parent, mutationConfig));
    }
  }

  return {
    profiles: newProfiles,
    currentGeneration: population.currentGeneration + 1,
    bestProfile,
    comparisons: population.comparisons,
  };
};

// --- Effect Operations ---

/**
 * Apply prompt modifiers to a task prompt.
 */
export const applyPromptModifiers = (
  prompt: string,
  modifiers?: PromptModifiers,
): string => {
  if (!modifiers) return prompt;

  let modified = prompt;

  if (modifiers.prefix) {
    modified = `${modifiers.prefix}\n\n${modified}`;
  }

  if (modifiers.context) {
    modified = `${modifiers.context}\n\n${modified}`;
  }

  if (modifiers.suffix) {
    modified = `${modified}\n\n${modifiers.suffix}`;
  }

  if (modifiers.thinkingStyle) {
    const styleHints: Record<string, string> = {
      "step-by-step": "Work through this step by step.",
      holistic: "Consider the big picture first.",
      minimal: "Be concise and direct.",
      verbose: "Explain your reasoning thoroughly.",
    };
    const hint = styleHints[modifiers.thinkingStyle];
    if (hint) {
      modified = `${hint}\n\n${modified}`;
    }
  }

  return modified;
};

/**
 * Create an evolution run effect.
 */
export const runEvolution = (
  seedProfile: EvolutionProfile,
  generations: number,
  evaluator: (profile: EvolutionProfile) => Effect.Effect<TrainingStats, Error>,
  options?: {
    populationSize?: number;
    eliteCount?: number;
    mutationConfig?: MutationConfig;
  },
): Effect.Effect<EvolutionPopulation, Error> =>
  Effect.gen(function* () {
    const populationSize = options?.populationSize ?? 5;
    const eliteCount = options?.eliteCount ?? 2;
    const mutationConfig = options?.mutationConfig ?? DEFAULT_MUTATION_CONFIG;

    // Create initial population
    let population = createPopulation([seedProfile]);

    // Expand to full population size
    while (population.profiles.length < populationSize) {
      population.profiles.push(mutateProfile(seedProfile, mutationConfig));
    }

    // Run evolution for specified generations
    for (let gen = 0; gen < generations; gen++) {
      // Evaluate each profile
      for (const profile of population.profiles) {
        if (profile.fitness === undefined) {
          const stats = yield* evaluator(profile);
          const updated = updateProfileFitness(profile, stats);
          // Update in place
          const idx = population.profiles.findIndex((p) => p.id === profile.id);
          if (idx >= 0) {
            population.profiles[idx] = updated;
          }
        }
      }

      // Evolve to next generation (unless last iteration)
      if (gen < generations - 1) {
        population = evolvePopulation(population, mutationConfig, eliteCount, populationSize);
      }
    }

    return population;
  });
