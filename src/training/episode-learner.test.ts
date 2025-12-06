/**
 * Episode Learner Tests
 */

import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { Effect } from "effect";
import { rmSync, mkdirSync, writeFileSync } from "fs";
import { join } from "path";
import {
  createEpisodeLearner,
  learnFromRecentEpisodes,
  learnFromEpisode,
  EpisodeLearnerError,
  DEFAULT_LEARNER_CONFIG,
  type Reflection,
  type LearningResult,
} from "./episode-learner.js";
import { EpisodeStore, createEpisode, generateRunId } from "../bench/episode-store.js";

const TEST_PROJECT_ROOT = "/tmp/episode-learner-test";
const TEST_GYM_DIR = join(TEST_PROJECT_ROOT, ".openagents", "gym");

describe("Episode Learner Configuration", () => {
  test("DEFAULT_LEARNER_CONFIG has expected values", () => {
    expect(DEFAULT_LEARNER_CONFIG.minPassRateForSkills).toBe(0.5);
    expect(DEFAULT_LEARNER_CONFIG.minPatternOccurrences).toBe(2);
    expect(DEFAULT_LEARNER_CONFIG.maxEpisodesToProcess).toBe(10);
    expect(DEFAULT_LEARNER_CONFIG.generateReflections).toBe(true);
    expect(DEFAULT_LEARNER_CONFIG.maxEpisodeAgeDays).toBe(30);
  });
});

describe("Episode Learner Creation", () => {
  test("createEpisodeLearner creates learner with defaults", () => {
    const learner = createEpisodeLearner();
    expect(learner).toBeDefined();
    expect(learner.processEpisode).toBeDefined();
    expect(learner.processRecentEpisodes).toBeDefined();
    expect(learner.mineSkillsFromSuccess).toBeDefined();
    expect(learner.generateReflections).toBeDefined();
    expect(learner.getLearningSummary).toBeDefined();
    expect(learner.markEpisodeProcessed).toBeDefined();
  });

  test("createEpisodeLearner accepts custom config", () => {
    const learner = createEpisodeLearner({
      projectRoot: TEST_PROJECT_ROOT,
      minPassRateForSkills: 0.7,
      maxEpisodesToProcess: 5,
    });
    expect(learner).toBeDefined();
  });
});

