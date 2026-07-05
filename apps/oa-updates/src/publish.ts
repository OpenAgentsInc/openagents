import { assetKeyFromBytes, type AssetStore } from "./asset-store"
import { buildUpdateFromExport, type Platform, type Update } from "./publish-builder"

export type PublishExportInput = {
  readonly platform: Platform
  readonly branch: string
  readonly runtimeVersion: string
  readonly id: string
  readonly createdAt: string
  readonly baseUrl: string
  readonly store: AssetStore
  readonly launchBundle: {
    readonly key: string
    readonly bytes: Uint8Array
  }
  readonly assets: ReadonlyArray<{
    readonly key: string
    readonly bytes: Uint8Array
    readonly contentType: string
    readonly fileExtension: string
  }>
  readonly extra?: Record<string, unknown>
}

export type PublishExportResult = {
  readonly update: Update
  readonly assetHashes: {
    readonly launchBundle: string
    readonly assets: ReadonlyArray<{
      readonly key: string
      readonly hash: string
    }>
  }
}

export async function publishExport(input: PublishExportInput): Promise<PublishExportResult> {
  const launchBundleHash = assetKeyFromBytes(input.launchBundle.bytes)
  await input.store.put(input.launchBundle.bytes)

  const assets = await Promise.all(
    input.assets.map(async (asset) => {
      const hash = assetKeyFromBytes(asset.bytes)
      await input.store.put(asset.bytes)

      return {
        key: asset.key,
        hash,
        contentType: asset.contentType,
        fileExtension: asset.fileExtension,
      }
    }),
  )

  const update = buildUpdateFromExport({
    id: input.id,
    platform: input.platform,
    branch: input.branch,
    runtimeVersion: input.runtimeVersion,
    createdAt: input.createdAt,
    baseUrl: input.baseUrl,
    launchBundle: {
      key: input.launchBundle.key,
      hash: launchBundleHash,
    },
    assets,
    extra: input.extra,
  })

  return {
    update,
    assetHashes: {
      launchBundle: launchBundleHash,
      assets: assets.map((asset) => ({ key: asset.key, hash: asset.hash })),
    },
  }
}
