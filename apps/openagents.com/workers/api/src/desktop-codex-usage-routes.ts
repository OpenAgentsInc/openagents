import { Effect, Schema as S } from "effect";

import { sha256Hex } from "./agent-registration";
import { parseJsonUnknown } from "./json-boundary";
import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  TokenUsageLedgerUnsafePayload,
  TokenUsageLedgerValidationError,
} from "./token-usage-ledger";

export const DESKTOP_CODEX_USAGE_INGEST_PATH = "/api/desktop/codex/turn-usage";
export const DESKTOP_CODEX_USAGE_SCHEMA = "openagents.desktop.codex_turn_usage.v1";
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

type UserBearerSessionBoundary<User, Bindings> = (
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => Promise<Readonly<{ user: User }> | undefined>;

export type DesktopCodexUsageRouteDependencies<User, Bindings> = Readonly<{
  ledger: (env: Bindings) => TokenUsageLedgerShape;
  requireUserBearerSession: UserBearerSessionBoundary<User, Bindings>;
  userIdFromSession: (session: Readonly<{ user: User }>) => string;
}>;

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