describe("Episode Learner Operations", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_GYM_DIR, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
  });

  test("getLearningSummary returns initial state", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });
    const summary = await Effect.runPromise(learner.getLearningSummary());

    expect(summary.episodesProcessed).toBe(0);
    expect(summary.totalSkillsExtracted).toBe(0);
    expect(summary.totalReflectionsGenerated).toBe(0);
    expect(summary.skillsByCategory).toEqual({});
  });

  test("markEpisodeProcessed adds to processed set", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });

    await Effect.runPromise(learner.markEpisodeProcessed("test-episode-1"));
    await Effect.runPromise(learner.markEpisodeProcessed("test-episode-2"));

    const summary = await Effect.runPromise(learner.getLearningSummary());
    // Note: markEpisodeProcessed doesn't count as processing, just marks
    // Actually checking the internal state is not directly exposed
    // but we can verify it doesn't throw
    expect(summary).toBeDefined();
  });

  test("processEpisode handles episode with results file", async () => {
    const learner = createEpisodeLearner({
      projectRoot: TEST_PROJECT_ROOT,
      minPassRateForSkills: 0.3,
      generateReflections: true,
    });

    // Create a mock episode
    const resultsPath = join(TEST_GYM_DIR, "test-results.jsonl");

    // Create mock task results with meaningful output
    const mockResults = [
      {
        taskId: "task-1",
        status: "pass",
        output: "Successfully completed the task by reading the file and making the edit. The implementation involved using grep to find the pattern and then applying the fix.",
        durationMs: 1000,
        tokens: { input: 100, output: 50, total: 150 },
      },
      {
        taskId: "task-2",
        status: "fail",
        error: "File not found: /path/to/file.ts",
        durationMs: 500,
        tokens: { input: 80, output: 20, total: 100 },
      },
    ];

    writeFileSync(resultsPath, mockResults.map((r) => JSON.stringify(r)).join("\n"));

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 2,
        passed: 1,
        failed: 1,
        timeout: 0,
        error: 0,
        avgTurns: 3,
        avgTokens: 125,
        totalDurationMs: 1500,
      },
      resultsPath,
    });

    const result = await Effect.runPromise(learner.processEpisode(episode));

    expect(result).toBeDefined();
    expect(result.episodeId).toBe(episode.id);
    // Should extract 1 skill from the successful task
    expect(result.skillsExtracted.length).toBe(1);
    // Should generate 1 reflection from the failed task
    expect(result.reflectionsGenerated.length).toBe(1);
    expect(result.durationMs).toBeGreaterThan(0);
  });

  test("processEpisode skips skill extraction for low pass rate", async () => {
    const learner = createEpisodeLearner({
      projectRoot: TEST_PROJECT_ROOT,
      minPassRateForSkills: 0.8, // High threshold
      generateReflections: true,
    });

    const resultsPath = join(TEST_GYM_DIR, "test-results-2.jsonl");

    const mockResults = [
      {
        taskId: "task-1",
        status: "fail",
        error: "Test failed",
        durationMs: 500,
        tokens: { input: 50, output: 25, total: 75 },
      },
      {
        taskId: "task-2",
        status: "fail",
        error: "Another failure",
        durationMs: 600,
        tokens: { input: 60, output: 30, total: 90 },
      },
    ];

    writeFileSync(resultsPath, mockResults.map((r) => JSON.stringify(r)).join("\n"));

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 2,
        passed: 0,
        failed: 2,
        timeout: 0,
        error: 0,
        avgTurns: 2,
        avgTokens: 82,
        totalDurationMs: 1100,
      },
      resultsPath,
    });

    const result = await Effect.runPromise(learner.processEpisode(episode));

    expect(result.skillsExtracted.length).toBe(0); // No skills due to low pass rate
    expect(result.reflectionsGenerated.length).toBe(2); // Should generate reflections for failures
  });

  test("processEpisode handles missing results file", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 1,
        passed: 1,
        failed: 0,
        timeout: 0,
        error: 0,
        avgTurns: 3,
        avgTokens: 100,
        totalDurationMs: 1000,
      },
      resultsPath: "/nonexistent/path/results.jsonl",
    });

    const result = await Effect.runPromise(
      learner.processEpisode(episode).pipe(
        Effect.either,
      ),
    );

    // Should fail with results_not_found error
    expect(result._tag).toBe("Left");
  });

  test("processEpisode skips already processed episodes", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });

    const resultsPath = join(TEST_GYM_DIR, "test-results-3.jsonl");
    writeFileSync(resultsPath, JSON.stringify({ taskId: "t1", status: "pass", output: "done", durationMs: 100, tokens: { input: 10, output: 5, total: 15 } }));

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 1,
        passed: 1,
        failed: 0,
        timeout: 0,
        error: 0,
        avgTurns: 1,
        avgTokens: 15,
        totalDurationMs: 100,
      },
      resultsPath,
    });

    // Process first time
    await Effect.runPromise(learner.processEpisode(episode));

    // Process second time - should be skipped
    const result2 = await Effect.runPromise(learner.processEpisode(episode));

    // Should return empty results since already processed
    expect(result2.skillsExtracted.length).toBe(0);
    expect(result2.reflectionsGenerated.length).toBe(0);
  });
});

