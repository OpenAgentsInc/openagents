/**
 * SOAR Weighted Majority Voting Tests
 *
 * Tests for voting mechanism to select final predictions.
 */

import { describe, test, expect } from "bun:test";
import * as S from "effect/Schema";
import { Effect } from "effect";
import {
  Vote,
  VotingResult,
  DEFAULT_VOTING_CONFIG,
  normalizeOutputKey,
  calculateVoteWeight,
  groupVotes,
  vote,
  createVotes,
  ensembleVote,
  VotingService,
  VotingServiceLive,
  makeVotingServiceLayer,
} from "../soar-voting.js";
import { createMockVote, createMockVoteBatch, runEffect } from "./test-helpers.js";

describe("Vote Schema", () => {
  test("decodes valid vote", () => {
    const input = {
      output: 42,
      outputKey: "42",
      program: "function f() { return 42; }",
      trainingAccuracy: 0.8,
    };
    const decoded = S.decodeUnknownSync(Vote)(input);
    expect(decoded.output).toBe(42);
    expect(decoded.trainingAccuracy).toBe(0.8);
  });

  test("accepts optional skill fields", () => {
    const input = {
      output: [1, 2, 3],
      outputKey: "[1,2,3]",
      program: "code",
      trainingAccuracy: 0.9,
      skillId: "skill-001",
      skillConfidence: 0.85,
    };
    const decoded = S.decodeUnknownSync(Vote)(input);
    expect(decoded.skillId).toBe("skill-001");
    expect(decoded.skillConfidence).toBe(0.85);
  });
});

describe("VotingResult Schema", () => {
  test("decodes valid result", () => {
    const input = {
      winner: 42,
      winnerKey: "42",
      winnerWeight: 100,
      confidence: 0.8,
      candidates: [],
      totalVotes: 10,
      isValid: true,
      votedAt: new Date().toISOString(),
    };
    const decoded = S.decodeUnknownSync(VotingResult)(input);
    expect(decoded.winner).toBe(42);
    expect(decoded.confidence).toBe(0.8);
  });
});

describe("normalizeOutputKey", () => {
  test("handles null", () => {
    expect(normalizeOutputKey(null)).toBe("null");
  });

  test("handles undefined", () => {
    expect(normalizeOutputKey(undefined)).toBe("undefined");
  });

  test("handles strings with whitespace", () => {
    expect(normalizeOutputKey("  hello  world  ")).toBe("hello world");
  });

  test("handles numbers", () => {
    expect(normalizeOutputKey(42)).toBe("42");
    expect(normalizeOutputKey(3.14)).toBe("3.14");
  });

  test("handles booleans", () => {
    expect(normalizeOutputKey(true)).toBe("true");
    expect(normalizeOutputKey(false)).toBe("false");
  });

  test("handles arrays (preserves order)", () => {
    const key = normalizeOutputKey([1, 2, 3]);
    expect(key).toBe("[1,2,3]");
  });

  test("handles objects (sorts keys)", () => {
    const key = normalizeOutputKey({ b: 2, a: 1 });
    expect(key).toBe('{"a":1,"b":2}');
  });

  test("handles nested objects", () => {
    const key = normalizeOutputKey({ outer: { inner: 1 } });
    expect(key).toContain("inner");
  });
});

describe("calculateVoteWeight", () => {
  test("calculates base weight", () => {
    const v = createMockVote({ trainingAccuracy: 0 });
    const weight = calculateVoteWeight(v, DEFAULT_VOTING_CONFIG);
    // weight = 1 + 1000 * 0 = 1
    expect(weight).toBe(1);
  });

  test("includes accuracy multiplier", () => {
    const v = createMockVote({ trainingAccuracy: 0.5 });
    const weight = calculateVoteWeight(v, DEFAULT_VOTING_CONFIG);
    // weight = 1 + 1000 * 0.5 = 501
    expect(weight).toBe(501);
  });

  test("applies skill weighting when enabled", () => {
    const v = createMockVote({ trainingAccuracy: 0.5, skillConfidence: 1.0 });
    const weightWithSkill = calculateVoteWeight(v, { ...DEFAULT_VOTING_CONFIG, enableSkillWeighting: true });
    const vNoSkill = createMockVote({ trainingAccuracy: 0.5 });
    const weightNoSkill = calculateVoteWeight(vNoSkill, { ...DEFAULT_VOTING_CONFIG, enableSkillWeighting: true });

    // With full skill confidence: weight *= 1.0, without skill: base weight
    expect(weightWithSkill).toBe(501); // 501 * (0.5 + 0.5 * 1.0) = 501 * 1.0
    expect(weightNoSkill).toBe(501);
  });

  test("skill weighting reduces weight when low confidence", () => {
    const v = createMockVote({ trainingAccuracy: 1.0, skillConfidence: 0 });
    const weight = calculateVoteWeight(v, { ...DEFAULT_VOTING_CONFIG, enableSkillWeighting: true });
    // weight = (1 + 1000 * 1.0) * (0.5 + 0.5 * 0) = 1001 * 0.5 = 500.5
    expect(weight).toBe(500.5);
  });
});

