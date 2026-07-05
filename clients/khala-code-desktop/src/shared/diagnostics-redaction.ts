/**
 * Public-safe redaction for the Khala Code desktop diagnostics/debug-log
 * export surface (issue #8441).
 *
 * The debug-log export must never leak local secrets, provider payloads,
 * tokens, or raw prompts. This module is the single redaction chokepoint used
 * by the diagnostics log store (before anything is even retained in memory)
 * and again by the bundle/export builder as defense in depth.
 *
 * Two complementary strategies:
 * - Known-sensitive object keys (case-insensitive) are dropped wherever they
 *   appear, however deeply nested, and replaced with a redaction marker. This
 *   is the primary defense for structured context objects (provider payload
 *   fragments, request bodies, prompt text carried in a named field, etc).
 * - Remaining string values (and the flat text of log messages) are scanned
 *   for well-known secret-shaped substrings (API keys, bearer tokens, JWTs,
 *   etc) as a second line of defense for values that were not caught by key
 *   name alone.
 *
 * This module intentionally cannot detect arbitrary raw prompt text that
 * carries no recognizable secret shape and is not stored under a known key —
 * callers must not hand raw prompt/message bodies to the diagnostics log
 * store in the first place. See diagnostics-log-store.ts for the recording
 * contract.
 */

export const KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER = "[redacted]"

/**
 * Object keys (case-insensitive, matched as a whole path segment) whose
 * values are always dropped regardless of depth or shape.
 */
const SENSITIVE_KEY_PATTERN = new RegExp(
  [
    "api[_-]?key",
    "apikey",
    "access[_-]?token",
    "refresh[_-]?token",
    "id[_-]?token",
    "bearer",
    "token",
    "secret",
    "password",
    "passwd",
    "authorization",
    "auth[_-]?header",
    "cookie",
    "session[_-]?id",
    "private[_-]?key",
    "mnemonic",
    "client[_-]?secret",
    "prompt",
    "raw[_-]?prompt",
    "user[_-]?prompt",
    "system[_-]?prompt",
    "messages",
    "rawEvents",
    "rawBody",
    "requestBody",
    "responseBody",
    "headers",
  ].map(term => `^${term}$`).join("|"),
  "i",
)

/** Regexes for secret-shaped substrings found inside otherwise-plain text. */
const SECRET_VALUE_PATTERNS: readonly RegExp[] = [
  // OpenAI-style secret keys (sk-..., sk-proj-...).
  /\bsk-[A-Za-z0-9_-]{16,}\b/g,
  // Anthropic-style keys.
  /\bsk-ant-[A-Za-z0-9_-]{16,}\b/g,
  // GitHub tokens (classic PAT, fine-grained PAT, OAuth token).
  /\bgh[oprsu]_[A-Za-z0-9]{16,}\b/g,
  // Slack tokens.
  /\bxox[baprs]-[A-Za-z0-9-]{10,}\b/g,
  // Google API keys.
  /\bAIza[0-9A-Za-z_-]{20,}\b/g,
  // Stripe-style secret/restricted keys.
  /\b(sk|rk)_(live|test)_[A-Za-z0-9]{10,}\b/g,
  // JWT-shaped triples.
  /\beyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\b/g,
  // Explicit "Bearer <token>" headers appearing in free text.
  /\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/gi,
  // AWS-style access key ids.
  /\b(AKIA|ASIA)[0-9A-Z]{16}\b/g,
]

/** Applies the secret-value regex passes to a single string. */
export const redactKhalaCodeDesktopDiagnosticsText = (input: string): string => {
  let output = input
  for (const pattern of SECRET_VALUE_PATTERNS) {
    output = output.replace(
      pattern,
      KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER,
    )
  }
  return output
}

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const MAX_REDACTION_DEPTH = 12

/**
 * Recursively redacts a JSON-shaped value: known-sensitive keys are dropped
 * wherever found, and remaining string leaves are scanned for secret-shaped
 * substrings. Non-JSON values (functions, symbols, etc) are stringified
 * defensively rather than passed through unredacted.
 */
export const redactKhalaCodeDesktopDiagnosticsValue = (
  value: unknown,
  depth = 0,
): unknown => {
  if (depth >= MAX_REDACTION_DEPTH) {
    return KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER
  }
  if (typeof value === "string") {
    return redactKhalaCodeDesktopDiagnosticsText(value)
  }
  if (typeof value === "number" || typeof value === "boolean" || value === null || value === undefined) {
    return value
  }
  if (Array.isArray(value)) {
    return value.map(item => redactKhalaCodeDesktopDiagnosticsValue(item, depth + 1))
  }
  if (isPlainObject(value)) {
    const result: Record<string, unknown> = {}
    for (const [key, nested] of Object.entries(value)) {
      if (SENSITIVE_KEY_PATTERN.test(key)) {
        result[key] = KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER
        continue
      }
      result[key] = redactKhalaCodeDesktopDiagnosticsValue(nested, depth + 1)
    }
    return result
  }
  // Functions, symbols, class instances, etc — never forward as-is.
  return KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER
}

/**
 * Redacts a context record (the shape used by diagnostics log entries)
 * without altering the top-level key set — only sensitive keys are replaced
 * with the redaction marker, everything else is redacted recursively.
 */
export const redactKhalaCodeDesktopDiagnosticsContext = (
  context: Readonly<Record<string, unknown>> | undefined,
): Readonly<Record<string, unknown>> | undefined => {
  if (context === undefined) return undefined
  const redacted = redactKhalaCodeDesktopDiagnosticsValue(context)
  return isPlainObject(redacted) ? redacted : { value: redacted }
}