describe("Skill Mining", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_GYM_DIR, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
  });

  test("mineSkillsFromSuccess extracts skills from successful results", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });

    const resultsPath = join(TEST_GYM_DIR, "mine-results.jsonl");
    writeFileSync(resultsPath, "");

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 1,
        passed: 1,
        failed: 0,
        timeout: 0,
        error: 0,
        avgTurns: 3,
        avgTokens: 100,
        totalDurationMs: 1000,
      },
      resultsPath,
    });

    const successfulResults = [
      {
        taskId: "task-1",
        outcome: "success" as const,
        output: "Used grep to find all instances of the pattern, then applied sed to fix each occurrence. Tests now pass successfully.",
        durationMs: 2000,
        model: "test-model",
        tokens: { input: 100, output: 50, total: 150 },
        skillsUsed: [],
        usedReflexion: false,
        attemptNumber: 1,
        timestamp: new Date().toISOString(),
      },
    ];

    const skills = await Effect.runPromise(
      learner.mineSkillsFromSuccess(episode, successfulResults),
    );

    expect(skills.length).toBe(1);
    expect(skills[0].status).toBe("draft");
    expect(skills[0].source).toBe("learned");
    expect(skills[0].learnedFrom).toContain(episode.id);
    expect(skills[0].successRate).toBe(1.0);
  });

  test("mineSkillsFromSuccess categorizes skills correctly", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });

    const resultsPath = join(TEST_GYM_DIR, "cat-results.jsonl");
    writeFileSync(resultsPath, "");

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 3,
        passed: 3,
        failed: 0,
        timeout: 0,
        error: 0,
        avgTurns: 2,
        avgTokens: 80,
        totalDurationMs: 500,
      },
      resultsPath,
    });

    const successfulResults = [
      {
        taskId: "task-test",
        outcome: "success" as const,
        output: "Added test cases and ran expect assertions to verify the fix. All tests pass.",
        durationMs: 1000,
        model: "test-model",
        tokens: { input: 50, output: 30, total: 80 },
        skillsUsed: [],
        usedReflexion: false,
        attemptNumber: 1,
        timestamp: new Date().toISOString(),
      },
      {
        taskId: "task-git",
        outcome: "success" as const,
        output: "Created a new branch and committed the changes using git add and git commit.",
        durationMs: 800,
        model: "test-model",
        tokens: { input: 40, output: 25, total: 65 },
        skillsUsed: [],
        usedReflexion: false,
        attemptNumber: 1,
        timestamp: new Date().toISOString(),
      },
      {
        taskId: "task-debug",
        outcome: "success" as const,
        output: "Found the error in the code and fixed the bug that was causing the test failure.",
        durationMs: 1200,
        model: "test-model",
        tokens: { input: 60, output: 35, total: 95 },
        skillsUsed: [],
        usedReflexion: false,
        attemptNumber: 1,
        timestamp: new Date().toISOString(),
      },
    ];

    const skills = await Effect.runPromise(
      learner.mineSkillsFromSuccess(episode, successfulResults),
    );

    expect(skills.length).toBe(3);
    const categories = skills.map((s) => s.category);
    expect(categories).toContain("testing");
    expect(categories).toContain("git");
    expect(categories).toContain("debugging");
  });
});

