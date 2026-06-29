import { Effect, Schema as S } from "effect";
import {
  APPLE_FM_BACKEND_KIND,
  APPLE_FM_DEFAULT_BASE_URL,
  APPLE_FM_DEFAULT_MODEL_ID,
  APPLE_FM_LOCAL_PROFILE_ID,
} from "./apple-fm/contract.js";
import {
  GEMINI_API_PROFILE_ID,
  GEMINI_BACKEND_KIND,
  GEMINI_DEFAULT_BASE_URL,
  GEMINI_DEFAULT_MODEL_ID,
} from "./gemini/contract.js";
import {
  PSIONIC_QWEN_BACKEND_KIND,
  PSIONIC_QWEN_DEFAULT_BASE_URL,
  PSIONIC_QWEN_DEFAULT_MODEL_ID,
  PSIONIC_QWEN_LOCAL_PROFILE_ID,
} from "./psionic-qwen/contract.js";
import { type ProbeBackendProfile, type ResolvedProbeBackendProfile, type ResolveProbeBackendProfileOptions } from "./backend-profile.js";

export const APPLE_FM_LOCAL_PROFILE: ProbeBackendProfile = {
  id: APPLE_FM_LOCAL_PROFILE_ID,
  kind: APPLE_FM_BACKEND_KIND,
  defaultBaseUrl: APPLE_FM_DEFAULT_BASE_URL,
  model: APPLE_FM_DEFAULT_MODEL_ID,
  attachMode: "attach_existing",
  auth: "none",
  readinessPath: "/health",
  streamMode: "snapshot",
};

export const GEMINI_API_PROFILE: ProbeBackendProfile = {
  id: GEMINI_API_PROFILE_ID,
  kind: GEMINI_BACKEND_KIND,
  defaultBaseUrl: GEMINI_DEFAULT_BASE_URL,
  model: GEMINI_DEFAULT_MODEL_ID,
  attachMode: "direct_api",
  auth: "api_key",
  readinessPath: "",
  streamMode: "sse",
};

export const PSIONIC_QWEN_LOCAL_PROFILE: ProbeBackendProfile = {
  id: PSIONIC_QWEN_LOCAL_PROFILE_ID,
  kind: PSIONIC_QWEN_BACKEND_KIND,
  defaultBaseUrl: PSIONIC_QWEN_DEFAULT_BASE_URL,
  model: PSIONIC_QWEN_DEFAULT_MODEL_ID,
  attachMode: "attach_existing",
  auth: "none",
  readinessPath: "/health",
  streamMode: "sse",
};

export const DEFAULT_BACKEND_PROFILES: ReadonlyArray<ProbeBackendProfile> = [
  APPLE_FM_LOCAL_PROFILE,
  GEMINI_API_PROFILE,
  PSIONIC_QWEN_LOCAL_PROFILE,
];

export class ProbeBackendRegistryError extends S.TaggedErrorClass<ProbeBackendRegistryError>()(
  "ProbeBackendRegistryError",
  {
    reason: S.String,
  },
) {}

export function lookupBackendProfile(
  profileId: string,
  profiles: ReadonlyArray<ProbeBackendProfile> = DEFAULT_BACKEND_PROFILES,
): Effect.Effect<ProbeBackendProfile, ProbeBackendRegistryError> {
  const profile = profiles.find((candidate) => candidate.id === profileId);

  return profile === undefined
    ? Effect.fail(new ProbeBackendRegistryError({ reason: `unknown backend profile: ${profileId}` }))
    : Effect.succeed(profile);
}

export function resolveBackendProfile(
  options: ResolveProbeBackendProfileOptions = {},
  profiles: ReadonlyArray<ProbeBackendProfile> = DEFAULT_BACKEND_PROFILES,
): Effect.Effect<ResolvedProbeBackendProfile, ProbeBackendRegistryError> {
  return Effect.gen(function* () {
    const profile = yield* lookupBackendProfile(options.profileId ?? APPLE_FM_LOCAL_PROFILE_ID, profiles);
    const resolvedBaseUrl = resolveBaseUrlForProfile(profile, options);

    return {
      ...profile,
      baseUrl: resolvedBaseUrl.baseUrl,
      baseUrlSource: resolvedBaseUrl.baseUrlSource,
    };
  });
}

export function resolveAppleFmBackendProfile(
  options: ResolveProbeBackendProfileOptions = {},
): Effect.Effect<ResolvedProbeBackendProfile, ProbeBackendRegistryError> {
  return resolveBackendProfile({ ...options, profileId: options.profileId ?? APPLE_FM_LOCAL_PROFILE_ID });
}

