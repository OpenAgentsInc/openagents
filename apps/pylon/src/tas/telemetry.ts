import { createHash } from "node:crypto"

export type TasTelemetryEventType =
  | "account_health"
  | "rate_limit"
  | "provider_routing"
  | "reconnect"

export type TelemetryRedactionReason =
  | "credentialed_url"
  | "email"
  | "secret"

export type TelemetryRedactionRef = {
  readonly field: string
  readonly reason: TelemetryRedactionReason
  readonly digestRef: `digest.pylon.telemetry.${string}`
}

export type TelemetryEventBase = {
  readonly type: TasTelemetryEventType
  readonly eventRef: string
  readonly occurredAtMs: number
  readonly accountRef?: string
  readonly providerId?: string
}

export type AccountHealthTelemetryEvent = TelemetryEventBase & {
  readonly type: "account_health"
  readonly status: "healthy" | "degraded" | "unavailable"
  readonly latencyMs?: number
  readonly consecutiveFailures?: number
  readonly errorClass?: string
  readonly email?: string
  readonly accessToken?: string
}

export type RateLimitTelemetryEvent = TelemetryEventBase & {
  readonly type: "rate_limit"
  readonly limited: boolean
  readonly remainingRequests?: number
  readonly retryAfterMs?: number
  readonly resetAtMs?: number
  readonly sourceDigestRef?: string
  readonly providerPayload?: unknown
  readonly bearerToken?: string
}

export type ProviderRoutingTelemetryEvent = TelemetryEventBase & {
  readonly type: "provider_routing"
  readonly routeRef: string
  readonly selectedProviderId: string
  readonly candidateProviderIds: readonly string[]
  readonly reason:
    | "account_health"
    | "budget"
    | "capability"
    | "rate_limit"
    | "user_policy"
  readonly latencyMs?: number
  readonly rawUrl?: string
}

export type ReconnectTelemetryEvent = TelemetryEventBase & {
  readonly type: "reconnect"
  readonly connectionRef: string
  readonly attempt: number
  readonly success: boolean
  readonly backoffMs?: number
  readonly durationMs?: number
  readonly lastErrorClass?: string
  readonly operatorEmail?: string
  readonly reconnectUrl?: string
}

export type TasTelemetryEvent =
  | AccountHealthTelemetryEvent
  | RateLimitTelemetryEvent
  | ProviderRoutingTelemetryEvent
  | ReconnectTelemetryEvent

export type PublicTelemetryEvent = Omit<
  TasTelemetryEvent,
  | "accessToken"
  | "bearerToken"
  | "email"
  | "operatorEmail"
  | "providerPayload"
  | "rawUrl"
  | "reconnectUrl"
> & {
  readonly redactionRefs: readonly TelemetryRedactionRef[]
  readonly [key: string]: unknown
}

const SENSITIVE_KEY_PATTERN =
  /(^|[_-])(access[_-]?token|api[_-]?key|authorization|bearer[_-]?token|cookie|credential|email|mnemonic|password|preimage|private[_-]?key|provider[_-]?payload|raw[_-]?(command|content|invoice|output|payload|prompt|url)|secret|seed[_-]?phrase|token|wallet)([_-]|$)/i

const SENSITIVE_QUERY_KEY_PATTERN =
  /^(access_token|api_key|authorization|auth|bearer|code|cookie|key|password|secret|token)$/i

function digestRef(value: unknown): `digest.pylon.telemetry.${string}` {
  const digest = createHash("sha256")
    .update(stableStringify(value))
    .digest("hex")
    .slice(0, 24)

  return `digest.pylon.telemetry.${digest}`
}

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value)
  }

  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(",")}]`
  }

  const entries = Object.entries(value as Record<string, unknown>).sort(([left], [right]) =>
    left.localeCompare(right),
  )

  return `{${entries
    .map(([key, entryValue]) => `${JSON.stringify(key)}:${stableStringify(entryValue)}`)
    .join(",")}}`
}

function credentialedUrlReason(value: unknown): TelemetryRedactionReason | null {
  if (typeof value !== "string") {
    return null
  }

  try {
    const url = new URL(value)
    if (url.username !== "" || url.password !== "") {
      return "credentialed_url"
    }

    for (const key of url.searchParams.keys()) {
      if (SENSITIVE_QUERY_KEY_PATTERN.test(key)) {
        return "credentialed_url"
      }
    }
  } catch {
    return null
  }

  return null
}

function sensitiveKeyReason(
  key: string,
  value: unknown,
): TelemetryRedactionReason | null {
  const urlReason = credentialedUrlReason(value)
  if (urlReason) {
    return urlReason
  }

  if (/email/i.test(key)) {
    return "email"
  }

  if (!SENSITIVE_KEY_PATTERN.test(key)) {
    return null
  }

  return "secret"
}

function redactValue(
  value: unknown,
  path: string,
  redactionRefs: TelemetryRedactionRef[],
): unknown {
  if (Array.isArray(value)) {
    return value
      .map((entry, index) => redactValue(entry, `${path}[${index}]`, redactionRefs))
      .filter((entry) => entry !== undefined)
  }

  if (value === null || typeof value !== "object") {
    return value
  }

  const output: Record<string, unknown> = {}
  for (const [key, entryValue] of Object.entries(value as Record<string, unknown>)) {
    const fieldPath = path === "" ? key : `${path}.${key}`
    const reason = sensitiveKeyReason(key, entryValue)

    if (reason) {
      redactionRefs.push({
        field: fieldPath,
        reason,
        digestRef: digestRef(entryValue),
      })
      continue
    }

    const redacted = redactValue(entryValue, fieldPath, redactionRefs)
    if (redacted !== undefined) {
      output[key] = redacted
    }
  }

  return output
}

export function redactTelemetry(event: TasTelemetryEvent): PublicTelemetryEvent {
  const redactionRefs: TelemetryRedactionRef[] = []
  // redactValue walks the event and returns a structurally-equivalent record
  // with sensitive fields removed; it is typed `unknown`, so we treat it as a
  // public-safe record and attach the collected redaction refs. PublicTelemetryEvent
  // carries an open `[key: string]: unknown` index, so the concrete event-shape
  // fields flow through that index.
  const redacted = redactValue(event, "", redactionRefs) as Record<string, unknown>

  return {
    ...redacted,
    redactionRefs,
  } as unknown as PublicTelemetryEvent
}