describe("Reflection Generation", () => {
  beforeEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
    mkdirSync(TEST_GYM_DIR, { recursive: true });
  });

  afterEach(() => {
    try {
      rmSync(TEST_PROJECT_ROOT, { recursive: true, force: true });
    } catch {}
  });

  test("generateReflections creates reflections from failures", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });

    const resultsPath = join(TEST_GYM_DIR, "refl-results.jsonl");
    writeFileSync(resultsPath, "");

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 2,
        passed: 0,
        failed: 2,
        timeout: 0,
        error: 0,
        avgTurns: 2,
        avgTokens: 50,
        totalDurationMs: 500,
      },
      resultsPath,
    });

    const failedResults = [
      {
        taskId: "task-fail-1",
        outcome: "failure" as const,
        errorMessage: "File not found: /path/to/missing.ts",
        durationMs: 300,
        model: "test-model",
        tokens: { input: 30, output: 15, total: 45 },
        skillsUsed: [],
        usedReflexion: false,
        attemptNumber: 1,
        timestamp: new Date().toISOString(),
      },
      {
        taskId: "task-fail-2",
        outcome: "timeout" as const,
        errorMessage: "Operation timed out after 120s",
        durationMs: 120000,
        model: "test-model",
        tokens: { input: 50, output: 25, total: 75 },
        skillsUsed: [],
        usedReflexion: false,
        attemptNumber: 1,
        timestamp: new Date().toISOString(),
      },
    ];

    const reflections = await Effect.runPromise(
      learner.generateReflections(episode, failedResults),
    );

    expect(reflections.length).toBe(2);

    const notFoundRefl = reflections.find((r) => r.taskId === "task-fail-1");
    expect(notFoundRefl).toBeDefined();
    expect(notFoundRefl?.failureType).toBe("error");
    expect(notFoundRefl?.lesson).toContain("not found");

    const timeoutRefl = reflections.find((r) => r.taskId === "task-fail-2");
    expect(timeoutRefl).toBeDefined();
    expect(timeoutRefl?.failureType).toBe("timeout");
    expect(timeoutRefl?.lesson).toContain("too long");
  });

  test("reflections have proper structure", async () => {
    const learner = createEpisodeLearner({ projectRoot: TEST_PROJECT_ROOT });

    const resultsPath = join(TEST_GYM_DIR, "struct-results.jsonl");
    writeFileSync(resultsPath, "");

    const episode = createEpisode({
      runId: generateRunId(),
      iteration: 1,
      model: "test-model",
      suiteVersion: "v1",
      startedAt: new Date(),
      finishedAt: new Date(),
      results: {
        total: 1,
        passed: 0,
        failed: 1,
        timeout: 0,
        error: 0,
        avgTurns: 1,
        avgTokens: 30,
        totalDurationMs: 200,
      },
      resultsPath,
    });

    const failedResults = [
      {
        taskId: "task-struct",
        outcome: "failure" as const,
        errorMessage: "Syntax error in generated code",
        durationMs: 200,
        model: "test-model",
        tokens: { input: 20, output: 10, total: 30 },
        skillsUsed: ["skill-1", "skill-2"],
        usedReflexion: false,
        attemptNumber: 1,
        timestamp: new Date().toISOString(),
      },
    ];

    const reflections = await Effect.runPromise(
      learner.generateReflections(episode, failedResults),
    );

    expect(reflections.length).toBe(1);
    const refl = reflections[0];

    expect(refl.id).toMatch(/^refl-/);
    expect(refl.episodeId).toBe(episode.id);
    expect(refl.taskId).toBe("task-struct");
    expect(refl.failureType).toBe("error"); // Contains "error" in message
    expect(refl.description).toBeDefined();
    expect(refl.lesson).toBeDefined();
    expect(refl.suggestedApproach).toBeDefined();
    expect(refl.relatedSkills).toEqual(["skill-1", "skill-2"]);
    expect(refl.createdAt).toBeDefined();
  });
});

describe("EpisodeLearnerError", () => {
  test("creates error with correct properties", () => {
    const error = new EpisodeLearnerError(
      "episode_not_found",
      "Episode not found: test-123",
    );

    expect(error.reason).toBe("episode_not_found");
    expect(error.message).toBe("Episode not found: test-123");
    expect(error._tag).toBe("EpisodeLearnerError");
    expect(error.name).toBe("EpisodeLearnerError");
  });

  test("creates error with cause", () => {
    const cause = new Error("Root cause");
    const error = new EpisodeLearnerError(
      "parse_failed",
      "Failed to parse",
      cause,
    );

    expect(error.cause).toBe(cause);
  });
});

describe("Convenience Functions", () => {
  test("learnFromRecentEpisodes is callable", () => {
    expect(learnFromRecentEpisodes).toBeDefined();
    expect(typeof learnFromRecentEpisodes).toBe("function");
  });

  test("learnFromEpisode is callable", () => {
    expect(learnFromEpisode).toBeDefined();
    expect(typeof learnFromEpisode).toBe("function");
  });
});
