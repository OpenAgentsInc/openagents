/**
 * Skill Evolution Service Tests
 */

import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { Effect, Layer } from "effect";
import { rmSync, mkdirSync } from "fs";
import { join } from "path";
import {
  SkillEvolutionService,
  SkillEvolutionError,
  makeSkillEvolutionLayer,
  DEFAULT_EVOLUTION_CONFIG,
  type EvolutionResult,
  type EvolutionReport,
} from "./evolution.js";
import { SkillStore, makeSkillStoreLayer } from "./store.js";
import { createSkill, type Skill } from "./schema.js";

const TEST_PROJECT_ROOT = "/tmp/skill-evolution-test";
const TEST_SKILLS_DIR = join(TEST_PROJECT_ROOT, ".openagents", "skills");

// Helper to create a test layer
const makeTestLayer = () => {
  const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);
  const evolutionLayer = makeSkillEvolutionLayer();
  return Layer.provide(evolutionLayer, storeLayer);
};

// Helper to run with test layer
const runWithTestLayer = <A, E>(
  effect: Effect.Effect<A, E, SkillEvolutionService>,
) =>
  Effect.runPromise(
    effect.pipe(Effect.provide(makeTestLayer())),
  );

// Helper to create skills with specific stats
const createTestSkill = (
  name: string,
  status: "active" | "draft" | "archived" | "failed",
  successRate?: number,
  usageCount?: number,
): Skill => {
  const skill = createSkill({
    name,
    description: `Test skill: ${name}`,
    code: `// ${name} code`,
    category: "testing",
    status,
    source: "manual",
  });
  if (successRate !== undefined) {
    (skill as any).successRate = successRate;
  }
  if (usageCount !== undefined) {
    (skill as any).usageCount = usageCount;
  }
  return skill;
};

describe("Skill Evolution Configuration", () => {
  test("DEFAULT_EVOLUTION_CONFIG has expected values", () => {
    expect(DEFAULT_EVOLUTION_CONFIG.promotionThreshold).toBe(0.7);
    expect(DEFAULT_EVOLUTION_CONFIG.promotionMinUsage).toBe(3);
    expect(DEFAULT_EVOLUTION_CONFIG.demotionThreshold).toBe(0.4);
    expect(DEFAULT_EVOLUTION_CONFIG.demotionMinUsage).toBe(5);
    expect(DEFAULT_EVOLUTION_CONFIG.pruneThreshold).toBe(0.2);
    expect(DEFAULT_EVOLUTION_CONFIG.pruneMinUsage).toBe(10);
    expect(DEFAULT_EVOLUTION_CONFIG.maxUnusedAgeDays).toBe(30);
  });
});

