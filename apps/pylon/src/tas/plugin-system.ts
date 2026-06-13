export type PluginContributions = Readonly<{
  tools?: readonly string[]
  commands?: readonly string[]
  hooks?: readonly string[]
}>

export type Plugin = Readonly<{
  name: string
  version: string
  contributes: PluginContributions
  enabled: boolean
}>

export type PluginRegistry = readonly Plugin[]

export type ContributionKind = "tools" | "commands" | "hooks"

export type ContributionConflict = Readonly<{
  kind: ContributionKind
  name: string
  pluginNames: readonly string[]
}>

export type EffectivePluginContributions = Readonly<{
  tools: readonly string[]
  commands: readonly string[]
  hooks: readonly string[]
  conflicts: readonly ContributionConflict[]
}>

export function enablePlugin(
  registry: PluginRegistry,
  pluginName: string,
): PluginRegistry {
  assertUniquePluginNames(registry)

  return registry.map(plugin =>
    plugin.name === pluginName ? { ...plugin, enabled: true } : plugin,
  )
}

export function disablePlugin(
  registry: PluginRegistry,
  pluginName: string,
): PluginRegistry {
  assertUniquePluginNames(registry)

  return registry.map(plugin =>
    plugin.name === pluginName ? { ...plugin, enabled: false } : plugin,
  )
}

export function effectiveContributions(
  registry: PluginRegistry,
): EffectivePluginContributions {
  assertUniquePluginNames(registry)

  const enabledPlugins = registry.filter(plugin => plugin.enabled)

  return {
    tools: collectContributionNames(enabledPlugins, "tools"),
    commands: collectContributionNames(enabledPlugins, "commands"),
    hooks: collectContributionNames(enabledPlugins, "hooks"),
    conflicts: [
      ...collectContributionConflicts(enabledPlugins, "tools"),
      ...collectContributionConflicts(enabledPlugins, "commands"),
      ...collectContributionConflicts(enabledPlugins, "hooks"),
    ],
  }
}

function assertUniquePluginNames(registry: PluginRegistry): void {
  const seen = new Set<string>()

  for (const plugin of registry) {
    if (seen.has(plugin.name)) {
      throw new Error(`Duplicate plugin name: ${plugin.name}`)
    }

    seen.add(plugin.name)
  }
}

function collectContributionNames(
  plugins: readonly Plugin[],
  kind: ContributionKind,
): readonly string[] {
  const names: string[] = []
  const seen = new Set<string>()

  for (const plugin of plugins) {
    for (const name of plugin.contributes[kind] ?? []) {
      if (seen.has(name)) {
        continue
      }

      seen.add(name)
      names.push(name)
    }
  }

  return names
}

function collectContributionConflicts(
  plugins: readonly Plugin[],
  kind: ContributionKind,
): readonly ContributionConflict[] {
  const pluginNamesByContribution = new Map<string, Set<string>>()

  for (const plugin of plugins) {
    for (const name of plugin.contributes[kind] ?? []) {
      const pluginNames = pluginNamesByContribution.get(name) ?? new Set()
      pluginNames.add(plugin.name)
      pluginNamesByContribution.set(name, pluginNames)
    }
  }

  return Array.from(pluginNamesByContribution.entries())
    .filter(([, pluginNames]) => pluginNames.size > 1)
    .map(([name, pluginNames]) => ({
      kind,
      name,
      pluginNames: Array.from(pluginNames),
    }))
}
