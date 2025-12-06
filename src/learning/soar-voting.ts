/**
 * SOAR Weighted Majority Voting
 *
 * Implements SOAR's voting mechanism for selecting final predictions.
 * Combines multiple program outputs using weighted voting.
 *
 * Weight formula: weight = count + K × training_accuracy
 * where K = 1000 (default) balances frequency vs quality
 *
 * This creates an ensemble effect:
 * - High-accuracy programs have more influence
 * - Common outputs (high count) are preferred
 * - Breaks ties using quality scores
 */

import * as S from "effect/Schema";
import { Effect, Context, Layer } from "effect";

// --- Vote Configuration ---

export interface VotingConfig {
  /** Weight multiplier for training accuracy (default: 1000) */
  accuracyMultiplier: number;

  /** Minimum votes required for valid result */
  minVotes: number;

  /** Tie-breaking strategy */
  tieBreaker: "accuracy" | "count" | "random";

  /** Confidence threshold for accepting result */
  confidenceThreshold: number;

  /** Enable skill-based voting weight adjustment */
  enableSkillWeighting: boolean;
}

export const DEFAULT_VOTING_CONFIG: VotingConfig = {
  accuracyMultiplier: 1000,
  minVotes: 1,
  tieBreaker: "accuracy",
  confidenceThreshold: 0.1, // At least 10% confidence
  enableSkillWeighting: true,
};

// --- Vote Schema ---

export const Vote = S.Struct({
  /** The output/prediction being voted for */
  output: S.Unknown,

  /** Stringified output for grouping */
  outputKey: S.String,

  /** Program/code that produced this output */
  program: S.String,

  /** Training accuracy of this program */
  trainingAccuracy: S.Number,

  /** Skill ID if applicable */
  skillId: S.optional(S.String),

  /** Skill confidence if applicable */
  skillConfidence: S.optional(S.Number),
});
export type Vote = S.Schema.Type<typeof Vote>;

// --- Voting Result ---

export const VotingResult = S.Struct({
  /** Winning output */
  winner: S.Unknown,

  /** Winner's output key */
  winnerKey: S.String,

  /** Total weight for winner */
  winnerWeight: S.Number,

  /** Confidence (winner weight / total weight) */
  confidence: S.Number,

  /** All candidates with their weights */
  candidates: S.Array(
    S.Struct({
      outputKey: S.String,
      output: S.Unknown,
      weight: S.Number,
      voteCount: S.Number,
      averageAccuracy: S.Number,
    }),
  ),

  /** Total votes cast */
  totalVotes: S.Number,

  /** Whether result is considered valid */
  isValid: S.Boolean,

  /** Voting timestamp */
  votedAt: S.String,
});
export type VotingResult = S.Schema.Type<typeof VotingResult>;

// --- Output Normalization ---

/**
 * Normalize output to a string key for grouping.
 * Handles common output formats consistently.
 */
export const normalizeOutputKey = (output: unknown): string => {
  if (output === null) return "null";
  if (output === undefined) return "undefined";

  if (typeof output === "string") {
    // Normalize whitespace
    return output.trim().replace(/\s+/g, " ");
  }

  if (typeof output === "number" || typeof output === "boolean") {
    return String(output);
  }

  if (Array.isArray(output)) {
    // Sort arrays for consistent comparison (when order doesn't matter)
    // Note: This may need task-specific handling
    return JSON.stringify(output);
  }

  if (typeof output === "object") {
    // Sort object keys for consistent comparison
    const sorted = Object.keys(output as Record<string, unknown>)
      .sort()
      .reduce(
        (acc, key) => {
          acc[key] = (output as Record<string, unknown>)[key];
          return acc;
        },
        {} as Record<string, unknown>,
      );
    return JSON.stringify(sorted);
  }

  return JSON.stringify(output);
};

// --- Weight Calculation ---

/**
 * Calculate vote weight for a single vote.
 * weight = 1 + K × training_accuracy (where K = accuracyMultiplier)
 *
 * The +1 ensures every vote has baseline weight.
 */
export const calculateVoteWeight = (vote: Vote, config: VotingConfig): number => {
  let weight = 1 + config.accuracyMultiplier * vote.trainingAccuracy;

  // Apply skill weighting if enabled
  if (config.enableSkillWeighting && vote.skillConfidence !== undefined) {
    weight *= 0.5 + 0.5 * vote.skillConfidence; // Range: 0.5x to 1x
  }

  return weight;
};

// --- Voting Functions ---

/**
 * Group votes by output and calculate weights.
 */
