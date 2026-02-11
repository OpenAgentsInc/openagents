import { makeLndDeterministicLayer } from "@openagentsinc/lnd-effect/adapters";
import { makeInvoicePayerLndEffectLayer } from "@openagentsinc/lightning-effect/lnd-effect";
import {
  CredentialCacheInMemoryLayer,
  L402ClientLiveLayer,
  L402ClientService,
  L402TransportError,
  L402TransportService,
  makeSpendPolicyLayer,
  type L402FetchResult,
} from "@openagentsinc/lightning-effect";
import { Context, Effect, Layer } from "effect";

import type { ExecutorTask } from "./model";

export type L402ExecutionResult = Readonly<
  | {
      readonly status: "paid" | "cached";
      readonly amountMsats: number;
      readonly paymentId: string | null;
      readonly proofReference: string;
      readonly responseStatusCode: number;
      readonly cacheStatus: L402FetchResult["cacheStatus"];
    }
  | {
      readonly status: "blocked" | "failed";
      readonly errorCode: string;
      readonly denyReason: string;
    }
>;

export type L402ExecutorApi = Readonly<{
  readonly execute: (task: ExecutorTask) => Effect.Effect<L402ExecutionResult>;
}>;

export class L402ExecutorService extends Context.Tag("@openagents/desktop/L402ExecutorService")<
  L402ExecutorService,
  L402ExecutorApi
>() {}

const parseHeaders = (headers: Headers): Record<string, string> => {
  const out: Record<string, string> = {};
  headers.forEach((value, key) => {
    out[key] = value;
  });
  return out;
};

const LiveL402TransportLayer = Layer.succeed(
  L402TransportService,
  L402TransportService.of({
    send: (request) =>
      Effect.gen(function* () {
        const response = yield* Effect.tryPromise({
          try: async () => {
            const init: RequestInit = {
              method: request.method ?? "GET",
              cache: "no-store",
              ...(request.headers ? { headers: request.headers } : {}),
            };
            if (request.body !== undefined) init.body = request.body;
            return fetch(request.url, init);
          },
          catch: (error) =>
            L402TransportError.make({
              reason: String(error),
            }),
        });

        const bodyText = yield* Effect.tryPromise({
          try: async () => await response.text(),
          catch: (error) =>
            L402TransportError.make({
              reason: String(error),
              status: response.status,
            }),
        });

        return {
          status: response.status,
          headers: parseHeaders(response.headers),
          ...(bodyText.length > 0 ? { body: bodyText } : {}),
        };
      }),
  }),
);

const toDeniedResult = (error: { readonly _tag: string; readonly reason?: unknown }): L402ExecutionResult => ({
  status: "blocked",
  errorCode: error._tag,
  denyReason: typeof error.reason === "string" && error.reason.trim().length > 0 ? error.reason.trim() : error._tag,
});

const toFailedResult = (error: unknown): L402ExecutionResult => {
  if (error && typeof error === "object" && "_tag" in error && typeof (error as { _tag: unknown })._tag === "string") {
    const tagged = error as { readonly _tag: string; readonly reason?: unknown; readonly message?: unknown };
    const denyReason =
      typeof tagged.reason === "string" && tagged.reason.trim().length > 0
        ? tagged.reason
        : typeof tagged.message === "string" && tagged.message.trim().length > 0
          ? tagged.message
          : tagged._tag;
    return {
      status: "failed",
      errorCode: tagged._tag,
      denyReason,
    };
  }
  return {
    status: "failed",
    errorCode: "unknown_error",
    denyReason: String(error),
  };
};

export const L402ExecutorLive = Layer.effect(
  L402ExecutorService,
  Effect.gen(function* () {
    const baseDepsLayer = Layer.mergeAll(
      CredentialCacheInMemoryLayer,
      makeSpendPolicyLayer({
        defaultMaxSpendMsats: 5_000_000,
        allowedHosts: [],
        blockedHosts: [],
      }),
      LiveL402TransportLayer,
      makeInvoicePayerLndEffectLayer({
        fallbackAmountMsats: "request_max",
      }).pipe(Layer.provide(makeLndDeterministicLayer())),
    );
    const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseDepsLayer));

    const l402Client = yield* Effect.gen(function* () {
      return yield* L402ClientService;
    }).pipe(Effect.provide(clientLayer));

    const execute = Effect.fn("L402Executor.execute")(function* (task: ExecutorTask) {
      const exit = yield* Effect.either(l402Client.fetchWithL402(task.request));
      if (exit._tag === "Left") {
        const err = exit.left as { readonly _tag?: unknown; readonly reason?: unknown };
        if (err._tag === "BudgetExceededError" || err._tag === "DomainNotAllowedError") {
          return toDeniedResult({
            _tag: String(err._tag),
            reason: err.reason,
          });
        }
        return toFailedResult(exit.left);
      }

      const fetchResult = exit.right;
      const outcome: L402ExecutionResult = fetchResult.paid
        ? {
            status: "paid",
            amountMsats: fetchResult.amountMsats,
            paymentId: fetchResult.paymentId,
            proofReference: fetchResult.proofReference,
            responseStatusCode: fetchResult.statusCode,
            cacheStatus: fetchResult.cacheStatus,
          }
        : {
            status: "cached",
            amountMsats: fetchResult.amountMsats,
            paymentId: fetchResult.paymentId,
            proofReference: fetchResult.proofReference,
            responseStatusCode: fetchResult.statusCode,
            cacheStatus: fetchResult.cacheStatus,
          };
      return outcome;
    });

    return L402ExecutorService.of({
      execute,
    });
  }),
);