export function resolveGeminiBackendProfile(
  options: ResolveProbeBackendProfileOptions = {},
): Effect.Effect<ResolvedProbeBackendProfile, ProbeBackendRegistryError> {
  return resolveBackendProfile({ ...options, profileId: options.profileId ?? GEMINI_API_PROFILE_ID });
}

export function resolvePsionicQwenBackendProfile(
  options: ResolveProbeBackendProfileOptions = {},
): Effect.Effect<ResolvedProbeBackendProfile, ProbeBackendRegistryError> {
  return resolveBackendProfile({ ...options, profileId: options.profileId ?? PSIONIC_QWEN_LOCAL_PROFILE_ID });
}

function resolveBaseUrlForProfile(
  profile: ProbeBackendProfile,
  options: ResolveProbeBackendProfileOptions,
): Pick<ResolvedProbeBackendProfile, "baseUrl" | "baseUrlSource"> {
  if (profile.kind === GEMINI_BACKEND_KIND) {
    return resolveGeminiBaseUrl(profile.defaultBaseUrl, options);
  }

  if (profile.kind === PSIONIC_QWEN_BACKEND_KIND) {
    return resolvePsionicQwenBaseUrl(profile.defaultBaseUrl, options);
  }

  return resolveAppleFmBaseUrl(profile.defaultBaseUrl, options);
}

function resolveAppleFmBaseUrl(
  defaultBaseUrl: string,
  options: ResolveProbeBackendProfileOptions,
): Pick<ResolvedProbeBackendProfile, "baseUrl" | "baseUrlSource"> {
  if (isNonEmptyString(options.explicitBaseUrl)) {
    return { baseUrl: options.explicitBaseUrl, baseUrlSource: "explicit" };
  }

  if (isNonEmptyString(options.env?.PROBE_APPLE_FM_BASE_URL)) {
    return { baseUrl: options.env.PROBE_APPLE_FM_BASE_URL, baseUrlSource: "PROBE_APPLE_FM_BASE_URL" };
  }

  if (isNonEmptyString(options.env?.OPENAGENTS_APPLE_FM_BASE_URL)) {
    return { baseUrl: options.env.OPENAGENTS_APPLE_FM_BASE_URL, baseUrlSource: "OPENAGENTS_APPLE_FM_BASE_URL" };
  }

  return { baseUrl: defaultBaseUrl, baseUrlSource: "default" };
}

function resolveGeminiBaseUrl(
  defaultBaseUrl: string,
  options: ResolveProbeBackendProfileOptions,
): Pick<ResolvedProbeBackendProfile, "baseUrl" | "baseUrlSource"> {
  if (isNonEmptyString(options.explicitBaseUrl)) {
    return { baseUrl: options.explicitBaseUrl, baseUrlSource: "explicit" };
  }

  if (isNonEmptyString(options.env?.PROBE_GEMINI_BASE_URL)) {
    return { baseUrl: options.env.PROBE_GEMINI_BASE_URL, baseUrlSource: "PROBE_GEMINI_BASE_URL" };
  }

  if (isNonEmptyString(options.env?.PROBE_OMEGA_BASE_URL) && isNonEmptyString(options.env?.PROBE_OMEGA_BEARER_TOKEN)) {
    return {
      baseUrl: `${withoutTrailingSlash(options.env.PROBE_OMEGA_BASE_URL)}/api/provider-accounts/google-gemini`,
      baseUrlSource: "PROBE_OMEGA_BASE_URL",
    };
  }

  return { baseUrl: defaultBaseUrl, baseUrlSource: "default" };
}

function resolvePsionicQwenBaseUrl(
  defaultBaseUrl: string,
  options: ResolveProbeBackendProfileOptions,
): Pick<ResolvedProbeBackendProfile, "baseUrl" | "baseUrlSource"> {
  if (isNonEmptyString(options.explicitBaseUrl)) {
    return { baseUrl: options.explicitBaseUrl, baseUrlSource: "explicit" };
  }

  if (isNonEmptyString(options.env?.PYLON_PSIONIC_BASE_URL)) {
    return { baseUrl: options.env.PYLON_PSIONIC_BASE_URL, baseUrlSource: "PYLON_PSIONIC_BASE_URL" };
  }

  if (isNonEmptyString(options.env?.PROBE_PSIONIC_BASE_URL)) {
    return { baseUrl: options.env.PROBE_PSIONIC_BASE_URL, baseUrlSource: "PROBE_PSIONIC_BASE_URL" };
  }

  return { baseUrl: defaultBaseUrl, baseUrlSource: "default" };
}

function isNonEmptyString(value: string | undefined): value is string {
  return value !== undefined && value.trim().length > 0;
}

function withoutTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
