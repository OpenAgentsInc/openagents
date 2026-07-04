import { readFile as nodeReadFile } from "node:fs/promises"
import { join } from "node:path"

import {
  buildDesktopUpdateManifest,
  normalizeDesktopReleaseSeed,
  type DesktopReleaseSeed,
  type DesktopUpdateManifest,
} from "./desktop-release.ts"
import type { UpdatesServer } from "./server.ts"

type SeedDesktopReleasesInput = {
  readonly server: UpdatesServer
  readonly distDir: string
  readonly baseUrl: string
  readonly readFile?: (path: string) => Promise<Uint8Array>
}

export type SeedDesktopReleasesResult = {
  readonly releases: ReadonlyArray<{
    readonly product: DesktopReleaseSeed["product"]
    readonly channel: string
    readonly manifest: DesktopUpdateManifest
  }>
}

const defaultReadFile = async (path: string): Promise<Uint8Array> => {
  const bytes = await nodeReadFile(path)

  return new Uint8Array(bytes)
}

export async function seedDesktopReleases(
  input: SeedDesktopReleasesInput,
): Promise<SeedDesktopReleasesResult> {
  const readFile = input.readFile ?? defaultReadFile
  const releasesPath = join(input.distDir, "releases.json")
  const releasesBytes = await readFile(releasesPath)
  const raw = JSON.parse(new TextDecoder().decode(releasesBytes)) as unknown
  const seeds = normalizeDesktopReleaseList(raw)
  const releases: SeedDesktopReleasesResult["releases"] = []

  for (const seed of seeds) {
    const artifactBytes = await readFile(join(input.distDir, seed.artifactPath))
    const bsdiffBytes = seed.bsdiffPath
      ? await readFile(join(input.distDir, seed.bsdiffPath))
      : undefined
    const result = await buildDesktopUpdateManifest({
      version: seed.version,
      artifactBytes,
      bsdiffBytes,
      bsdiffFromVersion: seed.bsdiffFromVersion,
      artifactContentType: seed.artifactContentType,
      createdAt: seed.createdAt,
      baseUrl: input.baseUrl,
      store: {
        put: (bytes) =>
          input.server.putAsset(
            bytes,
            bytes === bsdiffBytes
              ? "application/octet-stream"
              : seed.artifactContentType ?? "application/zip",
          ),
        get: async () => null,
      },
    })

    input.server.registerDesktopUpdate(seed.channel, result.manifest, seed.product)
    releases.push({
      product: seed.product,
      channel: seed.channel,
      manifest: result.manifest,
    })
  }

  return { releases }
}

function normalizeDesktopReleaseList(raw: unknown): DesktopReleaseSeed[] {
  if (Array.isArray(raw)) {
    return raw.map(normalizeDesktopReleaseSeed)
  }

  if (isRecord(raw) && Array.isArray(raw.releases)) {
    return raw.releases.map(normalizeDesktopReleaseSeed)
  }

  throw new Error("desktop releases.json must be an array or { releases: [] }")
}

function isRecord(value: unknown): value is { readonly releases?: unknown } {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
