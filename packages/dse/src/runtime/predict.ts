import { Effect, JSONSchema, Schema } from "effect";

import type { DseSignature } from "../signature.js";
import type { DseParams } from "../params.js";
import type { BlobRef } from "../blob.js";

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
import { executeRlmAction, RlmActionV1Schema, RlmKernelError } from "./rlmKernel.js";
import { VarSpaceError, VarSpaceService } from "./varSpace.js";

export type PredictEnv =
  | LmClientService
  | PolicyRegistryService
  | BlobStoreService
  | VarSpaceService
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
  | RlmKernelError
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
    let rlmTrace: { readonly blob: BlobRef; readonly eventCount: number } | undefined;

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
          ...(rlmTrace ? { rlmTrace } : {}),
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

    const rlmLitePredict: Effect.Effect<O, PredictError, PredictEnv> = Effect.gen(function* () {
      const budget0 = yield* budget.snapshot();
      const maxIterations = budget0.limits.maxRlmIterations;
      const maxSubLmCalls = budget0.limits.maxSubLmCalls;
      if (maxIterations === undefined) {
        return yield* Effect.fail(
          PredictStrategyError.make({
            strategyId,
            message:
              "rlm_lite.v1 requires budgets.maxRlmIterations to be set (pinned in params/artifact) to prevent unbounded loops."
          })
        );
      }
      if (maxSubLmCalls === undefined) {
        return yield* Effect.fail(
          PredictStrategyError.make({
            strategyId,
            message:
              "rlm_lite.v1 requires budgets.maxSubLmCalls to be set (pinned in params/artifact) to prevent unbounded recursion/fanout."
          })
        );
      }

      // Render the base prompt in "metadata only" mode for blob context entries so we don't
      // push full blob previews into token space. The full blobs remain accessible via RLM ops.
      const rendered = yield* renderPromptMessagesWithStats({
        signature,
        input,
        params,
        blobContextMode: "metadata_only"
      });
      promptRenderStats = rendered.stats;
      contextPressure = contextPressureFromRenderStats(rendered.stats);

      const baseMessages = rendered.messages;
      renderedHash = yield* renderedPromptHash(baseMessages);

      const actionSchemaJson = JSONSchema.make(RlmActionV1Schema);
      const actionSchemaText = canonicalJson(actionSchemaJson);

      const controllerSystem = [
        baseMessages.find((m) => m.role === "system")?.content ?? "",
        "You are an RLM-lite controller. You must choose the next action to execute.",
        "Rules:",
        "- Output MUST be a single JSON object that matches the RLM Action schema exactly.",
        "- Do not wrap in markdown fences.",
        "- Prefer bounded operations: preview/search/chunk + extract_over_chunks, then Final.",
        `- Budget: maxIterations=${maxIterations} maxSubLmCalls=${maxSubLmCalls}.`,
        "RLM Action schema (JSON Schema):",
        actionSchemaText
      ]
        .map((s) => s.trim())
        .filter((s) => s.length > 0)
        .join("\n\n");

      const controllerMessages: Array<{ readonly role: "system" | "user" | "assistant"; readonly content: string }> = [
        { role: "system", content: controllerSystem }
      ];

      for (const m of baseMessages) {
        if (m.role === "system") continue;
        controllerMessages.push(m);
      }

      const traceEvents: Array<unknown> = [];

      const stableBudgetForPrompt = (snap: {
        readonly limits: any;
        readonly usage: any;
      }) => ({
        limits: snap.limits,
        usage: {
          lmCalls: snap.usage.lmCalls,
          toolCalls: snap.usage.toolCalls,
          rlmIterations: snap.usage.rlmIterations,
          subLmCalls: snap.usage.subLmCalls,
          outputChars: snap.usage.outputChars
        }
      });

      const pushTrace = (event: unknown) => {
        const MAX_EVENTS = 10_000;
        if (traceEvents.length >= MAX_EVENTS) return;
        traceEvents.push(event);
      };

      const flushTraceToBlob = Effect.gen(function* () {
        if (traceEvents.length === 0) return;
        const blobStore = yield* BlobStoreService;
        const doc = {
          format: "openagents.dse.rlm_trace",
          formatVersion: 1,
          strategyId,
          events: traceEvents
        };
        const text = canonicalJson(doc);
        const attempt = yield* Effect.either(
          blobStore.putText({ text, mime: "application/json" })
        );
        if (attempt._tag === "Right") {
          rlmTrace = { blob: attempt.right, eventCount: traceEvents.length };
        }
      });

      // Seed VarSpace from the signature prompt context entries so blobs live in programmatic space.
      const run = Effect.gen(function* () {
        const vars = yield* VarSpaceService;

        const seedJsonIfAbsent = (name: string, value: unknown) =>
          vars.get(name).pipe(
            Effect.catchAll((cause: VarSpaceError) =>
              Effect.fail(
                RlmKernelError.make({
                  message: `VarSpace.get failed during seed (name=${name})`,
                  cause
                })
              )
            ),
            Effect.flatMap((existing) => {
              if (existing != null) return Effect.void;
              return vars.putJson(name, value).pipe(
                Effect.catchAll((cause: VarSpaceError) =>
                  Effect.fail(
                    RlmKernelError.make({
                      message: `VarSpace.putJson failed during seed (name=${name})`,
                      cause
                    })
                  )
                )
              );
            })
          );

        const seedBlobIfAbsent = (name: string, blob: BlobRef) =>
          vars.get(name).pipe(
            Effect.catchAll((cause: VarSpaceError) =>
              Effect.fail(
                RlmKernelError.make({
                  message: `VarSpace.get failed during seed (name=${name})`,
                  cause
                })
              )
            ),
            Effect.flatMap((existing) => {
              if (existing != null) return Effect.void;
              return vars.putBlob(name, blob).pipe(
                Effect.catchAll((cause: VarSpaceError) =>
                  Effect.fail(
                    RlmKernelError.make({
                      message: `VarSpace.putBlob failed during seed (name=${name})`,
                      cause
                    })
                  )
                )
              );
            })
          );

        for (const block of signature.prompt.blocks) {
          if (block._tag !== "Context") continue;
          for (const entry of block.entries) {
            if ("value" in entry) {
              yield* seedJsonIfAbsent(entry.key, entry.value);
            } else {
              yield* seedBlobIfAbsent(entry.key, entry.blob);
            }
          }
        }

        let lastObservation: unknown = null;

        for (let iteration = 1; iteration <= Math.max(0, Math.floor(maxIterations)); iteration++) {
          yield* budget.onRlmIteration();
          yield* budget.checkTime();

          const metas = yield* vars.list().pipe(
            Effect.catchAll((cause) =>
              Effect.fail(
                RlmKernelError.make({
                  message: "VarSpace.list failed",
                  cause
                })
              )
            )
          );
          const snap = yield* budget.snapshot();

          const state = {
            iteration,
            budgets: stableBudgetForPrompt(snap),
            vars: metas,
            ...(lastObservation != null ? { lastObservation } : {})
          };

          controllerMessages.push({
            role: "user",
            content: "RLM state:\n" + canonicalJson(state)
          });

          // Keep controller history bounded (system + base prompt + recent state/action/obs).
          const MAX_CONTROLLER_MESSAGES = 30;
          if (controllerMessages.length > MAX_CONTROLLER_MESSAGES) {
            const head = controllerMessages.slice(0, 2);
            const tail = controllerMessages.slice(controllerMessages.length - (MAX_CONTROLLER_MESSAGES - 2));
            controllerMessages.splice(0, controllerMessages.length, ...head, ...tail);
          }

          const promptHash = yield* renderedPromptHash(controllerMessages);

          yield* budget.onLmCall();
          const resp = yield* lm.complete({
            messages: controllerMessages,
            modelId: params.model?.modelId,
            temperature: params.model?.temperature,
            topP: params.model?.topP,
            maxTokens: Math.min(1024, params.model?.maxTokens ?? 1024)
          });
          yield* budget.onOutputChars(resp.text.length);

          controllerMessages.push({ role: "assistant", content: resp.text });

          const action = yield* decodeJsonOutputWithMode(RlmActionV1Schema, resp.text, { mode: "jsonish" });

          pushTrace({ iteration, promptHash, action });

          const step = yield* executeRlmAction({ action, params, budget });
          if (step._tag === "Final") {
            // Decode into the signature output schema.
            const decoded = yield* Effect.try({
              try: () => Schema.decodeUnknownSync(signature.output)(step.output),
	              catch: (cause) =>
	                OutputDecodeError.make({
	                  message: "RLM Final output did not match signature output schema",
	                  cause,
	                  outputPreview: (() => {
	                    try {
	                      return canonicalJson(step.output).slice(0, 500);
	                    } catch {
	                      return String(step.output).slice(0, 500);
	                    }
	                  })()
	                })
	            });

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
          }

          lastObservation = step.observation;
          pushTrace({ iteration, observation: step.observation });

          controllerMessages.push({
            role: "user",
            content: "Observation:\n" + canonicalJson(step.observation).slice(0, 10_000)
          });
        }

        return yield* Effect.fail(
          PredictStrategyError.make({
            strategyId,
            message: `rlm_lite.v1 exhausted iterations without producing Final (maxIterations=${maxIterations})`
          })
        );
      }).pipe(
        Effect.ensuring(flushTraceToBlob)
      );

      return yield* run;
    });

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
