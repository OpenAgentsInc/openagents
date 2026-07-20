import { Effect, Result, Schema as S } from "effect";

import { canonicalStringify } from "../internal/canonical.js";
import type {
  CompiledProgram,
  DseSignature,
  DseTimestamp,
  PredictReceipt,
} from "../contract/index.js";
import {
  PREDICT_RECEIPT_SCHEMA_LITERAL,
  PredictReceipt as PredictReceiptSchema,
} from "../contract/artifact.js";
import { DseModel } from "./model.js";

/**
 * `Predict`: render the compiled program deterministically, call the model,
 * decode with the signature's output schema, run a bounded repair on a first
 * decode failure, and write an append-only predict receipt.
 *
 * Predict has no promotion or compile authority. It resolves the program it is
 * given; it never publishes, admits, or replaces an artifact.
 */

export class DseDecodeError extends S.TaggedErrorClass<DseDecodeError>()("dse/DseDecodeError", {
  reason: S.String,
  repairCount: S.Number,
}) {}

export interface PredictDeps {
  readonly sha256: (text: string) => string;
  readonly now: () => typeof DseTimestamp.Type;
}

export interface PredictArgs<I, O> {
  readonly signature: DseSignature<I, O>;
  readonly candidateId: string;
  readonly program: CompiledProgram;
  readonly input: I;
  readonly deps: PredictDeps;
}

export interface PredictOutcome<O> {
  readonly output: O;
  readonly receipt: PredictReceipt;
}

const decodeReceipt = S.decodeUnknownSync(PredictReceiptSchema);

type JsonParse = { readonly ok: true; readonly value: unknown } | { readonly ok: false };
const tryParseJson = (text: string): JsonParse => {
  try {
    return { ok: true, value: JSON.parse(text) };
  } catch {
    return { ok: false };
  }
};

/** Deterministically render the compiled prompt with the per-call context. */
export const renderPrompt = (program: CompiledProgram, context: string): string =>
  [
    program.promptIr.system,
    program.promptIr.instruction,
    program.promptIr.toolPolicy,
    `Few-shot: ${program.promptIr.fewShotExampleIds.join(",")}`,
    program.promptIr.outputFormat,
    `Context:\n${context}`,
  ].join("\n\n");

const REPAIR_SUFFIX =
  "\n\nYour previous output did not decode. Return ONLY strict JSON matching the output format. Do not add prose.";

export const predict = <I, O>(
  args: PredictArgs<I, O>,
): Effect.Effect<PredictOutcome<O>, DseDecodeError, DseModel> =>
  Effect.gen(function* () {
    const model = yield* DseModel;
    // Decoding is pure, so a Result-returning decoder keeps the Effect context
    // free of the schema's requirement channel.
    const decodeOutput = S.decodeUnknownResult(args.signature.output);
    const context = canonicalStringify(args.input);
    const basePrompt = renderPrompt(args.program, context);
    const maxRepairs = args.program.decodePolicy.maxRepairs;

    let repairCount = 0;
    let lastReason = "no attempt";

    while (repairCount <= maxRepairs) {
      const rendered = repairCount === 0 ? basePrompt : basePrompt + REPAIR_SUFFIX;
      const completion = yield* model
        .complete({ rendered, maxOutputChars: args.program.decodePolicy.maxOutputChars })
        .pipe(
          Effect.mapError((error) => new DseDecodeError({ reason: error.reason, repairCount })),
        );

      const parsed = tryParseJson(completion.text);
      if (!parsed.ok) {
        lastReason = "output was not valid JSON";
        repairCount += 1;
        continue;
      }

      const decoded = decodeOutput(parsed.value);
      if (Result.isSuccess(decoded)) {
        const receipt = decodeReceipt({
          schema: PREDICT_RECEIPT_SCHEMA_LITERAL,
          receiptId: `receipt:${args.deps.sha256(`${args.candidateId}:${completion.text}`)}`,
          signatureId: args.signature.signatureId,
          candidateId: args.candidateId,
          promptDigest: args.deps.sha256(rendered),
          outputDigest: args.deps.sha256(completion.text),
          decodeOutcome: repairCount === 0 ? "decoded" : "repaired",
          repairCount,
          usageTruth: completion.usageTruth,
          outputChars: completion.text.length,
          observedAt: args.deps.now(),
        });
        return { output: decoded.success, receipt };
      }

      lastReason = "output did not match the schema";
      repairCount += 1;
    }

    return yield* new DseDecodeError({ reason: lastReason, repairCount: maxRepairs });
  });
