import crypto from "node:crypto";
import * as Fs from "node:fs/promises";
import * as Http from "node:http";
import type { AddressInfo } from "node:net";
import * as Path from "node:path";

import {
  BudgetExceededError,
  DomainNotAllowedError,
  L402TransportError,
  PaymentFailedError,
  PaymentMissingPreimageError,
  buildAuthorizationHeader,
  fetchWithL402,
  parseChallengeHeader,
  type FetchWithL402Deps,
  type InvoicePayerApi,
  type L402Credential,
} from "@openagentsinc/lightning-effect";
import { Effect } from "effect";

import { ConfigError } from "../errors.js";

export type Ep212FullFlowMode = "mock" | "live";

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

type FullFlowEvent = Readonly<{
  ts: number;
  stage: string;
  requestId: string;
  status: "ok" | "failed";
  details?: unknown;
}>;

export type Ep212FullFlowSummary = Readonly<{
  ok: true;
  requestId: string;
  mode: Ep212FullFlowMode;
  walletBackend: "mock" | "wallet_executor";
  sats4ai: Readonly<{
    endpointUrl: string;
    challengeStatusCode: number;
    firstStatusCode: number;
    firstPaid: boolean;
    firstPaymentId: string | null;
    firstAmountMsats: number;
    firstProofReference: string;
    firstCacheStatus: string;
    firstResponseBytes: number;
    firstResponseSha256: string;
    secondStatusCode: number;
    secondPaid: boolean;
    secondCacheStatus: string;
    cacheHit: boolean;
    payerCallsAfterFirst: number;
    payerCallsAfterSecond: number;
  }>;
  openAgentsRoute: Readonly<{
    endpointUrl: string;
    challengeStatusCode: number;
    paidStatusCode: number;
    paidAmountMsats: number;
    paymentId: string | null;
    proofReference: string;
    responseBytes: number;
    responseSha256: string;
  }>;
  overCap: Readonly<{
    endpointUrl: string;
    challengeStatusCode: number;
    quotedAmountMsats: number | null;
    maxSpendMsats: number;
    blocked: true;
    denyReasonCode: string;
    payerCallsBefore: number;
    payerCallsAfter: number;
  }>;
  artifacts: Readonly<{
    artifactDir: string;
    summaryPath: string;
    eventsPath: string;
    generatedAtMs: number;
  }>;
}>;

type LocalFixtureServer = Readonly<{
  server: Http.Server;
  sats4aiUrl: string;
  openAgentsSuccessUrl: string;
  openAgentsOverCapUrl: string;
}>;

const DEFAULT_SATS4AI_URL = "https://sats4ai.com/api/l402/text-generation";
const DEFAULT_OPENAGENTS_ROUTE_A_URL = "https://l402.openagents.com/ep212/premium-signal";
const DEFAULT_OPENAGENTS_ROUTE_B_URL = "https://l402.openagents.com/ep212/expensive-signal";
const DEFAULT_CAP_MSATS = 100_000;
const DEFAULT_OPENAGENTS_ROUTE_A_METHOD = "GET";
const SATS4AI_SCOPE = "ep212-sats4ai-text";
const OPENAGENTS_SCOPE = "ep212-openagents-success";
const OVERCAP_SCOPE = "ep212-openagents-overcap";

const SATS4AI_AMOUNT_MSATS = 70_000;
const OPENAGENTS_AMOUNT_MSATS = 70_000;
const OPENAGENTS_OVERCAP_AMOUNT_MSATS = 250_000;

const SATS4AI_INVOICE = "lnmock_ep212_sats4ai";
const SATS4AI_MACAROON = "mac_ep212_sats4ai";

const OA_UNDER_INVOICE = "lnmock_ep212_oa_under";
const OA_UNDER_MACAROON = "mac_ep212_oa_under";

const OA_OVER_INVOICE = "lnmock_ep212_oa_over";
const OA_OVER_MACAROON = "mac_ep212_oa_over";

const sats4aiBody = JSON.stringify({
  input: [
    { role: "User", content: "Tell me a story of a bird" },
    {
      role: "Assistant",
      content: "Once upon a time, there was a little bird who loved to sing.",
    },
    { role: "User", content: "What happened next?" },
  ],
  model: "Best",
});

const normalizeHttpMethod = (value: string): "GET" | "POST" => {
  const method = value.trim().toUpperCase();
  if (method === "POST") return "POST";
  return "GET";
};

const sha256Hex = (value: string): string =>
  crypto.createHash("sha256").update(value, "utf8").digest("hex");

const deterministicPreimage = (invoice: string): string =>
  sha256Hex(`ep212:full-flow:${invoice}`).slice(0, 64);

