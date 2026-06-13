const own = Object.prototype.hasOwnProperty

const stripOuterSlashes = (value: string): string =>
  value.trim().replace(/^\/+|\/+$/g, "")

const normalizeHost = (host: string): string =>
  stripOuterSlashes(host)
    .replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
    .replace(/\/+$/g, "")

const normalizeOwner = (owner: string): string =>
  encodeURIComponent(stripOuterSlashes(owner))

const getHeader = (
  headers: Record<string, string>,
  wantedName: string,
): string | null => {
  if (headers === null || typeof headers !== "object") {
    return null
  }

  const wanted = wantedName.toLowerCase()

  for (const [name, value] of Object.entries(headers)) {
    if (name.toLowerCase() !== wanted || typeof value !== "string") {
      continue
    }

    const trimmed = value.trim()
    return trimmed.length > 0 ? trimmed : null
  }

  return null
}

export function buildUpdatesUrl(input: {
  host: string
  owner: string
}): string {
  return `https://${normalizeHost(input.host)}/${normalizeOwner(input.owner)}/manifest`
}

export function parseChannelFromHeaders(headers: Record<string, string>): {
  channel: string | null
  runtimeVersion: string | null
  platform: string | null
} {
  return {
    channel: getHeader(headers, "expo-channel-name"),
    runtimeVersion: getHeader(headers, "expo-runtime-version"),
    platform: getHeader(headers, "expo-platform"),
  }
}

export function resolveBranchForChannel(
  channel: string,
  map: Record<string, string>,
): string {
  if (map === null || typeof map !== "object") {
    return channel
  }

  return own.call(map, channel) && typeof map[channel] === "string"
    ? map[channel]
    : channel
}
