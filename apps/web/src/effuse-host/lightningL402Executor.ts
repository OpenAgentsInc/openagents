import {
  CredentialCacheService,
  L402ClientLiveLayer,
  L402ClientService,
  L402TransportError,
  L402TransportService,
  PaymentFailedError,
  PaymentMissingPreimageError,
  PaymentTimeoutError,
  SparkPaymentService,
  makeInvoicePayerSparkLayer,
  makeSpendPolicyLayer,
  type InvoicePaymentRequest,
  type InvoicePaymentResult,
  type L402Credential,
  type L402FetchRequest,
} from "@openagentsinc/lightning-effect";
import { Effect, Layer } from "effect";

import type { WorkerEnv } from "./env";

const MAX_RESPONSE_PREVIEW_BYTES = 8_192;
const DEFAULT_CACHE_TTL_MS = 10 * 60 * 1_000;
// Needs to cover: Cloud Run cold start + Spark payment completion timeout (default 45s).
// Cloudflare Workers can await network I/O without consuming much CPU, so a 60s wall timeout is acceptable here.
const DEFAULT_WALLET_EXECUTOR_TIMEOUT_MS = 60_000;

type CacheEntry = Readonly<{
  readonly credential: L402Credential;
  readonly expiresAtMs: number;
}>;

const ownerCredentialCache = new Map<string, Map<string, CacheEntry>>();

export type HostPolicyDecision =
  | Readonly<{
      readonly allowed: true;
      readonly host: string;
      readonly allowlistEnabled: boolean;
    }>
  | Readonly<{
      readonly allowed: false;
      readonly host: string;
      readonly denyReasonCode: "host_not_allowlisted" | "host_blocked";
      readonly denyReason: string;
    }>;

export type WalletExecutorAvailability =
  | Readonly<{ readonly configured: true; readonly baseUrl: string; readonly timeoutMs: number; readonly authToken: string | null }>
  | Readonly<{ readonly configured: false }>;

export type LightningExecutionResult =
  | Readonly<{
      readonly status: "completed" | "cached";
      readonly host: string;
      readonly amountMsats: number;
      readonly quotedAmountMsats: number | null;
      readonly paymentId: string | null;
      readonly proofReference: string;
      readonly responseStatusCode: number;
      readonly responseContentType: string | null;
      readonly responseBytes: number | null;
      readonly responseBodyTextPreview: string | null;
      readonly responseBodySha256: string | null;
      readonly cacheHit: boolean;
      readonly paid: boolean;
      readonly cacheStatus: "miss" | "hit" | "stale" | "invalid";
      readonly paymentBackend: "spark";
    }>
  | Readonly<{
      readonly status: "blocked";
      readonly host: string | null;
      readonly denyReason: string;
      readonly denyReasonCode: string | null;
      readonly maxSpendMsats: number | null;
      readonly quotedAmountMsats: number | null;
      readonly paymentBackend: "spark";
    }>
  | Readonly<{
      readonly status: "failed";
      readonly host: string | null;
      readonly denyReason: string;
      readonly errorCode: string;
      readonly paymentBackend: "spark";
    }>;

const normalizeHost = (value: string): string => value.trim().toLowerCase();

const parseHostFromUrl = (url: string): string | null => {
  try {
    return normalizeHost(new URL(url).host);
  } catch {
    return null;
  }
};

const parseCsvHosts = (value: string | undefined): ReadonlyArray<string> => {
  if (typeof value !== "string" || value.trim().length === 0) return [];
  const hosts = value
    .split(",")
    .map((item) => item.trim().toLowerCase())
    .filter((item) => item.length > 0);
  return Array.from(new Set(hosts));
};

const parseTimeoutMs = (value: string | undefined): number => {
  if (typeof value !== "string" || value.trim().length === 0) return DEFAULT_WALLET_EXECUTOR_TIMEOUT_MS;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return DEFAULT_WALLET_EXECUTOR_TIMEOUT_MS;
  return Math.max(1_000, Math.min(60_000, Math.floor(parsed)));
};

