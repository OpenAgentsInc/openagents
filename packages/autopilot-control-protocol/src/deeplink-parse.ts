export type AutopilotDeepLinkKind = "session" | "node" | "ship" | "unknown"

export type AutopilotDeepLinkParseResult = {
  kind: AutopilotDeepLinkKind
  sessionRef: string | null
  nodeRef: string | null
}

const UNKNOWN_DEEP_LINK: AutopilotDeepLinkParseResult = {
  kind: "unknown",
  sessionRef: null,
  nodeRef: null,
}

export function parseAutopilotDeepLink(url: unknown): AutopilotDeepLinkParseResult {
  if (typeof url !== "string") {
    return UNKNOWN_DEEP_LINK
  }

  const input = url.trim()
  const prefix = "autopilot://"

  if (!input.startsWith(prefix)) {
    return UNKNOWN_DEEP_LINK
  }

  const body = input.slice(prefix.length)
  const separatorIndex = body.indexOf("/")

  if (separatorIndex <= 0) {
    return UNKNOWN_DEEP_LINK
  }

  const kind = body.slice(0, separatorIndex)
  const ref = readFirstPathSegment(body.slice(separatorIndex + 1))

  if (ref === null) {
    return UNKNOWN_DEEP_LINK
  }

  if (kind === "session") {
    return {
      kind,
      sessionRef: ref,
      nodeRef: null,
    }
  }

  if (kind === "node") {
    return {
      kind,
      sessionRef: null,
      nodeRef: ref,
    }
  }

  if (kind === "ship") {
    return {
      kind,
      sessionRef: ref,
      nodeRef: null,
    }
  }

  return UNKNOWN_DEEP_LINK
}

function readFirstPathSegment(path: string): string | null {
  const boundary = firstBoundaryIndex(path)
  const rawRef = (boundary === -1 ? path : path.slice(0, boundary)).trim()

  if (rawRef === "") {
    return null
  }

  try {
    const decodedRef = decodeURIComponent(rawRef).trim()
    return decodedRef === "" ? null : decodedRef
  } catch {
    return rawRef
  }
}

function firstBoundaryIndex(value: string): number {
  let boundary = -1

  for (const marker of ["/", "?", "#"]) {
    const index = value.indexOf(marker)
    if (index !== -1 && (boundary === -1 || index < boundary)) {
      boundary = index
    }
  }

  return boundary
}
