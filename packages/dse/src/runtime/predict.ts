import { Effect, JSONSchema, Schema } from "effect";

import type { DseSignature } from "../signature.js";
import type { DseParams } from "../params.js";
import { isBlobRef, type BlobRef } from "../blob.js";

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

    const resolveModelConfig = (
      role: "main" | "sub" | "repair" | "judge"
    ): { readonly modelId?: string; readonly temperature?: number; readonly topP?: number; readonly maxTokens?: number } => {
      const base = params.model ?? {};
      const roles = params.modelRoles ?? {};
      const override = (roles as any)[role] ?? {};
      return {
        ...(base.modelId ? { modelId: base.modelId } : {}),
        ...(typeof base.temperature === "number" ? { temperature: base.temperature } : {}),
        ...(typeof base.topP === "number" ? { topP: base.topP } : {}),
        ...(typeof base.maxTokens === "number" ? { maxTokens: base.maxTokens } : {}),
        ...(override.modelId ? { modelId: override.modelId } : {}),
        ...(typeof override.temperature === "number" ? { temperature: override.temperature } : {}),
        ...(typeof override.topP === "number" ? { topP: override.topP } : {}),
        ...(typeof override.maxTokens === "number" ? { maxTokens: override.maxTokens } : {})
      };
    };

    const modelMain = resolveModelConfig("main");

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
            ...(modelMain.modelId ? { modelId: modelMain.modelId } : {}),
            ...(typeof modelMain.temperature === "number"
              ? { temperature: modelMain.temperature }
              : {}),
            ...(typeof modelMain.topP === "number" ? { topP: modelMain.topP } : {}),
            ...(typeof modelMain.maxTokens === "number"
              ? { maxTokens: modelMain.maxTokens }
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
        modelId: modelMain.modelId,
        temperature: modelMain.temperature,
        topP: modelMain.topP,
        maxTokens: modelMain.maxTokens ?? signature.defaults.constraints.maxTokens
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

        const modelRepair = resolveModelConfig("repair");
        yield* budget.onLmCall();
        const repaired = yield* lm.complete({
          messages: [
            ...messages,
            { role: "assistant", content: rawText.slice(0, 5000) },
            { role: "user", content: repairPromptFor(rawText, lastError) }
          ],
          modelId: modelRepair.modelId ?? modelMain.modelId,
          temperature: 0,
          topP: 1,
          maxTokens: Math.min(256, modelRepair.maxTokens ?? modelMain.maxTokens ?? 256)
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

      const controllerExtra = params.rlmLite?.controllerInstructions;
      const chunkDefaults = params.rlmLite?.chunkDefaults;
      const chunkDefaultsText = chunkDefaults
        ? [
            "Default chunking policy (use unless you have a good reason not to):",
            `- chunkChars=${Math.max(1, Math.floor(chunkDefaults.chunkChars))}`,
            `- overlapChars=${Math.max(0, Math.floor(chunkDefaults.overlapChars ?? 0))}`,
            `- maxChunks=${Math.max(1, Math.floor(chunkDefaults.maxChunks ?? 50))}`
          ].join("\n")
        : null;

      const controllerSystem = [
        baseMessages.find((m) => m.role === "system")?.content ?? "",
        "You are an RLM-lite controller. You must choose the next action to execute.",
        ...(controllerExtra ? [controllerExtra] : []),
        "Rules:",
        "- Output MUST be a single JSON object that matches the RLM Action schema exactly.",
        "- Do not wrap in markdown fences.",
        "- Prefer bounded operations: preview/search/chunk + extract_over_chunks, then Final.",
        ...(chunkDefaultsText ? [chunkDefaultsText] : []),
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
          signatureId: signature.id,
          receiptId,
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

        const encodedInput = yield* Effect.try({
          try: () => Schema.encodeSync(signature.input)(input),
          catch: (cause) =>
            RlmKernelError.make({
              message: "Failed to encode signature input for RLM trace export",
              cause
            })
        });

        // Phase F: include the encoded signature input in the trace so we can export candidate
        // labeled examples (inputJson + expectedJson) from RLM traces.
        pushTrace({ _tag: "Input", input: encodedInput });

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
            modelId: modelMain.modelId,
            temperature: modelMain.temperature,
            topP: modelMain.topP,
            maxTokens: Math.min(1024, modelMain.maxTokens ?? 1024)
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

    // Phase F: a distilled tactic derived from common RLM traces for long-context "needle in haystack"
    // tasks. It attempts a deterministic search + parse pass first (0 LM calls), and falls back to
    // RLM-lite (or direct) when the tactic doesn't apply.
    const distilledSearchLineExtractPredict: Effect.Effect<O, PredictError, PredictEnv> = Effect.gen(function* () {
      const rendered = yield* renderPromptMessagesWithStats({
        signature,
        input,
        params,
        blobContextMode: "metadata_only"
      });
      promptRenderStats = rendered.stats;
      contextPressure = contextPressureFromRenderStats(rendered.stats);
      renderedHash = yield* renderedPromptHash(rendered.messages);

      const encodedInput = yield* Effect.try({
        try: () => Schema.encodeSync(signature.input)(input),
        catch: (cause) =>
          PredictStrategyError.make({
            strategyId,
            message: `distilled.search_line_extract.v1 failed to encode input: ${String(cause)}`
          })
      });

      const record =
        encodedInput && typeof encodedInput === "object"
          ? (encodedInput as Record<string, unknown>)
          : null;

      const question = typeof record?.question === "string" ? String(record.question) : null;
      const blobsRaw = Array.isArray(record?.blobs) ? (record.blobs as ReadonlyArray<unknown>) : null;
      const blobs: Array<BlobRef> = blobsRaw
        ? blobsRaw.filter((b): b is BlobRef => isBlobRef(b))
        : [];

      if (!question || blobs.length === 0) {
        // Not applicable; fall back to baseline behavior.
        return yield* directPredict;
      }

      const queries: Array<string> = [];

      for (const m of question.matchAll(/\"([^\"]{3,})\"/g)) {
        const s = String(m[1] ?? "").trim();
        if (s.length > 0) queries.push(s);
      }

      if (question.includes("unexpected EOF while reading headers")) {
        queries.push("unexpected EOF while reading headers");
      }

      const qLower = question.toLowerCase();
      if (qLower.includes("oa_req")) queries.push("oa_req=");
      if (question.includes("NEEDLE")) queries.push("export const NEEDLE");

      if (queries.length === 0) {
        // Fallback heuristic: search for a short prefix of the question.
        queries.push(question.slice(0, 80).trim());
      }

      const uniqQueries = Array.from(
        new Set(
          queries
            .map((q) => q.trim())
            .filter((q) => q.length >= 3)
        )
      ).sort((a, b) => b.length - a.length);

      const blobStore = yield* BlobStoreService;

      const lineAroundIndex = (text: string, index: number): string => {
        const start = text.lastIndexOf("\n", Math.max(0, index - 1));
        const end = text.indexOf("\n", index);
        const from = start === -1 ? 0 : start + 1;
        const to = end === -1 ? text.length : end;
        return text.slice(from, to);
      };

      for (const blob of blobs) {
        yield* budget.checkTime();

        const text = yield* blobStore.getText(blob.id).pipe(
          Effect.catchAll(() =>
            Effect.fail(
              PredictStrategyError.make({
                strategyId,
                message: `distilled.search_line_extract.v1 failed to read blob (blobId=${blob.id})`
              })
            )
          )
        );

        if (text == null) continue;

        for (const query of uniqQueries) {
          const idx = text.indexOf(query);
          if (idx < 0) continue;

          const quote = lineAroundIndex(text, idx);
          if (quote.trim().length === 0) continue;

          let answer: string | null = null;
          const oaReq = quote.match(/oa_req=([^\s]+)/);
          if (oaReq && oaReq[1]) answer = String(oaReq[1]);

          const needle = quote.match(/NEEDLE\\s*=\\s*\"([^\"]+)\"/);
          if (!answer && needle && needle[1]) answer = String(needle[1]);

          if (!answer) continue;

          const candidateUnknown = {
            answer,
            evidence: { blobId: blob.id, quote }
          };

          let decoded: O | null = null;
          try {
            decoded = Schema.decodeUnknownSync(signature.output)(candidateUnknown);
          } catch {
            decoded = null;
          }

          if (!decoded) continue;

          const outputEncoded = yield* Effect.try({
            try: () => Schema.encodeSync(signature.output)(decoded),
            catch: (cause) =>
              OutputDecodeError.make({
                message: "Failed to encode decoded output (distilled strategy)",
                cause
              })
          });

          outputHash = yield* sha256IdFromCanonicalJson(outputEncoded);
          return decoded;
        }
      }

      // No confident extraction; fall back to a more general strategy.
      const snap = yield* budget.snapshot();
      const canRlm =
        snap.limits.maxRlmIterations !== undefined && snap.limits.maxSubLmCalls !== undefined;
      if (canRlm) return yield* rlmLitePredict;
      return yield* directPredict;
    });

    const main: Effect.Effect<O, PredictError, PredictEnv> = (() => {
      switch (strategyId) {
        case "direct.v1":
          return directPredict;
        case "rlm_lite.v1":
          return rlmLitePredict;
        case "distilled.search_line_extract.v1":
          return distilledSearchLineExtractPredict;
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
