export type Platform = "ios" | "android"

export type Asset = {
  readonly key: string
  readonly hash: string
  readonly contentType: string
  readonly fileExtension: string
  readonly url: string
}

export type LaunchAsset = {
  readonly key: string
  readonly hash: string
  readonly contentType: "application/javascript"
  readonly url: string
}

export type Update = {
  readonly id: string
  readonly platform: Platform
  readonly branch: string
  readonly runtimeVersion: string
  readonly createdAt: string
  readonly launchAsset: LaunchAsset
  readonly assets: ReadonlyArray<Asset>
  readonly metadata: Record<string, never>
  // `extra.expoClient` carries the resolved public app config (the same
  // shape `expo config --type public --json` produces). expo-constants /
  // expo-linking need this on a *downloaded* (non-embedded) update to resolve
  // things like the URI scheme at runtime — an empty `extra` here makes any
  // module that touches Constants.expoConfig throw ("runtime not ready") the
  // instant this update launches, which expo-updates treats as a failed
  // launch and silently rolls back to the previously cached/embedded update.
  readonly extra: Record<string, unknown>
}

export type BuildUpdateFromExportInput = {
  readonly id: string
  readonly platform: Platform
  readonly branch: string
  readonly runtimeVersion: string
  readonly createdAt: string
  readonly baseUrl: string
  readonly launchBundle: {
    readonly key: string
    readonly hash: string
  }
  readonly assets: ReadonlyArray<{
    readonly key: string
    readonly hash: string
    readonly contentType: string
    readonly fileExtension: string
  }>
  readonly extra?: Record<string, unknown>
}

export function buildUpdateFromExport(input: BuildUpdateFromExportInput): Update {
  return {
    id: input.id,
    platform: input.platform,
    branch: input.branch,
    runtimeVersion: input.runtimeVersion,
    createdAt: input.createdAt,
    launchAsset: {
      key: input.launchBundle.key,
      hash: input.launchBundle.hash,
      contentType: "application/javascript",
      url: `${input.baseUrl}/assets/${input.launchBundle.hash}`,
    },
    assets: input.assets.map((asset) => ({
      key: asset.key,
      hash: asset.hash,
      contentType: asset.contentType,
      fileExtension: asset.fileExtension,
      url: `${input.baseUrl}/assets/${asset.hash}`,
    })),
    metadata: {},
    extra: input.extra ?? {},
  }
}
