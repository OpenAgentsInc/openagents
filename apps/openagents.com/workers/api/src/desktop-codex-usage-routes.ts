import { Effect, Schema as S } from "effect";

import { sha256Hex } from "./agent-registration";
import { parseJsonUnknown } from "./json-boundary";
import type { AuthKvStore } from "./auth/auth-kv";
import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  TokenUsageLedgerUnsafePayload,
  TokenUsageLedgerValidationError,
} from "./token-usage-ledger";

export const DESKTOP_CODEX_USAGE_INGEST_PATH = "/api/desktop/codex/turn-usage";
export const DESKTOP_CODEX_USAGE_ADMISSION_PATH = "/api/desktop/codex/turn-admission";
export const DESKTOP_CODEX_USAGE_SCHEMA = "openagents.desktop.codex_turn_usage.v1";
export const DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA = "openagents.desktop.codex_turn_admission.v1";
export const DESKTOP_CODEX_USAGE_RESPONSE_SCHEMA = "openagents.desktop.codex_turn_usage_result.v1";

const MAX_BODY_BYTES = 8_192;
const MAX_TOKEN_COUNT = 2_147_483_647;
const BoundedRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const TokenCount = S.Int.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(MAX_TOKEN_COUNT));

export const DesktopCodexUsageIngestSchema = S.Struct({
  schemaVersion: S.Literal(DESKTOP_CODEX_USAGE_SCHEMA),
  admissionRef: BoundedRef,
  turnRef: BoundedRef,
  model: BoundedRef,
  observedAt: S.String.check(
    S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
    S.isMaxLength(80),
  ),
  usage: S.Struct({
    inputTokens: TokenCount,
    cachedInputTokens: TokenCount,
    outputTokens: TokenCount,
    reasoningTokens: TokenCount,
    totalTokens: TokenCount,
  }),
});

export type DesktopCodexUsageIngest = typeof DesktopCodexUsageIngestSchema.Type;

const DesktopCodexUsageAdmissionRequestSchema = S.Struct({
  schemaVersion: S.Literal(DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA),
  turnRef: BoundedRef,
  model: BoundedRef,
});