const envString = (
  key: string,
  fallback?: string,
): Effect.Effect<string, ConfigError> =>
  Effect.sync(() => process.env[key]?.trim() || fallback || "").pipe(
    Effect.flatMap((value) =>
      value.length > 0
        ? Effect.succeed(value)
        : ConfigError.make({
            field: key,
            message: "missing required environment variable",
          }),
    ),
  );

const envInt = (
  key: string,
  fallback: number,
): Effect.Effect<number, ConfigError> =>
  Effect.sync(() => process.env[key]?.trim() || String(fallback)).pipe(
    Effect.flatMap((raw) => {
      const parsed = Number(raw);
      if (Number.isFinite(parsed) && parsed > 0) return Effect.succeed(Math.floor(parsed));
      return ConfigError.make({
        field: key,
        message: "must be a positive integer",
      });
    }),
  );

const parseAuthorizationHeader = (
  headers: Headers | Record<string, string> | undefined,
): string | null => {
  if (!headers) return null;
  if (headers instanceof Headers) {
    const value = headers.get("authorization");
    return value && value.trim().length > 0 ? value.trim() : null;
  }
  for (const [key, value] of Object.entries(headers)) {
    if (key.toLowerCase() !== "authorization") continue;
    const next = String(value ?? "").trim();
    return next.length > 0 ? next : null;
  }
  return null;
};

const toHeadersRecord = (headers: Headers): Record<string, string> => {
  const out: Record<string, string> = {};
  headers.forEach((value, key) => {
    out[key] = value;
  });
  return out;
};

const parseChallengeFromResponse = (response: Response) =>
  Effect.gen(function* () {
    if (response.status !== 402) {
      return yield* Effect.fail(new Error(`expected 402 challenge, got status ${response.status}`));
    }
    const header = response.headers.get("www-authenticate");
    if (!header || header.trim().length === 0) {
      return yield* Effect.fail(new Error("expected www-authenticate header on 402 challenge"));
    }
    const challenge = yield* parseChallengeHeader(header);
    return {
      statusCode: response.status,
      amountMsats: typeof challenge.amountMsats === "number" ? challenge.amountMsats : null,
    };
  });

const checkChallenge = (input: {
  readonly fetchFn: FetchLike;
  readonly url: string;
  readonly method: "GET" | "POST";
  readonly headers?: Readonly<Record<string, string>>;
  readonly body?: string;
}) =>
  Effect.tryPromise({
    try: () => {
      const init: RequestInit = {
        method: input.method,
      };
      if (input.headers) init.headers = input.headers;
      if (input.body !== undefined) init.body = input.body;
      return input.fetchFn(input.url, init);
    },
    catch: (error) => new Error(String(error)),
  }).pipe(Effect.flatMap(parseChallengeFromResponse));

const createL402Deps = (input: {
  readonly fetchFn: FetchLike;
  readonly payer: InvoicePayerApi;
  readonly defaultMaxSpendMsats: number;
  readonly allowedHosts: ReadonlyArray<string>;
}): FetchWithL402Deps => {
  const cacheStore = new Map<
    string,
    { credential: L402Credential; expiresAtMs: number | null }
  >();
  const normalizeHost = (value: string): string => value.trim().toLowerCase();
  const allowedHosts = [...new Set(input.allowedHosts.map(normalizeHost))];
  const cacheKey = (host: string, scope: string): string =>
    `${normalizeHost(host)}::${scope}`;

  return {
    credentialCache: {
      getByHost: (host, scope, nowMs) =>
        Effect.sync(() => {
          const key = cacheKey(host, scope);
          const hit = cacheStore.get(key);
          if (!hit) return { _tag: "miss" } as const;
          if (hit.expiresAtMs !== null && nowMs > hit.expiresAtMs) {
            return { _tag: "stale", credential: hit.credential } as const;
          }
          return { _tag: "hit", credential: hit.credential } as const;
        }),
      putByHost: (host, scope, credential, options) =>
        Effect.sync(() => {
          const ttlMs =
            typeof options?.ttlMs === "number" && Number.isFinite(options.ttlMs)
              ? Math.max(0, Math.floor(options.ttlMs))
              : null;
          cacheStore.set(cacheKey(host, scope), {
            credential,
            expiresAtMs: ttlMs === null ? null : Date.now() + ttlMs,
          });
        }),
      markInvalid: (host, scope) =>
        Effect.sync(() => void cacheStore.delete(cacheKey(host, scope))),
      clearHost: (host, scope) =>
        Effect.sync(() => void cacheStore.delete(cacheKey(host, scope))),
    },
    payer: input.payer,
    policy: {
      policy: {
        defaultMaxSpendMsats: input.defaultMaxSpendMsats,
        allowedHosts,
        blockedHosts: [],
      },
      ensureRequestAllowed: ({ host, quotedAmountMsats, maxSpendMsats }) =>
        Effect.gen(function* () {
          const normalizedHost = normalizeHost(host);
          if (!allowedHosts.includes(normalizedHost)) {
            return yield* DomainNotAllowedError.make({
              host: normalizedHost,
              reasonCode: "host_not_allowlisted",
              reason: "Host is not present in allowlist",
            });
          }
          const effectiveCap = Math.min(
            input.defaultMaxSpendMsats,
            Math.max(0, Math.floor(maxSpendMsats)),
          );
          if (quotedAmountMsats > effectiveCap) {
            return yield* BudgetExceededError.make({
              maxSpendMsats: effectiveCap,
              quotedAmountMsats,
              reasonCode: "amount_over_cap",
              reason: "Quoted invoice amount exceeds configured spend cap",
            });
          }
        }),
    },
    transport: {
      send: (request) =>
        Effect.tryPromise({
          try: async () => {
            const body =
              request.body === undefined
                ? undefined
                : typeof request.body === "string"
                  ? request.body
                  : JSON.stringify(request.body);
            const requestInit: RequestInit = {
              method: request.method ?? "GET",
            };
            if (request.headers) requestInit.headers = request.headers;
            if (body !== undefined) requestInit.body = body;
            const response = await input.fetchFn(request.url, requestInit);
            const responseBody = await response.text();
            return {
              status: response.status,
              headers: toHeadersRecord(response.headers),
              body: responseBody,
            };
          },
          catch: (error) =>
            L402TransportError.make({
              reason: String(error),
            }),
        }),
    },
  };
};

