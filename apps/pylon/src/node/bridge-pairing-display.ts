import { COMPANION_E2EE_PROTOCOL } from "@openagentsinc/autopilot-control-protocol"

export type PairingDisplayInput = {
  baseUrl: string
  bootstrapId: string
  secret: string
  relayUrl?: string
  serverPublicKey?: string
}

export type ParsedPairingUri = {
  baseUrl: string
  bootstrapId: string
  secret: string
  relayUrl?: string
  serverPublicKey?: string
  protocol?: typeof COMPANION_E2EE_PROTOCOL
}

function validateHttpUrl(value: string, label: string): string {
  const normalized = value.trim()
  if (!normalized) {
    throw new Error(`${label} must be a non-empty http(s) URL`)
  }

  let parsed: URL
  try {
    parsed = new URL(normalized)
  } catch {
    throw new Error(`${label} must be a non-empty http(s) URL`)
  }

  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error(`${label} must be a non-empty http(s) URL`)
  }

  return normalized
}

export function buildPairingUri(input: PairingDisplayInput): string {
  const baseUrl = validateHttpUrl(input.baseUrl, "baseUrl")
  const params = new URLSearchParams()
  params.set("host", baseUrl)
  params.set("bid", input.bootstrapId)
  params.set("s", input.secret)
  if (input.relayUrl !== undefined) {
    params.set("relay", validateHttpUrl(input.relayUrl, "relayUrl"))
  }
  if (input.serverPublicKey !== undefined) {
    params.set("spk", input.serverPublicKey)
    params.set("proto", COMPANION_E2EE_PROTOCOL)
  }

  return `autopilot://pair?${params.toString()}`
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
  const relayUrl = parsed.searchParams.get("relay") ?? undefined
  const serverPublicKey = parsed.searchParams.get("spk") ?? undefined
  const protocol = parsed.searchParams.get("proto")
  if (!baseUrl || !bootstrapId || !secret) {
    return null
  }

  try {
    return {
      baseUrl: validateHttpUrl(baseUrl, "baseUrl"),
      bootstrapId,
      secret,
      ...(relayUrl === undefined ? {} : { relayUrl: validateHttpUrl(relayUrl, "relayUrl") }),
      ...(serverPublicKey === undefined ? {} : { serverPublicKey }),
      ...(protocol === COMPANION_E2EE_PROTOCOL ? { protocol } : {}),
    }
  } catch {
    return null
  }
}

export function renderPairingText(input: PairingDisplayInput): string {
  const baseUrl = validateHttpUrl(input.baseUrl, "baseUrl")
  const code = `${input.bootstrapId}:${input.secret}`

  // TODO: QR-image rendering is a follow-up that needs a qr library.
  return [
    "Pylon bridge pairing",
    `Base URL: ${baseUrl}`,
    ...(input.relayUrl === undefined ? [] : [`Relay URL: ${validateHttpUrl(input.relayUrl, "relayUrl")}`]),
    ...(input.serverPublicKey === undefined
      ? []
      : [
          `E2EE protocol: ${COMPANION_E2EE_PROTOCOL}`,
          `Server public key: ${input.serverPublicKey.slice(0, 12)}...${input.serverPublicKey.slice(-8)}`,
        ]),
    `One-time pairing code: ${code}`,
    "The secret is one-time-use. Do not log it elsewhere.",
  ].join("\n")
}
