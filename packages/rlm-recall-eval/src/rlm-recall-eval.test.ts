import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { priceUsage, RLM_EVAL_DEFAULT_MODEL_ID } from "./price-catalog.ts";
import {
  generateLinearTranscript,
  generatePairTranscript,
  generateConstantTranscript,
} from "./transcripts.ts";
import { runAllTiersForQuestion, runTierD, runDirect, runBoundedWindow } from "./tiers.ts";
import { classifyOutcome, scoreCitations } from "./scoring.ts";
import { decideLive } from "./live-cli.ts";
import { DEFAULT_HERMETIC_CONFIG, runHermeticMatrix } from "./harness.ts";
import { serializeAggregate } from "./generate-report.ts";

describe("transcript generators are deterministic", () => {
  test("same size yields byte-identical corpus digests", () => {
    const a = generateLinearTranscript(64);
    const b = generateLinearTranscript(64);
    expect(a.corpusInput.manifest.contentDigest).toBe(b.corpusInput.manifest.contentDigest);
    expect(a.corpusInput.manifest.manifestDigest).toBe(b.corpusInput.manifest.manifestDigest);
  });

  test("linear plants three keyfacts, pair plants two conflicting turns", () => {
    const linear = generateLinearTranscript(256);
    expect(linear.questions.length).toBe(3);
    const pair = generatePairTranscript(256);
    expect(pair.questions.length).toBe(1);
    expect(pair.questions[0]!.expectedEntryRefs.length).toBe(2);
  });
});

describe("price catalog honesty", () => {
  test("unknown usage is never priced as zero", () => {
    const unavailable = priceUsage(RLM_EVAL_DEFAULT_MODEL_ID, {
      inputTokens: null,
      outputTokens: null,
      completeness: "unavailable",
    });
    expect(unavailable.usd).toBeNull();
    expect(unavailable.disposition).toBe("unknown_usage");
  });

  test("unknown model returns unknown, not a fabricated price", () => {
    const unknown = priceUsage("no-such-model", {
      inputTokens: 100,
      outputTokens: 100,
      completeness: "complete",
    });
    expect(unknown.usd).toBeNull();
    expect(unknown.disposition).toBe("unknown_model");
  });

  test("known usage prices deterministically", () => {
    const known = priceUsage("gemini-3.5-flash", {
      inputTokens: 1_000_000,
      outputTokens: 1_000_000,
      completeness: "complete",
    });
    expect(known.disposition).toBe("known");
    expect(known.usd).toBeCloseTo(0.375, 6);
  });
});

describe("outcome classification honesty", () => {
  test("honest abstention is never a wrong answer", () => {
    expect(
      classifyOutcome({
        family: "linear",
        answerContainsExpected: false,
        synthesized: true,
        abstained: true,
        bothPairValuesSurfaced: false,
      }),
    ).toBe("refused");
  });

  test("pair requires synthesis: retrieval-only surfacing is partial", () => {
    expect(
      classifyOutcome({
        family: "pair",
        answerContainsExpected: true,
        synthesized: false,
        abstained: false,
        bothPairValuesSurfaced: true,
      }),
    ).toBe("partial");
    expect(
      classifyOutcome({
        family: "pair",
        answerContainsExpected: true,
        synthesized: true,
        abstained: false,
        bothPairValuesSurfaced: true,
      }),
    ).toBe("success");
  });
});

describe("citation scoring resolves against exact expected refs", () => {
  test("coverage and exactness", () => {
    const score = scoreCitations(["t5#0"], ["t5#0"]);
    expect(score.coverage).toBe(1);
    expect(score.exactness).toBe(1);
  });
});

