const SOURCE_ANCHORS = new Set([
  "apps",
  "clients",
  "docs",
  "packages",
  "scripts",
  "src",
  "tests",
  "workers",
])

const WORKSPACE_MARKERS = new Set([
  "repo",
  "repos",
  "work",
  "workspace",
  "workspaces",
])

const LOCAL_PATH_PATTERN =
  /\/(?:Users|private\/tmp|tmp|var|Volumes|workspace|workspaces|repo|repos|mnt|home)\/[^\s"'`<>),\]}]+/gu

const normalizeSlashes = (value: string): string =>
  value.replace(/\\/g, "/").replace(/\/{2,}/g, "/")

const trimPathValue = (value: string): string =>
  normalizeSlashes(value.trim()).replace(/^file:\/\//u, "")

const stripTrailingPunctuation = (
  value: string,
): Readonly<{ path: string; trailing: string }> => {
  const match = /[.;:,]+$/u.exec(value)
  if (match === null) return { path: value, trailing: "" }
  return {
    path: value.slice(0, -match[0].length),
    trailing: match[0],
  }
}

const isAbsoluteDisplayPath = (value: string): boolean =>
  value.startsWith("/") || /^[A-Za-z]:\//u.test(value)

const cleanRelative = (value: string): string => {
  const cleaned = normalizeSlashes(value)
    .replace(/^\.\/+/u, "")
    .replace(/\/$/u, "")
  return cleaned.length === 0 ? "." : cleaned
}

const relativeToRoot = (path: string, root: string | undefined): string | null => {
  if (root === undefined || root.trim().length === 0) return null
  const normalizedRoot = trimPathValue(root).replace(/\/$/u, "")
  if (!isAbsoluteDisplayPath(normalizedRoot)) return null
  if (path === normalizedRoot) return "."
  if (!path.startsWith(`${normalizedRoot}/`)) return null
  return cleanRelative(path.slice(normalizedRoot.length + 1))
}

const anchoredRelativePath = (path: string): string | null => {
  const parts = path.split("/").filter(Boolean)
  const anchorIndex = parts.findIndex(part => SOURCE_ANCHORS.has(part))
  if (anchorIndex >= 0) return cleanRelative(parts.slice(anchorIndex).join("/"))

  const markerIndex = parts.findIndex(part => WORKSPACE_MARKERS.has(part))
  if (markerIndex >= 0 && markerIndex < parts.length - 1) {
    const afterMarker = parts.slice(markerIndex + 1)
    if (afterMarker.length > 2 && /(?:wt|worktree|openagents|khala)/iu.test(afterMarker[0] ?? "")) {
      return cleanRelative(afterMarker.slice(1).join("/"))
    }
    return cleanRelative(afterMarker.join("/"))
  }

  return null
}

export const displayPathForKhalaCode = (
  value: string,
  root?: string,
): string => {
  const { path: rawPath, trailing } = stripTrailingPunctuation(trimPathValue(value))
  if (!isAbsoluteDisplayPath(rawPath)) return `${cleanRelative(rawPath)}${trailing}`

  const rootRelative = relativeToRoot(rawPath, root)
  if (rootRelative !== null) return `${rootRelative}${trailing}`

  const anchored = anchoredRelativePath(rawPath)
  if (anchored !== null) return `${anchored}${trailing}`

  const basename = rawPath.split("/").filter(Boolean).at(-1)
  return `${basename ?? "."}${trailing}`
}

export const displayLocalPathsForKhalaCode = (
  value: string,
  root?: string,
): string =>
  value.replace(LOCAL_PATH_PATTERN, match => displayPathForKhalaCode(match, root))