const createMockPayer = (calls: { count: number }): InvoicePayerApi => ({
  payInvoice: (request) =>
    Effect.sync(() => {
      calls.count += 1;
      const amountMsats =
        request.invoice === SATS4AI_INVOICE
          ? SATS4AI_AMOUNT_MSATS
          : request.invoice === OA_UNDER_INVOICE
            ? OPENAGENTS_AMOUNT_MSATS
            : OPENAGENTS_OVERCAP_AMOUNT_MSATS;
      return {
        paymentId: `mock_pay_${calls.count}`,
        amountMsats,
        preimageHex: deterministicPreimage(request.invoice),
        paidAtMs: 1_736_000_000_000 + calls.count,
      };
    }),
});

const createWalletExecutorPayer = (input: {
  readonly fetchFn: FetchLike;
  readonly baseUrl: string;
  readonly authToken: string | null;
  readonly timeoutMs: number;
  readonly requestId: string;
  readonly calls: { count: number };
}): InvoicePayerApi => ({
  payInvoice: (request) =>
    Effect.gen(function* () {
      input.calls.count += 1;
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), input.timeoutMs);

      const response = yield* Effect.tryPromise({
        try: () =>
          input.fetchFn(`${input.baseUrl}/pay-bolt11`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
              "x-request-id": input.requestId,
              ...(input.authToken
                ? { authorization: `Bearer ${input.authToken}` }
                : {}),
            },
            body: JSON.stringify({
              requestId: input.requestId,
              payment: request,
            }),
            signal: controller.signal,
          }),
        catch: (error) =>
          PaymentFailedError.make({
            invoice: request.invoice,
            reason: String(error),
          }),
      }).pipe(Effect.ensuring(Effect.sync(() => clearTimeout(timeout))));

      const payloadRaw = yield* Effect.tryPromise({
        try: () => response.text(),
        catch: () =>
          PaymentFailedError.make({
            invoice: request.invoice,
            reason: "wallet_executor_response_read_failed",
          }),
      });

      const parsed = (() => {
        if (!payloadRaw || payloadRaw.trim().length === 0) return null;
        try {
          return JSON.parse(payloadRaw) as Record<string, unknown>;
        } catch {
          return null;
        }
      })();

      const errorRec =
        parsed && parsed.error && typeof parsed.error === "object"
          ? (parsed.error as Record<string, unknown>)
          : null;

      if (!response.ok) {
        const code =
          typeof errorRec?.code === "string"
            ? errorRec.code
            : `wallet_executor_http_${response.status}`;
        const message = typeof errorRec?.message === "string" ? errorRec.message : null;
        return yield* PaymentFailedError.make({
          invoice: request.invoice,
          reason: [code, message].filter(Boolean).join(": "),
        });
      }

      const payment =
        parsed &&
        parsed.result &&
        typeof parsed.result === "object" &&
        (parsed.result as Record<string, unknown>).payment &&
        typeof (parsed.result as Record<string, unknown>).payment === "object"
          ? ((parsed.result as Record<string, unknown>).payment as Record<string, unknown>)
          : null;

      const paymentId =
        typeof payment?.paymentId === "string" ? payment.paymentId.trim() : "";
      const preimageHex =
        typeof payment?.preimageHex === "string" ? payment.preimageHex.trim() : "";
      const amountMsats =
        typeof payment?.amountMsats === "number" && Number.isFinite(payment.amountMsats)
          ? Math.max(0, Math.floor(payment.amountMsats))
          : Number.NaN;
      const paidAtMs =
        typeof payment?.paidAtMs === "number" && Number.isFinite(payment.paidAtMs)
          ? Math.max(0, Math.floor(payment.paidAtMs))
          : Date.now();

      if (!paymentId || !Number.isFinite(amountMsats)) {
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
        amountMsats,
        preimageHex,
        paidAtMs,
      };
    }),
});

