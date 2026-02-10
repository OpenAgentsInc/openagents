import { Effect, Schema } from "effect";

import type { BlobRef } from "../blob.js";
import { BlobRefSchema } from "../blob.js";

import * as PromptIR from "../promptIr.js";
import * as Signature from "../signature.js";
import * as Params from "../params.js";

import { BlobStoreError, BlobStoreService } from "../runtime/blobStore.js";

import * as Dataset from "./dataset.js";
import * as Metric from "./metric.js";
import * as Reward from "./reward.js";

export type LongContextQaInput = {
  readonly question: string;
  readonly blobs: ReadonlyArray<BlobRef>;
};

export type LongContextQaOutput = {
  readonly answer: string;
  readonly evidence: {
    readonly blobId: string;
    readonly quote: string;
  };
};

export type LongContextQaExpected = LongContextQaOutput;

export const LongContextQaSignature = Signature.make<LongContextQaInput, LongContextQaOutput>({
  id: "@openagents/autopilot/eval/LongContextQa.v1",
  input: Schema.Struct({
    question: Schema.String,
    blobs: Schema.Array(BlobRefSchema),
  }).annotations({
    description:
      "Long-context QA input: one or more BlobRefs that may be too large to fit in token space."
  }),
  output: Schema.Struct({
    answer: Schema.String,
    evidence: Schema.Struct({
      blobId: Schema.String,
      quote: Schema.String,
    }),
  }).annotations({
    description:
      "Answer + evidence citation. evidence.quote must be an exact substring from the cited blob."
  }),
  prompt: {
    version: 1,
    blocks: [
      PromptIR.system(
        "You are a long-context QA module. You must answer using ONLY the provided BlobRefs."
      ),
      PromptIR.instruction(
        "Given Input.question and Input.blobs:\n" +
          "- Find the answer in the provided blobs.\n" +
          "- Output JSON only.\n" +
          "- Provide evidence as an exact quote substring from the blob.\n" +
          "- Set evidence.blobId to the blob you quoted.\n" +
          "- If multiple blobs match, pick the most direct one.\n" +
          "\n" +
          "Hard rule: Never invent evidence. If you cannot find a quote, return answer=\"unknown\" and evidence.quote=\"\"."
      ),
      PromptIR.outputJsonOnly(),
    ],
  },
  defaults: {
    params: {
      ...Params.emptyParamsV1,
      decode: { mode: "strict_json", maxRepairs: 1 },
      budgets: { maxTimeMs: 15_000, maxLmCalls: 20, maxOutputChars: 120_000 },
    } satisfies Params.DseParamsV1,
  },
});

function buildFillerLines(options: {
  readonly prefix: string;
  readonly count: number;
  readonly payloadChars: number;
}): string {
  const payload = "x".repeat(Math.max(0, Math.floor(options.payloadChars)));
  const out: Array<string> = [];
  for (let i = 0; i < options.count; i++) {
    out.push(`${options.prefix} ${String(i).padStart(5, "0")} ${payload}`);
  }
  return out.join("\n");
}

function buildPseudoRepo(options: {
  readonly fillerFiles: number;
  readonly fillerLinesPerFile: number;
  readonly needleFilePath: string;
  readonly needleLine: string;
}): string {
  const out: Array<string> = [];
  for (let i = 0; i < options.fillerFiles; i++) {
    out.push(`// FILE: src/filler_${String(i).padStart(3, "0")}.ts`);
    out.push(buildFillerLines({ prefix: "export const filler =", count: options.fillerLinesPerFile, payloadChars: 60 }));
    out.push("");
  }
  out.push(`// FILE: ${options.needleFilePath}`);
  out.push("/* needle */");
  out.push(options.needleLine);
  out.push("");
  return out.join("\n");
}

export function makeDummyLongContextLogQaDataset(): Effect.Effect<
  Dataset.Dataset<LongContextQaInput, LongContextQaExpected>,
  Dataset.DatasetError | BlobStoreError,
  BlobStoreService
