export type Asset = {
  hash: string
  key: string
  contentType: string
  fileExtension?: string
  url: string
}

export type Update = {
  id: string
  platform: "ios" | "android"
  branch: string
  runtimeVersion: string
  createdAt: string
  launchAsset: Asset
  assets: Asset[]
  metadata: Record<string, string>
  extra: Record<string, unknown>
}

export type ParsedRequest = {
  platform?: "ios" | "android"
  runtimeVersion?: string
  channelName?: string
  currentUpdateId?: string
}

type ResponseHeaders = Record<string, string>

export type ManifestResolution =
  | {
      kind: "manifest"
      manifest: Update
      responseHeaders: ResponseHeaders
    }
  | {
      kind: "directive"
      directive:
        | { type: "noUpdateAvailable" }
        | { type: "rollBackToEmbedded"; parameters: { commitTime: string } }
      responseHeaders: ResponseHeaders
    }

export const expoUpdatesResponseHeaders = {
  "expo-protocol-version": "1",
  "expo-sfv-version": "0",
} as const satisfies ResponseHeaders

const getHeader = (
  headers: Record<string, string>,
  wantedName: string,
): string | undefined => {
  const wanted = wantedName.toLowerCase()

  for (const [name, value] of Object.entries(headers)) {
    if (name.toLowerCase() === wanted) {
      return value
    }
  }

  return undefined
}

const parsePlatform = (value: string | undefined): "ios" | "android" | undefined =>
  value === "ios" || value === "android" ? value : undefined

export const parseManifestRequest = (
  headers: Record<string, string>,
): ParsedRequest => ({
  platform: parsePlatform(getHeader(headers, "Expo-Platform")),
  runtimeVersion: getHeader(headers, "Expo-Runtime-Version"),
  channelName: getHeader(headers, "Expo-Channel-Name"),
  currentUpdateId: getHeader(headers, "Expo-Current-Update-ID"),
})

export const resolveManifest = (input: {
  updates: Update[]
  channelToBranch: Record<string, string>
  request: ParsedRequest
  rolledBackBranches?: Record<string, { commitTime: string }>
}): ManifestResolution => {
  const responseHeaders = { ...expoUpdatesResponseHeaders }
  const branch =
    input.request.channelName === undefined
      ? undefined
      : input.channelToBranch[input.request.channelName]

  if (branch !== undefined) {
    const rollback = input.rolledBackBranches?.[branch]

    if (rollback !== undefined) {
      return {
        kind: "directive",
        directive: {
          type: "rollBackToEmbedded",
          parameters: { commitTime: rollback.commitTime },
        },
        responseHeaders,
      }
    }
  }

  const manifest = input.updates
    .filter(
      (update) =>
        update.branch === branch &&
        update.runtimeVersion === input.request.runtimeVersion &&
        update.platform === input.request.platform,
    )
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt))[0]

  if (manifest === undefined) {
    return {
      kind: "directive",
      directive: { type: "noUpdateAvailable" },
      responseHeaders,
    }
  }

  return {
    kind: "manifest",
    manifest,
    responseHeaders,
  }
}
