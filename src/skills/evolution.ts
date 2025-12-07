/**
 * Skill Evolution Service
 *
 * Manages skill lifecycle: promotion, demotion, pruning, and evolution tracking.
 *
 * Features:
 * - Promote draft skills to active when they prove successful
 * - Demote active skills that start failing
 * - Prune skills with consistently low performance
 * - Track evolution history for analysis
 *
 * Based on Voyager research: skills should evolve based on performance.
 */

import { Effect, Context, Layer } from "effect";
import { SkillStore, type SkillStoreError } from "./store.js";
import { type Skill, type SkillStatus } from "./schema.js";

// --- Configuration ---

export interface SkillEvolutionConfig {
  /** Minimum success rate to promote draft → active (default: 0.7) */
  promotionThreshold: number;
  /** Minimum usage count before promotion eligibility (default: 3) */
  promotionMinUsage: number;
  /** Success rate below which active skills are demoted (default: 0.4) */
  demotionThreshold: number;
  /** Minimum usage count before demotion eligibility (default: 5) */
  demotionMinUsage: number;
  /** Success rate below which skills are pruned (default: 0.2) */
  pruneThreshold: number;
  /** Minimum usage count before prune eligibility (default: 10) */
  pruneMinUsage: number;
  /** Maximum age in days for unused skills before archiving (default: 30) */
  maxUnusedAgeDays: number;
}

export const DEFAULT_EVOLUTION_CONFIG: SkillEvolutionConfig = {
  promotionThreshold: 0.7,
  promotionMinUsage: 3,
  demotionThreshold: 0.4,
  demotionMinUsage: 5,
  pruneThreshold: 0.2,
  pruneMinUsage: 10,
  maxUnusedAgeDays: 30,
};

// --- Error Types ---

export class SkillEvolutionError extends Error {
  readonly _tag = "SkillEvolutionError";
  constructor(
    readonly reason: "store_error" | "config_error" | "evolution_failed",
    override readonly message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "SkillEvolutionError";
  }

  static fromStoreError(e: SkillStoreError): SkillEvolutionError {
    return new SkillEvolutionError("store_error", e.message, e);
  }
}

// --- Evolution Result Types ---

/**
 * Result of a single skill evolution action.
 */
export interface SkillEvolutionAction {
  skillId: string;
  skillName: string;
  action: "promoted" | "demoted" | "pruned" | "unchanged";
  previousStatus: SkillStatus;
  newStatus: SkillStatus;
  successRate: number;
  usageCount: number;
  reason: string;
}

/**
 * Result of an evolution run.
 */
export interface EvolutionResult {
  /** Timestamp of the evolution run */
  timestamp: string;
  /** Skills that were promoted */
  promoted: SkillEvolutionAction[];
  /** Skills that were demoted */
  demoted: SkillEvolutionAction[];
  /** Skills that were pruned (archived) */
  pruned: SkillEvolutionAction[];
  /** Skills that were unchanged */
  unchanged: number;
  /** Total skills evaluated */
  totalEvaluated: number;
  /** Duration of evolution run */
  durationMs: number;
}

/**
 * Evolution report showing library health.
 */
export interface EvolutionReport {
  /** Total skills in library */
  totalSkills: number;
  /** Skills by status */
  byStatus: Record<SkillStatus, number>;
  /** Skills at risk of demotion (active but declining) */
  atRiskOfDemotion: Skill[];
  /** Skills eligible for promotion (draft but performing well) */
  eligibleForPromotion: Skill[];
  /** Skills at risk of pruning (low performers) */
  atRiskOfPruning: Skill[];
  /** Top performers */
  topPerformers: Skill[];
  /** Recently used skills */
  recentlyUsed: Skill[];
  /** Unused skills (potential stale) */
  unused: Skill[];
  /** Average success rate */
  averageSuccessRate: number;
  /** Average usage count */
  averageUsageCount: number;
}

// --- Service Interface ---

export interface ISkillEvolutionService {
  /**
   * Promote eligible draft skills to active.
   */
  readonly promoteSkills: () => Effect.Effect<SkillEvolutionAction[], SkillEvolutionError>;

  /**
   * Demote underperforming active skills to draft.
   */
  readonly demoteSkills: () => Effect.Effect<SkillEvolutionAction[], SkillEvolutionError>;

