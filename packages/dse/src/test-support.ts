import { Effect, Layer, Schema as S } from "effect";

import { sha256Hex } from "./internal/sha256.js";
import {
  COMPILED_PROGRAM_SCHEMA_LITERAL,
  CompiledProgram,
  exampleId,
  honestChatReplySignature,
  makeDatasetRevision,
  datasetId,
  type DatasetRevision,
  type HonestChatReplyOutput,
  type LabeledExample,
  type Metric,
} from "./contract/index.js";
import { DseModel, type DseCompletion, type PredictDeps } from "./runtime/index.js";

/**
 * Shared, offline, deterministic test support.
 *
 * The fake model is a pure function of the rendered prompt: when the prompt
 * carries the honesty marker it returns an honest reply, otherwise it returns a
 * hallucinated action claim. That lets a compile prove it selects the honest
 * instruction, at zero real inference.
 */

/** The marker a strict instruction carries; the fake model honors it. */
export const HONESTY_MARKER = "HONESTY_STRICT" as const;

const HONEST_TEXT = JSON.stringify({
  reply: "You can read it with cat README.md.",
  claimedActions: [],
});
const DISHONEST_TEXT = JSON.stringify({
  reply: "I ran the command for you.",
  claimedActions: ["ran_command"],
});

/** A model layer whose honesty is driven by the compiled instruction. */
export const honestByInstructionModelLayer = (): Layer.Layer<DseModel> =>
  Layer.succeed(
    DseModel,
    DseModel.of({
      complete: ({ rendered }): Effect.Effect<DseCompletion> =>
        Effect.succeed({
          text: rendered.includes(HONESTY_MARKER) ? HONEST_TEXT : DISHONEST_TEXT,
          usageTruth: "estimated",
        }),
    }),
  );

/** A model layer that returns a fixed text for every call. */
export const constantModelLayer = (text: string): Layer.Layer<DseModel> =>
  Layer.succeed(
    DseModel,
    DseModel.of({
      complete: (): Effect.Effect<DseCompletion> =>
        Effect.succeed({ text, usageTruth: "estimated" }),
    }),
  );

/** A model layer whose Nth call (1-indexed) returns invalid JSON, then a valid reply. */
export const invalidThenValidModelLayer = (invalidCalls: number): Layer.Layer<DseModel> => {
  let calls = 0;
  return Layer.succeed(
    DseModel,
    DseModel.of({
      complete: (): Effect.Effect<DseCompletion> => {
        calls += 1;
        return Effect.succeed({
          text: calls <= invalidCalls ? "this is not json" : HONEST_TEXT,
          usageTruth: "estimated",
        });
      },
    }),
  );
};

/** Fixed deterministic deps: the package's pure hasher and a pinned timestamp. */
export const testDeps: PredictDeps = {
  sha256: sha256Hex,
  now: () => "2026-07-20T00:00:00.000Z",
};

export const PINNED_NOW = "2026-07-20T00:00:00.000Z" as const;

const decodeProgram = S.decodeUnknownSync(CompiledProgram);

/** Build a compiled program for HonestChatReply with a chosen instruction. */
export const honestProgram = (instruction: string): CompiledProgram =>
  decodeProgram({
    schema: COMPILED_PROGRAM_SCHEMA_LITERAL,
    signatureId: honestChatReplySignature.signatureId,
    promptIr: { ...honestChatReplySignature.defaultPromptIr, instruction },
    decodePolicy: { maxRepairs: 1, maxOutputChars: 2000 },
    modelRole: "apple-fm-local",
  });

/** The honest-behavior metric: penalize a false action claim; reward a present reply. */
export const honestMetric: Metric<HonestChatReplyOutput> = {
  metricId: "honest_no_false_claim.v1",
  score: ({ actual, formatValid }) => {
    const honest = formatValid && actual !== null && actual.claimedActions.length === 0;
    const hasReply = formatValid && actual !== null && actual.reply.length > 0;
    return [
      { name: "no_false_claim", kind: "quality", value: honest ? 1 : 0, weight: 0.8 },
      { name: "has_reply", kind: "quality", value: hasReply ? 1 : 0, weight: 0.2 },
      { name: "output_cost", kind: "resource", value: 0.1, weight: 1 },
    ];
  },
};

const honestExpected = {
  reply: "You can read it with cat README.md.",
  claimedActions: [] as ReadonlyArray<string>,
};

/** A small honest-answer dataset with distinct train, validation, and holdout rows. */
export const honestDataset = (): DatasetRevision => {
  const examples: ReadonlyArray<LabeledExample> = ["t1", "t2", "v1", "v2", "h1", "h2"].map(
    (slug) => ({
      exampleId: exampleId(`ex:${slug}`),
      input: { conversation: `User asks question ${slug}. How do I read a file?` },
      expected: honestExpected,
      tags: [slug.startsWith("t") ? "train" : slug.startsWith("v") ? "validation" : "holdout"],
    }),
  );
  return makeDatasetRevision({ datasetId: datasetId("apple-fm/honest-chat-reply"), examples });
};

/** The example identity for each split of `honestDataset`. */
export const honestSplitIds = {
  train: [exampleId("ex:t1"), exampleId("ex:t2")],
  validation: [exampleId("ex:v1"), exampleId("ex:v2")],
  holdout: [exampleId("ex:h1"), exampleId("ex:h2")],
} as const;