export const groupVotes = (
  votes: Vote[],
  config: VotingConfig,
): Map<
  string,
  {
    output: unknown;
    weight: number;
    votes: Vote[];
    averageAccuracy: number;
  }
> => {
  const groups = new Map<
    string,
    {
      output: unknown;
      weight: number;
      votes: Vote[];
      averageAccuracy: number;
    }
  >();

  for (const vote of votes) {
    const key = normalizeOutputKey(vote.output);
    const existing = groups.get(key);

    if (existing) {
      const voteWeight = calculateVoteWeight(vote, config);
      const newVotes = [...existing.votes, vote];
      const totalAccuracy = newVotes.reduce((sum, v) => sum + v.trainingAccuracy, 0);

      groups.set(key, {
        output: existing.output,
        weight: existing.weight + voteWeight,
        votes: newVotes,
        averageAccuracy: totalAccuracy / newVotes.length,
      });
    } else {
      groups.set(key, {
        output: vote.output,
        weight: calculateVoteWeight(vote, config),
        votes: [vote],
        averageAccuracy: vote.trainingAccuracy,
      });
    }
  }

  return groups;
};

/**
 * Break ties between candidates with equal weight.
 */
const breakTie = (
  candidates: Array<{
    key: string;
    output: unknown;
    weight: number;
    votes: Vote[];
    averageAccuracy: number;
  }>,
  config: VotingConfig,
): string => {
  if (candidates.length === 0) return "";
  if (candidates.length === 1) return candidates[0]!.key;

  switch (config.tieBreaker) {
    case "accuracy": {
      // Prefer higher average accuracy
      let best = candidates[0]!;
      for (const c of candidates) {
        if (c.averageAccuracy > best.averageAccuracy) {
          best = c;
        }
      }
      return best.key;
    }
    case "count": {
      // Prefer more votes
      let best = candidates[0]!;
      for (const c of candidates) {
        if (c.votes.length > best.votes.length) {
          best = c;
        }
      }
      return best.key;
    }
    case "random":
    default:
      // Random selection
      const idx = Math.floor(Math.random() * candidates.length);
      return candidates[idx]!.key;
  }
};

/**
 * Perform weighted majority voting.
 */
export const vote = (votes: Vote[], config: VotingConfig = DEFAULT_VOTING_CONFIG): VotingResult => {
  if (votes.length < config.minVotes) {
    return {
      winner: null,
      winnerKey: "",
      winnerWeight: 0,
      confidence: 0,
      candidates: [],
      totalVotes: votes.length,
      isValid: false,
      votedAt: new Date().toISOString(),
    };
  }

  const groups = groupVotes(votes, config);
  const totalWeight = Array.from(groups.values()).reduce((sum, g) => sum + g.weight, 0);

  // Find candidates with maximum weight
  let maxWeight = 0;
  for (const group of Array.from(groups.values())) {
    if (group.weight > maxWeight) {
      maxWeight = group.weight;
    }
  }

  const topCandidates = Array.from(groups.entries())
    .filter(([_, g]) => g.weight === maxWeight)
    .map(([key, g]) => ({
      key,
      output: g.output,
      weight: g.weight,
      votes: g.votes,
      averageAccuracy: g.averageAccuracy,
    }));

  // Break tie if necessary
  const winnerKey = breakTie(topCandidates, config);
  const winnerGroup = groups.get(winnerKey)!;

  const confidence = totalWeight > 0 ? winnerGroup.weight / totalWeight : 0;

  // Build candidates list sorted by weight
  const candidates = Array.from(groups.entries())
    .map(([key, g]) => ({
      outputKey: key,
      output: g.output,
      weight: g.weight,
      voteCount: g.votes.length,
      averageAccuracy: g.averageAccuracy,
    }))
    .sort((a, b) => b.weight - a.weight);

  return {
    winner: winnerGroup.output,
    winnerKey,
    winnerWeight: winnerGroup.weight,
    confidence,
    candidates,
    totalVotes: votes.length,
    isValid: confidence >= config.confidenceThreshold,
    votedAt: new Date().toISOString(),
  };
};

// --- Ensemble Voting ---

/**
 * Create votes from program outputs.
 */
export const createVotes = (
  outputs: Array<{
    output: unknown;
    program: string;
    trainingAccuracy: number;
    skillId?: string;
    skillConfidence?: number;
  }>,
): Vote[] =>
  outputs.map((o) => ({
    output: o.output,
    outputKey: normalizeOutputKey(o.output),
    program: o.program,
    trainingAccuracy: o.trainingAccuracy,
    skillId: o.skillId,
    skillConfidence: o.skillConfidence,
  }));

