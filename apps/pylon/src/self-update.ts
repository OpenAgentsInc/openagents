// Pylon default-on OTA self-updater.
//
// Fetches the signed feed from our GCP (updates.openagents.com), picks the newest
// eligible release, downloads it, verifies sha256 + the ed25519 signature against
// the PINNED OpenAgents release key (fail closed — host/TLS is never the trust
// boundary), then atomically replaces the running binary and relaunches. The
// compiled binary self-replaces; a `bun src/index.ts` dev run is a no-op target.
//
// Feed contract: apps/oa-updates/src/pylon-release.ts (openagents.pylon.feed.v1).
import { createHash, createPublicKey, verify as edVerify } from "node:crypto"
import { chmod, rename, writeFile, rm } from "node:fs/promises"
import { dirname, join } from "node:path"

import { PYLON_VERSION } from "./version.js"

// Pinned OpenAgents release public key (ed25519). Verifying against this and
// failing closed is the whole point — rotating the key requires shipping a new
// client (mirrors apps/oa-updates/keys/release-pubkey.json, kid below).
export const PINNED_RELEASE_KEY = {
  kid: "2dbe811d19f67528",
  x: "P9steasTKRx6gr9QQlbah4kXm17aAh2wLHLAL-Txwak",
} as const

export const PYLON_UPDATE_FEED_BASE = "https://updates.openagents.com"
export const PYLON_UPDATE_CHANNEL = "rc"

export type PylonPlatform =
  | "darwin-arm64"
  | "darwin-x64"
  | "linux-x64"
  | "linux-arm64"

export type FeedRelease = {
  readonly version: string
  readonly channel: string
  readonly platform: PylonPlatform
  readonly artifactUrl: string
  readonly sha256: string
  readonly signature: string
  readonly kid: string
  readonly rolloutPercent?: number
  readonly minVersion?: string
  readonly yanked?: boolean
}

export type PylonFeed = {
  readonly schema: string
  readonly product: string
  readonly channel: string
  readonly platform: PylonPlatform
  readonly releases: ReadonlyArray<FeedRelease>
}

// The path to overwrite on self-update: the compiled standalone binary. Returns
// null for a dev run (`bun src/index.ts`) or when running under bun/node, so we
// never clobber the interpreter — auto-update is a no-op there.
export function resolveSelfBinaryPath(
  execPath: string = process.execPath,
  argv: ReadonlyArray<string> = process.argv,
): string | null {
  const base = (execPath.split("/").pop() ?? "").toLowerCase()
  if (base === "bun" || base === "node" || base === "bun.exe" || base === "node.exe") {
    return null
  }
  if ((argv[1] ?? "").endsWith(".ts") || (argv[1] ?? "").endsWith(".js")) {
    return null
  }
  return execPath
}

export function currentPlatform(
  os: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): PylonPlatform | null {
  if (os === "darwin" && arch === "arm64") return "darwin-arm64"
  if (os === "darwin" && arch === "x64") return "darwin-x64"
  if (os === "linux" && arch === "x64") return "linux-x64"
  if (os === "linux" && arch === "arm64") return "linux-arm64"
  return null
}

// Stable per-install bucket in [0,100) for staged rollout — matches the server's
// rolloutBucket so a client's decision agrees with intent and never flaps.
export function rolloutBucket(clientId: string, version: string): number {
  const digest = createHash("sha256").update(`${clientId}:${version}`).digest()
  return digest.readUInt32BE(0) % 100
}

export function compareVersions(left: string, right: string): number {
  const core = (v: string) =>
    (v.split("-")[0] ?? v).split(".").map((p) => {
      const n = Number.parseInt(p, 10)
      return Number.isFinite(n) ? n : 0
    })
  const pre = (v: string) => {
    const tail = v.split("-").slice(1).join("-")
    if (tail.length === 0) return Number.MAX_SAFE_INTEGER
    const m = tail.match(/(\d+)\s*$/)
    return m ? Number.parseInt(m[1], 10) : 0
  }
  const l = core(left)
  const r = core(right)
  for (let i = 0; i < Math.max(l.length, r.length); i += 1) {
    const diff = (l[i] ?? 0) - (r[i] ?? 0)
    if (diff !== 0) return diff
  }
  return pre(left) - pre(right)
}

