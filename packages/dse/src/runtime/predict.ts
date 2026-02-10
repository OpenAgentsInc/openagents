import { Effect, JSONSchema, Schema } from "effect";

import type { DseSignature } from "../signature.js";
import type { DseParams } from "../params.js";

import { canonicalJson } from "../internal/canonicalJson.js";
import {
  paramsHash,
  promptIrHash,
  renderedPromptHash,
  schemaJsonHash,
  sha256IdFromCanonicalJson,
  type HashError
} from "../hashes.js";
import { decodeJsonOutputWithMode, OutputDecodeError } from "./decode.js";
import { BudgetExceededError, ExecutionBudgetService } from "./budget.js";
import { BlobStoreService } from "./blobStore.js";
import { LmClientError, LmClientService } from "./lm.js";
import { PolicyRegistryError, PolicyRegistryService } from "./policyRegistry.js";
import {
  contextPressureFromRenderStats,
  type ContextPressureV1
} from "./contextPressure.js";
import {
  PromptRenderError,
  renderPromptMessagesWithStats,
  type PromptRenderStatsV1
} from "./render.js";
import { ReceiptRecorderError, ReceiptRecorderService } from "./receipt.js";

export type PredictEnv =
  | LmClientService
  | PolicyRegistryService
  | BlobStoreService
  | ReceiptRecorderService
  | ExecutionBudgetService;

export class PredictStrategyError extends Schema.TaggedError<PredictStrategyError>()(
  "PredictStrategyError",
  {
    strategyId: Schema.String,
    message: Schema.String
  }
) {}

export type PredictError =
  | BudgetExceededError
  | LmClientError
  | PolicyRegistryError
  | PromptRenderError
  | OutputDecodeError
  | PredictStrategyError
  | ReceiptRecorderError
  | HashError;

