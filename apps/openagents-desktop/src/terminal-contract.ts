/**
 * Workspace-bounded PTY terminal — IPC + projection contract (CUT-20, #8700).
 *
 * The typed boundary for the interactive terminal / stdin-steering capability
 * (audit D3). Every terminal operation crosses the sandbox as a schema-checked
 * TYPED INTENT — the renderer never provides a shell, an argv, a cwd, or an
 * environment. The main process alone binds each session to the currently
 * authorized workspace root + a bounded environment (see `terminal-host.ts`),
 * so a compromised renderer can steer stdin but can never choose WHAT is
 * spawned or WHERE.
 *
 * Security boundary law (mirrors mcp-config-contract.ts / fable-local-contract):
 * - create/input/resize/interrupt/restart/close carry ONLY a session ref and,
 *   for input/resize, bounded data / integer geometry. No process authority.
 * - The output the renderer sees is BOUNDED (ring buffer, byte cap) and
 *   REDACTED (secret-shaped env values and token-shaped strings are scrubbed
 *   in main before the chunk is ever sent). Secret env VALUES never cross this
 *   line.
 * - Preview open resolves an EXPLICIT announced port (parsed from the session's
 *   own output, never a port scan) and opens it out-of-process behind a
 *   confirmation. No arbitrary in-app navigation.
 */
import { Exit, Schema } from "@effect-native/core/effect"

import { decode } from "./chat-contract.ts"

export const TerminalCreateChannel = "openagents-desktop/terminal-create" as const
export const TerminalInputChannel = "openagents-desktop/terminal-input" as const
export const TerminalResizeChannel = "openagents-desktop/terminal-resize" as const
export const TerminalInterruptChannel = "openagents-desktop/terminal-interrupt" as const
export const TerminalRestartChannel = "openagents-desktop/terminal-restart" as const
export const TerminalCloseChannel = "openagents-desktop/terminal-close" as const
export const TerminalSnapshotChannel = "openagents-desktop/terminal-snapshot" as const
export const TerminalPreviewOpenChannel = "openagents-desktop/terminal-preview-open" as const
export const TerminalEventChannel = "openagents-desktop/terminal-event" as const

/** Session refs are host-minted; the renderer echoes them, never invents shells. */
export const terminalSessionRefPattern = /^terminal\.[A-Za-z0-9._-]{1,80}$/

/** Bounded interactive geometry — a terminal cannot be absurdly large. */
export const TERMINAL_MIN_COLS = 1
export const TERMINAL_MAX_COLS = 1_000
export const TERMINAL_MIN_ROWS = 1
export const TERMINAL_MAX_ROWS = 1_000
/** A single typed input frame is bounded so a runaway paste cannot flood main. */
export const TERMINAL_MAX_INPUT_BYTES = 8_192

const BoundedCols = Schema.Number.check(
  Schema.isInt(),
  Schema.isBetween({ minimum: TERMINAL_MIN_COLS, maximum: TERMINAL_MAX_COLS }),
)
const BoundedRows = Schema.Number.check(
  Schema.isInt(),
  Schema.isBetween({ minimum: TERMINAL_MIN_ROWS, maximum: TERMINAL_MAX_ROWS }),
)
const SessionRef = Schema.String.check(Schema.isMinLength(9), Schema.isMaxLength(96))

// ---------------------------------------------------------------------------
// Requests (renderer -> main). No shell/argv/cwd/env EVER.
// ---------------------------------------------------------------------------

/**
 * Create carries only optional geometry and an OPTIONAL desired session ref
 * (for deterministic re-open); the host mints one when absent and rejects a
 * duplicate. The cwd + environment come exclusively from the authorized
 * workspace, bound in main.
 */
export const TerminalCreateRequestSchema = Schema.Struct({
  sessionRef: Schema.optional(SessionRef),
  cols: Schema.optional(BoundedCols),
  rows: Schema.optional(BoundedRows),
})
export type TerminalCreateRequest = typeof TerminalCreateRequestSchema.Type