describe("tier behaviour on OpenAgents transcript shapes", () => {
  test("Tier D answers a linear keyfact with an exact citation and zero cost", async () => {
    const t = generateLinearTranscript(64);
    const q = t.questions[1]!; // mid
    const result = await Effect.runPromise(
      runTierD({ transcript: t, question: q, modelId: RLM_EVAL_DEFAULT_MODEL_ID }),
    );
    expect(result.outcome).toBe("success");
    expect(result.answerContainsExpected).toBe(true);
    expect(result.citation.coverage).toBe(1);
    expect(result.modelCalls).toBe(0);
    expect(result.cost.disposition).toBe("known");
    expect(result.cost.usd).toBe(0);
  });

  test("Tier D is partial on a pair task: it surfaces both spans but does not synthesise", async () => {
    const t = generatePairTranscript(128);
    const q = t.questions[0]!;
    const result = await Effect.runPromise(
      runTierD({ transcript: t, question: q, modelId: RLM_EVAL_DEFAULT_MODEL_ID }),
    );
    expect(result.outcome).toBe("partial");
    expect(result.citation.coverage).toBe(1);
  });

  test("a synthesising semantic tier answers the pair task with exact citations", async () => {
    const t = generatePairTranscript(128);
    const q = t.questions[0]!;
    const results = await Effect.runPromise(
      runAllTiersForQuestion({ transcript: t, question: q, modelId: RLM_EVAL_DEFAULT_MODEL_ID }),
    );
    const modelmap = results.find((r) => r.tierId === "semantic_modelmap")!;
    expect(modelmap.outcome).toBe("success");
    expect(modelmap.answerContainsExpected).toBe(true);
    expect(modelmap.citation.coverage).toBe(1);
    expect(modelmap.citation.exactness).toBe(1);
    expect(modelmap.subcalls + modelmap.modelCalls).toBeGreaterThan(1);
    expect(modelmap.cost.disposition).toBe("known");
  });

  test("bounded window misses a distant linear fact and abstains honestly", async () => {
    const t = generateLinearTranscript(1024);
    const early = t.questions[0]!; // ordinal ~102, far outside a 32-turn window
    const result = await Effect.runPromise(
      runBoundedWindow({ transcript: t, question: early, modelId: RLM_EVAL_DEFAULT_MODEL_ID }),
    );
    expect(result.outcome).toBe("refused");
    expect(result.producedAnswer).toBe("UNKNOWN");
  });

  test("direct refuses when the whole corpus exceeds the per-call prompt headroom", async () => {
    const t = generateLinearTranscript(4096);
    const q = t.questions[0]!;
    const result = await Effect.runPromise(
      runDirect({ transcript: t, question: q, modelId: RLM_EVAL_DEFAULT_MODEL_ID }),
    );
    expect(result.outcome).toBe("refused");
    expect(result.capsHit).toContain("maxPromptTokensPerCall");
    expect(result.modelCalls).toBe(0);
  });
});

describe("live command is a safe gate", () => {
  test("refuses without the live env flag", () => {
    const d = decideLive({
      admittedAccounts: ["acct"],
      caps: { maxModelCalls: 10, maxUsd: 1 },
      modelModule: "/x",
      liveEnvSet: false,
    });
    expect(d.kind).toBe("refused");
  });

  test("refuses without admitted accounts even with the flag", () => {
    const d = decideLive({
      admittedAccounts: [],
      caps: { maxModelCalls: 10, maxUsd: 1 },
      modelModule: "/x",
      liveEnvSet: true,
    });
    expect(d.kind).toBe("refused");
  });

  test("refuses without a bound model module", () => {
    const d = decideLive({
      admittedAccounts: ["acct"],
      caps: { maxModelCalls: 10, maxUsd: 1 },
      modelModule: null,
      liveEnvSet: true,
    });
    expect(d.kind).toBe("refused");
  });
});

describe("hermetic matrix", () => {
  test("is reproducible byte-for-byte from a clean run", async () => {
    const a = await Effect.runPromise(runHermeticMatrix());
    const b = await Effect.runPromise(runHermeticMatrix());
    expect(serializeAggregate(a)).toBe(serializeAggregate(b));
  });

  test("gates stay admitted=false; depth>1 does not pass, escalation criteria are explicit", async () => {
    const out = await Effect.runPromise(runHermeticMatrix());
    expect(out.gates.escalation.admitted).toBe(false);
    expect(out.gates.depth.admitted).toBe(false);
    // Depth-2 ties depth-1 in the pinned SDK (single-level recursion), so the
    // strict-improvement criterion fails and depth>1 stays disabled.
    expect(out.gates.depth.wouldPass).toBe(false);
    expect(out.gates.escalation.criteria.length).toBeGreaterThan(0);
  });

  test("unknown usage is excluded from cost, never priced as zero", async () => {
    const out = await Effect.runPromise(runHermeticMatrix());
    expect(out.honesty.runs).toBeGreaterThan(0);
    expect(out.honesty.unknownUsageCount).toBe(out.honesty.runs);
    expect(out.honesty.costExcludedCorrectly).toBe(true);
  });

  test("the whole default config runs without a network or spend", async () => {
    const out = await Effect.runPromise(runHermeticMatrix(DEFAULT_HERMETIC_CONFIG));
    expect(out.meta.kind).toBe("hermetic");
    expect(out.meta.totalRuns).toBeGreaterThan(0);
    const tierD = out.tierAggregates.find((t) => t.tierId === "tier_d")!;
    // Tier D never issues a model call.
    expect(tierD.overall.modelCalls.max).toBe(0);
  });
});
