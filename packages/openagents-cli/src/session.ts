import { Effect, Option, Redacted } from "effect";

import type { ApiEndpoint, EndpointOverrides } from "./endpoint.js";
import { resolveEndpoint } from "./endpoint.js";
import { AuthenticationRequired } from "./errors.js";
import { CredentialStore } from "./credential-store.js";
import { EnvironmentConfiguration } from "./environment.js";
import { PersistedConfiguration } from "./persisted-configuration.js";

export interface ApiSession {
  readonly endpoint: ApiEndpoint;
  readonly token: Redacted.Redacted<string>;
}

export const resolveApiEndpoint = Effect.fn("Session.resolveApiEndpoint")(function* (
  overrides: EndpointOverrides,
) {
  const environment = yield* EnvironmentConfiguration;
  const persisted = yield* PersistedConfiguration;
  return yield* resolveEndpoint(
    overrides,
    {
      profile: environment.profile,
      apiUrl: environment.apiUrl,
    },
    persisted,
  );
});

export const findToken = Effect.fn("Session.findToken")(function* (origin: string) {
  const environment = yield* EnvironmentConfiguration;
  let result: Option.Option<{
    readonly token: Redacted.Redacted<string>;
    readonly source: "environment" | "store";
  }>;
  if (Option.isSome(environment.token)) {
    result = Option.some({ token: environment.token.value, source: "environment" });
    return result;
  }

  const credentials = yield* CredentialStore;
  const stored = yield* credentials.get(origin);
  result = Option.map(stored, (token) => ({ token, source: "store" as const }));
  return result;
});

export const resolveApiSession = Effect.fn("Session.resolveApiSession")(function* (
  overrides: EndpointOverrides,
) {
  const endpoint = yield* resolveApiEndpoint(overrides);
  const token = yield* findToken(endpoint.origin);
  if (Option.isNone(token)) {
    return yield* new AuthenticationRequired({
      origin: endpoint.origin,
      message: `No OpenAgents token is available for ${endpoint.origin}. Set OPENAGENTS_TOKEN.`,
    });
  }
  return { endpoint, token: token.value.token } satisfies ApiSession;
});