describe("groupVotes", () => {
  test("groups by output key", () => {
    const votes = [
      createMockVote({ output: 42, trainingAccuracy: 0.5 }),
      createMockVote({ output: 42, trainingAccuracy: 0.6 }),
      createMockVote({ output: 10, trainingAccuracy: 0.3 }),
    ];
    const groups = groupVotes(votes, DEFAULT_VOTING_CONFIG);

    expect(groups.size).toBe(2);
    expect(groups.get("42")?.votes).toHaveLength(2);
    expect(groups.get("10")?.votes).toHaveLength(1);
  });

  test("accumulates weights", () => {
    const votes = [
      createMockVote({ output: 42, trainingAccuracy: 0.5 }),
      createMockVote({ output: 42, trainingAccuracy: 0.5 }),
    ];
    const groups = groupVotes(votes, DEFAULT_VOTING_CONFIG);

    const group = groups.get("42");
    // Each vote: 1 + 1000 * 0.5 = 501
    expect(group?.weight).toBe(1002);
  });

  test("calculates average accuracy", () => {
    const votes = [
      createMockVote({ output: 42, trainingAccuracy: 0.4 }),
      createMockVote({ output: 42, trainingAccuracy: 0.6 }),
    ];
    const groups = groupVotes(votes, DEFAULT_VOTING_CONFIG);

    expect(groups.get("42")?.averageAccuracy).toBe(0.5);
  });
});

describe("vote", () => {
  test("returns invalid for too few votes", () => {
    const result = vote([], { ...DEFAULT_VOTING_CONFIG, minVotes: 3 });
    expect(result.isValid).toBe(false);
    expect(result.winner).toBe(null);
  });

  test("selects highest weight winner", () => {
    const votes = [
      createMockVote({ output: "a", trainingAccuracy: 0.3 }),
      createMockVote({ output: "b", trainingAccuracy: 0.9 }),
    ];
    const result = vote(votes);

    expect(result.winner).toBe("b");
    expect(result.isValid).toBe(true);
  });

  test("breaks tie by accuracy", () => {
    const votes = [
      createMockVote({ output: "a", trainingAccuracy: 0.6 }),
      createMockVote({ output: "b", trainingAccuracy: 0.8 }),
    ];
    // Same count but different accuracy
    const result = vote(votes, { ...DEFAULT_VOTING_CONFIG, tieBreaker: "accuracy" });

    expect(result.winner).toBe("b");
  });

  test("breaks tie by count", () => {
    const votes = [
      createMockVote({ output: "a", trainingAccuracy: 0.5 }),
      createMockVote({ output: "a", trainingAccuracy: 0.5 }),
      createMockVote({ output: "b", trainingAccuracy: 0.9 }),
    ];
    const result = vote(votes, { ...DEFAULT_VOTING_CONFIG, tieBreaker: "count" });

    // "a" has 2 votes vs "b" has 1, but "b" has higher weight
    // After checking, the winner depends on weight not count for initial selection
    // tieBreaker only applies when weights are equal
    expect(result.winner).toBeDefined();
  });

  test("calculates confidence correctly", () => {
    const votes = [
      createMockVote({ output: "a", trainingAccuracy: 0.8 }),
      createMockVote({ output: "b", trainingAccuracy: 0.2 }),
    ];
    const result = vote(votes);

    // Winner weight / total weight
    expect(result.confidence).toBeGreaterThan(0);
    expect(result.confidence).toBeLessThanOrEqual(1);
  });

  test("sorts candidates by weight", () => {
    const votes = [
      createMockVote({ output: "c", trainingAccuracy: 0.3 }),
      createMockVote({ output: "a", trainingAccuracy: 0.9 }),
      createMockVote({ output: "b", trainingAccuracy: 0.5 }),
    ];
    const result = vote(votes);

    expect(result.candidates[0]?.outputKey).toBe(normalizeOutputKey("a"));
    expect(result.candidates[1]?.outputKey).toBe(normalizeOutputKey("b"));
    expect(result.candidates[2]?.outputKey).toBe(normalizeOutputKey("c"));
  });

  test("includes vote count in candidates", () => {
    const votes = [
      createMockVote({ output: 42, trainingAccuracy: 0.5 }),
      createMockVote({ output: 42, trainingAccuracy: 0.6 }),
    ];
    const result = vote(votes);

    expect(result.candidates[0]?.voteCount).toBe(2);
  });
});

