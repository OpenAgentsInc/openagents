import { Exit, Schema } from "@effect-native/core/effect"
export const providerRuntimeKinds = ["codex_cli", "claude_agent_sdk"] as const
export type ProviderRuntimeKind = (typeof providerRuntimeKinds)[number]
export const providerRuntimeCompatibilityStates = ["compatible", "missing", "malformed", "incompatible"] as const

/** Exact versions exercised by this source/lockfile. Updates require a new receipt. */
export const supportedProviderRuntimeVersions = {
  codex_cli: ">=0.144.1",
  claude_agent_sdk: "0.3.172",
} as const satisfies Readonly<Record<ProviderRuntimeKind, string>>

export const ProviderRuntimeCompatibilitySchema = Schema.Struct({
  kind: Schema.Literals(providerRuntimeKinds),
  state: Schema.Literals(providerRuntimeCompatibilityStates),
  expectedVersion: Schema.String,
  observedVersion: Schema.NullOr(Schema.String),
  reason: Schema.Literals(["verified", "not_found", "unreadable_version", "unverified_version"]),
})
export type ProviderRuntimeCompatibility = Schema.Schema.Type<typeof ProviderRuntimeCompatibilitySchema>

const versionFrom = (kind: ProviderRuntimeKind, raw: string): string | null => {
  const value = raw.trim()
  const match = kind === "codex_cli"
    ? /^(?:codex-cli|codex)\s+(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/u.exec(value)
    : /^(?:@anthropic-ai\/claude-agent-sdk\s+)?(\d+\.\d+\.\d+)$/u.exec(value)
  return match?.[1] ?? null
}

/**
 * Fail-closed compatibility classification. An unreceipted patch is not
 * guessed compatible: the catalog reports it as incompatible until the lock
 * version, corpus, and live receipt advance together.
 */
export const classifyProviderRuntimeCompatibility = (
  kind: ProviderRuntimeKind,
  rawVersion: string | null,
): ProviderRuntimeCompatibility => {
  const expectedVersion = supportedProviderRuntimeVersions[kind]
  if (rawVersion === null) return { kind, state: "missing", expectedVersion, observedVersion: null, reason: "not_found" }
  const observedVersion = versionFrom(kind, rawVersion)
  if (observedVersion === null) return { kind, state: "malformed", expectedVersion, observedVersion: null, reason: "unreadable_version" }
  const compatible = kind === "codex_cli"
    ? codexVersionAtLeast(observedVersion, "0.144.1")
    : observedVersion === expectedVersion
  return compatible
    ? { kind, state: "compatible", expectedVersion, observedVersion, reason: "verified" }
    : { kind, state: "incompatible", expectedVersion, observedVersion, reason: "unverified_version" }
}

const codexVersionAtLeast = (observed: string, minimum: string): boolean => {
  const components = (value: string): ReadonlyArray<number> =>
    value.split("-", 1)[0]!.split(".").map(component => Number.parseInt(component, 10))
  const actual = components(observed)
  const required = components(minimum)
  for (let index = 0; index < 3; index += 1) {
    if (actual[index]! > required[index]!) return true
    if (actual[index]! < required[index]!) return false
  }
  return true
}

export const decodeProviderRuntimeCompatibility = (value: unknown): ProviderRuntimeCompatibility | null => {
  const decoded = Schema.decodeUnknownExit(ProviderRuntimeCompatibilitySchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}
