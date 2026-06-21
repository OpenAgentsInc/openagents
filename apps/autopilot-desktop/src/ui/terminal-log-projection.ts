// VCODE-11 (#5928): projected Terminal/Log pane.
//
// The default Verse coding terminal is not a raw terminal emulator. It renders
// controlled session-event excerpts, attaches digest refs when output is unsafe
// or oversized, and exposes focus-ownership metadata for the DOM layer.

import type { SessionEventRow } from "../shared/rpc.js"

export type TerminalLogRow = {
  readonly key: string
  readonly eventIndex: number
  readonly phase: string
  readonly state: string
  readonly observedAt: string
  readonly text: string
  readonly meta: string
  readonly redacted: boolean
  readonly digestRef: string | null
}

export type TerminalLogProjection = {
  readonly sessionRef: string
  readonly rows: readonly TerminalLogRow[]
  readonly copyText: string
  readonly focusOwner: "terminal-log"
  readonly sceneControlsWhileFocused: "blocked"
  readonly hiddenPanePointerPolicy: "inert"
}

const SECRET_TOKEN =
  /\b(?:sk-[A-Za-z0-9_-]{12,}|gh[pousr]_[A-Za-z0-9_]{12,}|xox[baprs]-[A-Za-z0-9-]{12,}|mdk_[A-Za-z0-9_]{12,}|Bearer\s+[A-Za-z0-9._-]{12,})\b/g
const ENV_SECRET =
  /\b[A-Z][A-Z0-9_]*(?:TOKEN|SECRET|KEY|MNEMONIC|PASSWORD|PRIVATE|ACCESS|SPARK|WALLET)[A-Z0-9_]*\s*=\s*["']?[^"'\s]+["']?/g
const WALLET_MATERIAL =
  /\b(?:xprv|yprv|zprv|tprv|uprv|vprv|lnbc|lntb|sp1q|spark1)[A-Za-z0-9_-]{12,}\b/gi
const LOCAL_PATH =
  /(?:~\/|\/Users\/|\/private\/|\/var\/folders\/|[A-Za-z]:\\)[^\s"'`),;]+/g
const PROVIDER_JSON = /\{[\s\S]{80,}\}/g
const LONG_OUTPUT_LIMIT = 360

const matches = (pattern: RegExp, text: string): boolean => {
  pattern.lastIndex = 0
  const ok = pattern.test(text)
  pattern.lastIndex = 0
  return ok
}

const unsafeReason = (text: string): string | null => {
  if (matches(SECRET_TOKEN, text)) return "secret token"
  if (matches(ENV_SECRET, text)) return "raw env"
  if (matches(WALLET_MATERIAL, text)) return "wallet material"
  if (matches(LOCAL_PATH, text)) return "local path"
  if (matches(PROVIDER_JSON, text)) return "provider payload"
  if (text.length > LONG_OUTPUT_LIMIT) return "large output"
  return null
}

const stableDigest = (text: string): string => {
  let hash = 0x811c9dc5
  for (let i = 0; i < text.length; i += 1) {
    hash ^= text.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193)
  }
  return (hash >>> 0).toString(16).padStart(8, "0")
}

export const terminalLogDigestRef = (text: string): string =>
  `digest.terminal_log.${stableDigest(text)}`

export const redactTerminalLogText = (text: string): string => {
  const clean = text
    .replace(PROVIDER_JSON, "[provider payload]")
    .replace(ENV_SECRET, "[env]")
    .replace(SECRET_TOKEN, "[secret]")
    .replace(WALLET_MATERIAL, "[wallet material]")
    .replace(LOCAL_PATH, "[local path]")
    .replace(/\s+/g, " ")
    .trim()
  if (clean.length <= 180) return clean
  return `${clean.slice(0, 177)}...`
}

const terminalLogSource = (event: SessionEventRow): string =>
  event.full && event.full.trim().length > 0 ? event.full : event.detail

export const projectTerminalLogPane = (input: {
  readonly sessionRef: string
  readonly events: readonly SessionEventRow[]
}): TerminalLogProjection => {
  const rows = input.events.map((event) => {
    const raw = terminalLogSource(event)
    const reason = unsafeReason(raw)
    const redacted = reason !== null
    const digestRef = redacted ? terminalLogDigestRef(raw) : null
    const text = redacted
      ? `[redacted ${reason}] ${redactTerminalLogText(raw)}`
      : raw.trim()
    const time = event.observedAt.slice(11, 19)
    return {
      key: `${input.sessionRef}:terminal:${event.eventIndex}`,
      eventIndex: event.eventIndex,
      phase: event.phase,
      state: event.state,
      observedAt: event.observedAt,
      text: text.length === 0 ? event.phase : text,
      meta: `${event.phase} · #${event.eventIndex} · ${time}`,
      redacted,
      digestRef,
    } satisfies TerminalLogRow
  })
  return {
    sessionRef: input.sessionRef,
    rows,
    copyText: rows.map((row) => `${row.meta} ${row.text}${row.digestRef ? ` ${row.digestRef}` : ""}`).join("\n"),
    focusOwner: "terminal-log",
    sceneControlsWhileFocused: "blocked",
    hiddenPanePointerPolicy: "inert",
  }
}