  /**
   * Prune (archive) consistently failing skills.
   */
  readonly pruneSkills: () => Effect.Effect<SkillEvolutionAction[], SkillEvolutionError>;

  /**
   * Run full evolution cycle: promote, demote, prune.
   */
  readonly evolveLibrary: () => Effect.Effect<EvolutionResult, SkillEvolutionError>;

  /**
   * Get evolution report showing library health.
   */
  readonly getEvolutionReport: () => Effect.Effect<EvolutionReport, SkillEvolutionError>;

  /**
   * Update skill stats after usage.
   * Uses exponential moving average for smooth stat updates.
   */
  readonly updateSkillStats: (
    skillId: string,
    success: boolean,
  ) => Effect.Effect<Skill | null, SkillEvolutionError>;

  /**
   * Batch update stats for multiple skills.
   */
  readonly batchUpdateStats: (
    updates: Array<{ skillId: string; success: boolean }>,
  ) => Effect.Effect<number, SkillEvolutionError>;

  /**
   * Get skills sorted by performance.
   */
  readonly getByPerformance: (options?: {
    limit?: number;
    ascending?: boolean;
  }) => Effect.Effect<Skill[], SkillEvolutionError>;
}

// --- Service Tag ---

export class SkillEvolutionService extends Context.Tag("SkillEvolutionService")<
  SkillEvolutionService,
  ISkillEvolutionService
>() {}

// --- Implementation ---

