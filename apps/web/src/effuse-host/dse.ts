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

export const layerDsePolicyRegistryFromConvex: Layer.Layer<
  Policy.PolicyRegistryService,
  never,
  ConvexService | RequestContextService
> = Layer.effect(
  Policy.PolicyRegistryService,
  Effect.gen(function* () {
    const convex = yield* ConvexService;
    const requestContext = yield* RequestContextService;

      const getActive: Policy.PolicyRegistry["getActive"] = (signatureId) =>
        convex.query(api.dse.active.getActive, { signatureId } as any).pipe(
          Effect.provideService(RequestContextService, requestContext),
          Effect.map((res: any) => {
            const compiled_id = typeof res?.compiled_id === "string" ? res.compiled_id : null;
            return compiled_id ? { compiledId: compiled_id } : null;
          }),
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
    layerDsePolicyRegistryFromConvex,
    BlobStore.layerInMemory(),
    Budget.layerInMemory(),
    layerDseReceiptRecorderFromConvex(input),
  );
