import { Effect, Schema as S } from "effect";
import {
  GEMINI_API_PROFILE_ID,
  GEMINI_BACKEND_KIND,
  GeminiAuthResolutionReceipt,
  type GeminiApiKeySource,
  type GeminiAuthResolutionReceipt,
} from "./contract";

export interface ResolveGeminiApiKeyOptions {
  readonly apiKey?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly profileId?: string;
}

export interface ResolvedGeminiApiKey {
  readonly apiKey: string;
  readonly source: GeminiApiKeySource;
  readonly receipt: GeminiAuthResolutionReceipt;
}

export class GeminiAuthError extends S.TaggedErrorClass<GeminiAuthError>()("GeminiAuthError", {
  reason: S.String,
  missingCredential: S.Literal(true),
}) {}

export function resolveGeminiApiKey(options: ResolveGeminiApiKeyOptions = {}): Effect.Effect<ResolvedGeminiApiKey, GeminiAuthError> {
  const explicit = nonEmpty(options.apiKey);

  if (explicit !== undefined) {
    return Effect.succeed(resolved(explicit, "explicit", options.profileId));
  }

  const google = nonEmpty(options.env?.GOOGLE_GENERATIVE_AI_API_KEY);

  if (google !== undefined) {
    return Effect.succeed(resolved(google, "GOOGLE_GENERATIVE_AI_API_KEY", options.profileId));
  }

  const gemini = nonEmpty(options.env?.GEMINI_API_KEY);

  if (gemini !== undefined) {
    return Effect.succeed(resolved(gemini, "GEMINI_API_KEY", options.profileId));
  }

  const omegaBearer =
    nonEmpty(options.env?.PROBE_OMEGA_BASE_URL) === undefined
      ? undefined
      : nonEmpty(options.env?.PROBE_OMEGA_BEARER_TOKEN);

  if (omegaBearer !== undefined) {
    return Effect.succeed(resolved(omegaBearer, "PROBE_OMEGA_BEARER_TOKEN", options.profileId));
  }

  return Effect.fail(
    new GeminiAuthError({
      reason: "missing Gemini API key: set GOOGLE_GENERATIVE_AI_API_KEY or GEMINI_API_KEY",
      missingCredential: true,
    }),
  );
}

export function makeGeminiAuthHeaders(resolvedKey: ResolvedGeminiApiKey): Readonly<Record<string, string>> {
  if (resolvedKey.source === "PROBE_OMEGA_BEARER_TOKEN") {
    return {
      Authorization: `Bearer ${resolvedKey.apiKey}`,
    };
  }

  return {
    "x-goog-api-key": resolvedKey.apiKey,
  };
}

function resolved(apiKey: string, source: GeminiApiKeySource, profileId = GEMINI_API_PROFILE_ID): ResolvedGeminiApiKey {
  return {
    apiKey,
    source,
    receipt: {
      kind: "probe_gemini_auth_resolution",
      backendKind: GEMINI_BACKEND_KIND,
      profileId,
      apiKeySource: source,
      apiKeyRedacted: true,
    },
  };
}

function nonEmpty(value: string | undefined): string | undefined {
  return value === undefined || value.trim().length === 0 ? undefined : value;
}