const jsonResponse = (
  res: Http.ServerResponse,
  statusCode: number,
  body: unknown,
  extraHeaders?: Readonly<Record<string, string>>,
): void => {
  res.statusCode = statusCode;
  res.setHeader("content-type", "application/json; charset=utf-8");
  for (const [key, value] of Object.entries(extraHeaders ?? {})) {
    res.setHeader(key, value);
  }
  res.end(JSON.stringify(body));
};

const readBody = (req: Http.IncomingMessage): Promise<string> =>
  new Promise((resolve, reject) => {
    let raw = "";
    req.setEncoding("utf8");
    req.on("data", (chunk) => {
      raw += String(chunk);
    });
    req.on("end", () => resolve(raw));
    req.on("error", reject);
  });

const startLocalFixtureServer = (): Promise<LocalFixtureServer> =>
  new Promise((resolve, reject) => {
    let lastSats4aiBody = "";

    const server = Http.createServer(async (req, res) => {
      const method = req.method?.toUpperCase() ?? "GET";
      const path = new URL(req.url ?? "/", "http://127.0.0.1").pathname;
      const body = await readBody(req);
      const authHeader =
        typeof req.headers.authorization === "string"
          ? req.headers.authorization.trim()
          : "";

      const oaUnderExpectedAuth = buildAuthorizationHeader({
        host: "127.0.0.1",
        scope: OPENAGENTS_SCOPE,
        macaroon: OA_UNDER_MACAROON,
        preimageHex: deterministicPreimage(OA_UNDER_INVOICE),
        amountMsats: OPENAGENTS_AMOUNT_MSATS,
        issuedAtMs: 0,
      }, "macaroon_preimage_colon");

      if (path === "/sats4ai/text-generation") {
        if (method !== "POST") {
          jsonResponse(res, 405, { ok: false, error: "method_not_allowed" });
          return;
        }
        if (!authHeader) {
          lastSats4aiBody = body;
          jsonResponse(
            res,
            402,
            { ok: false, error: "payment_required" },
            {
              "www-authenticate": `L402 invoice="${SATS4AI_INVOICE}", macaroon="${SATS4AI_MACAROON}", amount_msats=${SATS4AI_AMOUNT_MSATS}`,
            },
          );
          return;
        }
        const expectedAuth = `L402 ${SATS4AI_MACAROON}:${deterministicPreimage(SATS4AI_INVOICE)}`;
        if (authHeader !== expectedAuth) {
          jsonResponse(res, 401, { ok: false, error: "credential_rejected" });
          return;
        }
        if (body !== lastSats4aiBody) {
          jsonResponse(res, 400, { ok: false, error: "request_body_mismatch" });
          return;
        }
        jsonResponse(res, 200, {
          ok: true,
          source: "sats4ai.mock",
          output:
            "The little bird soared higher and found a hidden valley of musical flowers.",
        });
        return;
      }

      if (path === "/openagents/premium-signal") {
        if (method !== "GET") {
          jsonResponse(res, 405, { ok: false, error: "method_not_allowed" });
          return;
        }
        if (!authHeader) {
          jsonResponse(
            res,
            402,
            { ok: false, error: "payment_required" },
            {
              "www-authenticate": `L402 invoice="${OA_UNDER_INVOICE}", macaroon="${OA_UNDER_MACAROON}", amount_msats=${OPENAGENTS_AMOUNT_MSATS}`,
            },
          );
          return;
        }
        if (authHeader !== oaUnderExpectedAuth) {
          jsonResponse(res, 401, { ok: false, error: "credential_rejected" });
          return;
        }
        jsonResponse(res, 200, {
          ok: true,
          source: "openagents.mock",
          route: "/openagents/premium-signal",
          signal: {
            symbol: "BTC",
            confidence: 0.8123,
            direction: "up",
            horizon: "4h",
          },
        });
        return;
      }

      if (path === "/openagents/expensive-signal") {
        if (method !== "GET") {
          jsonResponse(res, 405, { ok: false, error: "method_not_allowed" });
          return;
        }
        jsonResponse(
          res,
          402,
          { ok: false, error: "payment_required" },
          {
            "www-authenticate": `L402 invoice="${OA_OVER_INVOICE}", macaroon="${OA_OVER_MACAROON}", amount_msats=${OPENAGENTS_OVERCAP_AMOUNT_MSATS}`,
          },
        );
        return;
      }

      jsonResponse(res, 404, { ok: false, error: "not_found" });
    });

    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("ep212_fixture_bind_failed"));
        return;
      }
      const port = (address as AddressInfo).port;
      const baseUrl = `http://127.0.0.1:${port}`;
      resolve({
        server,
        sats4aiUrl: `${baseUrl}/sats4ai/text-generation`,
        openAgentsSuccessUrl: `${baseUrl}/openagents/premium-signal`,
        openAgentsOverCapUrl: `${baseUrl}/openagents/expensive-signal`,
      });
    });
  });

