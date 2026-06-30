import { Schema as S } from "effect"

export const KhalaFeatureStage = S.Literals([
  "under-development",
  "experimental",
  "stable",
  "deprecated",
  "removed",
])
export type KhalaFeatureStage = typeof KhalaFeatureStage.Type

export const KhalaFeatureSpec = S.Struct({
  defaultEnabled: S.Boolean,
  description: S.optional(S.String),
  name: S.String,
  stage: KhalaFeatureStage,
})
export type KhalaFeatureSpec = typeof KhalaFeatureSpec.Type

export type KhalaFeatureConfig<Name extends string = string> = Readonly<{
  features?: Partial<Record<Name, boolean>>
}>

export type KhalaFeatureOverride<Name extends string = string> = Readonly<{
  enabled: boolean
  name: Name
  source: "cli" | "config" | "runtime"
}>

export type KhalaFeatureResolution<Name extends string = string> = Readonly<{
  disabledFeatures: ReadonlyArray<Name>
  enabled: Readonly<Record<Name, boolean>>
  enabledFeatures: ReadonlyArray<Name>
  registry: ReadonlyArray<KhalaFeatureSpec & Readonly<{ name: Name }>>
}>

export interface KhalaFeatureRegistry<Name extends string = string> {
  readonly defaults: () => Readonly<Record<Name, boolean>>
  readonly get: (name: Name) => (KhalaFeatureSpec & Readonly<{ name: Name }>) | undefined
  readonly has: (name: string) => name is Name
  readonly list: () => ReadonlyArray<KhalaFeatureSpec & Readonly<{ name: Name }>>
  readonly resolve: (input?: {
    readonly config?: KhalaFeatureConfig<Name>
    readonly overrides?: ReadonlyArray<KhalaFeatureOverride<Name>>
  }) => KhalaFeatureResolution<Name>
}

export function defineKhalaFeatureRegistry<const Spec extends ReadonlyArray<KhalaFeatureSpec>>(
  specs: Spec,
): KhalaFeatureRegistry<Spec[number]["name"]> {
  type Name = Spec[number]["name"]
  const byName = new Map<Name, KhalaFeatureSpec & Readonly<{ name: Name }>>()
  for (const spec of specs) {
    validateFeatureName(spec.name)
    if (byName.has(spec.name as Name)) {
      throw new Error(`duplicate_feature_flag: ${spec.name}`)
    }
    byName.set(spec.name as Name, { ...spec, name: spec.name as Name })
  }
  const ordered = [...byName.values()]

  const assertKnown = (name: string): Name => {
    if (!byName.has(name as Name)) {
      throw new Error(`unknown_feature_flag: ${name}`)
    }
    return name as Name
  }

  const assertMutable = (name: Name, enabled: boolean): void => {
    const spec = byName.get(name)
    if (enabled && spec?.stage === "removed") {
      throw new Error(`removed_feature_flag: ${name}`)
    }
  }

  const defaults = (): Readonly<Record<Name, boolean>> =>
    Object.freeze(Object.fromEntries(ordered.map(spec => [spec.name, spec.defaultEnabled])) as Record<Name, boolean>)

  return {
    defaults,
    get: name => byName.get(name),
    has: (name: string): name is Name => byName.has(name as Name),
    list: () => ordered,
    resolve: input => {
      const enabled = { ...defaults() } as Record<Name, boolean>
      for (const [rawName, value] of Object.entries(input?.config?.features ?? {})) {
        if (typeof value !== "boolean") {
          throw new Error(`invalid_feature_flag_value: ${rawName}`)
        }
        const name = assertKnown(rawName)
        assertMutable(name, value)
        enabled[name] = value
      }
      for (const override of input?.overrides ?? []) {
        const name = assertKnown(override.name)
        assertMutable(name, override.enabled)
        enabled[name] = override.enabled
      }
      const enabledFeatures = ordered
        .filter(spec => enabled[spec.name])
        .map(spec => spec.name)
      const disabledFeatures = ordered
        .filter(spec => !enabled[spec.name])
        .map(spec => spec.name)
      return Object.freeze({
        disabledFeatures,
        enabled: Object.freeze(enabled),
        enabledFeatures,
        registry: ordered,
      })
    },
  }
}

export function parseKhalaFeatureFlagArgs<Name extends string = string>(
  args: ReadonlyArray<string>,
): Readonly<{
  overrides: ReadonlyArray<KhalaFeatureOverride<Name>>
  passthroughArgs: ReadonlyArray<string>
}> {
  const overrides: KhalaFeatureOverride<Name>[] = []
  const passthroughArgs: string[] = []

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--enable" || arg === "--disable") {
      const next = args[index + 1]
      if (next === undefined || next.startsWith("--")) {
        throw new Error(`missing_feature_flag_name: ${arg}`)
      }
      appendFeatureOverrides(overrides, arg === "--enable", next)
      index += 1
      continue
    }
    if (arg.startsWith("--enable=")) {
      appendFeatureOverrides(overrides, true, arg.slice("--enable=".length))
      continue
    }
    if (arg.startsWith("--disable=")) {
      appendFeatureOverrides(overrides, false, arg.slice("--disable=".length))
      continue
    }
    passthroughArgs.push(arg)
  }

  return { overrides, passthroughArgs }
}

export function isKhalaFeatureEnabled<Name extends string>(
  resolution: KhalaFeatureResolution<Name>,
  name: Name,
): boolean {
  return resolution.enabled[name] === true
}

function appendFeatureOverrides<Name extends string>(
  overrides: KhalaFeatureOverride<Name>[],
  enabled: boolean,
  value: string,
): void {
  const names = value
    .split(",")
    .map(name => name.trim())
    .filter(name => name.length > 0)
  if (names.length === 0) {
    throw new Error("missing_feature_flag_name")
  }
  for (const name of names) {
    validateFeatureName(name)
    overrides.push({ enabled, name: name as Name, source: "cli" })
  }
}

function validateFeatureName(name: string): void {
  if (!/^[a-z][a-z0-9_.-]*$/u.test(name)) {
    throw new Error(`invalid_feature_flag_name: ${name}`)
  }
}