// The install decision: newest eligible release strictly newer than current,
// honoring minVersion (hard floor, skips rollout) and staged rollout.
export function selectUpdate(
  feed: PylonFeed,
  currentVersion: string,
  clientId: string,
): FeedRelease | null {
  const sorted = [...feed.releases]
    .filter((r) => r.yanked !== true)
    .sort((a, b) => compareVersions(b.version, a.version))
  for (const release of sorted) {
    if (compareVersions(release.version, currentVersion) <= 0) continue
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

// Fail-closed verification of downloaded bytes against a feed release. Throws on
// ANY mismatch — wrong kid, bad sha256, or invalid ed25519 signature.
export function verifyArtifact(bytes: Uint8Array, release: FeedRelease): void {
  if (release.kid !== PINNED_RELEASE_KEY.kid) {
    throw new Error(
      `release kid ${release.kid} is not the pinned key ${PINNED_RELEASE_KEY.kid}`,
    )
  }
  const sha = createHash("sha256").update(bytes).digest("hex")
  if (sha !== release.sha256) {
    throw new Error(`sha256 mismatch (feed ${release.sha256}, bytes ${sha})`)
  }
  const pub = createPublicKey({
    key: { kty: "OKP", crv: "Ed25519", x: PINNED_RELEASE_KEY.x },
    format: "jwk",
  })
  const ok = edVerify(
    null,
    bytes,
    pub,
    Buffer.from(release.signature, "base64url"),
  )
  if (!ok) {
    throw new Error("ed25519 signature does not verify against the pinned key")
  }
}

export type CheckResult =
  | { readonly status: "up-to-date"; readonly currentVersion: string }
  | { readonly status: "disabled"; readonly reason: string }
  | { readonly status: "unsupported"; readonly reason: string }
  | {
      readonly status: "update-available"
      readonly currentVersion: string
      readonly release: FeedRelease
    }

export type UpdateDeps = {
  readonly currentVersion?: string
  readonly clientId: string
  readonly platform?: PylonPlatform | null
  readonly feedBase?: string
  readonly channel?: string
  readonly fetchFn?: typeof fetch
  readonly env?: NodeJS.ProcessEnv
}

export function feedUrl(
  feedBase: string,
  channel: string,
  platform: PylonPlatform,
): string {
  return `${feedBase.replace(/\/+$/, "")}/pylon/${channel}/${platform}/feed.json`
}

// Default-on: auto-update unless the operator explicitly opts out.
export function autoUpdateDisabledReason(
  env: NodeJS.ProcessEnv = process.env,
): string | null {
  const flag = env.PYLON_DISABLE_AUTOUPDATE ?? env.PYLON_AUTOUPDATE
  if (env.PYLON_DISABLE_AUTOUPDATE && env.PYLON_DISABLE_AUTOUPDATE !== "0" && env.PYLON_DISABLE_AUTOUPDATE !== "false") {
    return "PYLON_DISABLE_AUTOUPDATE is set"
  }
  if (env.PYLON_AUTOUPDATE === "0" || env.PYLON_AUTOUPDATE === "false") {
    return "PYLON_AUTOUPDATE is disabled"
  }
  void flag
  return null
}

export async function checkForUpdate(deps: UpdateDeps): Promise<CheckResult> {
  const env = deps.env ?? process.env
  const currentVersion = deps.currentVersion ?? PYLON_VERSION
  const platform = deps.platform === undefined ? currentPlatform() : deps.platform
  if (platform === null) {
    return { status: "unsupported", reason: `${process.platform}/${process.arch}` }
  }
  const fetchFn = deps.fetchFn ?? fetch
  const url = feedUrl(
    deps.feedBase ?? PYLON_UPDATE_FEED_BASE,
    deps.channel ?? PYLON_UPDATE_CHANNEL,
    platform,
  )
  const response = await fetchFn(url, { headers: { accept: "application/json" } })
  if (!response.ok) {
    throw new Error(`update feed ${url} returned ${response.status}`)
  }
  const feed = (await response.json()) as PylonFeed
  const release = selectUpdate(feed, currentVersion, deps.clientId)
  if (release === null) return { status: "up-to-date", currentVersion }
  return { status: "update-available", currentVersion, release }
}

export type ApplyDeps = {
  readonly release: FeedRelease
  readonly targetPath: string
  readonly fetchFn?: typeof fetch
  // injected for tests
  readonly writeFileFn?: typeof writeFile
  readonly renameFn?: typeof rename
  readonly chmodFn?: typeof chmod
  readonly rmFn?: typeof rm
}

export type ApplyResult = {
  readonly version: string
  readonly targetPath: string
  readonly backupPath: string
}

// Download → verify (fail closed) → atomic replace of the running binary.
// On unix, rename() onto the live executable is atomic and safe (the running
// process keeps the old inode); we keep a .old backup until the next launch.
export async function downloadAndApply(deps: ApplyDeps): Promise<ApplyResult> {
  const fetchFn = deps.fetchFn ?? fetch
  const writeFileFn = deps.writeFileFn ?? writeFile
  const renameFn = deps.renameFn ?? rename
  const chmodFn = deps.chmodFn ?? chmod
  const rmFn = deps.rmFn ?? rm

  const response = await fetchFn(deps.release.artifactUrl)
  if (!response.ok) {
    throw new Error(
      `artifact ${deps.release.artifactUrl} returned ${response.status}`,
    )
  }
  const bytes = new Uint8Array(await response.arrayBuffer())
  // Verify BEFORE anything touches disk near the live binary.
  verifyArtifact(bytes, deps.release)

  const dir = dirname(deps.targetPath)
  const stamp = `${deps.release.version}.${process.pid}`
  const stagedPath = join(dir, `.pylon-update-${stamp}`)
  const backupPath = `${deps.targetPath}.old-${stamp}`

  await writeFileFn(stagedPath, bytes)
  await chmodFn(stagedPath, 0o755)
  // Keep a backup, then atomically swap the new binary into place.
  await renameFn(deps.targetPath, backupPath)
  try {
    await renameFn(stagedPath, deps.targetPath)
  } catch (error) {
    // Roll back if the final swap failed so we never leave a missing binary.
    await renameFn(backupPath, deps.targetPath).catch(() => {})
    await rmFn(stagedPath, { force: true }).catch(() => {})
    throw error
  }
  return {
    version: deps.release.version,
    targetPath: deps.targetPath,
    backupPath,
  }
}
