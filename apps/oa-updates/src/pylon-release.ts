import { createHash } from "node:crypto"

import type { AssetStore } from "./asset-store.ts"

// Pylon OTA feed. Unlike the desktop (Electrobun) feed, Pylon releases are
// per-platform `bun --compile` binaries and each release carries the ed25519
// release signature + sha256 + kid so the self-updater (#5042) verifies against
// the PINNED public key and fails closed — host/TLS is never the trust boundary.
// Channel is `rc` until owner GA; rollout/yank/minVersion gate staged delivery.

export type PylonPlatform =
  | "darwin-arm64"
  | "darwin-x64"
  | "linux-x64"
  | "linux-arm64"

export const PYLON_PLATFORMS: readonly PylonPlatform[] = [
  "darwin-arm64",
  "darwin-x64",
  "linux-x64",
  "linux-arm64",
]

export type PylonReleaseManifest = {
  readonly version: string
  readonly channel: string
  readonly platform: PylonPlatform
  readonly artifactUrl: string
  readonly sha256: string
  // Detached ed25519 signature over the artifact bytes (base64url), produced by
  // sign-release.ts; clients verify against the pinned release-pubkey.json.
  readonly signature: string
  readonly kid: string
  readonly createdAt?: string
  // Staged rollout: 0..100. A client deterministically in-bucket installs it;
  // others wait. Absent/100 means full rollout.
  readonly rolloutPercent?: number
  // Hard floor: clients older than this MUST update (skips rollout gating).
  readonly minVersion?: string
  // Pulled release — clients must never install it (kept for audit/visibility).
  readonly yanked?: boolean
}

export type PylonFeed = {
  readonly schema: "openagents.pylon.feed.v1"
  readonly product: "pylon"
  readonly channel: string
  readonly platform: PylonPlatform
  readonly releases: ReadonlyArray<PylonReleaseManifest>
}

export type BuildPylonReleaseInput = {
  readonly version: string
  readonly channel: string
  readonly platform: PylonPlatform
  readonly artifactBytes: Uint8Array
  readonly signature: string
  readonly kid: string
  readonly baseUrl: string
  readonly store: AssetStore
  readonly createdAt?: string
  readonly rolloutPercent?: number
  readonly minVersion?: string
}

export type BuildPylonReleaseResult = {
  readonly manifest: PylonReleaseManifest
  readonly artifactHash: string
}

export function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex")
}

export async function buildPylonReleaseManifest(
  input: BuildPylonReleaseInput,
): Promise<BuildPylonReleaseResult> {
  assertNonEmpty(input.version, "version")
  assertNonEmpty(input.channel, "channel")
  assertPlatform(input.platform)
  assertNonEmpty(input.signature, "signature")
  assertNonEmpty(input.kid, "kid")
  if (input.rolloutPercent !== undefined) assertRollout(input.rolloutPercent)

  const artifact = await input.store.put(input.artifactBytes)
  const manifest: PylonReleaseManifest = {
    version: input.version.trim(),
    channel: input.channel.trim(),
    platform: input.platform,
    artifactUrl: assetUrl(input.baseUrl, artifact.hash),
    sha256: sha256Hex(input.artifactBytes),
    signature: input.signature.trim(),
    kid: input.kid.trim(),
    ...(input.createdAt ? { createdAt: input.createdAt.trim() } : {}),
    ...(input.rolloutPercent !== undefined
      ? { rolloutPercent: input.rolloutPercent }
      : {}),
    ...(input.minVersion ? { minVersion: input.minVersion.trim() } : {}),
  }
  return { manifest, artifactHash: artifact.hash }
}

// Latest non-yanked release first. Yanked releases are dropped from the served
// feed so a client never even sees them as install candidates.
export function buildPylonFeed(
  channel: string,
  platform: PylonPlatform,
  releases: ReadonlyArray<PylonReleaseManifest>,
): PylonFeed {
  const live = releases
    .filter((release) => release.yanked !== true)
    .filter((release) => release.channel === channel && release.platform === platform)
  return {
    schema: "openagents.pylon.feed.v1",
    product: "pylon",
    channel,
    platform,
    releases: sortPylonReleases(live),
  }
}

export function sortPylonReleases(
  releases: ReadonlyArray<PylonReleaseManifest>,
): PylonReleaseManifest[] {
  return [...releases].sort((left, right) =>
    compareVersions(right.version, left.version),
  )
}

// Deterministic per-client rollout bucket in [0,100). A client whose bucket is
// below rolloutPercent is eligible. Uses a stable hash of (clientId+version) so
// a given client's decision never flaps between polls.
export function rolloutBucket(clientId: string, version: string): number {
  const digest = createHash("sha256").update(`${clientId}:${version}`).digest()
  // First 4 bytes -> uint32 -> [0,100)
  const n = digest.readUInt32BE(0)
  return n % 100
}

// The decision the self-updater makes: given a feed, the running version, and a
// stable client id, return the release to install (or null). Honors yank (already
// filtered), minVersion (hard floor, skips rollout), and staged rollout.
export function selectPylonUpdate(
  feed: PylonFeed,
  currentVersion: string,
  clientId: string,
): PylonReleaseManifest | null {
  for (const release of feed.releases) {
    if (compareVersions(release.version, currentVersion) <= 0) continue
    // Hard floor: anyone below minVersion updates regardless of rollout.
    if (
      release.minVersion !== undefined &&
      compareVersions(currentVersion, release.minVersion) < 0
    ) {
      return release
    }
    const rollout = release.rolloutPercent ?? 100
    if (rollout >= 100) return release
    if (rolloutBucket(clientId, release.version) < rollout) return release
  }
  return null
}

export function normalizePylonPlatform(value: string): PylonPlatform {
  const trimmed = value.trim()
  if (!PYLON_PLATFORMS.includes(trimmed as PylonPlatform)) {
    throw new Error(`Unknown Pylon platform: ${value}`)
  }
  return trimmed as PylonPlatform
}

function assetUrl(baseUrl: string, hash: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/assets/${hash}`
}

function assertNonEmpty(value: string | undefined, field: string): void {
  if (value === undefined || value.trim().length === 0) {
    throw new Error(`Pylon release ${field} is required`)
  }
}

function assertPlatform(value: string): void {
  if (!PYLON_PLATFORMS.includes(value as PylonPlatform)) {
    throw new Error(`Pylon release platform must be one of ${PYLON_PLATFORMS.join(", ")}`)
  }
}

function assertRollout(value: number): void {
  if (!Number.isFinite(value) || value < 0 || value > 100) {
    throw new Error("Pylon rolloutPercent must be between 0 and 100")
  }
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
  // Equal numeric core: a release WITHOUT a prerelease tag outranks one WITH
  // (1.0.0 > 1.0.0-rc.1), matching semver ordering for the rc->stable bump.
  return prereleaseRank(left) - prereleaseRank(right)
}

// Numeric core only (1.0.0-rc.1 -> [1,0,0]); the prerelease tail is ranked
// separately so rc ordering is correct.
function versionParts(version: string): number[] {
  const core = version.split("-")[0] ?? version
  return core.split(".").map((part) => {
    const value = Number.parseInt(part, 10)
    return Number.isFinite(value) ? value : 0
  })
}

// A release with no prerelease tag ranks above one with a tag; among tagged
// releases, compare the trailing integer (rc.1 < rc.2).
function prereleaseRank(version: string): number {
  const tail = version.split("-").slice(1).join("-")
  if (tail.length === 0) return Number.MAX_SAFE_INTEGER
  const match = tail.match(/(\d+)\s*$/)
  return match ? Number.parseInt(match[1], 10) : 0
}