describe("Skill Evolution Service", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch { }
    mkdirSync(TEST_SKILLS_DIR, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch { }
  });

  describe("Skill Promotion", () => {
    test("promotes draft skills with high success rate", async () => {
      // Add a draft skill with good stats
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        const skill = createTestSkill("good-draft", "draft", 0.8, 5);
        yield* store.add(skill);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      // Run promotion
      const actions = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.promoteSkills();
        }),
      );

      expect(actions.length).toBe(1);
      expect(actions[0].action).toBe("promoted");
      expect(actions[0].previousStatus).toBe("draft");
      expect(actions[0].newStatus).toBe("active");
    });

    test("does not promote draft skills with insufficient usage", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        // High success rate but only 1 use
        const skill = createTestSkill("new-draft", "draft", 0.9, 1);
        yield* store.add(skill);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const actions = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.promoteSkills();
        }),
      );

      expect(actions.length).toBe(0);
    });

    test("does not promote draft skills with low success rate", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        // Low success rate with enough usage
        const skill = createTestSkill("bad-draft", "draft", 0.3, 10);
        yield* store.add(skill);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const actions = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.promoteSkills();
        }),
      );

      expect(actions.length).toBe(0);
    });
  });

  describe("Skill Demotion", () => {
    test("demotes active skills with low success rate", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        // Active skill that's performing poorly
        const skill = createTestSkill("failing-active", "active", 0.2, 10);
        yield* store.add(skill);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const actions = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.demoteSkills();
        }),
      );

      expect(actions.length).toBe(1);
      expect(actions[0].action).toBe("demoted");
      expect(actions[0].previousStatus).toBe("active");
      expect(actions[0].newStatus).toBe("draft");
    });

    test("does not demote active skills with good success rate", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        const skill = createTestSkill("good-active", "active", 0.8, 10);
        yield* store.add(skill);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const actions = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.demoteSkills();
        }),
      );

      expect(actions.length).toBe(0);
    });
  });

  describe("Skill Pruning", () => {
    test("prunes skills with very low success rate", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        // Very low performing skill with lots of usage
        const skill = createTestSkill("terrible-skill", "active", 0.1, 15);
        yield* store.add(skill);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const actions = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.pruneSkills();
        }),
      );

      expect(actions.length).toBe(1);
      expect(actions[0].action).toBe("pruned");
      expect(actions[0].newStatus).toBe("archived");
    });
  });

  describe("Full Evolution Cycle", () => {
    test("evolveLibrary runs all phases", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        // Draft skill ready for promotion
        yield* store.add(createTestSkill("promote-me", "draft", 0.85, 5));
        // Active skill that should be demoted (but not pruned - success rate above prune threshold)
        yield* store.add(createTestSkill("demote-me", "active", 0.35, 10));
        // Draft skill that should be pruned (low performance, high usage)
        yield* store.add(createTestSkill("prune-me", "draft", 0.1, 15));
        // Good skill that should be unchanged
        yield* store.add(createTestSkill("keep-me", "active", 0.9, 20));
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const result: EvolutionResult = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.evolveLibrary();
        }),
      );

      expect(result.promoted.length).toBe(1);
      expect(result.demoted.length).toBe(1);
      expect(result.pruned.length).toBe(1);
      expect(result.unchanged).toBe(1);
      expect(result.totalEvaluated).toBe(4);
      expect(result.durationMs).toBeGreaterThanOrEqual(0);
      expect(result.timestamp).toBeDefined();
    });
  });

  describe("Evolution Report", () => {
    test("getEvolutionReport provides library health overview", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        yield* store.add(createTestSkill("active-1", "active", 0.9, 10));
        yield* store.add(createTestSkill("active-2", "active", 0.6, 5));
        yield* store.add(createTestSkill("draft-1", "draft", 0.8, 4));
        yield* store.add(createTestSkill("draft-2", "draft", 0.3, 2));
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const report: EvolutionReport = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.getEvolutionReport();
        }),
      );

      expect(report.totalSkills).toBe(4);
      expect(report.byStatus.active).toBe(2);
      expect(report.byStatus.draft).toBe(2);
      expect(report.topPerformers.length).toBeGreaterThan(0);
      expect(report.averageSuccessRate).toBeGreaterThan(0);
    });
  });

  describe("Stats Tracking", () => {
    test("updateSkillStats updates skill with EMA", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);
      const skillId = "test-stats-skill";

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        const skill = createTestSkill("stats-test", "active", 0.5, 5);
        (skill as any).id = skillId; // Override ID
        yield* store.add(skill);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      // Record a success
      const updated = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.updateSkillStats(skillId, true);
        }),
      );

      expect(updated).not.toBeNull();
      expect(updated!.usageCount).toBe(6);
      expect(updated!.successRate).toBeGreaterThan(0.5); // Should increase
      expect(updated!.lastUsed).toBeDefined();
    });

    test("updateSkillStats returns null for non-existent skill", async () => {
      const result = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.updateSkillStats("non-existent", true);
        }),
      );

      expect(result).toBeNull();
    });

    test("batchUpdateStats updates multiple skills", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);
      const skill1Id = "batch-skill-1";
      const skill2Id = "batch-skill-2";

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        const skill1 = createTestSkill("batch-1", "active", 0.5, 0);
        (skill1 as any).id = skill1Id;
        const skill2 = createTestSkill("batch-2", "active", 0.5, 0);
        (skill2 as any).id = skill2Id;
        yield* store.add(skill1);
        yield* store.add(skill2);
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const count = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.batchUpdateStats([
            { skillId: skill1Id, success: true },
            { skillId: skill2Id, success: false },
            { skillId: "non-existent", success: true },
          ]);
        }),
      );

      expect(count).toBe(2); // Only 2 existing skills updated
    });
  });

  describe("Performance Ranking", () => {
    test("getByPerformance returns skills sorted by success rate", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        yield* store.add(createTestSkill("low-perf", "active", 0.3, 5));
        yield* store.add(createTestSkill("high-perf", "active", 0.9, 10));
        yield* store.add(createTestSkill("mid-perf", "active", 0.6, 7));
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const skills = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.getByPerformance({ limit: 10 });
        }),
      );

      expect(skills.length).toBe(3);
      expect(skills[0].successRate).toBe(0.9);
      expect(skills[1].successRate).toBe(0.6);
      expect(skills[2].successRate).toBe(0.3);
    });

    test("getByPerformance supports ascending order", async () => {
      const storeLayer = makeSkillStoreLayer(TEST_PROJECT_ROOT);

      const setup = Effect.gen(function* () {
        const store = yield* SkillStore;
        yield* store.add(createTestSkill("low", "active", 0.3, 5));
        yield* store.add(createTestSkill("high", "active", 0.9, 10));
      }).pipe(Effect.provide(storeLayer));

      await Effect.runPromise(setup);

      const skills = await runWithTestLayer(
        Effect.gen(function* () {
          const service = yield* SkillEvolutionService;
          return yield* service.getByPerformance({ ascending: true });
        }),
      );

      expect(skills[0].successRate).toBe(0.3); // Lowest first
    });
  });
});

describe("SkillEvolutionError", () => {
  test("creates error with correct properties", () => {
    const error = new SkillEvolutionError(
      "evolution_failed",
      "Evolution cycle failed",
    );

    expect(error.reason).toBe("evolution_failed");
    expect(error.message).toBe("Evolution cycle failed");
    expect(error._tag).toBe("SkillEvolutionError");
  });

  test("creates error with cause", () => {
    const cause = new Error("Root cause");
    const error = new SkillEvolutionError(
      "store_error",
      "Store operation failed",
      cause,
    );

    expect(error.cause).toBe(cause);
  });
});