const closeLocalFixtureServer = (server: Http.Server): Promise<void> =>
  new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });

const addEvent = (
  events: Array<FullFlowEvent>,
  input: Omit<FullFlowEvent, "ts">,
): void => {
  events.push({
    ts: Date.now(),
    ...input,
  });
};

const writeTextFile = (path: string, content: string) =>
  Effect.tryPromise({
    try: async () => {
      await Fs.mkdir(Path.dirname(path), { recursive: true });
      await Fs.writeFile(path, content, "utf8");
    },
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });

const buildEventsJsonl = (events: ReadonlyArray<FullFlowEvent>): string =>
  `${events.map((event) => JSON.stringify(event)).join("\n")}\n`;

const sanitizePathSegment = (value: string): string =>
  value.replaceAll(/[^a-zA-Z0-9._-]+/g, "_").slice(0, 120);

const defaultArtifactDir = (requestId: string): string =>
  Path.resolve(
    process.cwd(),
    "../../output/lightning-ops/ep212-full-flow",
    sanitizePathSegment(requestId),
  );

const runEp212FullFlow = (input: {
  readonly mode: Ep212FullFlowMode;
  readonly requestId: string;
  readonly fetchFn: FetchLike;
  readonly sats4aiUrl: string;
  readonly openAgentsSuccessUrl: string;
  readonly openAgentsSuccessMethod: "GET" | "POST";
  readonly openAgentsSuccessBody?: string;
  readonly openAgentsOverCapUrl: string;
  readonly maxSpendMsats: number;
  readonly payer: InvoicePayerApi;
  readonly payerCalls: { count: number };
  readonly walletBackend: "mock" | "wallet_executor";
  readonly artifactDir: string;
}) =>
  Effect.gen(function* () {
    const events: Array<FullFlowEvent> = [];
    const summaryPath = Path.join(input.artifactDir, "summary.json");
    const eventsPath = Path.join(input.artifactDir, "events.jsonl");

    const satsHost = new URL(input.sats4aiUrl).host.toLowerCase();
    const openAgentsSuccessHost = new URL(input.openAgentsSuccessUrl).host.toLowerCase();
    const openAgentsOverCapHost = new URL(input.openAgentsOverCapUrl).host.toLowerCase();

    const deps = createL402Deps({
      fetchFn: input.fetchFn,
      payer: input.payer,
      defaultMaxSpendMsats: input.maxSpendMsats,
      allowedHosts: [satsHost, openAgentsSuccessHost, openAgentsOverCapHost],
    });

    const satsHeaders: Readonly<Record<string, string>> = {
      "content-type": "application/json",
    };

    const satsChallenge = yield* checkChallenge({
      fetchFn: input.fetchFn,
      url: input.sats4aiUrl,
      method: "POST",
      headers: satsHeaders,
      body: sats4aiBody,
    });
    addEvent(events, {
      stage: "sats4ai.challenge",
      requestId: input.requestId,
      status: "ok",
      details: satsChallenge,
    });

    const satsFirst = yield* fetchWithL402(
      {
        url: input.sats4aiUrl,
        method: "POST",
        headers: satsHeaders,
        body: sats4aiBody,
        scope: SATS4AI_SCOPE,
        maxSpendMsats: input.maxSpendMsats,
        forceRefresh: true,
        cacheTtlMs: 600_000,
        authorizationHeaderStrategyByHost: {
          [satsHost]: "macaroon_preimage_colon",
        },
      },
      deps,
    );
    const payerCallsAfterFirst = input.payerCalls.count;
    if (satsFirst.statusCode !== 200 || satsFirst.paid !== true) {
      return yield* Effect.fail(
        new Error(
          `sats4ai_first_fetch_failed status=${satsFirst.statusCode} paid=${satsFirst.paid}`,
        ),
      );
    }
    addEvent(events, {
      stage: "sats4ai.first",
      requestId: input.requestId,
      status: "ok",
      details: {
        statusCode: satsFirst.statusCode,
        paymentId: satsFirst.paymentId,
        amountMsats: satsFirst.amountMsats,
        cacheStatus: satsFirst.cacheStatus,
      },
    });

    const satsSecondAttempt = yield* fetchWithL402(
      {
        url: input.sats4aiUrl,
        method: "POST",
        headers: satsHeaders,
        body: sats4aiBody,
        scope: SATS4AI_SCOPE,
        maxSpendMsats: input.maxSpendMsats,
        cacheTtlMs: 600_000,
        authorizationHeaderStrategyByHost: {
          [satsHost]: "macaroon_preimage_colon",
        },
      },
      deps,
    );
    let satsSecond = satsSecondAttempt;
    let payerCallsAfterSecond = input.payerCalls.count;
    let satsCacheHit = satsSecond.cacheStatus === "hit" && satsSecond.statusCode === 200;

    if (satsCacheHit && payerCallsAfterSecond !== payerCallsAfterFirst) {
      return yield* Effect.fail(
        new Error(
          `sats4ai_cache_expected_no_new_payment before=${payerCallsAfterFirst} after=${payerCallsAfterSecond}`,
        ),
      );
    }

    if (!satsCacheHit) {
      const satsRepaid = yield* fetchWithL402(
        {
          url: input.sats4aiUrl,
          method: "POST",
          headers: satsHeaders,
          body: sats4aiBody,
          scope: SATS4AI_SCOPE,
          maxSpendMsats: input.maxSpendMsats,
          forceRefresh: true,
          cacheTtlMs: 600_000,
          authorizationHeaderStrategyByHost: {
            [satsHost]: "macaroon_preimage_colon",
          },
        },
        deps,
      );
      payerCallsAfterSecond = input.payerCalls.count;
      if (satsRepaid.statusCode !== 200 || satsRepaid.paid !== true) {
        return yield* Effect.fail(
          new Error(
            `sats4ai_second_attempt_failed status=${satsRepaid.statusCode} paid=${satsRepaid.paid}`,
          ),
        );
      }
      if (payerCallsAfterSecond <= payerCallsAfterFirst) {
        return yield* Effect.fail(
          new Error(
            `sats4ai_second_attempt_expected_new_payment before=${payerCallsAfterFirst} after=${payerCallsAfterSecond}`,
          ),
        );
      }
      satsSecond = satsRepaid;
      satsCacheHit = false;
    }

    addEvent(events, {
      stage: "sats4ai.cached",
      requestId: input.requestId,
      status: "ok",
      details: {
        statusCode: satsSecond.statusCode,
        cacheStatus: satsSecond.cacheStatus,
        cacheHit: satsCacheHit,
      },
    });

    const openAgentsHeaders: Readonly<Record<string, string>> | undefined =
      input.openAgentsSuccessBody !== undefined
        ? { "content-type": "application/json" }
        : undefined;

    const openAgentsChallenge = yield* checkChallenge({
      fetchFn: input.fetchFn,
      url: input.openAgentsSuccessUrl,
      method: input.openAgentsSuccessMethod,
      ...(openAgentsHeaders ? { headers: openAgentsHeaders } : {}),
      ...(input.openAgentsSuccessBody !== undefined
        ? { body: input.openAgentsSuccessBody }
        : {}),
    });
    const openAgentsPaid = yield* fetchWithL402(
      {
        url: input.openAgentsSuccessUrl,
        method: input.openAgentsSuccessMethod,
        ...(openAgentsHeaders ? { headers: openAgentsHeaders } : {}),
        ...(input.openAgentsSuccessBody !== undefined
          ? { body: input.openAgentsSuccessBody }
          : {}),
        scope: OPENAGENTS_SCOPE,
        maxSpendMsats: input.maxSpendMsats,
        forceRefresh: true,
        cacheTtlMs: 600_000,
        authorizationHeaderStrategyByHost: {
          [openAgentsSuccessHost]: "macaroon_preimage_colon" as const,
        },
      },
      deps,
    );
    if (openAgentsPaid.statusCode !== 200 || openAgentsPaid.paid !== true) {
      return yield* Effect.fail(
        new Error(
          `openagents_paid_fetch_failed status=${openAgentsPaid.statusCode} paid=${openAgentsPaid.paid}`,
        ),
      );
    }
    addEvent(events, {
      stage: "openagents.route_success",
      requestId: input.requestId,
      status: "ok",
      details: {
        statusCode: openAgentsPaid.statusCode,
        paymentId: openAgentsPaid.paymentId,
        amountMsats: openAgentsPaid.amountMsats,
      },
    });

    const overCapChallenge = yield* checkChallenge({
      fetchFn: input.fetchFn,
      url: input.openAgentsOverCapUrl,
      method: "GET",
    });
    const payerCallsBeforeOverCap = input.payerCalls.count;
    const overCapAttempt = yield* Effect.either(
      fetchWithL402(
        {
          url: input.openAgentsOverCapUrl,
          method: "GET",
          scope: OVERCAP_SCOPE,
          maxSpendMsats: input.maxSpendMsats,
          forceRefresh: true,
          cacheTtlMs: 600_000,
        },
        deps,
      ),
    );
    const payerCallsAfterOverCap = input.payerCalls.count;

    if (overCapAttempt._tag !== "Left") {
      return yield* Effect.fail(
        new Error(
          `over_cap_expected_failure status=${overCapAttempt.right.statusCode}`,
        ),
      );
    }
    if (overCapAttempt.left._tag !== "BudgetExceededError") {
      return yield* Effect.fail(
        new Error(
          `over_cap_expected_budget_error actual=${overCapAttempt.left._tag}`,
        ),
      );
    }
    if (payerCallsAfterOverCap !== payerCallsBeforeOverCap) {
      return yield* Effect.fail(
        new Error(
          `over_cap_expected_no_payment before=${payerCallsBeforeOverCap} after=${payerCallsAfterOverCap}`,
        ),
      );
    }
    addEvent(events, {
      stage: "openagents.over_cap_block",
      requestId: input.requestId,
      status: "ok",
      details: {
        denyReasonCode: overCapAttempt.left.reasonCode,
        quotedAmountMsats: overCapAttempt.left.quotedAmountMsats,
        maxSpendMsats: overCapAttempt.left.maxSpendMsats,
      },
    });

    const generatedAtMs = Date.now();
    const summary = {
      ok: true as const,
      requestId: input.requestId,
      mode: input.mode,
      walletBackend: input.walletBackend,
      sats4ai: {
        endpointUrl: input.sats4aiUrl,
        challengeStatusCode: satsChallenge.statusCode,
        firstStatusCode: satsFirst.statusCode,
        firstPaid: satsFirst.paid,
        firstPaymentId: satsFirst.paymentId,
        firstAmountMsats: satsFirst.amountMsats,
        firstProofReference: satsFirst.proofReference,
        firstCacheStatus: satsFirst.cacheStatus,
        firstResponseBytes: Buffer.byteLength(satsFirst.responseBody ?? "", "utf8"),
        firstResponseSha256: sha256Hex(satsFirst.responseBody ?? ""),
        secondStatusCode: satsSecond.statusCode,
        secondPaid: satsSecond.paid,
        secondCacheStatus: satsSecond.cacheStatus,
        cacheHit: satsCacheHit,
        payerCallsAfterFirst,
        payerCallsAfterSecond,
      },
      openAgentsRoute: {
        endpointUrl: input.openAgentsSuccessUrl,
        challengeStatusCode: openAgentsChallenge.statusCode,
        paidStatusCode: openAgentsPaid.statusCode,
        paidAmountMsats: openAgentsPaid.amountMsats,
        paymentId: openAgentsPaid.paymentId,
        proofReference: openAgentsPaid.proofReference,
        responseBytes: Buffer.byteLength(openAgentsPaid.responseBody ?? "", "utf8"),
        responseSha256: sha256Hex(openAgentsPaid.responseBody ?? ""),
      },
      overCap: {
        endpointUrl: input.openAgentsOverCapUrl,
        challengeStatusCode: overCapChallenge.statusCode,
        quotedAmountMsats: overCapChallenge.amountMsats,
        maxSpendMsats: input.maxSpendMsats,
        blocked: true as const,
        denyReasonCode: overCapAttempt.left.reasonCode,
        payerCallsBefore: payerCallsBeforeOverCap,
        payerCallsAfter: payerCallsAfterOverCap,
      },
      artifacts: {
        artifactDir: input.artifactDir,
        summaryPath,
        eventsPath,
        generatedAtMs,
      },
    } satisfies Ep212FullFlowSummary;

    yield* writeTextFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
    yield* writeTextFile(eventsPath, buildEventsJsonl(events));

    return summary;
  });

