export const desktopPreviewChangeKinds = [
  "css_hmr",
  "react_fast_refresh",
  "renderer_reload_required",
  "host_restart_required",
  "dependency_sync_required",
] as const

export type DesktopPreviewChangeKind = (typeof desktopPreviewChangeKinds)[number]

const dependencyFiles = new Set([
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "catalogs.json",
  "apps/openagents-desktop/package.json",
])

const hostPrefixes = [
  "apps/openagents-desktop/scripts/",
  "apps/openagents-desktop/src/main",
  "apps/openagents-desktop/src/preload",
  "apps/openagents-desktop/src/workers/",
  "apps/openagents-desktop/native/",
  "apps/openagents-desktop/forge.config",
  "apps/openagents-desktop/vite.config",
  "packages/",
] as const

const rendererPrefix = "apps/openagents-desktop/src/renderer/"

/**
 * Conservative preview disposition. Only CSS and component-only TSX/JSX
 * modules may flow through Vite automatically. Everything else either waits
 * for an explicit renderer reload or a fresh host/dependency launch.
 */
export const classifyDesktopPreviewChange = (repositoryRelativePath: string): DesktopPreviewChangeKind => {
  const normalized = repositoryRelativePath.replaceAll("\\", "/").replace(/^\.\//, "")
  if (dependencyFiles.has(normalized)) return "dependency_sync_required"
  if (hostPrefixes.some(prefix => normalized.startsWith(prefix))) return "host_restart_required"
  if (normalized.startsWith(rendererPrefix)) {
    if (normalized.endsWith(".css")) return "css_hmr"
    if (normalized.endsWith(".tsx") || normalized.endsWith(".jsx")) return "react_fast_refresh"
    return "renderer_reload_required"
  }
  if (normalized === "apps/openagents-desktop/index.dev.html") return "renderer_reload_required"
  return "host_restart_required"
}

export type DesktopPreviewChangeEvent = Readonly<{
  kind: DesktopPreviewChangeKind
  pathRef: string
}>

export const decodeDesktopPreviewChangeEvent = (value: unknown): DesktopPreviewChangeEvent | null => {
  if (typeof value !== "object" || value === null) return null
  const input = value as { kind?: unknown; pathRef?: unknown }
  if (!desktopPreviewChangeKinds.includes(input.kind as DesktopPreviewChangeKind)) return null
  if (typeof input.pathRef !== "string" || input.pathRef.length === 0 || input.pathRef.length > 240) return null
  if (/^(?:\/|[A-Za-z]:[\\/])/.test(input.pathRef) || input.pathRef.split("/").includes("..")) return null
  return { kind: input.kind as DesktopPreviewChangeKind, pathRef: input.pathRef }
}
