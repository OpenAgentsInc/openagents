export type KhalaCodeDesktopReleaseChannel = "stable" | "rc"

export type KhalaCodeDesktopReleasePlanInput = {
  readonly version: string
  readonly channel: KhalaCodeDesktopReleaseChannel
  readonly artifactFileName: string
}

export type KhalaCodeDesktopReleasePlan = {
  readonly product: typeof KHALA_CODE_DESKTOP_RELEASE_PRODUCT
  readonly version: string
  readonly channel: KhalaCodeDesktopReleaseChannel
  readonly artifactFileName: string
  readonly githubTag: string
  readonly githubPrerelease: boolean
  readonly latestEligible: boolean
  readonly updateFeedUrl: string
  readonly updateFeedBucketPrefix: string
  readonly needsOwnerRef: string
}

export type KhalaCodeDesktopReleaseReceiptSet = {
  readonly signedAppReceiptRef?: string
  readonly notarizedAppReceiptRef?: string
  readonly stapledAppReceiptRef?: string
  readonly recreatedDmgReceiptRef?: string
  readonly signedDmgReceiptRef?: string
  readonly notarizedDmgReceiptRef?: string
  readonly stapledDmgReceiptRef?: string
  readonly feedUploadReceiptRef?: string
  readonly githubReleaseReceiptRef?: string
  readonly cleanMacFirstRunSmokeReceiptRef?: string
}

export type KhalaCodeDesktopReleaseReceiptValidation = {
  readonly ok: boolean
  readonly missing: readonly (keyof KhalaCodeDesktopReleaseReceiptSet)[]
}

export const KHALA_CODE_DESKTOP_RELEASE_PRODUCT = "khala-code-desktop"
export const KHALA_CODE_DESKTOP_GITHUB_TAG_PREFIX =
  "khala-code-desktop-v"
export const KHALA_CODE_DESKTOP_UPDATE_FEED_BASE_URL =
  "https://updates.openagents.com/desktop/khala-code-desktop"
export const KHALA_CODE_DESKTOP_UPDATE_BUCKET_PREFIX =
  "gs://openagentsgemini-oa-updates/desktop/khala-code-desktop"
export const KHALA_CODE_DESKTOP_NEEDS_OWNER_REF =
  "NEEDS_OWNER.md#khala-code-desktop-signed-release-gate"

export const REQUIRED_KHALA_CODE_RELEASE_RECEIPTS = [
  "signedAppReceiptRef",
  "notarizedAppReceiptRef",
  "stapledAppReceiptRef",
  "recreatedDmgReceiptRef",
  "signedDmgReceiptRef",
  "notarizedDmgReceiptRef",
  "stapledDmgReceiptRef",
  "feedUploadReceiptRef",
  "githubReleaseReceiptRef",
  "cleanMacFirstRunSmokeReceiptRef",
] as const satisfies readonly (keyof KhalaCodeDesktopReleaseReceiptSet)[]

const SEMVER_WITH_OPTIONAL_PRERELEASE =
  /^\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/

export function buildKhalaCodeDesktopReleasePlan(
  input: KhalaCodeDesktopReleasePlanInput,
): KhalaCodeDesktopReleasePlan {
  const version = input.version.trim()
  const artifactFileName = input.artifactFileName.trim()

  if (!SEMVER_WITH_OPTIONAL_PRERELEASE.test(version)) {
    throw new Error("Khala Code Desktop release version must be semver")
  }

  if (!artifactFileName.endsWith(".dmg")) {
    throw new Error("Khala Code Desktop release artifact must be a DMG")
  }

  const prerelease = isPrereleaseVersion(version)
  if (input.channel === "stable" && prerelease) {
    throw new Error("Khala Code Desktop RC builds must not publish to stable")
  }

  if (input.channel === "rc" && !prerelease) {
    throw new Error("Khala Code Desktop rc channel requires a prerelease version")
  }

  return {
    product: KHALA_CODE_DESKTOP_RELEASE_PRODUCT,
    version,
    channel: input.channel,
    artifactFileName,
    githubTag: `${KHALA_CODE_DESKTOP_GITHUB_TAG_PREFIX}${version}`,
    githubPrerelease: prerelease,
    latestEligible: !prerelease && input.channel === "stable",
    updateFeedUrl: `${KHALA_CODE_DESKTOP_UPDATE_FEED_BASE_URL}/${input.channel}/feed.json`,
    updateFeedBucketPrefix: `${KHALA_CODE_DESKTOP_UPDATE_BUCKET_PREFIX}/${input.channel}/`,
    needsOwnerRef: KHALA_CODE_DESKTOP_NEEDS_OWNER_REF,
  }
}

export function validateKhalaCodeDesktopReleaseReceipts(
  receipts: KhalaCodeDesktopReleaseReceiptSet,
): KhalaCodeDesktopReleaseReceiptValidation {
  const missing = REQUIRED_KHALA_CODE_RELEASE_RECEIPTS.filter((key) => {
    const value = receipts[key]
    return value === undefined || value.trim().length === 0
  })

  return {
    ok: missing.length === 0,
    missing,
  }
}

function isPrereleaseVersion(version: string): boolean {
  return version.includes("-")
}
