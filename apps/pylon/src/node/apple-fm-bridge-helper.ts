import { existsSync } from "node:fs"
import { dirname, join, resolve } from "node:path"

export const APPLE_FM_BRIDGE_DEFAULT_PORT = 11435 as const
export const APPLE_FM_BRIDGE_PATH_ENV = "OPENAGENTS_APPLE_FM_BRIDGE_PATH" as const

export type AppleFmBridgeHelperSource =
  | "env"
  | "source-wrapper"
  | "source-build"
  | "packaged-resource"

export type DiscoveredAppleFmBridgeHelper = {
  readonly path: string
  readonly source: AppleFmBridgeHelperSource
}

export type DiscoverAppleFmBridgeHelperOptions = {
  readonly cwd?: string
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly resourcesDir?: string
  readonly fileExists?: (path: string) => boolean
}

export function discoverAppleFmBridgeHelper(
  options: DiscoverAppleFmBridgeHelperOptions = {},
): DiscoveredAppleFmBridgeHelper | null {
  const env = options.env ?? process.env
  const fileExists = options.fileExists ?? existsSync
  const explicitPath = env[APPLE_FM_BRIDGE_PATH_ENV]

  if (explicitPath !== undefined && explicitPath.trim().length > 0) {
    const path = resolve(explicitPath)
    if (fileExists(path)) {
      return { path, source: "env" }
    }
  }

  for (const pylonRoot of candidatePylonRoots(resolve(options.cwd ?? process.cwd()))) {
    const wrapper = join(pylonRoot, "bin", "foundation-bridge")
    if (fileExists(wrapper)) {
      return { path: wrapper, source: "source-wrapper" }
    }

    const sourceBuild = join(pylonRoot, "swift", "foundation-bridge", ".build", "release", "foundation-bridge")
    if (fileExists(sourceBuild)) {
      return { path: sourceBuild, source: "source-build" }
    }
  }

  const resourcesDir = options.resourcesDir ?? defaultResourcesDir()
  if (resourcesDir !== undefined) {
    const packaged = join(resourcesDir, "app", "apple-fm-bridge", "foundation-bridge")
    if (fileExists(packaged)) {
      return { path: packaged, source: "packaged-resource" }
    }
  }

  return null
}

function candidatePylonRoots(cwd: string): ReadonlyArray<string> {
  const roots: string[] = []
  const seen = new Set<string>()

  for (const ancestor of ancestors(cwd)) {
    for (const candidate of [ancestor, join(ancestor, "apps", "pylon")]) {
      if (!seen.has(candidate)) {
        roots.push(candidate)
        seen.add(candidate)
      }
    }
  }

  return roots
}

function ancestors(start: string): ReadonlyArray<string> {
  const values: string[] = []
  let current = start

  while (true) {
    values.push(current)
    const next = dirname(current)
    if (next === current) {
      return values
    }
    current = next
  }
}

function defaultResourcesDir(): string | undefined {
  const processWithResources = process as NodeJS.Process & { readonly resourcesPath?: string }
  return processWithResources.resourcesPath
}
