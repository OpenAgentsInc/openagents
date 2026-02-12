import {
  CredentialCacheInMemoryLayer,
  L402ClientLiveLayer,
  L402ClientService,
  L402TransportError,
  L402TransportService,
  makeInvoicePayerSparkLayer,
  InvoicePayerService,
  makeInvoicePayerLndEffectLayer,
  makeSpendPolicyLayer,
  SparkPaymentService,
  type L402FetchResult,
} from "@openagentsinc/lightning-effect";
import { makeLndDeterministicLayer } from "@openagentsinc/lnd-effect";
import { Context, Effect, Layer } from "effect";
import crypto from "node:crypto";

import type { ExecutorTask } from "./model";
import { CredentialCacheDesktopPersistentLayer } from "./l402CredentialCachePersistent";
import { SparkWalletGatewayService } from "./sparkWalletGateway";

const MAX_RESPONSE_PREVIEW_BYTES = 8_192;

const truncateUtf8 = (value: string, maxBytes: number): string => {
  const buf = Buffer.from(value, "utf8");
  if (buf.length <= maxBytes) return value;
  return buf.subarray(0, Math.max(0, maxBytes)).toString("utf8");
};

export type L402ExecutionResult = Readonly<
  | {
      readonly status: "paid" | "cached";
      readonly amountMsats: number;
      readonly paymentId: string | null;
      readonly proofReference: string;
      readonly responseStatusCode: number;
      readonly responseContentType: string | null;
      readonly responseBytes: number | null;
      readonly responseBodyTextPreview: string | null;
      readonly responseBodySha256: string | null;
      readonly cacheHit: boolean;
      readonly paid: boolean;
      readonly cacheStatus: L402FetchResult["cacheStatus"];
      readonly paymentBackend: "spark" | "lnd_deterministic";
    }
  | {
      readonly status: "blocked";
      readonly errorCode: string;
      readonly denyReason: string;
      readonly denyReasonCode: string | null;
      readonly host: string | null;
      readonly maxSpendMsats: number | null;
      readonly quotedAmountMsats: number | null;
      readonly paymentBackend: "spark" | "lnd_deterministic";
    }
  | {
      readonly status: "failed";
      readonly errorCode: string;
      readonly denyReason: string;
      readonly paymentBackend: "spark" | "lnd_deterministic";
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

const hostFromUrl = (url: string): string | null => {
  try {
    return new URL(url).host;
  } catch {
    return null;
  }
};

const toFailedResult = (
  error: unknown,
  paymentBackend: "spark" | "lnd_deterministic",
): L402ExecutionResult => {
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
      paymentBackend,
    };
  }
  return {
    status: "failed",
    errorCode: "unknown_error",
    denyReason: String(error),
    paymentBackend,
  };
};

