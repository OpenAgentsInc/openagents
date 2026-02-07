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
import { BlobStoreService } from "./blobStore.js";
import { LmClientError, LmClientService } from "./lm.js";
import { PolicyRegistryError, PolicyRegistryService } from "./policyRegistry.js";
import { PromptRenderError, renderPromptMessages } from "./render.js";
import { ReceiptRecorderError, ReceiptRecorderService } from "./receipt.js";

export type PredictEnv =
  | LmClientService
  | PolicyRegistryService
  | BlobStoreService
  | ReceiptRecorderService;

export type PredictError =
  | LmClientError
  | PolicyRegistryError
  | PromptRenderError
  | OutputDecodeError
  | ReceiptRecorderError
  | HashError;

export function make<I, O>(signature: DseSignature<I, O>) {
  return Effect.fn(`dse.Predict(${signature.id})`)(function* (input: I) {
    const startedAtMs = Date.now();
    const lm = yield* LmClientService;
    const registry = yield* PolicyRegistryService;
    const receipts = yield* ReceiptRecorderService;

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

    const messages = yield* renderPromptMessages({
      signature,
      input,
      params
    });

    const renderedHash = yield* renderedPromptHash(messages);

    const response = yield* lm.complete({
      messages,
      modelId: params.model?.modelId,
      temperature: params.model?.temperature,
      topP: params.model?.topP,
      maxTokens: params.model?.maxTokens ?? signature.defaults.constraints.maxTokens
    });

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

    let rawText = response.text;
    let decoded: O | null = null;
    let repairs = 0;
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

    const outputHash = yield* sha256IdFromCanonicalJson(outputEncoded);

    const endedAtMs = Date.now();

    yield* receipts.record({
      format: "openagents.dse.predict_receipt",
      formatVersion: 1,
      receiptId: crypto.randomUUID(),
      createdAt: new Date().toISOString(),
      signatureId: signature.id,
      compiled_id,
      hashes: {
        inputSchemaHash,
        outputSchemaHash,
        promptIrHash: promptHash,
        renderedPromptHash: renderedHash,
        paramsHash: paramsHashValue,
        outputHash
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
      ...(response.usage ? { usage: response.usage } : {}),
      timing: {
        startedAtMs,
        endedAtMs,
        durationMs: Math.max(0, endedAtMs - startedAtMs)
      },
      ...(repairs > 0 ? { repairCount: repairs } : {}),
      result: { _tag: "Ok" }
    });

    return decoded;
  });
}
