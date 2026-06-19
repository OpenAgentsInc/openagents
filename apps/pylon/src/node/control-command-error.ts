// #5453: a typed, client-facing validation error for the `/command` control
// path. The control server turns *any* thrown error from `runCommand` into an
// HTTP response; without a distinguishable type, a bad-request command (a
// malformed/missing field) and a genuine internal failure both surfaced as a
// raw HTTP 500 (`control 500` in the desktop composer). A
// `ControlCommandValidationError` lets the server answer 400 with a clean typed
// reason for caller mistakes, reserving 500 for true internal faults.
//
// It carries a short, non-secret `reason` code (never raw provider text) and a
// human-readable message. The reason is safe to surface to clients.
export class ControlCommandValidationError extends Error {
  readonly _tag = "ControlCommandValidationError"
  readonly reason: string

  constructor(reason: string, message?: string) {
    super(message ?? reason)
    this.name = "ControlCommandValidationError"
    this.reason = reason
  }
}

export const isControlCommandValidationError = (
  error: unknown,
): error is ControlCommandValidationError =>
  error instanceof ControlCommandValidationError ||
  (typeof error === "object" &&
    error !== null &&
    (error as { _tag?: unknown })._tag === "ControlCommandValidationError")

// Returns the non-secret validation reason code when `error` is a control
// command validation error, else `null`. Returning the string (rather than a
// narrowing type guard) keeps callers in `catch (error: unknown)` blocks clean
// across module boundaries without leaning on cross-module type narrowing.
export const controlCommandValidationReason = (error: unknown): string | null => {
  if (error instanceof ControlCommandValidationError) return error.reason
  if (
    typeof error === "object" &&
    error !== null &&
    (error as { _tag?: unknown })._tag === "ControlCommandValidationError"
  ) {
    const reason = (error as { reason?: unknown }).reason
    return typeof reason === "string" ? reason : "validation_error"
  }
  return null
}
