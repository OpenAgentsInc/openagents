const safeSegment = /^[A-Za-z0-9][A-Za-z0-9._-]*$/u

export type DesktopReleaseArtifactNameInput = Readonly<{
  product: string
  version: string
  platform: string
  arch: string
  extension: string
}>

/**
 * Canonical public filename for a generated Desktop release artifact.
 * OS-specific installers do not repeat the platform; neutral archives do.
 */
export const desktopReleaseArtifactName = (
  input: DesktopReleaseArtifactNameInput,
): string => {
  const extension = input.extension.startsWith(".")
    ? input.extension.toLowerCase()
    : `.${input.extension.toLowerCase()}`
  for (const [label, value] of Object.entries({
    product: input.product,
    version: input.version,
    platform: input.platform,
    arch: input.arch,
  })) {
    if (!safeSegment.test(value)) throw new Error(`Invalid release artifact ${label}`)
  }
  if (!/^\.[a-z0-9]+$/u.test(extension)) throw new Error("Invalid release artifact extension")
  const platform = extension === ".dmg" ? "" : `-${input.platform}`
  return `${input.product}-${input.version}${platform}-${input.arch}${extension}`
}
