/**
 * Codex account reconnect bridge contract (#8574, #8640 unblock).
 *
 * The renderer may only ever see: account refs, readiness states, the
 * device-auth verification URL + user code, and typed connect status. No
 * tokens, emails, file paths, or raw child-process output cross this line.
 * Channels are renderer-argument-free with ONE bounded exception: the
 * reconnect-start channel (EP250 owner mandate — the UI owns reconnect, not
 * the CLI) carries a single account ref, grammar-validated on both sides AND
 * re-validated by main against the refs main itself read from the pylon
 * registry. Every other input — including the verification URL main opens —
 * stays main-held, so a compromised renderer still cannot point the flow
 * anywhere.
 */
import { Exit, Schema } from "@effect-native/core/effect"

export const CodexAccountsChannel = "openagents-desktop/codex-accounts" as const
export const CodexConnectStartChannel = "openagents-desktop/codex-connect-start" as const
export const CodexReconnectStartChannel = "openagents-desktop/codex-reconnect-start" as const
export const CodexConnectStatusChannel = "openagents-desktop/codex-connect-status" as const
export const CodexConnectOpenChannel = "openagents-desktop/codex-connect-open" as const

/** Pylon account-ref grammar (mirrors apps/pylon auth's accountRefPattern). */
export const codexAccountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/
/** Codex device-auth one-time code, e.g. `8260-DUG55`. */
export const codexUserCodePattern = /^[A-Z0-9]{4}-[A-Z0-9]{4,6}$/

export const CodexAccountEntrySchema = Schema.Struct({
  ref: Schema.String,
  /** Readiness state string, e.g. "ready" | "credentials_revoked" | ... */
  readiness: Schema.String,
})

export const CodexAccountsResultSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("ok"),
    accounts: Schema.Array(CodexAccountEntrySchema),
  }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    message: Schema.String,
  }),
])

export type CodexAccountsResult = typeof CodexAccountsResultSchema.Type

export const CodexConnectStatusSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("idle") }),
  Schema.Struct({ state: Schema.Literal("starting") }),
  Schema.Struct({
    state: Schema.Literal("awaiting_browser"),
    url: Schema.String,
    code: Schema.String,
  }),
  Schema.Struct({ state: Schema.Literal("connected"), ref: Schema.String }),
  Schema.Struct({ state: Schema.Literal("failed"), reason: Schema.String }),
])

export type CodexConnectStatus = typeof CodexConnectStatusSchema.Type

export const unavailableCodexAccountsResult = (): CodexAccountsResult => ({
  state: "unavailable",
  message: "Local Pylon runtime is unavailable. No accounts were read.",
})

export const decodeCodexAccountsResult = (value: unknown): CodexAccountsResult => {
  const decoded = Schema.decodeUnknownExit(CodexAccountsResultSchema)(value)
  if (!Exit.isSuccess(decoded)) return unavailableCodexAccountsResult()
  if (decoded.value.state === "unavailable") return decoded.value
  // Defense-in-depth: drop entries whose ref does not fit the pylon grammar.
  return {
    state: "ok",
    accounts: decoded.value.accounts.filter((account) =>
      codexAccountRefPattern.test(account.ref) && account.readiness.length <= 80
    ),
  }
}

export const decodeCodexConnectStatus = (value: unknown): CodexConnectStatus => {
  const decoded = Schema.decodeUnknownExit(CodexConnectStatusSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return { state: "failed", reason: "invalid_bridge_payload" }
  }
  const status = decoded.value
  if (status.state === "awaiting_browser") {
    // Only an https auth URL and a short user code may reach the renderer.
    if (!status.url.startsWith("https://") || status.url.length > 200) {
      return { state: "failed", reason: "invalid_verification_url" }
    }
    if (!codexUserCodePattern.test(status.code)) {
      return { state: "failed", reason: "invalid_user_code" }
    }
  }
  if (status.state === "connected" && !codexAccountRefPattern.test(status.ref)) {
    return { state: "failed", reason: "invalid_account_ref" }
  }
  if (status.state === "failed" && status.reason.length > 120) {
    return { state: "failed", reason: status.reason.slice(0, 120) }
  }
  return status
}
