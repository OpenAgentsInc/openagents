import { Effect, Option, Schema as S } from "effect";

/**
 * HTTP client for the Worker `POST/GET /v1/cloud-coding-sessions` Agent Computer
 * launch surface. Credentials stay on the call site — never on
 * {@link HarnessEnvironment}.
 */

export const CloudCodingSessionState = S.Literals([
  "queued",
  "running",
  "completed",
  "failed",
  "cancelled",
]);
export type CloudCodingSessionState = typeof CloudCodingSessionState.Type;

export const CloudCodingLane = S.Literals(["cloud-gcp"]);
export type CloudCodingLane = typeof CloudCodingLane.Type;

export const CloudCodingAdapter = S.Literals(["codex", "claude_agent"]);
export type CloudCodingAdapter = typeof CloudCodingAdapter.Type;

export const RepoTrustTier = S.Literals(["public", "private", "regulated"]);
export type RepoTrustTier = typeof RepoTrustTier.Type;

/** Public-safe launch request body for `POST /v1/cloud-coding-sessions`. */
export const CloudCodingSessionLaunchRequest = S.Struct({
  repoRef: S.String,
  objective: S.String,
  lane: S.optionalKey(CloudCodingLane),
  repoTrustTier: S.optionalKey(RepoTrustTier),
  adapter: S.optionalKey(CloudCodingAdapter),
  workContextRef: S.optionalKey(S.String),
  threadRef: S.optionalKey(S.String),
  repoBindingRef: S.optionalKey(S.String),
  verify: S.optionalKey(S.Array(S.String)),
  timeoutSeconds: S.optionalKey(S.Number),
});
export interface CloudCodingSessionLaunchRequest
  extends S.Schema.Type<typeof CloudCodingSessionLaunchRequest> {}

/**
 * Public-safe projection returned by launch/lifecycle routes
 * (`projectSession` on the Worker). Extra fields are ignored.
 */
export const CloudCodingSessionProjection = S.Struct({
  object: S.optionalKey(S.String),
  product_object: S.optionalKey(S.String),
  id: S.String,
  state: CloudCodingSessionState,
  lane: S.optionalKey(S.String),
  adapter: S.optionalKey(S.String),
  repo_ref: S.optionalKey(S.String),
  repo_trust_tier: S.optionalKey(S.String),
  timeout_seconds: S.optionalKey(S.Number),
  placement_ref: S.optionalKey(S.NullOr(S.String)),
  lease_refs: S.optionalKey(S.Array(S.String)),
  work_context_ref: S.optionalKey(S.String),
  agent_computer_ref: S.optionalKey(S.NullOr(S.String)),
  agent_computer_state: S.optionalKey(S.String),
  lifecycle_receipt_refs: S.optionalKey(S.Array(S.String)),
  resource_usage_receipt_refs: S.optionalKey(S.Array(S.String)),
  artifact_ref: S.optionalKey(S.NullOr(S.String)),
  created_at: S.optionalKey(S.String),
  metered: S.optionalKey(S.Boolean),
  receipt_ref: S.optionalKey(S.NullOr(S.String)),
});
export interface CloudCodingSessionProjection
  extends S.Schema.Type<typeof CloudCodingSessionProjection> {}

export const CloudCodingSessionErrorBody = S.Struct({
  error: S.optionalKey(S.String),
  reason: S.optionalKey(S.String),
  reason_ref: S.optionalKey(S.String),
});
export interface CloudCodingSessionErrorBody
  extends S.Schema.Type<typeof CloudCodingSessionErrorBody> {}

