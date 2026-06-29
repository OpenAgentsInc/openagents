import { Schema as S } from "effect";

export const PSIONIC_QWEN_BACKEND_KIND = "psionic_qwen35" as const;
export const PSIONIC_QWEN_LOCAL_PROFILE_ID = "psionic-qwen35-local" as const;
export const PSIONIC_QWEN_DEFAULT_BASE_URL = "http://127.0.0.1:8080" as const;
export const PSIONIC_QWEN_DEFAULT_MODEL_ID = "qwen3.5-2b" as const;
export const PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY = "probe.backend.psionic_qwen35" as const;

export const PSIONIC_QWEN_MODEL_REFS = {
  qwen35_0_8b: "model.psionic.qwen35.0_8b.q8_0",
  qwen35_2b: "model.psionic.qwen35.2b.q8_0",
} as const;

export const PSIONIC_QWEN_KNOWN_ARTIFACT_DIGESTS = {
  qwen35_0_8b: "afb707b6b8fac6e475acc42bc8380fc0b8d2e0e4190be5a969fbf62fcc897db5",
  qwen35_2b: undefined,
} as const;

export const PSIONIC_QWEN_SUPPORTED_ENDPOINT_REFS = {
  health: "endpoint.psionic.health",
  models: "endpoint.psionic.v1.models",
  chatCompletions: "endpoint.psionic.v1.chat_completions",
  responses: "endpoint.psionic.v1.responses",
} as const;

export const PsionicQwenHealthResponse = S.Struct({
  ready: S.optional(S.Boolean),
  backend: S.optional(S.String),
  execution_engine: S.optional(S.String),
  executionEngine: S.optional(S.String),
  default_model: S.optional(S.String),
  defaultModel: S.optional(S.String),
  model: S.optional(S.String),
  models: S.optional(S.Array(S.String)),
  supported_endpoints: S.optional(S.Array(S.String)),
  supportedEndpoints: S.optional(S.Array(S.String)),
  message: S.optional(S.String),
});
export type PsionicQwenHealthResponse = typeof PsionicQwenHealthResponse.Type;

export const PsionicQwenModelListResponse = S.Struct({
  object: S.optional(S.String),
  data: S.optional(S.Array(S.Union([S.String, S.Struct({
    id: S.String,
    object: S.optional(S.String),
    artifact_digest: S.optional(S.String),
    artifactDigest: S.optional(S.String),
    artifact_manifest_ref: S.optional(S.String),
    artifactManifestRef: S.optional(S.String),
    metadata: S.optional(S.Record(S.String, S.Unknown)),
  })]))),
});
export type PsionicQwenModelListResponse = typeof PsionicQwenModelListResponse.Type;