export const TerminalInputRequestSchema = Schema.Struct({
  sessionRef: SessionRef,
  /** Goes to the shell's STDIN only; never interpolated into any argv. */
  data: Schema.String.check(Schema.isMaxLength(TERMINAL_MAX_INPUT_BYTES)),
})
export type TerminalInputRequest = typeof TerminalInputRequestSchema.Type

export const TerminalResizeRequestSchema = Schema.Struct({
  sessionRef: SessionRef,
  cols: BoundedCols,
  rows: BoundedRows,
})
export type TerminalResizeRequest = typeof TerminalResizeRequestSchema.Type

export const TerminalSessionRequestSchema = Schema.Struct({ sessionRef: SessionRef })
export type TerminalSessionRequest = typeof TerminalSessionRequestSchema.Type

export const TerminalPreviewOpenRequestSchema = Schema.Struct({
  sessionRef: SessionRef,
  port: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 65_535 })),
})
export type TerminalPreviewOpenRequest = typeof TerminalPreviewOpenRequestSchema.Type

// ---------------------------------------------------------------------------
// Results (main -> renderer). Typed rejections, never throws.
// ---------------------------------------------------------------------------

export const TerminalCreateResultSchema = Schema.Union([
  Schema.Struct({
    ok: Schema.Literal(true),
    sessionRef: Schema.String,
    /** basename(workspace root) — identification only, never the abs path. */
    cwdLabel: Schema.String,
    shellLabel: Schema.String,
    cols: Schema.Number,
    rows: Schema.Number,
  }),
  Schema.Struct({
    ok: Schema.Literal(false),
    reason: Schema.Literals([
      "no_workspace",
      "duplicate",
      "at_capacity",
      "spawn_failed",
      "invalid_request",
      "disposed",
    ]),
    message: Schema.String,
  }),
])
export type TerminalCreateResult = typeof TerminalCreateResultSchema.Type

export const TerminalAckResultSchema = Schema.Union([
  Schema.Struct({ ok: Schema.Literal(true) }),
  Schema.Struct({
    ok: Schema.Literal(false),
    reason: Schema.Literals([
      "not_found",
      "grant_revoked",
      "exited",
      "invalid_request",
      "disposed",
    ]),
  }),
])
export type TerminalAckResult = typeof TerminalAckResultSchema.Type

export const TerminalPreviewOpenResultSchema = Schema.Union([
  Schema.Struct({ ok: Schema.Literal(true), url: Schema.String }),
  Schema.Struct({
    ok: Schema.Literal(false),
    reason: Schema.Literals([
      "not_found",
      "unknown_port",
      "declined",
      "grant_revoked",
      "invalid_request",
      "unavailable",
    ]),
  }),
])
export type TerminalPreviewOpenResult = typeof TerminalPreviewOpenResultSchema.Type

// ---------------------------------------------------------------------------
// Snapshot projection (restart recovery). Bounded, redacted tails only.
// ---------------------------------------------------------------------------

export const TerminalPreviewViewSchema = Schema.Struct({
  port: Schema.Number,
  url: Schema.String,
  ready: Schema.Boolean,
})
export type TerminalPreviewView = typeof TerminalPreviewViewSchema.Type

export const TerminalSessionViewSchema = Schema.Struct({
  sessionRef: Schema.String,
  cwdLabel: Schema.String,
  shellLabel: Schema.String,
  status: Schema.Literals(["running", "exited", "recovered"]),
  exitCode: Schema.NullOr(Schema.Number),
  /** true when the tail was reloaded from disk after an app restart. */
  recovered: Schema.Boolean,
  /** true when the persisted tail is known-incomplete (loss-accounted). */
  gap: Schema.Boolean,
  /** Bounded, already-redacted output tail. */
  tail: Schema.String,
  previews: Schema.Array(TerminalPreviewViewSchema),
})
export type TerminalSessionView = typeof TerminalSessionViewSchema.Type

export const TerminalSnapshotSchema = Schema.Struct({
  sessions: Schema.Array(TerminalSessionViewSchema),
})
export type TerminalSnapshot = typeof TerminalSnapshotSchema.Type