export function make<I, O>(
  signature: DseSignature<I, O>
): (input: I) => Effect.Effect<O, PredictError, PredictEnv> {
  return Effect.fn(`dse.Predict(${signature.id})`)(function* (input: I) {
    const receiptId = crypto.randomUUID();
    const startedAtMs = Date.now();
    const lm = yield* LmClientService;
    const registry = yield* PolicyRegistryService;
    const receipts = yield* ReceiptRecorderService;
    const budgets = yield* ExecutionBudgetService;

    const active = yield* registry.getActive(signature.id);
    const artifact = active
      ? yield* registry.getArtifact(signature.id, active.compiledId)
      : null;

    if (active && !artifact) {
      return yield* Effect.fail(
        PolicyRegistryError.make({
          message: `Active artifact not found (signatureId=${signature.id} compiledId=${active.compiledId})`
        })
      );
    }

    const params: DseParams = artifact?.params ?? signature.defaults.params;
    const budget = yield* budgets.start({
      runId: receiptId,
      startedAtMs,
      limits: params.budgets ?? {}
    });

    const decodeMode = params.decode?.mode ?? "strict_json";
    const maxRepairs = Math.max(0, params.decode?.maxRepairs ?? 0);

    const [inputSchemaHash, outputSchemaHash, promptHash, paramsHashValue] =
      yield* Effect.all([
        schemaJsonHash(signature.input),
        schemaJsonHash(signature.output),
        promptIrHash(signature.prompt),
        paramsHash(params)
      ]);

    // For now, compiled_id is the canonical policy hash (params). Artifacts, when present,
    // carry their own compiled_id which should match this value.
    const compiled_id = artifact?.compiled_id ?? paramsHashValue;

    const outputSchemaJson = JSONSchema.make(signature.output);

    const repairPromptFor = (rawText: string, lastError: OutputDecodeError) => {
      const MAX_PREVIEW = 2000;
      const preview = rawText.trim().slice(0, MAX_PREVIEW);
      return (
        "Your previous output did not match the required JSON schema.\n" +
        "- Output MUST be JSON only.\n" +
        "- Do not wrap in markdown fences.\n" +
        "- Do not include extra keys.\n" +
        "\n" +
        "Required schema:\n" +
        canonicalJson(outputSchemaJson) +
        "\n\n" +
        "Previous output (truncated):\n" +
        preview +
        "\n\n" +
        "Error:\n" +
        lastError.message
      );
    };

    const decodeOnce = (rawText: string) =>
      decodeJsonOutputWithMode(signature.output, rawText, { mode: decodeMode });

    let renderedHash: string | undefined;
    let repairs = 0;
    let usage: { readonly promptTokens?: number; readonly completionTokens?: number; readonly totalTokens?: number } | undefined;
    let outputHash: string | undefined;
    let promptRenderStats: PromptRenderStatsV1 | undefined;
    let contextPressure: ContextPressureV1 | undefined;

    // Phase B: strategy is pinned via params (and thus via compiled artifacts).
    const strategyId = params.strategy?.id ?? "direct.v1";

    const errorSummary = (error: unknown): { readonly errorName: string; readonly message: string } => {
      if (error && typeof error === "object") {
        const name = (error as any)._tag ?? (error as any).name;
        const message = (error as any).message;
        if (typeof message === "string") {
          return { errorName: typeof name === "string" ? name : "PredictError", message };
        }
      }
      return { errorName: "PredictError", message: String(error) };
    };

    const recordReceipt = (
      result: { readonly _tag: "Ok" } | { readonly _tag: "Error"; readonly errorName: string; readonly message: string }
    ) =>
      Effect.gen(function* () {
        const endedAtMs = Date.now();
        const budgetSnapshot = yield* budget.snapshot();

        yield* receipts.record({
          format: "openagents.dse.predict_receipt",
          formatVersion: 1,
          receiptId,
          runId: receiptId,
          createdAt: new Date().toISOString(),
          signatureId: signature.id,
          compiled_id,
          strategyId,
          hashes: {
            inputSchemaHash,
            outputSchemaHash,
            promptIrHash: promptHash,
            ...(renderedHash ? { renderedPromptHash: renderedHash } : {}),
            paramsHash: paramsHashValue,
            ...(outputHash ? { outputHash } : {})
          },
          model: {
            ...(params.model?.modelId ? { modelId: params.model.modelId } : {}),
            ...(typeof params.model?.temperature === "number"
              ? { temperature: params.model.temperature }
              : {}),
            ...(typeof params.model?.topP === "number" ? { topP: params.model.topP } : {}),
            ...(typeof params.model?.maxTokens === "number"
              ? { maxTokens: params.model.maxTokens }
              : {})
          },
          ...(usage ? { usage } : {}),
          timing: {
            startedAtMs,
            endedAtMs,
            durationMs: Math.max(0, endedAtMs - startedAtMs)
          },
          ...(promptRenderStats ? { promptRenderStats } : {}),
          ...(contextPressure ? { contextPressure } : {}),
          ...(repairs > 0 ? { repairCount: repairs } : {}),
          budget: budgetSnapshot,
          result
        });
      });

    const directPredict = Effect.gen(function* () {
      const rendered = yield* renderPromptMessagesWithStats({
        signature,
        input,
        params
      });
      const messages = rendered.messages;
      promptRenderStats = rendered.stats;
      contextPressure = contextPressureFromRenderStats(rendered.stats);

      renderedHash = yield* renderedPromptHash(messages);

      yield* budget.checkTime();
      yield* budget.onLmCall();
      const response = yield* lm.complete({
        messages,
        modelId: params.model?.modelId,
        temperature: params.model?.temperature,
        topP: params.model?.topP,
        maxTokens: params.model?.maxTokens ?? signature.defaults.constraints.maxTokens
      });

      usage = response.usage;
      yield* budget.onOutputChars(response.text.length);

      let rawText = response.text;
      let decoded: O | null = null;
      let lastError: OutputDecodeError | null = null;

      while (true) {
        const attempt = yield* Effect.either(decodeOnce(rawText));
        if (attempt._tag === "Right") {
          decoded = attempt.right;
          break;
        }

        lastError = attempt.left;

        if (repairs >= maxRepairs) {
          return yield* Effect.fail(lastError);
        }

        yield* budget.onLmCall();
        const repaired = yield* lm.complete({
          messages: [
            ...messages,
            { role: "assistant", content: rawText.slice(0, 5000) },
            { role: "user", content: repairPromptFor(rawText, lastError) }
          ],
          modelId: params.model?.modelId,
          temperature: 0,
          topP: 1,
          maxTokens: Math.min(256, params.model?.maxTokens ?? 256)
        });

        yield* budget.onOutputChars(repaired.text.length);

        rawText = repaired.text;
        repairs++;
      }

      const outputEncoded = yield* Effect.try({
        try: () => Schema.encodeSync(signature.output)(decoded),
        catch: (cause) =>
          OutputDecodeError.make({
            message: "Failed to encode decoded output",
            cause
          })
      });

      outputHash = yield* sha256IdFromCanonicalJson(outputEncoded);

      return decoded;
    });

    const rlmLitePredict: Effect.Effect<O, PredictError, PredictEnv> = Effect.fail(
      PredictStrategyError.make({
        strategyId,
        message: "Predict strategy rlm_lite.v1 is not implemented yet (Phase C)."
      })
    );

    const main: Effect.Effect<O, PredictError, PredictEnv> = (() => {
      switch (strategyId) {
        case "direct.v1":
          return directPredict;
        case "rlm_lite.v1":
          return rlmLitePredict;
        default:
          return Effect.fail(
            PredictStrategyError.make({
              strategyId,
              message: `Unknown predict strategy: ${strategyId}`
            })
          );
      }
    })();

    return yield* main.pipe(
      Effect.tap(() => recordReceipt({ _tag: "Ok" })),
      Effect.tapError((error) => {
        const summary = errorSummary(error);
        return recordReceipt({ _tag: "Error", errorName: summary.errorName, message: summary.message });
      })
    );
  });
}