const runMockEp212FullFlow = (input: {
  readonly requestId: string;
  readonly artifactDir: string;
}) =>
  Effect.gen(function* () {
    const fixture = yield* Effect.tryPromise({
      try: () => startLocalFixtureServer(),
      catch: (error) => (error instanceof Error ? error : new Error(String(error))),
    });

    const payerCalls = { count: 0 };

    return yield* runEp212FullFlow({
      mode: "mock",
      requestId: input.requestId,
      fetchFn: async (url, init) => fetch(url, init),
      sats4aiUrl: fixture.sats4aiUrl,
      openAgentsSuccessUrl: fixture.openAgentsSuccessUrl,
      openAgentsSuccessMethod: "GET",
      openAgentsOverCapUrl: fixture.openAgentsOverCapUrl,
      maxSpendMsats: DEFAULT_CAP_MSATS,
      payer: createMockPayer(payerCalls),
      payerCalls,
      walletBackend: "mock",
      artifactDir: input.artifactDir,
    }).pipe(
      Effect.ensuring(
        Effect.tryPromise({
          try: () => closeLocalFixtureServer(fixture.server),
          catch: (error) => (error instanceof Error ? error : new Error(String(error))),
        }).pipe(Effect.orDie),
      ),
    );
  });

const runLiveEp212FullFlow = (input: {
  readonly requestId: string;
  readonly artifactDir: string;
}) =>
  Effect.gen(function* () {
    const sats4aiUrl = yield* envString(
      "OA_LIGHTNING_OPS_EP212_SATS4AI_URL",
      DEFAULT_SATS4AI_URL,
    );
    const openAgentsSuccessUrl = yield* envString(
      "OA_LIGHTNING_OPS_EP212_ROUTE_A_URL",
      DEFAULT_OPENAGENTS_ROUTE_A_URL,
    );
    const openAgentsSuccessMethodRaw =
      process.env.OA_LIGHTNING_OPS_EP212_ROUTE_A_METHOD?.trim() ||
      DEFAULT_OPENAGENTS_ROUTE_A_METHOD;
    const openAgentsSuccessMethod = normalizeHttpMethod(openAgentsSuccessMethodRaw);
    const openAgentsSuccessBodyEnv = process.env.OA_LIGHTNING_OPS_EP212_ROUTE_A_BODY;
    const openAgentsSuccessBody =
      openAgentsSuccessMethod === "GET"
        ? undefined
        : openAgentsSuccessBodyEnv && openAgentsSuccessBodyEnv.trim().length > 0
          ? openAgentsSuccessBodyEnv
          : sats4aiBody;
    const openAgentsOverCapUrl = yield* envString(
      "OA_LIGHTNING_OPS_EP212_ROUTE_B_URL",
      DEFAULT_OPENAGENTS_ROUTE_B_URL,
    );
    const maxSpendMsats = yield* envInt(
      "OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS",
      DEFAULT_CAP_MSATS,
    );
    const walletExecutorBaseUrl = yield* envString(
      "OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL",
    );
    const walletExecutorAuthTokenRaw =
      process.env.OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN?.trim();
    const walletExecutorAuthToken =
      walletExecutorAuthTokenRaw && walletExecutorAuthTokenRaw.length > 0
        ? walletExecutorAuthTokenRaw
        : null;
    const walletTimeoutMs = yield* envInt(
      "OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS",
      12_000,
    );

    const payerCalls = { count: 0 };
    const fetchFn: FetchLike = async (url, init) => fetch(url, init);

    return yield* runEp212FullFlow({
      mode: "live",
      requestId: input.requestId,
      fetchFn,
      sats4aiUrl,
      openAgentsSuccessUrl,
      openAgentsSuccessMethod,
      ...(openAgentsSuccessBody !== undefined
        ? { openAgentsSuccessBody }
        : {}),
      openAgentsOverCapUrl,
      maxSpendMsats,
      payer: createWalletExecutorPayer({
        fetchFn,
        baseUrl: walletExecutorBaseUrl.replace(/\/+$/, ""),
        authToken: walletExecutorAuthToken,
        timeoutMs: walletTimeoutMs,
        requestId: input.requestId,
        calls: payerCalls,
      }),
      payerCalls,
      walletBackend: "wallet_executor",
      artifactDir: input.artifactDir,
    });
  });

export const runEp212FullFlowSmoke = (input?: {
  readonly mode?: Ep212FullFlowMode;
  readonly requestId?: string;
  readonly artifactDir?: string;
}) => {
  const mode = input?.mode ?? "mock";
  const requestId = input?.requestId ?? "smoke:ep212-full-flow";
  const artifactDir = input?.artifactDir ?? defaultArtifactDir(requestId);
  return mode === "live"
    ? runLiveEp212FullFlow({ requestId, artifactDir })
    : runMockEp212FullFlow({ requestId, artifactDir });
};
