export enum ErrorCategory {
  Quota = "quota",
  Auth = "auth",
  Network = "network",
  Validation = "validation",
  Sandbox = "sandbox",
  Internal = "internal",
}

export type ErrorClassificationInput = Readonly<{
  code?: string
  message: string
}>

export type ClassifiedError = Readonly<{
  category: ErrorCategory
  recoverable: boolean
  retryable: boolean
  blockerRef?: string
}>

export type ErrorBlockerRef = Readonly<{
  kind: "tas_error_blocker"
  blockerRef: string
  category: ErrorCategory
  recoverable: boolean
  retryable: boolean
}>

type CategoryRule = Readonly<{
  category: ErrorCategory
  recoverable: boolean
  retryable: boolean
  codes: readonly string[]
  messagePatterns: readonly RegExp[]
}>

const CATEGORY_RULES: readonly CategoryRule[] = [
  {
    category: ErrorCategory.Quota,
    recoverable: true,
    retryable: true,
    codes: ["429", "QUOTA_EXCEEDED", "RATE_LIMITED", "RATE_LIMIT"],
    messagePatterns: [
      /\brate[ -]?limit(?:ed)?\b/,
      /\bquota\b/,
      /\btoo many requests\b/,
    ],
  },
  {
    category: ErrorCategory.Auth,
    recoverable: true,
    retryable: false,
    codes: ["401", "403", "AUTH_FAILED", "UNAUTHORIZED", "FORBIDDEN"],
    messagePatterns: [
      /\bauth(?:entication|orization)?\b/,
      /\bunauthorized\b/,
      /\bforbidden\b/,
      /\binvalid api key\b/,
      /\bcredential\b/,
    ],
  },
  {
    category: ErrorCategory.Network,
    recoverable: true,
    retryable: true,
    codes: [
      "408",
      "502",
      "503",
      "504",
      "ECONNRESET",
      "ECONNREFUSED",
      "ETIMEDOUT",
      "EAI_AGAIN",
      "ENOTFOUND",
    ],
    messagePatterns: [
      /\bnetwork\b/,
      /\btimeout\b/,
      /\btimed out\b/,
      /\bconnection (?:reset|refused)\b/,
      /\bservice unavailable\b/,
    ],
  },
  {
    category: ErrorCategory.Validation,
    recoverable: true,
    retryable: false,
    codes: [
      "400",
      "422",
      "VALIDATION_ERROR",
      "INVALID_INPUT",
      "SCHEMA_INVALID",
    ],
    messagePatterns: [
      /\bvalidation\b/,
      /\binvalid input\b/,
      /\bschema\b/,
      /\brequired\b/,
      /\bmalformed\b/,
    ],
  },
  {
    category: ErrorCategory.Sandbox,
    recoverable: false,
    retryable: false,
    codes: [
      "EACCES",
      "EPERM",
      "SANDBOX_DENIED",
      "WORKSPACE_BOUNDARY",
      "PERMISSION_DENIED",
    ],
    messagePatterns: [
      /\bsandbox\b/,
      /\bworkspace boundary\b/,
      /\boutside (?:the )?workspace\b/,
      /\bpermission denied\b/,
      /\baccess denied\b/,
    ],
  },
]

const INTERNAL_CLASSIFICATION: ClassifiedError = {
  category: ErrorCategory.Internal,
  recoverable: false,
  retryable: false,
  blockerRef: "blocker.tas_error.internal",
}

export function classifyError(
  input: ErrorClassificationInput,
): ClassifiedError {
  const normalizedCode = normalizeCode(input.code)
  const normalizedMessage = input.message.toLowerCase()

  const rule =
    CATEGORY_RULES.find(
      (candidate) =>
        normalizedCode !== undefined &&
        candidate.codes.includes(normalizedCode),
    ) ??
    CATEGORY_RULES.find((candidate) =>
      candidate.messagePatterns.some((pattern) =>
        pattern.test(normalizedMessage),
      ),
    )

  if (!rule) {
    return INTERNAL_CLASSIFICATION
  }

  return {
    category: rule.category,
    recoverable: rule.recoverable,
    retryable: rule.retryable,
    blockerRef: `blocker.tas_error.${rule.category}`,
  }
}

export function toBlocker(classified: ClassifiedError): ErrorBlockerRef {
  return {
    kind: "tas_error_blocker",
    blockerRef:
      classified.blockerRef ?? `blocker.tas_error.${classified.category}`,
    category: classified.category,
    recoverable: classified.recoverable,
    retryable: classified.retryable,
  }
}

function normalizeCode(code: string | undefined): string | undefined {
  const trimmed = code?.trim()

  return trimmed ? trimmed.toUpperCase() : undefined
}