const digestSha256Hex = (value: string): Effect.Effect<string, unknown> =>
  Effect.tryPromise({
    try: async () => {
      const bytes = new TextEncoder().encode(value);
      const digest = await crypto.subtle.digest("SHA-256", bytes);
      return Array.from(new Uint8Array(digest))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
    },
    catch: (error) => error,
  });

const truncateUtf8 = (value: string, maxBytes: number): string => {
  const bytes = new TextEncoder().encode(value);
  if (bytes.length <= maxBytes) return value;
  return new TextDecoder().decode(bytes.slice(0, Math.max(0, maxBytes)));
};

const toCacheKey = (host: string, scope: string): string => `${normalizeHost(host)}::${scope.trim().toLowerCase()}`;

const getOwnerCacheBucket = (ownerKey: string): Map<string, CacheEntry> => {
  const normalized = ownerKey.trim().toLowerCase();
  const existing = ownerCredentialCache.get(normalized);
  if (existing) return existing;
  const created = new Map<string, CacheEntry>();
  ownerCredentialCache.set(normalized, created);
  return created;
};

const makeOwnerCredentialCacheLayer = (ownerKey: string): Layer.Layer<CredentialCacheService> =>
  Layer.succeed(
    CredentialCacheService,
    CredentialCacheService.of({
      getByHost: (host, scope, nowMs) =>
        Effect.sync(() => {
          const bucket = getOwnerCacheBucket(ownerKey);
          const entry = bucket.get(toCacheKey(host, scope));
          if (!entry) return { _tag: "miss" as const };
          if (nowMs >= entry.expiresAtMs) {
            return {
              _tag: "stale" as const,
              credential: entry.credential,
            };
          }
          return {
            _tag: "hit" as const,
            credential: entry.credential,
          };
        }),
      putByHost: (host, scope, credential, options) =>
        Effect.sync(() => {
          const ttlMs = Math.max(0, Math.floor(options?.ttlMs ?? DEFAULT_CACHE_TTL_MS));
          const bucket = getOwnerCacheBucket(ownerKey);
          bucket.set(toCacheKey(host, scope), {
            credential,
            expiresAtMs: credential.issuedAtMs + ttlMs,
          });
        }).pipe(Effect.asVoid),
      markInvalid: (host, scope) =>
        Effect.sync(() => {
          const bucket = getOwnerCacheBucket(ownerKey);
          bucket.delete(toCacheKey(host, scope));
        }).pipe(Effect.asVoid),
      clearHost: (host, scope) =>
        Effect.sync(() => {
          const bucket = getOwnerCacheBucket(ownerKey);
          bucket.delete(toCacheKey(host, scope));
        }).pipe(Effect.asVoid),
    }),
  );

const fetchWithTimeout = (
  url: string,
  init: RequestInit,
  timeoutMs: number,
): Effect.Effect<Response, PaymentTimeoutError | PaymentFailedError> =>
  Effect.tryPromise({
    try: async () => {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), timeoutMs);
      try {
        return await fetch(url, {
          ...init,
          signal: controller.signal,
          cache: "no-store",
        });
      } finally {
        clearTimeout(timeout);
      }
    },
    catch: (error) =>
      error instanceof Error && error.name === "AbortError"
        ? PaymentTimeoutError.make({
            invoice: "unknown",
            timeoutMs,
          })
        : PaymentFailedError.make({
            invoice: "unknown",
            reason: String(error),
          }),
  });

