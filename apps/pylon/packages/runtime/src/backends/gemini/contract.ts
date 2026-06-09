import { Schema as S } from "effect";

export const GEMINI_BACKEND_KIND = "gemini_api" as const;
export const GEMINI_API_PROFILE_ID = "gemini-api" as const;
export const GEMINI_DEFAULT_MODEL_ID = "gemini-3.5-flash" as const;
export const GEMINI_DEFAULT_BASE_URL = "https://generativelanguage.googleapis.com/v1beta" as const;
export const PROBE_GEMINI_BACKEND_CAPABILITY = "probe.backend.gemini_api" as const;

export const GeminiBackendKind = S.Literal(GEMINI_BACKEND_KIND);
export type GeminiBackendKind = typeof GeminiBackendKind.Type;

export const GeminiProfileId = S.Literal(GEMINI_API_PROFILE_ID);
export type GeminiProfileId = typeof GeminiProfileId.Type;

export const GeminiApiKeySource = S.Literals([
  "explicit",
  "GOOGLE_GENERATIVE_AI_API_KEY",
  "GEMINI_API_KEY",
  "PROBE_OMEGA_BEARER_TOKEN",
]);
export type GeminiApiKeySource = typeof GeminiApiKeySource.Type;

export const GeminiAuthResolutionReceipt = S.Struct({
  kind: S.Literal("probe_gemini_auth_resolution"),
  backendKind: GeminiBackendKind,
  profileId: S.String,
  apiKeySource: GeminiApiKeySource,
  apiKeyRedacted: S.Literal(true),
});
export type GeminiAuthResolutionReceipt = typeof GeminiAuthResolutionReceipt.Type;
