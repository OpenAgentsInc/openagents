// Trace redaction for default-on free-tier capture (openagents #6219, #6293,
// epic #6206).
//
// WHY THIS EXISTS
// ---------------
// The trace ingest/emit path is historically REJECT-on-leak: `atifTraceTripwire`
// drops any trajectory that still contains a secret / wallet / local-path /
// email VALUE. That is correct for the hand-opted / qa-runner surface (a leaky
// projection is better dropped than stored). But for DEFAULT-ON capture of all
// free-tier traffic (#6293) "drop the whole trace if it contains an email or a
// secret-shaped token" would silently lose a large fraction of real user
// conversations — real prompts/completions WILL contain emails, paths, and
// secret-shaped strings.
//
// So this module is the PRIMARY SCRUBBER: it deterministically replaces every
// sensitive span with a typed `[REDACTED:<category>]` placeholder BEFORE the
// trajectory reaches the tripwire. The tripwire stays the FAIL-CLOSED BACKSTOP:
// a trajectory that still trips AFTER redaction is dropped + logged, never
// stored (and the completion is never affected — capture is fire-and-forget).
//
// DESIGN
// ------
// - Pure, deterministic string transform. Same input -> same output, always.
//   No Effect service wrapper: the emitter is plain async and calls these
//   functions directly, so the dependency surface stays minimal and the worker
//   API does not depend on `apps/qa-runner`. (The qa-runner `TraceRedactor`
//   Effect service from #6219 stays the producer-side surface; this is the
//   gateway-emitter-side equivalent, with the SAME engine shape and additionally
//   covering EVERY category the trace tripwire rejects so post-redaction the
//   tripwire passes.)
// - CONSERVATIVE / over-redact rather than leak. The failure mode we choose is
//   "redact something that was actually benign", never "leak something real".
// - Covers a SUPERSET of `atifTraceTripwire`'s reject categories so a redacted
//   trajectory passes the backstop:
//     secret_material            sk-…, sk_live_/sk_test_, rk_live_, xox[baprs]-,
//                                gh[pousr]_…, AKIA…, AIza…, JWTs, generic bearer
//                                values, oa_agent_/oa_ tokens, PEM private keys,
//                                mnemonics, KEY=secret env lines
//     wallet_or_payment_material lnbc/lntb/lno1 invoices+offers, bc1 addresses,
//                                xpub/ypub/zpub/tpub
//     local_path                 /Users/<name>/…, /home/<name>/…, file://…,
//                                .secrets/…, the -Users-<name>- slug form
//     pii_email                  name@host.tld
//   plus internal IPs and long opaque hex/base64 blobs as extra hardening.
// - ALLOWLIST protects known false positives (the public `openagents/khala`
//   model id, public openagents.com/github.com OpenAgentsInc URLs, issue refs)
//   by fencing them with private-use sentinels before scanning and restoring
//   them after, so no category pattern can ever consume them.
// - Object KEYS are never rewritten (ATIF field names are structural; the
//   numeric `*_tokens` metric fields are numbers, never scanned as strings).
// - Returns a REPORT (`{ category -> count }` + total) so a caller can SEE that
//   a trace was scrubbed and how much, without surfacing any redacted VALUE.

/** Redaction categories (a superset of the tripwire's reject categories). */
export type TraceRedactionCategory =
  | 'private_key'
  | 'mnemonic'
  | 'jwt'
  | 'bearer'
  | 'provider_key'
  | 'oa_agent_token'
  | 'oa_token'
  | 'aws_key'
  | 'google_key'
  | 'slack_token'
  | 'github_token'
  | 'env_secret'
  | 'wallet_or_payment'
  | 'secrets_path'
  | 'home_path'
  | 'file_url'
  | 'email'
  | 'ip'
  | 'long_blob'

export type TraceRedactionReport = Readonly<{
  /** Per-category redaction counts (only categories with >0 are present). */
  counts: Readonly<Record<string, number>>
  /** Total redactions across all categories. */
  total: number
}>

export type TraceRedactionResult<T> = Readonly<{
  value: T
  report: TraceRedactionReport
}>

// ---------------------------------------------------------------------------
// Allowlist: spans that must NEVER be redacted (known false positives). They are
// masked with sentinels before scanning and restored after.
// ---------------------------------------------------------------------------

const ALLOWLIST_EXACT: ReadonlyArray<string> = ['openagents/khala']

