import { readFile as nodeReadFile } from "node:fs/promises"
import { join } from "node:path"

import { assetKeyFromBytes } from "./asset-store.ts"
import {
  buildPylonReleaseManifest,
  normalizePylonPlatform,
  type PylonPlatform,
  type PylonReleaseManifest,
} from "./pylon-release.ts"
import type { UpdatesServer } from "./server.ts"

const assetUrl = (baseUrl: string, hash: string): string =>
  `${baseUrl.replace(/\/+$/, "")}/assets/${hash}`

// A Pylon release as recorded on disk by publish-pylon-release.ts. The signature
// is precomputed by sign-release.ts (the build pipeline), not minted here.
export type PylonReleaseSeed = {
  readonly channel: string
  readonly version: string
  readonly platform: PylonPlatform
  readonly artifactPath: string
  readonly signature: string
  readonly kid: string
  readonly createdAt?: string
  readonly rolloutPercent?: number
  readonly minVersion?: string
}

type SeedPylonReleasesInput = {
  readonly server: UpdatesServer
  readonly distDir: string
  readonly baseUrl: string
  readonly readFile?: (path: string) => Promise<Uint8Array>
}

export type SeedPylonReleasesResult = {
  readonly releases: ReadonlyArray<PylonReleaseManifest>
}

const defaultReadFile = async (path: string): Promise<Uint8Array> =>
  new Uint8Array(await nodeReadFile(path))

export async function seedPylonReleases(
  input: SeedPylonReleasesInput,
): Promise<SeedPylonReleasesResult> {
  const readFile = input.readFile ?? defaultReadFile
  const raw = JSON.parse(
    new TextDecoder().decode(await readFile(join(input.distDir, "pylon-releases.json"))),
  ) as unknown
  const seeds = normalizePylonReleaseList(raw)
  const manifests: PylonReleaseManifest[] = []

  for (const seed of seeds) {
    const artifactPath = join(input.distDir, seed.artifactPath)
    const artifactBytes = await readFile(artifactPath)
    const result = await buildPylonReleaseManifest({
      version: seed.version,
      channel: seed.channel,
      platform: seed.platform,
      artifactBytes,
      signature: seed.signature,
      kid: seed.kid,
      baseUrl: input.baseUrl,
      // Non-retaining store: compute the content hash but DO NOT hold the bytes
      // in memory. The binary is served by streaming from disk (registerDiskAsset),
      // so seeding 100s of MB never blows the boot memory/timeout.
      store: {
        put: async (bytes) => ({ hash: assetKeyFromBytes(bytes), url: assetUrl(input.baseUrl, assetKeyFromBytes(bytes)) }),
        get: async () => null,
      },
      createdAt: seed.createdAt,
      rolloutPercent: seed.rolloutPercent,
      minVersion: seed.minVersion,
    })
    input.server.registerDiskAsset(result.artifactHash, artifactPath, "application/octet-stream")
    input.server.registerPylonUpdate(result.manifest)
    manifests.push(result.manifest)
  }

  return { releases: manifests }
}

export function normalizePylonReleaseList(value: unknown): PylonReleaseSeed[] {
  const rows = Array.isArray(value)
    ? value
    : isRecord(value) && Array.isArray(value.releases)
      ? value.releases
      : []
  return rows.map(normalizePylonReleaseSeed)
}

export function normalizePylonReleaseSeed(value: unknown): PylonReleaseSeed {
  if (!isRecord(value)) throw new Error("Pylon release seed must be an object")
  const seed: PylonReleaseSeed = {
    channel: requiredString(value, "channel"),
    version: requiredString(value, "version"),
    platform: normalizePylonPlatform(requiredString(value, "platform")),
    artifactPath: requiredString(value, "artifactPath"),
    signature: requiredString(value, "signature"),
    kid: requiredString(value, "kid"),
    ...(optionalString(value, "createdAt") ? { createdAt: optionalString(value, "createdAt") } : {}),
    ...(typeof value.rolloutPercent === "number" ? { rolloutPercent: value.rolloutPercent } : {}),
    ...(optionalString(value, "minVersion") ? { minVersion: optionalString(value, "minVersion") } : {}),
  }
  return seed
}

function requiredString(record: Record<string, unknown>, key: string): string {
  const value = optionalString(record, key)
  if (value === undefined) throw new Error(`Pylon release seed ${key} is required`)
  return value
}

function optionalString(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key]
  if (value === undefined) return undefined
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`Pylon release seed ${key} must be a non-empty string`)
  }
  return value.trim()
}

function isRecord(value: unknown): value is Record<string, unknown> & { releases?: unknown } {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