const makeEvolutionService = (
  config: SkillEvolutionConfig = DEFAULT_EVOLUTION_CONFIG,
): Effect.Effect<ISkillEvolutionService, never, SkillStore> =>
  Effect.gen(function* () {
    const store = yield* SkillStore;
    const mapStoreError = Effect.mapError(SkillEvolutionError.fromStoreError);

    const promoteSkills = (): Effect.Effect<SkillEvolutionAction[], SkillEvolutionError> =>
      Effect.gen(function* () {
        const draftSkills = yield* store.list({ status: ["draft"] }).pipe(mapStoreError);
        const actions: SkillEvolutionAction[] = [];

        for (const skill of draftSkills) {
          const successRate = skill.successRate ?? 0;
          const usageCount = skill.usageCount ?? 0;

          // Check if eligible for promotion
          if (usageCount >= config.promotionMinUsage && successRate >= config.promotionThreshold) {
            const updated: Skill = {
              ...skill,
              status: "active",
              updatedAt: new Date().toISOString(),
            };

            yield* store.update(updated).pipe(mapStoreError);

            actions.push({
              skillId: skill.id,
              skillName: skill.name,
              action: "promoted",
              previousStatus: "draft",
              newStatus: "active",
              successRate,
              usageCount,
              reason: `Success rate ${(successRate * 100).toFixed(0)}% >= ${(config.promotionThreshold * 100).toFixed(0)}% with ${usageCount} uses`,
            });
          }
        }

        return actions;
      });

    const demoteSkills = (): Effect.Effect<SkillEvolutionAction[], SkillEvolutionError> =>
      Effect.gen(function* () {
        const activeSkills = yield* store.list({ status: ["active"] }).pipe(mapStoreError);
        const actions: SkillEvolutionAction[] = [];

        for (const skill of activeSkills) {
          const successRate = skill.successRate ?? 1; // Assume good if never used
          const usageCount = skill.usageCount ?? 0;

          // Check if should be demoted
          if (usageCount >= config.demotionMinUsage && successRate < config.demotionThreshold) {
            const updated: Skill = {
              ...skill,
              status: "draft",
              updatedAt: new Date().toISOString(),
            };

            yield* store.update(updated).pipe(mapStoreError);

            actions.push({
              skillId: skill.id,
              skillName: skill.name,
              action: "demoted",
              previousStatus: "active",
              newStatus: "draft",
              successRate,
              usageCount,
              reason: `Success rate ${(successRate * 100).toFixed(0)}% < ${(config.demotionThreshold * 100).toFixed(0)}% with ${usageCount} uses`,
            });
          }
        }

        return actions;
      });

    const pruneSkillsFn = (): Effect.Effect<SkillEvolutionAction[], SkillEvolutionError> =>
      Effect.gen(function* () {
        const allSkills = yield* store.list({ status: ["active", "draft"] }).pipe(mapStoreError);
        const actions: SkillEvolutionAction[] = [];
        const now = Date.now();

        for (const skill of allSkills) {
          const successRate = skill.successRate ?? 1;
          const usageCount = skill.usageCount ?? 0;
          const lastUsedTime = skill.lastUsed ? new Date(skill.lastUsed).getTime() : 0;
          const daysSinceUse = (now - lastUsedTime) / (1000 * 60 * 60 * 24);

          let shouldPrune = false;
          let reason = "";

          // Prune if low performance with enough usage
          if (usageCount >= config.pruneMinUsage && successRate < config.pruneThreshold) {
            shouldPrune = true;
            reason = `Success rate ${(successRate * 100).toFixed(0)}% < ${(config.pruneThreshold * 100).toFixed(0)}% with ${usageCount} uses`;
          }
          // Prune if unused for too long
          else if (lastUsedTime > 0 && daysSinceUse > config.maxUnusedAgeDays && usageCount < 3) {
            shouldPrune = true;
            reason = `Unused for ${Math.floor(daysSinceUse)} days with only ${usageCount} uses`;
          }

          if (shouldPrune) {
            yield* store.archive(skill.id).pipe(mapStoreError);

            actions.push({
              skillId: skill.id,
              skillName: skill.name,
              action: "pruned",
              previousStatus: skill.status,
              newStatus: "archived",
              successRate,
              usageCount,
              reason,
            });
          }
        }

        return actions;
      });

    const evolveLibrary = (): Effect.Effect<EvolutionResult, SkillEvolutionError> =>
      Effect.gen(function* () {
        const startTime = Date.now();
        const allSkills = yield* store.list().pipe(mapStoreError);

        // Run evolution phases
        const promoted = yield* promoteSkills();
        const demoted = yield* demoteSkills();
        const pruned = yield* pruneSkillsFn();

        const changedCount = promoted.length + demoted.length + pruned.length;

        return {
          timestamp: new Date().toISOString(),
          promoted,
          demoted,
          pruned,
          unchanged: allSkills.length - changedCount,
          totalEvaluated: allSkills.length,
          durationMs: Date.now() - startTime,
        };
      });

    const getEvolutionReport = (): Effect.Effect<EvolutionReport, SkillEvolutionError> =>
      Effect.gen(function* () {
        const allSkills = yield* store.list().pipe(mapStoreError);

        // Count by status
        const byStatus: Record<SkillStatus, number> = {
          active: 0,
          draft: 0,
          archived: 0,
          failed: 0,
        };

        for (const skill of allSkills) {
          byStatus[skill.status]++;
        }

        const activeAndDraft = allSkills.filter(
          (s) => s.status === "active" || s.status === "draft",
        );

        // At risk of demotion (active but declining)
        const atRiskOfDemotion = activeAndDraft.filter(
          (s) =>
            s.status === "active" &&
            (s.usageCount ?? 0) >= config.demotionMinUsage * 0.5 &&
            (s.successRate ?? 1) < config.demotionThreshold * 1.5,
        );

        // Eligible for promotion (draft but performing well)
        const eligibleForPromotion = activeAndDraft.filter(
          (s) =>
            s.status === "draft" &&
            (s.usageCount ?? 0) >= config.promotionMinUsage * 0.5 &&
            (s.successRate ?? 0) >= config.promotionThreshold * 0.8,
        );

        // At risk of pruning
        const atRiskOfPruning = activeAndDraft.filter(
          (s) =>
            (s.usageCount ?? 0) >= config.pruneMinUsage * 0.5 &&
            (s.successRate ?? 1) < config.pruneThreshold * 1.5,
        );

        // Top performers (sorted by success rate then usage)
        const topPerformers = [...activeAndDraft]
          .filter((s) => (s.usageCount ?? 0) >= 2)
          .sort((a, b) => {
            const rateA = a.successRate ?? 0;
            const rateB = b.successRate ?? 0;
            if (rateB !== rateA) return rateB - rateA;
            return (b.usageCount ?? 0) - (a.usageCount ?? 0);
          })
          .slice(0, 10);

        // Recently used (last 7 days)
        const weekAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
        const recentlyUsed = activeAndDraft.filter(
          (s) => s.lastUsed && new Date(s.lastUsed).getTime() > weekAgo,
        );

        // Unused skills (never used or very old)
        const monthAgo = Date.now() - 30 * 24 * 60 * 60 * 1000;
        const unused = activeAndDraft.filter(
          (s) =>
            !s.lastUsed ||
            (new Date(s.lastUsed).getTime() < monthAgo && (s.usageCount ?? 0) < 3),
        );

        // Calculate averages
        const withUsage = activeAndDraft.filter((s) => (s.usageCount ?? 0) > 0);
        const averageSuccessRate =
          withUsage.length > 0
            ? withUsage.reduce((sum, s) => sum + (s.successRate ?? 0), 0) / withUsage.length
            : 0;
        const averageUsageCount =
          activeAndDraft.length > 0
            ? activeAndDraft.reduce((sum, s) => sum + (s.usageCount ?? 0), 0) / activeAndDraft.length
            : 0;

        return {
          totalSkills: allSkills.length,
          byStatus,
          atRiskOfDemotion,
          eligibleForPromotion,
          atRiskOfPruning,
          topPerformers,
          recentlyUsed,
          unused,
          averageSuccessRate,
          averageUsageCount,
        };
      });

    const updateSkillStats = (
      skillId: string,
      success: boolean,
    ): Effect.Effect<Skill | null, SkillEvolutionError> =>
      Effect.gen(function* () {
        const skill = yield* store.get(skillId).pipe(mapStoreError);

        if (!skill) {
          return null;
        }

        // Update stats using exponential moving average
        const currentRate = skill.successRate ?? 0.5; // Start at 50% if new
        const currentCount = skill.usageCount ?? 0;
        const newCount = currentCount + 1;

        // EMA with alpha that decreases as usage increases (more stable over time)
        const alpha = Math.max(0.1, 1 / (1 + Math.log(newCount + 1)));
        const newRate = alpha * (success ? 1 : 0) + (1 - alpha) * currentRate;

        const updated: Skill = {
          ...skill,
          successRate: newRate,
          usageCount: newCount,
          lastUsed: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        };

        yield* store.update(updated).pipe(mapStoreError);
        return updated;
      });

    const batchUpdateStats = (
      updates: Array<{ skillId: string; success: boolean }>,
    ): Effect.Effect<number, SkillEvolutionError> =>
      Effect.gen(function* () {
        let updatedCount = 0;

        for (const update of updates) {
          const result = yield* updateSkillStats(update.skillId, update.success);
          if (result) {
            updatedCount++;
          }
        }

        return updatedCount;
      });

    const getByPerformance = (
      options?: { limit?: number; ascending?: boolean },
    ): Effect.Effect<Skill[], SkillEvolutionError> =>
      Effect.gen(function* () {
        const skills = yield* store.list({ status: ["active"] }).pipe(mapStoreError);
        const limit = options?.limit ?? 20;
        const ascending = options?.ascending ?? false;

        const sorted = skills
          .filter((s) => (s.usageCount ?? 0) > 0)
          .sort((a, b) => {
            const rateA = a.successRate ?? 0;
            const rateB = b.successRate ?? 0;
            const diff = rateA - rateB;
            return ascending ? diff : -diff;
          });

        return sorted.slice(0, limit);
      });

    return {
      promoteSkills,
      demoteSkills,
      pruneSkills: pruneSkillsFn,
      evolveLibrary,
      getEvolutionReport,
      updateSkillStats,
      batchUpdateStats,
      getByPerformance,
    };
  });

// --- Layer Factory ---

/**
 * Create SkillEvolutionService layer with custom config.
 */
export const makeSkillEvolutionLayer = (
  config: Partial<SkillEvolutionConfig> = {},
): Layer.Layer<SkillEvolutionService, never, SkillStore> => {
  const fullConfig = { ...DEFAULT_EVOLUTION_CONFIG, ...config };
  return Layer.effect(SkillEvolutionService, makeEvolutionService(fullConfig));
};

/**
 * Default SkillEvolutionService layer.
 */
export const SkillEvolutionServiceLive: Layer.Layer<SkillEvolutionService, never, SkillStore> =
  makeSkillEvolutionLayer();
