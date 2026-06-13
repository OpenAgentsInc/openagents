export type ParsedBootstrapInput = {
  bootstrapId: string
  secret: string
  baseUrl?: string
}

export type BootstrapParseResult =
  | { ok: true; value: ParsedBootstrapInput }
  | { ok: false; reason: "empty" | "too_long" | "invalid_format" }

export type PairingExchangeRequestInput = {
  bootstrapId: string
  secret: string
  baseUrl: string
  clientId: string
}

export type PairingExchangeRequestDescriptor = {
  url: string
  method: "POST"
  headers: Record<string, string>
  body: {
    verb: "bridge.pair.exchange"
    bootstrapId: string
    clientId: string
  }
}

export type PairingStatusPhase = "unpaired" | "pairing" | "paired" | "error"

export type PairingStatusState = {
  phase: PairingStatusPhase
  pairingRef?: string
  error?: string
}

export type PairingStatusViewModel = {
  label: string
  tone: "neutral" | "info" | "success" | "danger"
}

const MAX_BOOTSTRAP_INPUT_LENGTH = 512
const MAX_BOOTSTRAP_FIELD_LENGTH = 128
const BOOTSTRAP_FIELD_PATTERN = /^[A-Za-z0-9._~+=/-]+$/

export function parseBootstrapInput(text: string): BootstrapParseResult {
  const input = text.trim()
  if (input.length === 0) return { ok: false, reason: "empty" }
  if (input.length > MAX_BOOTSTRAP_INPUT_LENGTH) return { ok: false, reason: "too_long" }

  const uriValue = parsePairingUri(input)
  if (uriValue) return { ok: true, value: uriValue }

  const renderedTextValue = parseRenderedPairingText(input)
  if (renderedTextValue) return { ok: true, value: renderedTextValue }

  const codeValue = parsePairingCode(input)
  if (codeValue) return { ok: true, value: codeValue }

  return { ok: false, reason: "invalid_format" }
}

export function buildPairingExchangeRequest(
  input: PairingExchangeRequestInput,
): PairingExchangeRequestDescriptor {
  assertValidField("bootstrapId", input.bootstrapId)
  assertValidField("secret", input.secret)
  assertValidField("clientId", input.clientId)

  return {
    url: joinBaseUrlAndPath(validateBaseUrl(input.baseUrl), "/bridge/pair/exchange"),
    method: "POST",
    headers: {
      Authorization: `Bearer ${input.secret}`,
      "content-type": "application/json",
    },
    body: {
      verb: "bridge.pair.exchange",
      bootstrapId: input.bootstrapId,
      clientId: input.clientId,
    },
  }
}

export function pairingStatusView(state: PairingStatusState): PairingStatusViewModel {
  switch (state.phase) {
    case "unpaired":
      return { label: "Not paired", tone: "neutral" }
    case "pairing":
      return { label: "Pairing", tone: "info" }
    case "paired":
      return {
        label: state.pairingRef ? `Paired: ${state.pairingRef}` : "Paired",
        tone: "success",
      }
    case "error":
      return { label: state.error ? `Pairing failed: ${state.error}` : "Pairing failed", tone: "danger" }
  }
}

function parsePairingUri(input: string): ParsedBootstrapInput | null {
  let parsed: URL
  try {
    parsed = new URL(input)
  } catch {
    return null
  }

  if (parsed.protocol !== "autopilot:" || parsed.hostname !== "pair") return null

  const baseUrl = parsed.searchParams.get("host")
  const bootstrapId = parsed.searchParams.get("bid")
  const secret = parsed.searchParams.get("s")
  if (!bootstrapId || !secret) return null
  if (!isValidField(bootstrapId) || !isValidField(secret)) return null

  if (baseUrl) {
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

  return { bootstrapId, secret }
}

function parseRenderedPairingText(input: string): ParsedBootstrapInput | null {
  const baseUrl = input.match(/^Base URL:\s*(\S+)\s*$/im)?.[1]
  const code = input.match(/^One-time pairing code:\s*(\S+)\s*$/im)?.[1]
  if (!code) return null

  const parsedCode = parsePairingCode(code)
  if (!parsedCode) return null
  if (!baseUrl) return parsedCode

  try {
    return { ...parsedCode, baseUrl: validateBaseUrl(baseUrl) }
  } catch {
    return null
  }
}

function parsePairingCode(input: string): ParsedBootstrapInput | null {
  const parts = input.split(":")
  if (parts.length !== 2) return null

  const [bootstrapId, secret] = parts
  if (!bootstrapId || !secret) return null
  if (!isValidField(bootstrapId) || !isValidField(secret)) return null

  return { bootstrapId, secret }
}

function assertValidField(name: string, value: string): void {
  if (!isValidField(value)) {
    throw new Error(`${name} must be 1-${MAX_BOOTSTRAP_FIELD_LENGTH} URL-safe characters`)
  }
}

function isValidField(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= MAX_BOOTSTRAP_FIELD_LENGTH &&
    BOOTSTRAP_FIELD_PATTERN.test(value)
  )
}

function validateBaseUrl(baseUrl: string): string {
  const normalized = baseUrl.trim()
  let parsed: URL
  try {
    parsed = new URL(normalized)
  } catch {
    throw new Error("baseUrl must be a non-empty http(s) URL")
  }

  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error("baseUrl must be a non-empty http(s) URL")
  }

  return normalized.replace(/\/+$/, "")
}

function joinBaseUrlAndPath(baseUrl: string, path: string): string {
  const trimmedBase = baseUrl.replace(/\/+$/, "")
  const trimmedPath = path.replace(/^\/+/, "")
  return trimmedPath.length > 0 ? `${trimmedBase}/${trimmedPath}` : trimmedBase
}