const buildRemoteSparkPaymentLayer = (input: {
  readonly baseUrl: string;
  readonly authToken: string | null;
  readonly timeoutMs: number;
  readonly requestId: string;
}): Layer.Layer<SparkPaymentService> =>
  Layer.succeed(
    SparkPaymentService,
    SparkPaymentService.of({
      payBolt11: (request: InvoicePaymentRequest) =>
        Effect.gen(function* () {
          const response = yield* fetchWithTimeout(
            `${input.baseUrl}/pay-bolt11`,
            {
              method: "POST",
              headers: {
                "content-type": "application/json",
                "x-request-id": input.requestId,
                ...(input.authToken ? { authorization: `Bearer ${input.authToken}` } : {}),
              },
              body: JSON.stringify({
                requestId: input.requestId,
                payment: request,
              }),
            },
            input.timeoutMs,
          ).pipe(
            Effect.catchTag("PaymentTimeoutError", () =>
              PaymentTimeoutError.make({
                invoice: request.invoice,
                timeoutMs: input.timeoutMs,
              }),
            ),
            Effect.catchTag("PaymentFailedError", (error) =>
              PaymentFailedError.make({
                invoice: request.invoice,
                reason: error.reason,
              }),
            ),
          );

          const payloadRaw = yield* Effect.tryPromise({
            try: async () => await response.text(),
            catch: () =>
              PaymentFailedError.make({
                invoice: request.invoice,
                reason: "wallet_executor_response_read_failed",
              }),
          });
          const payload = (() => {
            if (!payloadRaw || payloadRaw.trim().length === 0) return null;
            try {
              return JSON.parse(payloadRaw) as unknown;
            } catch {
              return null;
            }
          })();
          const rec = payload && typeof payload === "object" ? (payload as Record<string, unknown>) : null;
          const errorRec =
            rec && rec.error && typeof rec.error === "object" ? (rec.error as Record<string, unknown>) : null;
          const errorCode = typeof errorRec?.code === "string" ? errorRec.code : null;
          const errorMessage = typeof errorRec?.message === "string" ? errorRec.message : null;

          if (!response.ok) {
            if (errorCode === "payment_pending") {
              return yield* PaymentTimeoutError.make({
                invoice: request.invoice,
                timeoutMs: input.timeoutMs,
              });
            }
            return yield* PaymentFailedError.make({
              invoice: request.invoice,
              reason: [errorCode, errorMessage ?? `wallet_executor_http_${response.status}`].filter(Boolean).join(": "),
            });
          }

          const resultRec =
            rec &&
            rec.result &&
            typeof rec.result === "object" &&
            (rec.result as Record<string, unknown>).payment &&
            typeof (rec.result as Record<string, unknown>).payment === "object"
              ? ((rec.result as Record<string, unknown>).payment as Record<string, unknown>)
              : null;

          const paymentId = typeof resultRec?.paymentId === "string" ? resultRec.paymentId.trim() : "";
          const preimageHex = typeof resultRec?.preimageHex === "string" ? resultRec.preimageHex.trim() : "";
          const amountMsatsRaw = resultRec?.amountMsats;
          const paidAtMsRaw = resultRec?.paidAtMs;

          if (!paymentId || !Number.isFinite(amountMsatsRaw) || !Number.isFinite(paidAtMsRaw)) {
            return yield* PaymentFailedError.make({
              invoice: request.invoice,
              reason: "wallet_executor_invalid_payment_shape",
            });
          }

          if (!preimageHex) {
            return yield* PaymentMissingPreimageError.make({
              invoice: request.invoice,
              paymentId,
            });
          }

          return {
            paymentId,
            amountMsats: Math.max(0, Math.floor(amountMsatsRaw as number)),
            preimageHex,
            paidAtMs: Math.max(0, Math.floor(paidAtMsRaw as number)),
          } satisfies InvoicePaymentResult;
        }),
    }),
  );

export const walletExecutorAvailability = (env: WorkerEnv): WalletExecutorAvailability => {
  const baseRaw = typeof env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL === "string" ? env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL.trim() : "";
  if (!baseRaw) return { configured: false };
  try {
    const parsed = new URL(baseRaw);
    return {
      configured: true,
      baseUrl: parsed.toString().replace(/\/+$/, ""),
      timeoutMs: parseTimeoutMs(env.OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS),
      authToken:
        typeof env.OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN === "string" && env.OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN.trim().length > 0
          ? env.OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN.trim()
          : null,
    };
  } catch {
    return { configured: false };
  }
};