const ALLOWLIST_PATTERNS: ReadonlyArray<RegExp> = [
  /https?:\/\/openagents\.com\/[^\s"'`)<>]*/g,
  /https?:\/\/(?:www\.)?github\.com\/OpenAgentsInc\/[^\s"'`)<>]*/g,
  /#\d{1,6}\b/g,
]

// Unicode Private Use Area sentinels — never occur in normal text.
const SENT_OPEN = ''
const SENT_CLOSE = ''

const escapeRegExp = (s: string): string =>
  s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const maskAllowlist = (
  text: string,
): { masked: string; originals: string[] } => {
  const originals: string[] = []
  let masked = text
  const stash = (m: string): string => {
    const idx = originals.length
    originals.push(m)
    return `${SENT_OPEN}${idx}${SENT_CLOSE}`
  }
  for (const exact of ALLOWLIST_EXACT) {
    masked = masked.replace(new RegExp(escapeRegExp(exact), 'g'), m => stash(m))
  }
  for (const re of ALLOWLIST_PATTERNS) {
    re.lastIndex = 0
    masked = masked.replace(re, m => stash(m))
  }
  return { masked, originals }
}

const unmaskAllowlist = (
  masked: string,
  originals: ReadonlyArray<string>,
): string =>
  masked.replace(
    new RegExp(`${SENT_OPEN}(\\d+)${SENT_CLOSE}`, 'g'),
    (_m, idx: string) => originals[Number(idx)] ?? '',
  )

// ---------------------------------------------------------------------------
// Category rules. Order matters: higher-entropy / more specific patterns run
// before broad catch-alls so the most precise category label wins.
// ---------------------------------------------------------------------------

type Rule = Readonly<{
  category: TraceRedactionCategory
  pattern: RegExp
  replace: (match: string, ...groups: string[]) => string
}>

const tag = (cat: TraceRedactionCategory): string => `[REDACTED:${cat}]`

// 12/15/18/21/24-word lowercase BIP-39-shaped seed phrases (single-spaced runs
// of 3-8 char words). Intentionally aggressive — leaking a seed phrase is
// catastrophic.
const MNEMONIC = /\b(?:[a-z]{3,8} ){11}[a-z]{3,8}(?:(?: [a-z]{3,8}){3})*\b/g

const RULES: ReadonlyArray<Rule> = [
  {
    category: 'private_key',
    pattern:
      /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----/g,
    replace: () => tag('private_key'),
  },
  { category: 'mnemonic', pattern: MNEMONIC, replace: () => tag('mnemonic') },
  {
    category: 'jwt',
    pattern: /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{6,}\b/g,
    replace: () => tag('jwt'),
  },
  // Wallet / payment material (mirrors the tripwire's WALLET_OR_PAYMENT_MATERIAL
  // plus a wider preimage/hash hardening below via long_blob).
  {
    category: 'wallet_or_payment',
    pattern:
      /\b(?:lnbc[0-9][a-z0-9]{20,}|lntb[0-9][a-z0-9]{20,}|lno1[a-z0-9]{20,}|bc1[a-z0-9]{20,}|(?:xpub|ypub|zpub|tpub)[1-9A-HJ-NP-Za-km-z]{20,})\b/gi,
    replace: () => tag('wallet_or_payment'),
  },
  {
    category: 'aws_key',
    pattern: /\bAKIA[0-9A-Z]{16}\b/g,
    replace: () => tag('aws_key'),
  },
  {
    category: 'google_key',
    pattern: /\bAIza[0-9A-Za-z_-]{20,}\b/g,
    replace: () => tag('google_key'),
  },
  {
    category: 'slack_token',
    pattern: /\bxox[baprs]-[A-Za-z0-9-]{8,}\b/g,
    replace: () => tag('slack_token'),
  },
  {
    category: 'github_token',
    pattern: /\bgh[pousr]_[A-Za-z0-9]{16,}\b/g,
    replace: () => tag('github_token'),
  },
  {
    category: 'bearer',
    pattern: /\b([Bb]earer)\s+[A-Za-z0-9._~+/=-]{8,}/g,
    replace: (_m, scheme: string) => `${scheme} ${tag('bearer')}`,
  },
  {
    category: 'bearer',
    pattern:
      /\b(authorization)\s*[:=]\s*["']?(?:bearer\s+)?[A-Za-z0-9._~+/=-]{8,}["']?/gi,
    replace: () => `authorization: ${tag('bearer')}`,
  },
  {
    category: 'provider_key',
    // sk-…, sk-or-…, sk-proj-…, sk-ant-… and sk_live_/sk_test_/rk_live_ shapes.
    pattern: /\b(?:sk-(?:or-|proj-|ant-)?[A-Za-z0-9_-]{8,}|(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{8,})\b/g,
    replace: () => tag('provider_key'),
  },
  {
    category: 'oa_agent_token',
    pattern: /\boa_agent_[A-Za-z0-9_-]{6,}\b/g,
    replace: () => tag('oa_agent_token'),
  },
  {
    category: 'oa_token',
    pattern: /\boa_(?:live|test|sk|key|secret|tok|token)?_?[A-Za-z0-9]{12,}\b/g,
    replace: () => tag('oa_token'),
  },
  {
    category: 'env_secret',
    pattern:
      /\b([A-Z][A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|MNEMONIC|SEED|BEARER|CREDENTIAL|PRIVATE)[A-Z0-9_]*)\s*=\s*["']?([^\s"'#]+)["']?/g,
    replace: (_m, key: string) => `${key}=${tag('env_secret')}`,
  },
  {
    category: 'secrets_path',
    pattern: /(?:\.{1,2}\/)?\.secrets\/[^\s"'`)<>]+/g,
    replace: () => tag('secrets_path'),
  },
  {
    category: 'file_url',
    pattern: /\bfile:\/\/[^\s"'`)<>]*/g,
    replace: () => tag('file_url'),
  },
  // Home paths: redact the WHOLE `/Users/<name>/…` or `/home/<name>/…` path. We
  // do NOT keep a `/Users/` or `/home/` prefix in the placeholder — the trace
  // tripwire's LOCAL_PATH check rejects the literal substrings `/Users/` and
  // `/home/`, so a "redacted" path that kept the prefix would still trip the
  // backstop. The replacement is a bare tag with no path separators.
  {
    category: 'home_path',
    pattern: /\/Users\/[^\s"'`)<>]*/g,
    replace: () => tag('home_path'),
  },
  {
    category: 'home_path',
    pattern: /\/home\/[^\s"'`)<>]*/g,
    replace: () => tag('home_path'),
  },
  // Claude-Code-style project-dir slug (`-Users-<name>-work`); redact the
  // username segment (this form has no `/Users/` substring so it never trips the
  // tripwire, but we still scrub the username for privacy).
  {
    category: 'home_path',
    pattern: /-Users-[^/\s"'`)<>-]+-/g,
    replace: () => '-Users-[REDACTED:home]-',
  },
  {
    category: 'email',
    pattern: /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g,
    replace: () => tag('email'),
  },
  {
    category: 'ip',
    pattern:
      /\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|100\.\d{1,3}\.\d{1,3}\.\d{1,3})\b/g,
    replace: () => tag('ip'),
  },
  // Long opaque hex / base64 blobs (catch-all for preimages, payment hashes, and
  // other opaque high-entropy secrets). Runs LAST so labelled categories win.
  {
    category: 'long_blob',
    pattern: /\b[A-Fa-f0-9]{40,}\b/g,
    replace: () => tag('long_blob'),
  },
  {
    category: 'long_blob',
    pattern: /\b[A-Za-z0-9+/]{48,}={0,2}\b/g,
    replace: () => tag('long_blob'),
  },
]

// ---------------------------------------------------------------------------
// Core string redactor.
// ---------------------------------------------------------------------------

/**
 * Redact a single string. Deterministic. Returns the redacted value + a report.
 * Allowlisted spans are fenced out before scanning and restored after.
 */
export const redactTraceString = (
  input: string,
): TraceRedactionResult<string> => {
  const counts: Record<string, number> = {}
  const bump = (cat: TraceRedactionCategory): void => {
    counts[cat] = (counts[cat] ?? 0) + 1
  }

  const { masked, originals } = maskAllowlist(input)

  let working = masked
  for (const rule of RULES) {
    rule.pattern.lastIndex = 0
    working = working.replace(rule.pattern, (...args: unknown[]) => {
      const match = args[0] as string
      // Never redact a sentinel placeholder.
      if (match.includes(SENT_OPEN)) {
        return match
      }
      bump(rule.category)
      // args = [match, ...groups, offset, string]; strip offset+string.
      const groups = args.slice(1, -2) as string[]
      return rule.replace(match, ...groups)
    })
  }

  const value = unmaskAllowlist(working, originals)
  const total = Object.values(counts).reduce((a, b) => a + b, 0)
  return { value, report: { counts, total } }
}

// ---------------------------------------------------------------------------
// Deep value redactor (walks a whole JSON value, e.g. an ATIF Trajectory).
// ---------------------------------------------------------------------------

type Json = unknown

const mergeReports = (
  into: Record<string, number>,
  from: TraceRedactionReport,
): void => {
  for (const [cat, n] of Object.entries(from.counts)) {
    into[cat] = (into[cat] ?? 0) + n
  }
}

/**
 * Deep-redact any JSON value. Strings are scrubbed via `redactTraceString`;
 * numbers / booleans / null pass through (so ATIF `*_tokens` numeric metrics are
 * never touched); arrays and objects are walked recursively. Object KEYS are NOT
 * rewritten (ATIF field names are structural). Deterministic.
 */
export const redactTraceValue = <T extends Json>(
  value: T,
): TraceRedactionResult<T> => {
  const counts: Record<string, number> = {}

  const walk = (v: Json): Json => {
    if (typeof v === 'string') {
      const r = redactTraceString(v)
      mergeReports(counts, r.report)
      return r.value
    }
    if (Array.isArray(v)) {
      return v.map(walk)
    }
    if (v !== null && typeof v === 'object') {
      const out: Record<string, Json> = {}
      for (const [k, child] of Object.entries(v as Record<string, Json>)) {
        out[k] = walk(child)
      }
      return out
    }
    return v // number | boolean | null | undefined
  }

  const redacted = walk(value) as T
  const total = Object.values(counts).reduce((a, b) => a + b, 0)
  return { value: redacted, report: { counts, total } }
}
