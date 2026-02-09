import { Effect, Layer, Schema } from "effect";

import { BlobStore, Budget, CompiledArtifact, Lm, Policy, Receipt } from "@openagentsinc/dse";

import { api } from "../../convex/_generated/api";
import { ConvexService } from "../effect/convex";
import { RequestContextService } from "../effect/requestContext";

const asError = (u: unknown): Error => (u instanceof Error ? u : new Error(String(u)));

export const makeWorkersAiDseLmClient = (input: {
  readonly binding: Ai;
  readonly defaultModelId: string;
}): Lm.LmClient => {
  return {
    complete: (req) =>
      Effect.tryPromise({
        try: async () => {
          const modelId = req.modelId ?? input.defaultModelId;
          const messages = req.messages.map((m) => ({
            role: m.role,
            content: m.content,
          }));

          const output = (await input.binding.run(
            modelId as any,
            {
              model: modelId,
              max_tokens: req.maxTokens ?? 256,
              messages: messages as any,
              ...(typeof req.temperature === "number" ? { temperature: req.temperature } : {}),
              ...(typeof req.topP === "number" ? { top_p: req.topP } : {}),
            } as any,
            {},
          )) as any;

          const text =
            output?.choices?.[0]?.message?.content ??
            (typeof output?.response === "string" ? output.response : JSON.stringify(output?.response ?? ""));

          const usageRaw = output?.usage as any;
          const promptTokens = typeof usageRaw?.prompt_tokens === "number" ? usageRaw.prompt_tokens : undefined;
          const completionTokens =
            typeof usageRaw?.completion_tokens === "number" ? usageRaw.completion_tokens : undefined;
          const totalTokens =
            promptTokens !== undefined && completionTokens !== undefined ? promptTokens + completionTokens : undefined;

          return {
            text: typeof text === "string" ? text : String(text),
            ...(promptTokens !== undefined || completionTokens !== undefined || totalTokens !== undefined
              ? {
                  usage: {
                    ...(promptTokens !== undefined ? { promptTokens } : {}),
                    ...(completionTokens !== undefined ? { completionTokens } : {}),
                    ...(totalTokens !== undefined ? { totalTokens } : {}),
                  },
                }
              : {}),
          } satisfies Lm.LmResponse;
        },
        catch: (cause) =>
          Lm.LmClientError.make({
            message: "DSE LM client failed",
            cause,
          }),
      }),
  };
};

const OPENROUTER_BASE = "https://openrouter.ai/api/v1";

export const makeOpenRouterDseLmClient = (input: {
  readonly apiKey: string;
  readonly defaultModelId: string;
  readonly fetch?: typeof fetch;
}): Lm.LmClient => {
  const doFetch = input.fetch ?? fetch;
  return {
    complete: (req) =>
      Effect.tryPromise({
        try: async () => {
          const modelId = req.modelId ?? input.defaultModelId;
          const messages = req.messages.map((m) => ({ role: m.role, content: m.content }));
          const res = await doFetch(`${OPENROUTER_BASE}/chat/completions`, {
            method: "POST",
            headers: {
              Authorization: `Bearer ${input.apiKey}`,
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: modelId,
              max_tokens: req.maxTokens ?? 256,
              messages,
              ...(typeof req.temperature === "number" ? { temperature: req.temperature } : {}),
              ...(typeof req.topP === "number" ? { top_p: req.topP } : {}),
            }),
          });
          if (!res.ok) {
            const text = await res.text();
            throw new Error(`OpenRouter API error ${res.status}: ${text}`);
          }
          const output = (await res.json()) as any;
          const text = output?.choices?.[0]?.message?.content ?? "";
          const usageRaw = output?.usage;
          const promptTokens = typeof usageRaw?.prompt_tokens === "number" ? usageRaw.prompt_tokens : undefined;
          const completionTokens = typeof usageRaw?.completion_tokens === "number" ? usageRaw.completion_tokens : undefined;
          const totalTokens =
            promptTokens !== undefined && completionTokens !== undefined ? promptTokens + completionTokens : undefined;
          return {
            text: typeof text === "string" ? text : String(text),
            ...(promptTokens !== undefined || completionTokens !== undefined || totalTokens !== undefined
              ? {
                  usage: {
                    ...(promptTokens !== undefined ? { promptTokens } : {}),
                    ...(completionTokens !== undefined ? { completionTokens } : {}),
                    ...(totalTokens !== undefined ? { totalTokens } : {}),
                  },
                }
              : {}),
          } satisfies Lm.LmResponse;
        },
        catch: (cause) =>
          Lm.LmClientError.make({
            message: "OpenRouter DSE LM client failed",
            cause,
          }),
      }),
  };
};

