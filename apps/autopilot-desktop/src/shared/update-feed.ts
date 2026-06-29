export type UpdateManifest = {
  readonly version: string
  readonly artifactUrl: string
  readonly sha256: string
  readonly createdAt?: string
  readonly bsdiffFromVersion?: string
  readonly bsdiffUrl?: string
  readonly bsdiffSha256?: string
}

export type UpdateChoice =
  | { readonly action: "none"; readonly manifest?: undefined }
  | { readonly action: "full" | "bsdiff"; readonly manifest: UpdateManifest }

export function chooseUpdate(current: string, feed: UpdateManifest[]): UpdateChoice {
  const newest = feed
    .filter((manifest) => compareVersions(manifest.version, current) > 0)
    .sort((left, right) => compareVersions(right.version, left.version))[0]

  if (!newest) return { action: "none" }

  if (newest.bsdiffFromVersion === current && newest.bsdiffUrl) {
    return { action: "bsdiff", manifest: newest }
  }

  return { action: "full", manifest: newest }
}

function compareVersions(left: string, right: string): number {
  const leftParts = versionParts(left)
  const rightParts = versionParts(right)
  const partCount = Math.max(leftParts.length, rightParts.length)

  for (let index = 0; index < partCount; index += 1) {
    const leftPart = leftParts[index] ?? 0
    const rightPart = rightParts[index] ?? 0
    if (leftPart !== rightPart) return leftPart - rightPart
  }

  return 0
}

function versionParts(version: string): number[] {
  return version.split(".").map((part) => {
    const value = Number.parseInt(part, 10)
    return Number.isFinite(value) ? value : 0
  })
}
