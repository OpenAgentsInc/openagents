import { Effect, Option, Schema } from "effect";

import { ConfigurationError } from "./errors.js";

export const Profile = Schema.Literals(["production", "staging", "local"]);
export type Profile = typeof Profile.Type;

export const PROFILE_ORIGINS = {
  production: "https://openagents.com",
  staging: "https://staging.openagents.com",
  local: "http://localhost:4000",
} as const satisfies Record<Profile, string>;

export interface EndpointOverrides {
  readonly profile: Option.Option<Profile>;
  readonly apiUrl: Option.Option<string>;
}

export interface EndpointEnvironment {
  readonly profile: Option.Option<string>;
  readonly apiUrl: Option.Option<string>;
}

export interface ApiEndpoint {
  readonly origin: string;
  readonly profile: Profile | "custom";
}

export const loopbackHostname = (hostname: string): boolean => {
  const normalized = hostname.toLowerCase();
  return (
    normalized === "localhost" ||
    normalized.endsWith(".localhost") ||
    normalized === "127.0.0.1" ||
    normalized.startsWith("127.") ||
    normalized === "[::1]" ||
    normalized === "::1"
  );
};

export const normalizeApiOrigin = Effect.fn("Endpoint.normalizeApiOrigin")(function* (
  input: string,
) {
  const value = input.trim();
  if (value.length === 0) {
    return yield* new ConfigurationError({ message: "The API URL cannot be empty." });
  }

  const url = yield* Effect.try({
    try: () => new URL(value),
    catch: () => new ConfigurationError({ message: `Invalid API URL: ${value}` }),
  });

  if (url.username !== "" || url.password !== "") {
    return yield* new ConfigurationError({ message: "The API URL cannot contain credentials." });
  }
  if (url.pathname !== "/" || url.search !== "" || url.hash !== "") {
    return yield* new ConfigurationError({
      message: "The API URL must be an origin without a path, query, or fragment.",
    });
  }
  if (url.protocol !== "https:" && !(url.protocol === "http:" && loopbackHostname(url.hostname))) {
    return yield* new ConfigurationError({
      message: "API URLs must use HTTPS. HTTP is allowed only for loopback development.",
    });
  }

  return url.origin;
});

const decodeProfile = Effect.fn("Endpoint.decodeProfile")(function* (value: string) {
  return yield* Schema.decodeUnknownEffect(Profile)(value).pipe(
    Effect.mapError(
      () =>
        new ConfigurationError({
          message: `Unknown profile '${value}'. Use production, staging, or local.`,
        }),
    ),
  );
});

export const resolveEndpoint = Effect.fn("Endpoint.resolve")(function* (
  overrides: EndpointOverrides,
  environment: EndpointEnvironment,
) {
  if (Option.isSome(overrides.apiUrl)) {
    const origin = yield* normalizeApiOrigin(overrides.apiUrl.value);
    return { origin, profile: "custom" } satisfies ApiEndpoint;
  }

  if (Option.isSome(overrides.profile)) {
    const profile = overrides.profile.value;
    return { origin: PROFILE_ORIGINS[profile], profile } satisfies ApiEndpoint;
  }

  if (Option.isSome(environment.apiUrl)) {
    const origin = yield* normalizeApiOrigin(environment.apiUrl.value);
    return { origin, profile: "custom" } satisfies ApiEndpoint;
  }

  const profile = Option.isSome(environment.profile)
    ? yield* decodeProfile(environment.profile.value)
    : "production";
  return { origin: PROFILE_ORIGINS[profile], profile } satisfies ApiEndpoint;
});
