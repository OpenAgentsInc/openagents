import { readFile as nodeReadFile } from "node:fs/promises"
import { basename, join } from "node:path"

import type { AssetStore } from "./asset-store"
import { publishExport, type PublishExportResult } from "./publish"
import type { Platform } from "./publish-builder"

type ExpoExportMetadata = {
  readonly version: number
  readonly bundler: string
  readonly fileMetadata: Partial<Record<Platform, PlatformFileMetadata>>
}

type PlatformFileMetadata = {
  readonly bundle: string
  readonly assets: ReadonlyArray<{
    readonly path: string
    readonly ext: string
  }>
}

export type ReadExportedUpdateInput = {
  readonly distDir: string
  readonly platform: Platform
  readonly branch: string
  readonly runtimeVersion: string
  readonly id: string
  readonly createdAt: string
  readonly baseUrl: string
  readonly store: AssetStore
  readonly readFile?: (path: string) => Promise<Uint8Array>
  // The resolved public app config (same shape as `expo config --type public
  // --json`), embedded as `manifest.extra.expoClient` so expo-constants /
  // expo-linking work on a downloaded update, not just the embedded one.
  readonly expoClientConfig?: Record<string, unknown>
}

const defaultReadFile = async (path: string): Promise<Uint8Array> => {
  const bytes = await nodeReadFile(path)

  return new Uint8Array(bytes)
}

const contentTypeFromExtension = (extension: string): string => {
  switch (extension) {
    case ".js":
    case ".hbc":
      return "application/javascript"
    case ".png":
      return "image/png"
    default:
      return "application/octet-stream"
  }
}

const normalizeExtension = (extension: string): string =>
  extension.startsWith(".") ? extension : `.${extension}`

export async function readExportedUpdate(
  input: ReadExportedUpdateInput,
): Promise<PublishExportResult> {
  const readFile = input.readFile ?? defaultReadFile
  const metadataBytes = await readFile(join(input.distDir, "metadata.json"))
  const metadata = JSON.parse(new TextDecoder().decode(metadataBytes)) as ExpoExportMetadata
  const platformMetadata = metadata.fileMetadata[input.platform]

  if (!platformMetadata) {
    throw new Error(`No ${input.platform} export metadata found in metadata.json`)
  }

  const launchBundleBytes = await readFile(join(input.distDir, platformMetadata.bundle))
  const assets = await Promise.all(
    platformMetadata.assets.map(async (asset) => {
      const fileExtension = normalizeExtension(asset.ext)

      return {
        // The expo-updates client (FileDownloader) writes each downloaded asset
        // to disk at a path built directly from `key`, with no subdirectory
        // creation — a `key` containing a `/` (like the raw export path
        // "assets/<hash>") crashes/fails every asset write with "The folder
        // ... doesn't exist". Use the basename only; export filenames are
        // already content-hashed by Metro, so this stays unique.
        key: basename(asset.path),
        bytes: await readFile(join(input.distDir, asset.path)),
        contentType: contentTypeFromExtension(fileExtension),
        fileExtension,
      }
    }),
  )

  return publishExport({
    platform: input.platform,
    branch: input.branch,
    runtimeVersion: input.runtimeVersion,
    id: input.id,
    createdAt: input.createdAt,
    baseUrl: input.baseUrl,
    store: input.store,
    launchBundle: {
      key: basename(platformMetadata.bundle),
      bytes: launchBundleBytes,
    },
    assets,
    extra: input.expoClientConfig ? { expoClient: input.expoClientConfig } : undefined,
  })
}