export const preflightHostPolicy = (input: {
  readonly env: WorkerEnv;
  readonly host: string;
}): HostPolicyDecision => {
  const host = normalizeHost(input.host);
  const allowedHosts = parseCsvHosts(input.env.OA_LIGHTNING_L402_ALLOWED_HOSTS);
  const blockedHosts = parseCsvHosts(input.env.OA_LIGHTNING_L402_BLOCKED_HOSTS);

  if (blockedHosts.includes(host)) {
    return {
      allowed: false,
      host,
      denyReasonCode: "host_blocked",
      denyReason: "Host is blocked by policy",
    };
  }

  const allowlistEnabled = allowedHosts.length > 0;
  if (allowlistEnabled && !allowedHosts.includes(host)) {
    return {
      allowed: false,
      host,
      denyReasonCode: "host_not_allowlisted",
      denyReason: "Host is not present in allowlist",
    };
  }

  return {
    allowed: true,
    host,
    allowlistEnabled,
  };
};

export const executeLightningFetchWithWalletExecutor = (input: {
  readonly env: WorkerEnv;
  readonly requestId: string;
  readonly ownerScopeKey: string;
  readonly request: L402FetchRequest;
}): Effect.Effect<LightningExecutionResult> =>
  Effect.gen(function* () {
    const host = parseHostFromUrl(input.request.url);
    if (!host) {
      return {
        status: "failed",
        host: null,
        errorCode: "invalid_url",
        denyReason: "Request URL must be absolute",
        paymentBackend: "spark",
      } satisfies LightningExecutionResult;
    }

    const availability = walletExecutorAvailability(input.env);
    if (!availability.configured) {
      return {
        status: "failed",
        host,
        errorCode: "wallet_executor_not_configured",
        denyReason: "wallet executor is not configured",
        paymentBackend: "spark",
      } satisfies LightningExecutionResult;
    }

    const allowedHosts = parseCsvHosts(input.env.OA_LIGHTNING_L402_ALLOWED_HOSTS);
    const blockedHosts = parseCsvHosts(input.env.OA_LIGHTNING_L402_BLOCKED_HOSTS);

    const spendPolicyLayer = makeSpendPolicyLayer({
      defaultMaxSpendMsats: Math.max(0, Math.floor(input.request.maxSpendMsats)),
      allowedHosts,
      blockedHosts,
    });

    const transportTimeoutMs = Math.max(5_000, Math.min(30_000, availability.timeoutMs));

    const transportLayer = Layer.succeed(
      L402TransportService,
      L402TransportService.of({
        send: (request) =>
          Effect.tryPromise({
            try: async () => {
              const controller = new AbortController();
              const timeout = setTimeout(() => controller.abort(), transportTimeoutMs);
              try {
                const response = await fetch(request.url, {
                  method: request.method ?? "GET",
                  headers: request.headers,
                  ...(request.body !== undefined ? { body: request.body } : {}),
                  cache: "no-store",
                  signal: controller.signal,
                });
                const body = await response.text();
                const headers: Record<string, string> = {};
                response.headers.forEach((value, key) => {
                  headers[key] = value;
                });
                return {
                  status: response.status,
                  headers,
                  ...(body.length > 0 ? { body } : {}),
                };
              } finally {
                clearTimeout(timeout);
              }
            },
            catch: (error) =>
              L402TransportError.make({
                reason:
                  error instanceof Error && error.name === "AbortError"
                    ? `transport_timeout ${transportTimeoutMs}ms url=${request.url}`
                    : String(error),
              }),
          }),
      }),
    );

    const sparkPaymentLayer = buildRemoteSparkPaymentLayer({
      baseUrl: availability.baseUrl,
      authToken: availability.authToken,
      timeoutMs: availability.timeoutMs,
      requestId: input.requestId,
    });
    const invoicePayerLayer = makeInvoicePayerSparkLayer().pipe(Layer.provide(sparkPaymentLayer));
    const cacheLayer = makeOwnerCredentialCacheLayer(input.ownerScopeKey);
    const l402Layer = L402ClientLiveLayer.pipe(
      Layer.provide(Layer.mergeAll(cacheLayer, spendPolicyLayer, transportLayer, invoicePayerLayer)),
    );

    const scopeBase = input.request.scope?.trim() || "default";
    const scopedRequest: L402FetchRequest = {
      ...input.request,
      scope: `${input.ownerScopeKey}:${scopeBase}`,
      cacheTtlMs:
        typeof input.request.cacheTtlMs === "number" && Number.isFinite(input.request.cacheTtlMs)
          ? Math.max(0, Math.floor(input.request.cacheTtlMs))
          : DEFAULT_CACHE_TTL_MS,
      authorizationHeaderStrategyByHost: {
        ...(input.request.authorizationHeaderStrategyByHost ?? {}),
        "sats4ai.com": "macaroon_preimage_colon",
      },
    };

    const executed = yield* Effect.either(
      Effect.gen(function* () {
        const client = yield* L402ClientService;
        return yield* client.fetchWithL402(scopedRequest);
      }).pipe(Effect.provide(l402Layer)),
    );

    if (executed._tag === "Left") {
      const err = executed.left as unknown as Record<string, unknown>;
      const tag = typeof err?._tag === "string" ? err._tag : "unknown_error";

      if (tag === "BudgetExceededError") {
        return {
          status: "blocked",
          host,
          denyReason: typeof err.reason === "string" ? err.reason : "budget_exceeded",
          denyReasonCode: typeof err.reasonCode === "string" ? err.reasonCode : "amount_over_cap",
          maxSpendMsats:
            typeof err.maxSpendMsats === "number" && Number.isFinite(err.maxSpendMsats)
              ? Math.max(0, Math.floor(err.maxSpendMsats))
              : Math.max(0, Math.floor(input.request.maxSpendMsats)),
          quotedAmountMsats:
            typeof err.quotedAmountMsats === "number" && Number.isFinite(err.quotedAmountMsats)
              ? Math.max(0, Math.floor(err.quotedAmountMsats))
              : null,
          paymentBackend: "spark",
        } satisfies LightningExecutionResult;
      }

      if (tag === "DomainNotAllowedError") {
        return {
          status: "blocked",
          host: typeof err.host === "string" && err.host.trim().length > 0 ? err.host.trim() : host,
          denyReason: typeof err.reason === "string" ? err.reason : "host_not_allowed",
          denyReasonCode: typeof err.reasonCode === "string" ? err.reasonCode : "host_not_allowlisted",
          maxSpendMsats: Math.max(0, Math.floor(input.request.maxSpendMsats)),
          quotedAmountMsats: null,
          paymentBackend: "spark",
        } satisfies LightningExecutionResult;
      }

      const reason =
        typeof err.reason === "string" && err.reason.trim().length > 0
          ? err.reason
          : typeof err.message === "string" && err.message.trim().length > 0
            ? err.message
            : tag;

      return {
        status: "failed",
        host,
        errorCode: tag,
        denyReason: reason,
        paymentBackend: "spark",
      } satisfies LightningExecutionResult;
    }

    const result = executed.right;
    const responseBody = typeof result.responseBody === "string" ? result.responseBody : null;
    const responseBytes = responseBody ? new TextEncoder().encode(responseBody).byteLength : null;
    const responseBodyTextPreview = responseBody ? truncateUtf8(responseBody, MAX_RESPONSE_PREVIEW_BYTES) : null;
    const responseBodySha256 = responseBody
      ? yield* digestSha256Hex(responseBody).pipe(Effect.catchAll(() => Effect.succeed(null)))
      : null;

    return {
      status: result.paid ? "completed" : "cached",
      host,
      amountMsats: Math.max(0, Math.floor(result.amountMsats)),
      quotedAmountMsats: Math.max(0, Math.floor(result.amountMsats)),
      paymentId: result.paymentId,
      proofReference: result.proofReference,
      responseStatusCode: result.statusCode,
      responseContentType:
        typeof result.responseContentType === "string" && result.responseContentType.trim().length > 0
          ? result.responseContentType.trim()
          : null,
      responseBytes,
      responseBodyTextPreview,
      responseBodySha256,
      cacheHit: result.fromCache === true || result.cacheStatus === "hit",
      paid: result.paid,
      cacheStatus: result.cacheStatus,
      paymentBackend: "spark",
    } satisfies LightningExecutionResult;
  });