type UserBearerSessionBoundary<User, Bindings> = (
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => Promise<Readonly<{ user: User }> | undefined>;

export type DesktopCodexUsageRouteDependencies<User, Bindings> = Readonly<{
  ingestEnabled: (env: Bindings) => boolean;
  ledger: (env: Bindings) => TokenUsageLedgerShape;
  requireUserBearerSession: UserBearerSessionBoundary<User, Bindings>;
  userIdFromSession: (session: Readonly<{ user: User }>) => string;
  admissionStore: (env: Bindings) => AuthKvStore;
  now?: (() => Date) | undefined;
}>;

const ADMISSION_TTL_SECONDS = 24 * 60 * 60;
const ADMISSION_LIMIT_PER_HOUR = 120;
const admissionKey = (admissionRef: string): string => `desktop:codex:usage:admission:${admissionRef}`;

const parseAdmissionRequest = async (request: Request) => {
  try {
    const text = await request.text();
    if (new TextEncoder().encode(text).byteLength > MAX_BODY_BYTES) return undefined;
    return S.decodeUnknownSync(DesktopCodexUsageAdmissionRequestSchema)(parseJsonUnknown(text), {
      onExcessProperty: "error",
    });
  } catch {
    return undefined;
  }
};

export const handleDesktopCodexUsageAdmissionRequest = async <User, Bindings>(
  dependencies: DesktopCodexUsageRouteDependencies<User, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (!dependencies.ingestEnabled(env)) return noStoreJson({ error: "not_found" }, 404);
  if (request.method !== "POST") return noStoreJson({ error: "method_not_allowed" }, 405);
  const session = await dependencies.requireUserBearerSession(request, env, ctx);
  if (session === undefined) return noStoreJson({ error: "unauthorized" }, 401);
  const body = await parseAdmissionRequest(request);
  if (body === undefined) return noStoreJson({ error: "invalid_desktop_codex_usage_admission" }, 400);

  try {
    const ownerDigest = (await sha256Hex(dependencies.userIdFromSession(session))).slice(0, 32);
    const turnDigest = (await sha256Hex(`${ownerDigest}\u0000${body.turnRef}`)).slice(0, 32);
    const admissionRef = `admission.desktop.codex.${turnDigest}`;
    const store = dependencies.admissionStore(env);
    const existing = await store.get(admissionKey(admissionRef), "json");
    if (typeof existing === "object" && existing !== null) {
      const record = existing as Record<string, unknown>;
      if (record.ownerDigest === ownerDigest && record.turnRef === body.turnRef && record.model === body.model) {
        return noStoreJson({
          schemaVersion: DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA,
          admissionRef,
          admittedAt: record.admittedAt,
          expiresAt: record.expiresAt,
        }, 201);
      }
      return noStoreJson({ error: "desktop_codex_usage_admission_conflict" }, 409);
    }
    const now = (dependencies.now ?? (() => new Date()))();
    const nowMs = now.getTime();
    if (!Number.isFinite(nowMs)) return noStoreJson({ error: "desktop_codex_usage_unavailable" }, 503);
    const hour = now.toISOString().slice(0, 13);
    const ratePrefix = `desktop:codex:usage:rate:${ownerDigest}:${hour}:`;
    if ((await store.listPrefix(ratePrefix)).length >= ADMISSION_LIMIT_PER_HOUR) {
      return noStoreJson({ error: "desktop_codex_usage_rate_limited" }, 429);
    }
    const admittedAt = now.toISOString();
    const expiresAt = new Date(nowMs + ADMISSION_TTL_SECONDS * 1000).toISOString();
    await store.put(admissionKey(admissionRef), JSON.stringify({
      ownerDigest,
      turnRef: body.turnRef,
      model: body.model,
      admittedAt,
      expiresAt,
    }), { expirationTtl: ADMISSION_TTL_SECONDS });
    await store.put(`${ratePrefix}${turnDigest}`, "1", { expirationTtl: 2 * 60 * 60 });
    return noStoreJson({
      schemaVersion: DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA,
      admissionRef,
      admittedAt,
      expiresAt,
    }, 201);
  } catch {
    return noStoreJson({ error: "desktop_codex_usage_unavailable" }, 503);
  }
};

const noStoreJson = (body: unknown, status = 200): Response =>
  Response.json(body, {
    status,
    headers: {
      "Cache-Control": "no-store",
      "Content-Type": "application/json; charset=utf-8",
    },
  });

const parseRequest = async (request: Request): Promise<DesktopCodexUsageIngest | undefined> => {
  const contentLength = Number(request.headers.get("content-length") ?? "0");
  if (!Number.isFinite(contentLength) || contentLength > MAX_BODY_BYTES) {
    return undefined;
  }

  try {
    const text = await request.text();
    if (new TextEncoder().encode(text).byteLength > MAX_BODY_BYTES) {
      return undefined;
    }
    return S.decodeUnknownSync(DesktopCodexUsageIngestSchema)(parseJsonUnknown(text), {
      onExcessProperty: "error",
    });
  } catch {
    return undefined;
  }
};

const usageCounts = (body: DesktopCodexUsageIngest) => {
  const reasoningTokens = body.usage.reasoningTokens;
  const outputTokens = body.usage.outputTokens + reasoningTokens;
  return {
    cacheReadTokens: body.usage.cachedInputTokens,
    inputTokens: body.usage.inputTokens,
    outputTokens,
    reasoningTokens,
    totalTokens: body.usage.totalTokens,
  };
};

const ledgerErrorStatus = (
  error:
    | TokenUsageLedgerStorageError
    | TokenUsageLedgerUnsafePayload
    | TokenUsageLedgerValidationError,
): 400 | 503 => (error instanceof TokenUsageLedgerStorageError ? 503 : 400);

export const handleDesktopCodexUsageRequest = async <User, Bindings>(
  dependencies: DesktopCodexUsageRouteDependencies<User, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  // This route stays unreachable until rollout has both owner-approved consent
  // copy and an authoritative Desktop-turn admission boundary. A client-side
  // toggle alone must never expose caller-authored counts to the public ledger.
  if (!dependencies.ingestEnabled(env)) {
    return noStoreJson({ error: "not_found" }, 404);
  }

  if (request.method !== "POST") {
    return noStoreJson({ error: "method_not_allowed" }, 405);
  }

  const session = await dependencies.requireUserBearerSession(request, env, ctx);
  if (session === undefined) {
    return noStoreJson({ error: "unauthorized" }, 401);
  }

  const body = await parseRequest(request);
  if (body === undefined) {
    return noStoreJson({ error: "invalid_desktop_codex_usage" }, 400);
  }

  const counts = usageCounts(body);
  if (
    counts.totalTokens <= 0 ||
    counts.totalTokens !== counts.inputTokens + counts.outputTokens ||
    counts.outputTokens > MAX_TOKEN_COUNT ||
    counts.totalTokens > MAX_TOKEN_COUNT
  ) {
    return noStoreJson({ error: "invalid_desktop_codex_usage" }, 400);
  }

  const ownerUserId = dependencies.userIdFromSession(session);
  const observedAt = body.observedAt;
  if (!Number.isFinite(Date.parse(observedAt))) {
    return noStoreJson({ error: "desktop_codex_usage_unavailable" }, 503);
  }
  try {
    const ownerDigest = (await sha256Hex(ownerUserId)).slice(0, 32);
    const admission = await dependencies.admissionStore(env).get(
      admissionKey(body.admissionRef),
      "json",
    );
    if (typeof admission !== "object" || admission === null) {
      return noStoreJson({ error: "desktop_codex_usage_admission_required" }, 403);
    }
    const record = admission as Record<string, unknown>;
    if (
      record.ownerDigest !== ownerDigest ||
      record.turnRef !== body.turnRef ||
      record.model !== body.model ||
      typeof record.expiresAt !== "string" ||
      Date.parse(record.expiresAt) <= (dependencies.now ?? (() => new Date()))().getTime()
    ) {
      return noStoreJson({ error: "desktop_codex_usage_admission_invalid" }, 403);
    }
  } catch {
    return noStoreJson({ error: "desktop_codex_usage_unavailable" }, 503);
  }
  const digest = await sha256Hex(`${ownerUserId}\u0000${body.turnRef}`);
  const eventId = `event.inference.served-tokens.desktop-codex.${digest.slice(0, 32)}`;

  const result = await Effect.runPromise(
    dependencies
      .ledger(env)
      .ingestEvent({
        schemaVersion: "openagents.token_usage_event.v1",
        actor: { userId: ownerUserId },
        backendProfile: "desktop-codex-own-capacity",
        demand: {
          demandChannel: "direct_local",
          demandClient: "openagents_desktop",
          demandKind: "own_capacity",
          demandSource: "desktop_local_codex",
        },
        eventId,
        idempotencyKey: `desktop:codex:turn:${digest}`,
        model: body.model,
        observedAt,
        privacy: { leaderboardEligible: false, privacyOptOut: false },
        producerSystem: "omega",
        provider: "desktop-codex-own-capacity",
        roleRef: "coder",
        safeMetadata: { usageBasis: "desktop_codex_sdk_turn_completed" },
        sourceRefs: { taskRef: body.turnRef },
        // The shared ledger source-route vocabulary has no Desktop literal yet.
        // `unknown` is truthful; labeling local Codex as hosted Gemini is not.
        sourceRoute: "unknown",
        tokenCounts: {
          ...counts,
          cacheWrite1hTokens: 0,
          cacheWrite5mTokens: 0,
        },
        usageTruth: "exact",
      })
      .pipe(
        Effect.match({
          onFailure: (error) => ({ error, kind: "failure" as const }),
          onSuccess: (value) => ({ kind: "success" as const, value }),
        }),
      ),
  );

  if (result.kind === "failure") {
    const status = ledgerErrorStatus(result.error);
    return noStoreJson(
      {
        error: status === 503 ? "desktop_codex_usage_unavailable" : "invalid_desktop_codex_usage",
      },
      status,
    );
  }

  return noStoreJson({
    schemaVersion: DESKTOP_CODEX_USAGE_RESPONSE_SCHEMA,
    insertedTokenUsage: result.value.inserted,
    tokenUsageEventRef: eventId,
    tokensServedDelta: result.value.inserted ? counts.inputTokens + counts.outputTokens : 0,
  });
};

export const makeDesktopCodexUsageRouteHandler =
  <User, Bindings>(dependencies: DesktopCodexUsageRouteDependencies<User, Bindings>) =>
  (request: Request, env: Bindings, ctx: ExecutionContext): Promise<Response> =>
    handleDesktopCodexUsageRequest(dependencies, request, env, ctx);

export const makeDesktopCodexUsageAdmissionRouteHandler =
  <User, Bindings>(dependencies: DesktopCodexUsageRouteDependencies<User, Bindings>) =>
  (request: Request, env: Bindings, ctx: ExecutionContext): Promise<Response> =>
    handleDesktopCodexUsageAdmissionRequest(dependencies, request, env, ctx);