export type DseLmClientEnv = {
  readonly OPENROUTER_API_KEY?: string;
  readonly AI?: Ai;
};

/** DSE LM client: OpenRouter (moonshotai/kimi-k2.5) primary when OPENROUTER_API_KEY is set, Cloudflare Workers AI fallback. */
export const makeDseLmClientWithOpenRouterPrimary = (input: {
  readonly env: DseLmClientEnv;
  readonly defaultModelIdCf: string;
  readonly primaryModelOpenRouter: string;
}): Lm.LmClient => {
  const cfClient = input.env.AI
    ? makeWorkersAiDseLmClient({ binding: input.env.AI, defaultModelId: input.defaultModelIdCf })
    : null;
  const openRouterKey =
    typeof input.env.OPENROUTER_API_KEY === "string" && input.env.OPENROUTER_API_KEY.length > 0
      ? input.env.OPENROUTER_API_KEY
      : null;
  const openRouterClient = openRouterKey
    ? makeOpenRouterDseLmClient({
        apiKey: openRouterKey,
        defaultModelId: input.primaryModelOpenRouter,
      })
    : null;

  if (!cfClient) {
    return openRouterClient ?? { complete: () => Effect.fail(Lm.LmClientError.make({ message: "No AI binding", cause: undefined })) };
  }
  if (!openRouterClient) {
    return cfClient;
  }
  return {
    complete: (req) =>
      openRouterClient.complete(req).pipe(Effect.catchAll(() => cfClient.complete(req))),
  };
};

const stableBucket100 = (key: string): number => {
  // FNV-1a 32-bit hash, then mod 100. Deterministic across runtimes.
  let h = 2166136261;
  for (let i = 0; i < key.length; i++) {
    h ^= key.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0) % 100;
};

export const layerDsePolicyRegistryFromConvex = (input: {
  readonly threadId: string;
}): Layer.Layer<Policy.PolicyRegistryService, never, ConvexService | RequestContextService> =>
  Layer.effect(
    Policy.PolicyRegistryService,
    Effect.gen(function* () {
      const convex = yield* ConvexService;
      const requestContext = yield* RequestContextService;

      const getActive: Policy.PolicyRegistry["getActive"] = (signatureId) =>
        Effect.gen(function* () {
          // Stage 6: optional canary selection (deterministic per thread).
          const canaryRes = yield* convex
            .query(api.dse.canary.getCanary, { signatureId, threadId: input.threadId } as any)
            .pipe(Effect.provideService(RequestContextService, requestContext));

          const canary = (canaryRes as any)?.canary ?? null;
          if (canary && typeof canary === "object") {
            const enabled = Boolean((canary as any).enabled);
            const rolloutPct = Number((canary as any).rolloutPct ?? 0);
            const salt = String((canary as any).salt ?? "");
            const canaryCompiled = String((canary as any).canary_compiled_id ?? "");
            const controlCompiled = String((canary as any).control_compiled_id ?? "");

            if (enabled && rolloutPct > 0 && canaryCompiled && controlCompiled) {
              const bucket = stableBucket100(`${salt}:${input.threadId}:${signatureId}`);
              const chosen = bucket < rolloutPct ? canaryCompiled : controlCompiled;
              return { compiledId: chosen };
            }
          }

          const res = yield* convex
            .query(api.dse.active.getActive, { signatureId } as any)
            .pipe(Effect.provideService(RequestContextService, requestContext));

          const compiled_id = typeof (res as any)?.compiled_id === "string" ? (res as any).compiled_id : null;
          return compiled_id ? { compiledId: compiled_id } : null;
        }).pipe(
          Effect.mapError((cause) =>
            Policy.PolicyRegistryError.make({ message: "Failed to read active DSE policy", cause }),
          ),
        );

      const setActive: Policy.PolicyRegistry["setActive"] = (signatureId, policy) =>
        convex
          .mutation(api.dse.active.setActive, {
            signatureId,
            compiled_id: policy.compiledId,
            reason: "PolicyRegistryService.setActive",
          } as any)
          .pipe(
            Effect.provideService(RequestContextService, requestContext),
            Effect.asVoid,
            Effect.mapError((cause) =>
              Policy.PolicyRegistryError.make({ message: "Failed to set active DSE policy", cause }),
            ),
          );

      const clearActive: Policy.PolicyRegistry["clearActive"] = (signatureId) =>
        convex
          .mutation(api.dse.active.clearActive, { signatureId, reason: "PolicyRegistryService.clearActive" } as any)
          .pipe(
            Effect.provideService(RequestContextService, requestContext),
            Effect.asVoid,
            Effect.mapError((cause) =>
              Policy.PolicyRegistryError.make({ message: "Failed to clear active DSE policy", cause }),
            ),
          );

      const getArtifact: Policy.PolicyRegistry["getArtifact"] = (signatureId, compiledId) =>
        convex
          .query(api.dse.artifacts.getArtifact, { signatureId, compiled_id: compiledId } as any)
          .pipe(
            Effect.provideService(RequestContextService, requestContext),
            Effect.flatMap((res: any) => {
              const raw = res?.artifact as unknown;
              if (!raw) return Effect.succeed(null);
              return Effect.try({
                try: () => Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(raw),
                catch: (cause) =>
                  Policy.PolicyRegistryError.make({
                    message: "Failed to decode DSE artifact from Convex",
                    cause,
                  }),
              });
            }),
            Effect.mapError((cause) => {
              // Preserve typed PolicyRegistryError when thrown above.
              if (cause && typeof cause === "object" && (cause as any)._tag === "PolicyRegistryError") return cause as any;
              return Policy.PolicyRegistryError.make({ message: "Failed to read DSE artifact", cause: asError(cause) });
            }),
          );

      const putArtifact: Policy.PolicyRegistry["putArtifact"] = (artifact) =>
        Effect.try({
          try: () => Schema.encodeSync(CompiledArtifact.DseCompiledArtifactV1Schema)(artifact),
          catch: (cause) =>
            Policy.PolicyRegistryError.make({ message: "Failed to encode DSE artifact for storage", cause }),
        }).pipe(
          Effect.flatMap((encoded) =>
            convex.mutation(api.dse.artifacts.putArtifact, {
              signatureId: artifact.signatureId,
              compiled_id: artifact.compiled_id,
              json: encoded,
            } as any),
          ),
          Effect.provideService(RequestContextService, requestContext),
          Effect.asVoid,
          Effect.mapError((cause) => {
            if (cause && typeof cause === "object" && (cause as any)._tag === "PolicyRegistryError") return cause as any;
            return Policy.PolicyRegistryError.make({ message: "Failed to write DSE artifact", cause: asError(cause) });
          }),
        );

      return Policy.PolicyRegistryService.of({ getActive, setActive, clearActive, getArtifact, putArtifact });
    }),
  );