/**
 * Perform ensemble voting across multiple program outputs.
 */
export const ensembleVote = (
  outputs: Array<{
    output: unknown;
    program: string;
    trainingAccuracy: number;
    skillId?: string;
    skillConfidence?: number;
  }>,
  config: VotingConfig = DEFAULT_VOTING_CONFIG,
): VotingResult => {
  const votes = createVotes(outputs);
  return vote(votes, config);
};

// --- Service Interface ---

export interface IVotingService {
  /** Perform weighted majority voting */
  readonly vote: (votes: Vote[]) => Effect.Effect<VotingResult, never>;

  /** Create votes from outputs */
  readonly createVotes: (
    outputs: Array<{
      output: unknown;
      program: string;
      trainingAccuracy: number;
      skillId?: string;
      skillConfidence?: number;
    }>,
  ) => Effect.Effect<Vote[], never>;

  /** Perform ensemble voting */
  readonly ensembleVote: (
    outputs: Array<{
      output: unknown;
      program: string;
      trainingAccuracy: number;
      skillId?: string;
      skillConfidence?: number;
    }>,
  ) => Effect.Effect<VotingResult, never>;

  /** Calculate weight for a single vote */
  readonly calculateWeight: (vote: Vote) => Effect.Effect<number, never>;

  /** Normalize output to key */
  readonly normalizeOutput: (output: unknown) => Effect.Effect<string, never>;

  /** Get current config */
  readonly getConfig: () => Effect.Effect<VotingConfig, never>;

  /** Update config */
  readonly updateConfig: (updates: Partial<VotingConfig>) => Effect.Effect<VotingConfig, never>;

  /** Get voting statistics */
  readonly getStats: () => Effect.Effect<VotingStats, never>;
}

export interface VotingStats {
  totalVotingSessions: number;
  totalVotesCast: number;
  averageConfidence: number;
  averageCandidates: number;
  validResultRate: number;
}

// --- Service Tag ---

export class VotingService extends Context.Tag("VotingService")<VotingService, IVotingService>() {}

// --- Service Implementation ---

const makeVotingService = (initialConfig: VotingConfig = DEFAULT_VOTING_CONFIG): IVotingService => {
  let config = { ...initialConfig };
  let stats: VotingStats = {
    totalVotingSessions: 0,
    totalVotesCast: 0,
    averageConfidence: 0,
    averageCandidates: 0,
    validResultRate: 0,
  };
  let validCount = 0;

  const updateStats = (result: VotingResult): void => {
    stats.totalVotingSessions++;
    stats.totalVotesCast += result.totalVotes;

    // Update average confidence
    const prevConfSum = stats.averageConfidence * (stats.totalVotingSessions - 1);
    stats.averageConfidence = (prevConfSum + result.confidence) / stats.totalVotingSessions;

    // Update average candidates
    const prevCandSum = stats.averageCandidates * (stats.totalVotingSessions - 1);
    stats.averageCandidates = (prevCandSum + result.candidates.length) / stats.totalVotingSessions;

    // Update valid result rate
    if (result.isValid) validCount++;
    stats.validResultRate = validCount / stats.totalVotingSessions;
  };

  return {
    vote: (votes) =>
      Effect.sync(() => {
        const result = vote(votes, config);
        updateStats(result);
        return result;
      }),

    createVotes: (outputs) => Effect.sync(() => createVotes(outputs)),

    ensembleVote: (outputs) =>
      Effect.sync(() => {
        const result = ensembleVote(outputs, config);
        updateStats(result);
        return result;
      }),

    calculateWeight: (v) => Effect.sync(() => calculateVoteWeight(v, config)),

    normalizeOutput: (output) => Effect.sync(() => normalizeOutputKey(output)),

    getConfig: () => Effect.sync(() => ({ ...config })),

    updateConfig: (updates) =>
      Effect.sync(() => {
        config = { ...config, ...updates };
        return { ...config };
      }),

    getStats: () => Effect.sync(() => ({ ...stats })),
  };
};

// --- Layer ---

export const VotingServiceLive: Layer.Layer<VotingService, never, never> = Layer.succeed(
  VotingService,
  makeVotingService(),
);

/**
 * Create a VotingService layer with custom config.
 */
export const makeVotingServiceLayer = (
  config: Partial<VotingConfig> = {},
): Layer.Layer<VotingService, never, never> =>
  Layer.succeed(VotingService, makeVotingService({ ...DEFAULT_VOTING_CONFIG, ...config }));
