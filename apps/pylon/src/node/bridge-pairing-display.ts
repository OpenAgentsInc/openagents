export type PairingDisplayInput = {
  baseUrl: string
  bootstrapId: string
  secret: string
}

export type ParsedPairingUri = {
  baseUrl: string
  bootstrapId: string
  secret: string
}

function validateBaseUrl(baseUrl: string): string {
  const normalized = baseUrl.trim()
  if (!normalized) {
    throw new Error("baseUrl must be a non-empty http(s) URL")
  }

  let parsed: URL
  try {
    parsed = new URL(normalized)
  } catch {
    throw new Error("baseUrl must be a non-empty http(s) URL")
  }

  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error("baseUrl must be a non-empty http(s) URL")
  }

  return normalized
}

export function buildPairingUri(input: PairingDisplayInput): string {
  const baseUrl = validateBaseUrl(input.baseUrl)

  return [
    "autopilot://pair?host=",
    encodeURIComponent(baseUrl),
    "&bid=",
    encodeURIComponent(input.bootstrapId),
    "&s=",
    encodeURIComponent(input.secret),
  ].join("")
}

export function parsePairingUri(uri: string): ParsedPairingUri | null {
  let parsed: URL
  try {
    parsed = new URL(uri)
  } catch {
    return null
  }

  if (parsed.protocol !== "autopilot:" || parsed.hostname !== "pair") {
    return null
  }

  const baseUrl = parsed.searchParams.get("host")
  const bootstrapId = parsed.searchParams.get("bid")
  const secret = parsed.searchParams.get("s")
  if (!baseUrl || !bootstrapId || !secret) {
    return null
  }

  try {
    return {
      baseUrl: validateBaseUrl(baseUrl),
      bootstrapId,
      secret,
    }
  } catch {
    return null
  }
}

export function renderPairingText(input: PairingDisplayInput): string {
  const baseUrl = validateBaseUrl(input.baseUrl)
  const code = `${input.bootstrapId}:${input.secret}`

  // TODO: QR-image rendering is a follow-up that needs a qr library.
  return [
    "Pylon bridge pairing",
    `Base URL: ${baseUrl}`,
    `One-time pairing code: ${code}`,
    "The secret is one-time-use. Do not log it elsewhere.",
  ].join("\n")
}