export const layerDseReceiptRecorderFromConvex = (input: {
  readonly threadId: string;
  readonly anonKey: string | null;
  readonly runId: string;
  readonly onReceipt?: ((receipt: Receipt.Receipt) => void) | undefined;
}): Layer.Layer<Receipt.ReceiptRecorderService, never, ConvexService | RequestContextService> =>
  Layer.effect(
    Receipt.ReceiptRecorderService,
    Effect.gen(function* () {
      const convex = yield* ConvexService;
      const requestContext = yield* RequestContextService;

      const record: Receipt.ReceiptRecorder["record"] = (receipt) =>
        Effect.sync(() => {
          try {
            input.onReceipt?.(receipt);
          } catch {
            // Ignore callback errors; recording must remain best-effort and deterministic.
          }
        }).pipe(
          Effect.zipRight(
            convex.mutation(api.dse.receipts.recordPredictReceipt, {
              threadId: input.threadId,
              ...(input.anonKey ? { anonKey: input.anonKey } : {}),
              runId: input.runId,
              receipt,
            } as any),
          ),
          Effect.provideService(RequestContextService, requestContext),
          Effect.asVoid,
          Effect.mapError((cause) =>
            Receipt.ReceiptRecorderError.make({ message: "Failed to record DSE receipt", cause }),
          ),
        );

      return Receipt.ReceiptRecorderService.of({ record });
    }),
  );

export const layerDsePredictEnvForAutopilotRun = (input: {
  readonly threadId: string;
  readonly anonKey: string | null;
  readonly runId: string;
  readonly onReceipt?: ((receipt: Receipt.Receipt) => void) | undefined;
}): Layer.Layer<
  | Policy.PolicyRegistryService
  | BlobStore.BlobStoreService
  | Receipt.ReceiptRecorderService
  | Budget.ExecutionBudgetService,
  never,
  ConvexService | RequestContextService
> =>
  Layer.mergeAll(
    layerDsePolicyRegistryFromConvex({ threadId: input.threadId }),
    BlobStore.layerInMemory(),
    Budget.layerInMemory(),
    layerDseReceiptRecorderFromConvex(input),
  );
