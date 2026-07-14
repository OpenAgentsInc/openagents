import { Schema as S } from "effect"

import { canonicalArtifact, withoutKey } from "./artifact.ts"
import { Digest, NonEmptyString, PositiveInteger, RelativePath, StableRef } from "./schema.ts"

export const ASSURANCE_ENVIRONMENT_FORMAT_VERSION = "0.1" as const
export const ASSURANCE_ADAPTER_LOCK_FORMAT_VERSION = "0.1" as const

const StringList = S.Array(NonEmptyString)

export const AssuranceEnvironmentProfileDocumentSchema = S.Struct({
  environment_format_version: S.Literal(ASSURANCE_ENVIRONMENT_FORMAT_VERSION),
  profile_id: StableRef,
  revision: PositiveInteger,
  profile_digest: Digest,
  owner: S.Literals(["first_party", "external"]),
  target_class: S.Literals([
    "fixture",
    "local",
    "preview",
    "staging",
    "release_artifact",
    "device",
    "production",
  ]),
  mutability: S.Literals(["read_only", "isolated_write", "explicitly_armed_write", "blocked"]),
  platform: S.Struct({
    os: NonEmptyString,
    architecture: NonEmptyString,
    runtime: NonEmptyString,
    framework: NonEmptyString,
  }),
  capabilities: StringList,
  authentication_strategy: S.Literal("none"),
  isolation: S.Struct({
    fresh_identity: S.Boolean,
    reset_between_runs: S.Boolean,
    restart_supported: S.Boolean,
  }),
  data_classification: S.Literals(["public_fixture", "private_local", "restricted"]),
  evidence_visibility: S.Literals(["private", "reviewed_public_safe"]),
  retention: NonEmptyString,
  redaction_policy: NonEmptyString,
  permitted_actions: StringList,
  forbidden_actions: StringList,
  required_commands: StringList,
  dependency_lock: S.Struct({ path: RelativePath, digest: Digest }),
})
export type AssuranceEnvironmentProfileDocument = typeof AssuranceEnvironmentProfileDocumentSchema.Type

export const AssuranceAdapterLockSchema = S.Struct({
  adapter_lock_format_version: S.Literal(ASSURANCE_ADAPTER_LOCK_FORMAT_VERSION),
  adapters: S.Array(S.Struct({
    adapter_ref: StableRef,
    version: NonEmptyString,
    content_digest: Digest,
    techniques: StringList,
    capabilities: StringList,
  })),
})
export type AssuranceAdapterLock = typeof AssuranceAdapterLockSchema.Type

export const decodeAssuranceEnvironmentProfile = S.decodeUnknownSync(AssuranceEnvironmentProfileDocumentSchema)
export const decodeAssuranceAdapterLock = S.decodeUnknownSync(AssuranceAdapterLockSchema)

export const computeEnvironmentProfileDigest = (
  profile: Omit<AssuranceEnvironmentProfileDocument, "profile_digest">,
): `sha256:${string}` => canonicalArtifact(profile).digest

export const validateEnvironmentProfileDigest = (
  profile: AssuranceEnvironmentProfileDocument,
): boolean => computeEnvironmentProfileDigest(withoutKey(profile, "profile_digest")) === profile.profile_digest

export const validateAdapterLock = (lock: AssuranceAdapterLock): ReadonlyArray<string> => {
  const diagnostics: string[] = []
  const refs = new Set<string>()
  for (const adapter of lock.adapters) {
    if (refs.has(adapter.adapter_ref)) diagnostics.push(`duplicate_adapter_ref:${adapter.adapter_ref}`)
    refs.add(adapter.adapter_ref)
  }
  return diagnostics
}