export class CloudCodingSessionHttpError extends S.TaggedErrorClass<CloudCodingSessionHttpError>()(
  "AgentHarness.CloudCodingSessionHttpError",
  {
    failureClass: S.String,
    status: S.optionalKey(S.Number),
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

export type FetchLike = (
  input: string,
  init?: RequestInit,
) => Promise<Response>;

export interface CloudCodingSessionClientConfig {
  readonly launchUrl: string;
  readonly bearerToken: string;
  readonly fetch?: FetchLike;
}

const TERMINAL_STATES: ReadonlySet<CloudCodingSessionState> = new Set([
  "completed",
  "failed",
  "cancelled",
]);

export const isTerminalCloudCodingSessionState = (
  state: CloudCodingSessionState,
): boolean => TERMINAL_STATES.has(state);

const failureClassForStatus = (
  status: number,
  body: CloudCodingSessionErrorBody | undefined,
): string => {
  const named = body?.error ?? body?.reason;
  if (typeof named === "string" && named.trim() !== "") {
    return named.trim();
  }
  if (status === 401) return "unauthorized";
  if (status === 403) return "forbidden";
  if (status === 404) return "not_found";
  if (status === 429) return "rate_limited";
  if (status === 502) return "runtime_error";
  if (status >= 500) return "upstream_error";
  if (status >= 400) return "invalid_request";
  return "http_error";
};

const readJsonUnknown = (response: Response): Effect.Effect<unknown, CloudCodingSessionHttpError> =>
  Effect.tryPromise({
    try: () => response.json() as Promise<unknown>,
    catch: (cause) =>
      new CloudCodingSessionHttpError({
        failureClass: "invalid_response",
        status: response.status,
        detail: "Response body was not JSON.",
        cause,
      }),
  });

const decodeSession = (
  value: unknown,
  status: number,
): Effect.Effect<CloudCodingSessionProjection, CloudCodingSessionHttpError> =>
  S.decodeUnknownEffect(CloudCodingSessionProjection)(value, {
    onExcessProperty: "ignore",
  }).pipe(
    Effect.mapError(
      (cause) =>
        new CloudCodingSessionHttpError({
          failureClass: "invalid_response",
          status,
          detail: "Cloud coding session projection failed schema decode.",
          cause,
        }),
    ),
  );

const decodeErrorBody = (value: unknown): CloudCodingSessionErrorBody | undefined => {
  const decoded = S.decodeUnknownOption(CloudCodingSessionErrorBody)(value, {
    onExcessProperty: "ignore",
  });
  return Option.getOrUndefined(decoded);
};

const authHeaders = (bearerToken: string): HeadersInit => ({
  Accept: "application/json",
  Authorization: `Bearer ${bearerToken}`,
  "Content-Type": "application/json",
});

export interface CloudCodingSessionClient {
  readonly launch: (
    request: CloudCodingSessionLaunchRequest,
  ) => Effect.Effect<CloudCodingSessionProjection, CloudCodingSessionHttpError>;
  readonly get: (
    sessionId: string,
  ) => Effect.Effect<CloudCodingSessionProjection, CloudCodingSessionHttpError>;
}

export const makeCloudCodingSessionClient = (
  config: CloudCodingSessionClientConfig,
): CloudCodingSessionClient => {
  const fetchImpl = config.fetch ?? globalThis.fetch;
  const launchUrl = config.launchUrl.replace(/\/+$/u, "");

  const launch = Effect.fn("CloudCodingSessionClient.launch")(function* (
    request: CloudCodingSessionLaunchRequest,
  ) {
    const response = yield* Effect.tryPromise({
      try: (signal) =>
        fetchImpl(launchUrl, {
          method: "POST",
          headers: authHeaders(config.bearerToken),
          body: JSON.stringify(request),
          signal,
        }),
      catch: (cause) =>
        new CloudCodingSessionHttpError({
          failureClass: "network_failed",
          detail: "POST /v1/cloud-coding-sessions transport failed.",
          cause,
        }),
    });

    const body = yield* readJsonUnknown(response);
    if (!response.ok) {
      const errorBody = decodeErrorBody(body);
      const detail = errorBody?.reason_ref ?? errorBody?.reason ?? errorBody?.error;
      return yield* new CloudCodingSessionHttpError({
        failureClass: failureClassForStatus(response.status, errorBody),
        status: response.status,
        ...(detail === undefined ? {} : { detail }),
      });
    }
    return yield* decodeSession(body, response.status);
  });

  const get = Effect.fn("CloudCodingSessionClient.get")(function* (sessionId: string) {
    const url = `${launchUrl}/${encodeURIComponent(sessionId)}`;
    const response = yield* Effect.tryPromise({
      try: (signal) =>
        fetchImpl(url, {
          method: "GET",
          headers: authHeaders(config.bearerToken),
          signal,
        }),
      catch: (cause) =>
        new CloudCodingSessionHttpError({
          failureClass: "network_failed",
          detail: "GET /v1/cloud-coding-sessions/:id transport failed.",
          cause,
        }),
    });

    const body = yield* readJsonUnknown(response);
    if (!response.ok) {
      const errorBody = decodeErrorBody(body);
      const detail = errorBody?.reason_ref ?? errorBody?.reason ?? errorBody?.error;
      return yield* new CloudCodingSessionHttpError({
        failureClass: failureClassForStatus(response.status, errorBody),
        status: response.status,
        ...(detail === undefined ? {} : { detail }),
      });
    }
    return yield* decodeSession(body, response.status);
  });

  return { launch, get };
};
