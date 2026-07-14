import { Schema as S } from "effect"

export const KHALA_CODE_MODEL_ROLE_REGISTRY_SCHEMA =
  "openagents.khala_code.model_roles.v1" as const

export const KhalaCodeModelRoleSchema = S.Literals([
  "advisor",
  "architect",
  "coder",
  "judge",
])
export type KhalaCodeModelRole = typeof KhalaCodeModelRoleSchema.Type

export const KhalaCodeModelRoleHarnessSchema = S.Literals(["claude", "codex", "grok", "khala"])
export type KhalaCodeModelRoleHarness = typeof KhalaCodeModelRoleHarnessSchema.Type

export const KhalaCodeModelRoleEffortSchema = S.Literals([
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
])
export type KhalaCodeModelRoleEffort = typeof KhalaCodeModelRoleEffortSchema.Type

export const KhalaCodeModelRoleEntrySchema = S.Struct({
  effort: S.optional(KhalaCodeModelRoleEffortSchema),
  harness: KhalaCodeModelRoleHarnessSchema,
  model: S.optional(S.String),
  role: KhalaCodeModelRoleSchema,
})
export type KhalaCodeModelRoleEntry = typeof KhalaCodeModelRoleEntrySchema.Type

export const KhalaCodeModelRoleRegistrySchema = S.Struct({
  schema: S.Literal(KHALA_CODE_MODEL_ROLE_REGISTRY_SCHEMA),
  roles: S.Struct({
    advisor: KhalaCodeModelRoleEntrySchema,
    architect: KhalaCodeModelRoleEntrySchema,
    coder: KhalaCodeModelRoleEntrySchema,
    judge: KhalaCodeModelRoleEntrySchema,
  }),
})
export type KhalaCodeModelRoleRegistry = typeof KhalaCodeModelRoleRegistrySchema.Type

export const KHALA_CODE_MODEL_ROLE_ORDER: readonly KhalaCodeModelRole[] = [
  "architect",
  "coder",
  "judge",
  "advisor",
]

export const defaultKhalaCodeModelRoleRegistry =
  (): KhalaCodeModelRoleRegistry => ({
    schema: KHALA_CODE_MODEL_ROLE_REGISTRY_SCHEMA,
    roles: {
      advisor: { role: "advisor", harness: "codex", effort: "low" },
      architect: { role: "architect", harness: "codex", effort: "high" },
      coder: { role: "coder", harness: "codex", effort: "medium" },
      judge: { role: "judge", harness: "codex", effort: "high" },
    },
  })

export const decodeKhalaCodeModelRoleRegistry = (
  value: unknown,
): KhalaCodeModelRoleRegistry =>
  S.decodeUnknownSync(KhalaCodeModelRoleRegistrySchema)(value)

export const khalaCodeModelRoleRegistryWithEntry = (
  registry: KhalaCodeModelRoleRegistry,
  entry: KhalaCodeModelRoleEntry,
): KhalaCodeModelRoleRegistry => ({
  schema: KHALA_CODE_MODEL_ROLE_REGISTRY_SCHEMA,
  roles: {
    ...registry.roles,
    [entry.role]: entry,
  },
})