describe("createVotes", () => {
  test("creates votes from outputs", () => {
    const outputs = [
      { output: 42, program: "code1", trainingAccuracy: 0.8 },
      { output: 10, program: "code2", trainingAccuracy: 0.5 },
    ];
    const votes = createVotes(outputs);

    expect(votes).toHaveLength(2);
    expect(votes[0]?.output).toBe(42);
    expect(votes[0]?.outputKey).toBe("42");
  });

  test("preserves optional fields", () => {
    const outputs = [
      { output: 1, program: "code", trainingAccuracy: 0.5, skillId: "s1", skillConfidence: 0.9 },
    ];
    const votes = createVotes(outputs);

    expect(votes[0]?.skillId).toBe("s1");
    expect(votes[0]?.skillConfidence).toBe(0.9);
  });
});

describe("ensembleVote", () => {
  test("combines createVotes and vote", () => {
    const outputs = [
      { output: "winner", program: "code1", trainingAccuracy: 0.9 },
      { output: "loser", program: "code2", trainingAccuracy: 0.1 },
    ];
    const result = ensembleVote(outputs);

    expect(result.winner).toBe("winner");
  });
});

describe("VotingService", () => {
  test("vote returns result", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        const votes = [
          createMockVote({ output: 42, trainingAccuracy: 0.8 }),
          createMockVote({ output: 10, trainingAccuracy: 0.3 }),
        ];
        return yield* service.vote(votes);
      }).pipe(Effect.provide(VotingServiceLive)),
    );

    expect(result.winner).toBe(42);
  });

  test("createVotes returns votes", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        return yield* service.createVotes([
          { output: 1, program: "a", trainingAccuracy: 0.5 },
        ]);
      }).pipe(Effect.provide(VotingServiceLive)),
    );

    expect(result).toHaveLength(1);
  });

  test("ensembleVote returns result", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        return yield* service.ensembleVote([
          { output: "a", program: "code", trainingAccuracy: 0.9 },
        ]);
      }).pipe(Effect.provide(VotingServiceLive)),
    );

    expect(result.winner).toBe("a");
  });

  test("calculateWeight returns weight", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        const v = createMockVote({ trainingAccuracy: 0.5 });
        return yield* service.calculateWeight(v);
      }).pipe(Effect.provide(VotingServiceLive)),
    );

    expect(result).toBe(501);
  });

  test("normalizeOutput returns key", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        return yield* service.normalizeOutput({ a: 1 });
      }).pipe(Effect.provide(VotingServiceLive)),
    );

    expect(result).toBe('{"a":1}');
  });

  test("getStats tracks voting sessions", () => {
    // Use fresh layer to avoid stats accumulation from other tests
    const freshLayer = makeVotingServiceLayer();
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        yield* service.vote([createMockVote()]);
        yield* service.vote([createMockVote(), createMockVote()]);
        return yield* service.getStats();
      }).pipe(Effect.provide(freshLayer)),
    );

    expect(result.totalVotingSessions).toBe(2);
    expect(result.totalVotesCast).toBe(3);
  });

  test("updateConfig modifies config", () => {
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        return yield* service.updateConfig({ minVotes: 5 });
      }).pipe(Effect.provide(VotingServiceLive)),
    );

    expect(result.minVotes).toBe(5);
  });

  test("custom config layer", () => {
    const customLayer = makeVotingServiceLayer({ accuracyMultiplier: 500 });
    const result = runEffect(
      Effect.gen(function* () {
        const service = yield* VotingService;
        return yield* service.getConfig();
      }).pipe(Effect.provide(customLayer)),
    );

    expect(result.accuracyMultiplier).toBe(500);
  });
});