> {
  return Effect.gen(function* () {
    const blobs = yield* BlobStoreService;

    const needleSmall = "NEEDLE_LOG_SMALL_7b2c";
    const needleLarge = "NEEDLE_LOG_LARGE_91af";

    const logSmall = [
      buildFillerLines({ prefix: "INFO", count: 120, payloadChars: 90 }),
      `ERROR oa_req=${needleSmall} unexpected EOF while reading headers`,
      buildFillerLines({ prefix: "INFO", count: 20, payloadChars: 90 }),
    ].join("\n");

    const logLarge = [
      buildFillerLines({ prefix: "INFO", count: 900, payloadChars: 90 }),
      `ERROR oa_req=${needleLarge} unexpected EOF while reading headers`,
      buildFillerLines({ prefix: "INFO", count: 40, payloadChars: 90 }),
    ].join("\n");

    const blobSmall = yield* blobs.putText({ text: logSmall, mime: "text/plain" });
    const blobLarge = yield* blobs.putText({ text: logLarge, mime: "text/plain" });

    return yield* Dataset.make<LongContextQaInput, LongContextQaExpected>({
      datasetId: "autopilot.long_context.log_qa.v1",
      examples: [
        {
          exampleId: "log_small.oa_req",
          split: "test",
          tags: ["log", "small", "needle_in_preview"],
          input: {
            question:
              "In the logs, find the oa_req value for the ERROR line mentioning \"unexpected EOF while reading headers\". Return just the oa_req.",
            blobs: [blobSmall],
          },
          expected: {
            answer: needleSmall,
            evidence: {
              blobId: blobSmall.id,
              quote: `ERROR oa_req=${needleSmall} unexpected EOF while reading headers`,
            },
          },
        },
        {
          exampleId: "log_large.oa_req",
          split: "test",
          tags: ["log", "large", "needle_beyond_preview"],
          input: {
            question:
              "In the logs, find the oa_req value for the ERROR line mentioning \"unexpected EOF while reading headers\". Return just the oa_req.",
            blobs: [blobLarge],
          },
          expected: {
            answer: needleLarge,
            evidence: {
              blobId: blobLarge.id,
              quote: `ERROR oa_req=${needleLarge} unexpected EOF while reading headers`,
            },
          },
        },
      ],
    });
  });
}

export function makeDummyRepoNeedleDataset(): Effect.Effect<
  Dataset.Dataset<LongContextQaInput, LongContextQaExpected>,
  Dataset.DatasetError | BlobStoreError,
  BlobStoreService
> {
  return Effect.gen(function* () {
    const blobs = yield* BlobStoreService;

    const needleSmall = "NEEDLE_REPO_SMALL_2c1e";
    const needleLarge = "NEEDLE_REPO_LARGE_8e54";

    const repoSmall = buildPseudoRepo({
      fillerFiles: 2,
      fillerLinesPerFile: 30,
      needleFilePath: "src/needle.ts",
      needleLine: `export const NEEDLE = "${needleSmall}"`,
    });

    const repoLarge = buildPseudoRepo({
      fillerFiles: 18,
      fillerLinesPerFile: 80,
      needleFilePath: "src/needle.ts",
      needleLine: `export const NEEDLE = "${needleLarge}"`,
    });

    const blobSmall = yield* blobs.putText({ text: repoSmall, mime: "text/plain" });
    const blobLarge = yield* blobs.putText({ text: repoLarge, mime: "text/plain" });

    return yield* Dataset.make<LongContextQaInput, LongContextQaExpected>({
      datasetId: "autopilot.long_context.repo_needle.v1",
      examples: [
        {
          exampleId: "repo_small.NEEDLE_value",
          split: "test",
          tags: ["repo", "small", "needle_in_preview"],
          input: {
            question:
              "In the repo snapshot, what is the string value assigned to the exported constant NEEDLE?",
            blobs: [blobSmall],
          },
          expected: {
            answer: needleSmall,
            evidence: { blobId: blobSmall.id, quote: `export const NEEDLE = "${needleSmall}"` },
          },
        },
        {
          exampleId: "repo_large.NEEDLE_value",
          split: "test",
          tags: ["repo", "large", "needle_beyond_preview"],
          input: {
            question:
              "In the repo snapshot, what is the string value assigned to the exported constant NEEDLE?",
            blobs: [blobLarge],
          },
          expected: {
            answer: needleLarge,
            evidence: { blobId: blobLarge.id, quote: `export const NEEDLE = "${needleLarge}"` },
          },
        },
      ],
    });
  });
}

export function rewardForLongContextQa(): Reward.RewardBundle<
  LongContextQaInput,
  LongContextQaOutput,
  LongContextQaExpected
> {
  const metricExactAnswer = Metric.deterministic<LongContextQaOutput, LongContextQaExpected>({
    metricId: "exact_answer_match.v1",
    metricVersion: 1,
    score: (pred, expected) => (pred.answer.trim() === expected.answer.trim() ? 1 : 0),
    notes: (pred, expected) => (pred.answer.trim() === expected.answer.trim() ? undefined : "answer_mismatch"),
  });

  return Reward.makeBundle({
    rewardId: "reward_long_context_qa.v1",
    rewardVersion: 1,
    signals: [
      Reward.signalFormatValidity({ weight: 0.2 }),
      Reward.signalMetric(metricExactAnswer, { weight: 0.4, signalId: "exact_answer.signal.v1" }),
      Reward.signalEvidenceQuoteInBlobStore({
        signalId: "evidence_quote_in_blob.v1",
        weight: 0.3,
        extractEvidence: ({ pred }) => ({
          blobId: String((pred as any)?.evidence?.blobId ?? ""),
          quote: String((pred as any)?.evidence?.quote ?? ""),
        }),
      }),
      Reward.signalPredictCostPenalty({ weight: 0.1, targetDurationMs: 800, targetLmCalls: 1, targetToolCalls: 0 }),
    ],
  });
}