// ---------------------------------------------------------------------------
// Push events (main -> renderer). Every payload bounded + pre-redacted.
// ---------------------------------------------------------------------------

export const TerminalEventSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("ready"),
    sessionRef: Schema.String,
    cwdLabel: Schema.String,
    shellLabel: Schema.String,
    cols: Schema.Number,
    rows: Schema.Number,
  }),
  Schema.Struct({
    kind: Schema.Literal("output"),
    sessionRef: Schema.String,
    /** Bounded, redacted chunk. */
    chunk: Schema.String,
  }),
  Schema.Struct({
    kind: Schema.Literal("exit"),
    sessionRef: Schema.String,
    exitCode: Schema.NullOr(Schema.Number),
    signal: Schema.NullOr(Schema.String),
  }),
  Schema.Struct({
    kind: Schema.Literal("preview"),
    sessionRef: Schema.String,
    port: Schema.Number,
    url: Schema.String,
    ready: Schema.Boolean,
  }),
  Schema.Struct({
    kind: Schema.Literal("closed"),
    sessionRef: Schema.String,
    reason: Schema.Literals(["user", "workspace_revoked", "app_quit"]),
  }),
  Schema.Struct({
    kind: Schema.Literal("error"),
    sessionRef: Schema.String,
    message: Schema.String,
  }),
])
export type TerminalEvent = typeof TerminalEventSchema.Type

// ---------------------------------------------------------------------------
// Decoders — main-side request decode + renderer-side response decode.
// ---------------------------------------------------------------------------

export const decodeTerminalCreateRequest = (value: unknown): TerminalCreateRequest | null =>
  decode(TerminalCreateRequestSchema, value) as TerminalCreateRequest | null

export const decodeTerminalInputRequest = (value: unknown): TerminalInputRequest | null =>
  decode(TerminalInputRequestSchema, value) as TerminalInputRequest | null

export const decodeTerminalResizeRequest = (value: unknown): TerminalResizeRequest | null =>
  decode(TerminalResizeRequestSchema, value) as TerminalResizeRequest | null

export const decodeTerminalSessionRequest = (value: unknown): TerminalSessionRequest | null =>
  decode(TerminalSessionRequestSchema, value) as TerminalSessionRequest | null

export const decodeTerminalPreviewOpenRequest = (
  value: unknown,
): TerminalPreviewOpenRequest | null =>
  decode(TerminalPreviewOpenRequestSchema, value) as TerminalPreviewOpenRequest | null

export const decodeTerminalCreateResult = (value: unknown): TerminalCreateResult => {
  const decoded = Schema.decodeUnknownExit(TerminalCreateResultSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return { ok: false, reason: "invalid_request", message: "The terminal response was invalid." }
  }
  return decoded.value
}

export const decodeTerminalAckResult = (value: unknown): TerminalAckResult => {
  const decoded = Schema.decodeUnknownExit(TerminalAckResultSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : { ok: false, reason: "invalid_request" }
}

export const decodeTerminalPreviewOpenResult = (value: unknown): TerminalPreviewOpenResult => {
  const decoded = Schema.decodeUnknownExit(TerminalPreviewOpenResultSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : { ok: false, reason: "invalid_request" }
}

export const decodeTerminalSnapshot = (value: unknown): TerminalSnapshot | null => {
  const decoded = Schema.decodeUnknownExit(TerminalSnapshotSchema)(value)
  if (!Exit.isSuccess(decoded)) return null
  return {
    sessions: decoded.value.sessions.filter((session) =>
      terminalSessionRefPattern.test(session.sessionRef)),
  }
}

export const decodeTerminalEvent = (value: unknown): TerminalEvent | null => {
  const decoded = Schema.decodeUnknownExit(TerminalEventSchema)(value)
  if (!Exit.isSuccess(decoded)) return null
  return terminalSessionRefPattern.test(decoded.value.sessionRef) ? decoded.value : null
}