export const L402ExecutorLive = Layer.effect(
  L402ExecutorService,
  Effect.gen(function* () {
    const sparkWallet = yield* SparkWalletGatewayService;

    const credentialCacheLayer =
      typeof window !== "undefined" && window.openAgentsDesktop?.l402CredentialCache
        ? CredentialCacheDesktopPersistentLayer
        : CredentialCacheInMemoryLayer;

    const makeBaseDepsLayer = (
      invoicePayerLayer: Layer.Layer<InvoicePayerService, never, never>,
    ) =>
      Layer.mergeAll(
        credentialCacheLayer,
        makeSpendPolicyLayer({
          defaultMaxSpendMsats: 5_000_000,
          allowedHosts: [],
          blockedHosts: [],
        }),
        LiveL402TransportLayer,
        invoicePayerLayer,
      );

    const lndClientLayer = L402ClientLiveLayer.pipe(
      Layer.provide(
        makeBaseDepsLayer(
          makeInvoicePayerLndEffectLayer({
            fallbackAmountMsats: "request_max",
          }).pipe(Layer.provide(makeLndDeterministicLayer())),
        ),
      ),
    );

    const sparkPaymentLayer = Layer.succeed(
      SparkPaymentService,
      SparkPaymentService.of({
        payBolt11: (request) => sparkWallet.payInvoice(request),
      }),
    );
    const sparkClientLayer = L402ClientLiveLayer.pipe(
      Layer.provide(
        makeBaseDepsLayer(
          makeInvoicePayerSparkLayer().pipe(
            Layer.provide(sparkPaymentLayer),
          ),
        ),
      ),
    );

    const lndClient = yield* Effect.gen(function* () {
      return yield* L402ClientService;
    }).pipe(Effect.provide(lndClientLayer));
    const sparkClient = yield* Effect.gen(function* () {
      return yield* L402ClientService;
    }).pipe(Effect.provide(sparkClientLayer));

    const execute = Effect.fn("L402Executor.execute")(function* (task: ExecutorTask) {
      const sparkStatus = yield* sparkWallet.snapshot();
      const paymentBackend = sparkStatus.lifecycle === "connected" ? "spark" : "lnd_deterministic";
      const client = paymentBackend === "spark" ? sparkClient : lndClient;

      const exit = yield* Effect.either(client.fetchWithL402(task.request));
      if (exit._tag === "Left") {
        const err = exit.left as unknown as Record<string, unknown> | null;
        const tag = typeof err?._tag === "string" ? err._tag : null;
        const reason = typeof err?.reason === "string" && err.reason.trim().length > 0 ? err.reason.trim() : tag ?? "unknown_error";

        if (tag === "BudgetExceededError") {
          const maxSpendMsats = typeof err?.maxSpendMsats === "number" && Number.isFinite(err.maxSpendMsats)
            ? Math.max(0, Math.floor(err.maxSpendMsats))
            : Math.max(0, Math.floor(task.request.maxSpendMsats));
          const quotedAmountMsats = typeof err?.quotedAmountMsats === "number" && Number.isFinite(err.quotedAmountMsats)
            ? Math.max(0, Math.floor(err.quotedAmountMsats))
            : null;
          const denyReasonCode = typeof err?.reasonCode === "string" ? err.reasonCode : null;
          return {
            status: "blocked",
            errorCode: tag,
            denyReason: reason,
            denyReasonCode,
            host: hostFromUrl(task.request.url),
            maxSpendMsats,
            quotedAmountMsats,
            paymentBackend,
          } satisfies L402ExecutionResult;
        }

        if (tag === "DomainNotAllowedError") {
          const denyReasonCode = typeof err?.reasonCode === "string" ? err.reasonCode : null;
          const host = typeof err?.host === "string" && err.host.trim().length > 0
            ? err.host.trim()
            : hostFromUrl(task.request.url);
          return {
            status: "blocked",
            errorCode: tag,
            denyReason: reason,
            denyReasonCode,
            host,
            maxSpendMsats: Math.max(0, Math.floor(task.request.maxSpendMsats)),
            quotedAmountMsats: null,
            paymentBackend,
          } satisfies L402ExecutionResult;
        }

        return toFailedResult(exit.left, paymentBackend);
      }

      const fetchResult = exit.right;
      const responseBody = typeof fetchResult.responseBody === "string" ? fetchResult.responseBody : null;
      const responseBytes = responseBody ? Buffer.byteLength(responseBody, "utf8") : null;
      const responseBodySha256 = responseBody
        ? crypto.createHash("sha256").update(responseBody, "utf8").digest("hex")
        : null;
      const responseBodyTextPreview = responseBody ? truncateUtf8(responseBody, MAX_RESPONSE_PREVIEW_BYTES) : null;
      const responseContentType =
        typeof fetchResult.responseContentType === "string" && fetchResult.responseContentType.trim().length > 0
          ? fetchResult.responseContentType.trim()
          : null;
      const cacheHit = fetchResult.fromCache === true || fetchResult.cacheStatus === "hit";
      const outcome: L402ExecutionResult = fetchResult.paid
        ? {
            status: "paid",
            amountMsats: fetchResult.amountMsats,
            paymentId: fetchResult.paymentId,
            proofReference: fetchResult.proofReference,
            responseStatusCode: fetchResult.statusCode,
            responseContentType,
            responseBytes,
            responseBodyTextPreview,
            responseBodySha256,
            cacheHit,
            paid: true,
            cacheStatus: fetchResult.cacheStatus,
            paymentBackend,
          }
        : {
            status: "cached",
            amountMsats: fetchResult.amountMsats,
            paymentId: fetchResult.paymentId,
            proofReference: fetchResult.proofReference,
            responseStatusCode: fetchResult.statusCode,
            responseContentType,
            responseBytes,
            responseBodyTextPreview,
            responseBodySha256,
            cacheHit,
            paid: false,
            cacheStatus: fetchResult.cacheStatus,
            paymentBackend,
          };
      return outcome;
    });

    return L402ExecutorService.of({
      execute,
    });
  }),
);
