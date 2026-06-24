// Trace redaction (issue #6219): a reusable, deterministic Effect service that
// scrubs sensitive content from an agent trace BEFORE it is stored or shared as
// a public `/trace/{uuid}`. Required for publishing any trace — this session, an
// imported Claude Code / Codex session (#6220), or a QA run — without leaking
// secrets, keys, mnemonics, PII, owner identifiers, or internal infrastructure.
//
// DESIGN
// ------
// - The engine is a PURE, deterministic string transform: `redactString(text)`
//   applies an ordered set of category patterns, replacing each hit with a
//   stable `[REDACTED:<category>]` (or a category-specific shape, e.g.
//   `/Users/[REDACTED:home]/…`). Same input -> same output, every time.
// - `redactValue(value)` deep-walks a JSON value (the whole ATIF `Trajectory`).
//   It redacts every string leaf; numbers/booleans/null pass through, so the
//   ATIF spec-mandated `*_tokens` NUMERIC metric fields are never touched.
// - An ALLOWLIST protects known false positives: the public model id
//   `openagents/khala`, public `https://openagents.com/…` and
//   `github.com/OpenAgentsInc/…` URLs, and public issue refs (`#1234`).
//   Allowlisted spans are fenced with sentinels before scanning and restored
//   after, so no pattern can ever consume them. (The ATIF `*_tokens` field NAMES
//   are numbers' keys — never scanned as secret strings — so they are safe by
//   construction.)
// - Returns a REPORT: `{ category -> count }` + a total, so a publisher can SEE
//   that a trace was scrubbed and how much.
//
// Categories (aggressive by design — redact-to-safe is the safe failure mode):
//   private_key    -----BEGIN … PRIVATE KEY----- … blocks
//   mnemonic       12/15/18/21/24-word BIP-39 seed phrases
//   jwt            header.payload.signature JWTs
//   bearer         "Bearer <token>" / Authorization headers
//   provider_key   sk-…, sk-or-…, sk-proj-…, sk-ant-… keys
//   oa_agent_token oa_agent_… OpenAgents agent bearer tokens
//   x_code         oa-x-… X-verification codes
//   oa_token       other OpenAgents tokens (oa_live_/oa_sk_/oa_…)
//   owner_id       github:<digits> (and similar owner identifiers)
//   env_secret     KEY=value lines for secret-looking KEYs
//   secrets_path   .secrets/… file paths
//   home_path      /Users/<name>/…  -> /Users/[REDACTED:home]/…
//   email          name@host.tld (PII)
//   ip             internal IPv4 (private + tailscale 100.x)
//   long_blob      long hex / base64 blobs (catch-all for opaque secrets)
//
// Pluggable into the ATIF emitter, the trace ingest tripwire (#6208), and the
// importer (#6220). Unit-tested with known-secret fixtures + allowlist cases
// (`redaction.test.ts`).

import { Effect, Layer } from "effect";
import * as Context from "effect/Context";

/** The redaction categories. */
export type RedactionCategory =
  | "private_key"
  | "mnemonic"
  | "jwt"
  | "bearer"
  | "provider_key"
  | "oa_agent_token"
  | "x_code"
  | "oa_token"
  | "owner_id"
  | "env_secret"
  | "secrets_path"
  | "home_path"
  | "email"
  | "ip"
  | "long_blob"
  | "username";

export interface RedactOptions {
  /** Extra literal secrets to redact verbatim everywhere (category "username").
   * Used for the macOS username, which leaks outside `/Users/<name>/` paths
   * (e.g. the `ls -l` owner column, or the slug form `-Users-<name>-`). */
  readonly usernames?: ReadonlyArray<string>;
}

export interface RedactionReport {
  /** Count of redactions per category (only categories with >0 are present). */
  readonly counts: Readonly<Record<string, number>>;
  /** Total number of redactions across all categories. */
  readonly total: number;
}

export interface RedactionResult<T> {
  readonly value: T;
  readonly report: RedactionReport;
}

// ---------------------------------------------------------------------------
// Allowlist: spans that must NEVER be redacted (known false positives). We mask
// these with sentinels before scanning and restore them after, so no category
// pattern can consume them.
// ---------------------------------------------------------------------------

/** The public, shareable model id and other safe-verbatim literals. */
const ALLOWLIST_EXACT: ReadonlyArray<string> = ["openagents/khala"];

/** Allowlisted URL/ref patterns — public OpenAgents + GitHub surfaces, issue refs. */
const ALLOWLIST_PATTERNS: ReadonlyArray<RegExp> = [
  /https?:\/\/openagents\.com\/[^\s"'`)<>]*/g,
  /https?:\/\/(?:www\.)?github\.com\/OpenAgentsInc\/[^\s"'`)<>]*/g,
  /#\d{1,6}\b/g,
];

// A private-use sentinel fences allowlisted spans. / are in the
// Unicode Private Use Area and never occur in normal text.
const SENT_OPEN = "";
const SENT_CLOSE = "";

function maskAllowlist(text: string): { masked: string; originals: string[] } {
  const originals: string[] = [];
  let masked = text;
  const stash = (m: string): string => {
    const idx = originals.length;
    originals.push(m);
    return `${SENT_OPEN}${idx}${SENT_CLOSE}`;
  };
  for (const exact of ALLOWLIST_EXACT) {
    const re = new RegExp(exact.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "g");
    masked = masked.replace(re, (m) => stash(m));
  }
  for (const re of ALLOWLIST_PATTERNS) {
    re.lastIndex = 0;
    masked = masked.replace(re, (m) => stash(m));
  }
  return { masked, originals };
}

function unmaskAllowlist(masked: string, originals: ReadonlyArray<string>): string {
  return masked.replace(
    new RegExp(`${SENT_OPEN}(\\d+)${SENT_CLOSE}`, "g"),
    (_m, idx: string) => originals[Number(idx)] ?? "",
  );
}

// ---------------------------------------------------------------------------
// Category rules. Each: a category, a global RegExp, and a replacement (a fixed
// token or a function preserving a neutral shape). Order matters: more specific
// / higher-entropy patterns run before broader catch-alls.
// ---------------------------------------------------------------------------

interface Rule {
  readonly category: RedactionCategory;
  readonly pattern: RegExp;
  readonly replace: (match: string, ...groups: string[]) => string;
}

const tag = (cat: RedactionCategory): string => `[REDACTED:${cat}]`;

// Mnemonics are matched structurally: a run of 12/15/18/21/24 lowercase ascii
// words (3-8 chars) separated by single spaces. Intentionally aggressive — a
// seed phrase is catastrophic to leak.
const MNEMONIC = /\b(?:[a-z]{3,8} ){11}[a-z]{3,8}(?:(?: [a-z]{3,8}){3})*\b/g;

const RULES: ReadonlyArray<Rule> = [
  {
    category: "private_key",
    pattern: /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----/g,
    replace: () => tag("private_key"),
  },
  { category: "mnemonic", pattern: MNEMONIC, replace: () => tag("mnemonic") },
  {
    category: "jwt",
    pattern: /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/g,
    replace: () => tag("jwt"),
  },
  {
    category: "bearer",
    pattern: /\b([Bb]earer)\s+[A-Za-z0-9._~+/=-]{8,}/g,
    replace: (_m, scheme: string) => `${scheme} ${tag("bearer")}`,
  },
  {
    category: "bearer",
    pattern: /\b(authorization)\s*[:=]\s*["']?(?:bearer\s+)?[A-Za-z0-9._~+/=-]{8,}["']?/gi,
    replace: () => `authorization: ${tag("bearer")}`,
  },
  {
    category: "provider_key",
    pattern: /\bsk-(?:or-|proj-|ant-)?[A-Za-z0-9_-]{16,}\b/g,
    replace: () => tag("provider_key"),
  },
  {
    category: "oa_agent_token",
    pattern: /\boa_agent_[A-Za-z0-9_-]{6,}\b/g,
    replace: () => tag("oa_agent_token"),
  },
  {
    category: "x_code",
    pattern: /\boa-x-[A-Za-z0-9_-]{4,}\b/g,
    replace: () => tag("x_code"),
  },
  {
    category: "oa_token",
    pattern: /\boa_(?:live|test|sk|key|secret|tok|token)?_?[A-Za-z0-9]{12,}\b/g,
    replace: () => tag("oa_token"),
  },
  {
    category: "owner_id",
    pattern: /\b(github|gh|x|twitter|discord|telegram|nostr):\d{3,}\b/gi,
    replace: (_m, provider: string) => `${provider}:${tag("owner_id")}`,
  },
  {
    category: "env_secret",
    pattern:
      /\b([A-Z][A-Z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|MNEMONIC|SEED|API|BEARER|CREDENTIAL|PRIVATE)[A-Z0-9_]*)\s*=\s*["']?([^\s"'#]+)["']?/g,
    replace: (_m, key: string) => `${key}=${tag("env_secret")}`,
  },
  {
    category: "secrets_path",
    pattern: /(?:\.{1,2}\/)?\.secrets\/[^\s"'`)<>]+/g,
    replace: () => tag("secrets_path"),
  },
  {
    category: "home_path",
    pattern: /\/Users\/[^/\s"'`)<>]+/g,
    replace: () => "/Users/[REDACTED:home]",
  },
  // Slug form of an absolute home path, as Claude Code encodes project dirs:
  // `-Users-<name>-work` / `/-Users-<name>/`. Redact the username segment.
  {
    category: "home_path",
    pattern: /-Users-[^/\s"'`)<>-]+-/g,
    replace: () => "-Users-[REDACTED:home]-",
  },
  {
    category: "email",
    pattern: /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g,
    replace: () => tag("email"),
  },
  {
    category: "ip",
    pattern:
      /\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|100\.\d{1,3}\.\d{1,3}\.\d{1,3})\b/g,
    replace: () => tag("ip"),
  },
  // Long opaque hex / base64 blobs (catch-all). Runs LAST so labelled
  // categories win.
  {
    category: "long_blob",
    pattern: /\b[A-Fa-f0-9]{40,}\b/g,
    replace: () => tag("long_blob"),
  },
  {
    category: "long_blob",
    pattern: /\b[A-Za-z0-9+/]{48,}={0,2}\b/g,
    replace: () => tag("long_blob"),
  },
];

// ---------------------------------------------------------------------------
// Core string redactor.
// ---------------------------------------------------------------------------

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Redact a single string. Deterministic. Returns the redacted value + a report.
 * Allowlisted spans are fenced out before scanning and restored after.
 * `options.usernames` redacts those exact literals everywhere (the OS username
 * leaks outside `/Users/<name>/` paths, e.g. an `ls -l` owner column). */
export function redactString(input: string, options: RedactOptions = {}): RedactionResult<string> {
  const counts: Record<string, number> = {};
  const bump = (cat: RedactionCategory): void => {
    counts[cat] = (counts[cat] ?? 0) + 1;
  };

  const { masked, originals } = maskAllowlist(input);

  let working = masked;
  for (const rule of RULES) {
    rule.pattern.lastIndex = 0;
    working = working.replace(rule.pattern, (...args: unknown[]) => {
      const match = args[0] as string;
      // Never redact a sentinel placeholder.
      if (match.includes(SENT_OPEN)) return match;
      bump(rule.category);
      // args = [match, ...groups, offset, string]; strip offset+string.
      const groups = args.slice(1, -2) as string[];
      return rule.replace(match, ...groups);
    });
  }

  // Username literals: redact the bare OS username wherever it remains (it may
  // sit outside any `/Users/` path — file-owner columns, slugs, prose). Done
  // last so the path/slug rules above shape the common cases first.
  for (const name of options.usernames ?? []) {
    if (!name) continue;
    const re = new RegExp(escapeRegExp(name), "g");
    working = working.replace(re, (m) => {
      if (m.includes(SENT_OPEN)) return m;
      bump("username");
      return "[REDACTED:home]";
    });
  }

  const value = unmaskAllowlist(working, originals);
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  return { value, report: { counts, total } };
}

/** Heuristically collect OS usernames referenced anywhere in `text`, from
 * `/Users/<name>` and the slug form `-Users-<name>-`. Used to auto-redact the
 * bare username elsewhere. Excludes generic `Shared`. */
function collectUsernames(text: string): Set<string> {
  const names = new Set<string>();
  for (const m of text.matchAll(/\/Users\/([A-Za-z0-9._-]+)/g)) {
    if (m[1] && m[1] !== "Shared") names.add(m[1]);
  }
  for (const m of text.matchAll(/-Users-([A-Za-z0-9._]+?)-/g)) {
    if (m[1] && m[1] !== "Shared") names.add(m[1]);
  }
  return names;
}

// ---------------------------------------------------------------------------
// Deep value redactor (walks a whole JSON value, e.g. an ATIF Trajectory).
// ---------------------------------------------------------------------------

type Json = unknown;

function mergeReports(into: Record<string, number>, from: RedactionReport): void {
  for (const [cat, n] of Object.entries(from.counts)) {
    into[cat] = (into[cat] ?? 0) + n;
  }
}

/** Deep-redact any JSON value. Strings are scrubbed via `redactString`;
 * numbers/booleans/null pass through (so ATIF `*_tokens` numerics are never
 * touched); arrays and objects are walked recursively. Object KEYS are NOT
 * rewritten (ATIF field names are structural). Deterministic.
 *
 * A first pass auto-collects OS usernames from `/Users/<name>` / `-Users-<name>-`
 * anywhere in the value, then redacts those bare usernames everywhere (covers
 * `ls -l` owner columns and slugs). Pass `options.usernames` to force more. */
export function redactValue<T extends Json>(value: T, options: RedactOptions = {}): RedactionResult<T> {
  const counts: Record<string, number> = {};

  // Pre-scan: gather usernames across the WHOLE value so the bare username is
  // redacted even in strings that never contain a `/Users/` path themselves.
  const usernames = new Set<string>(options.usernames ?? []);
  const scan = (v: Json): void => {
    if (typeof v === "string") {
      for (const n of collectUsernames(v)) usernames.add(n);
    } else if (Array.isArray(v)) {
      v.forEach(scan);
    } else if (v !== null && typeof v === "object") {
      for (const child of Object.values(v as Record<string, Json>)) scan(child);
    }
  };
  scan(value);
  const opts: RedactOptions = { usernames: Array.from(usernames) };

  const walk = (v: Json): Json => {
    if (typeof v === "string") {
      const r = redactString(v, opts);
      mergeReports(counts, r.report);
      return r.value;
    }
    if (Array.isArray(v)) return v.map(walk);
    if (v !== null && typeof v === "object") {
      const out: Record<string, Json> = {};
      for (const [k, child] of Object.entries(v as Record<string, Json>)) {
        out[k] = walk(child);
      }
      return out;
    }
    return v; // number | boolean | null | undefined
  };

  const redacted = walk(value) as T;
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  return { value: redacted, report: { counts, total } };
}

// ---------------------------------------------------------------------------
// The Effect service.
// ---------------------------------------------------------------------------

/**
 * `TraceRedactor` — a reusable Effect service that redacts sensitive content
 * from traces before storage/publication. Deterministic; the default layer wraps
 * the pure engine above. Inject `TraceRedactor.Default` and `yield* TraceRedactor`
 * to use `redact` / `redactString`.
 */
export class TraceRedactor extends Context.Service<
  TraceRedactor,
  {
    /** Deep-redact any JSON value (e.g. a whole ATIF trajectory). */
    readonly redact: <T>(value: T, options?: RedactOptions) => Effect.Effect<RedactionResult<T>>;
    /** Redact a single string. */
    readonly redactString: (
      text: string,
      options?: RedactOptions,
    ) => Effect.Effect<RedactionResult<string>>;
  }
>()("@openagents/qa-runner/TraceRedactor") {
  /** Default layer: the pure, deterministic engine. */
  static readonly Default = Layer.succeed(TraceRedactor, {
    redact: <T>(value: T, options?: RedactOptions) => Effect.sync(() => redactValue(value, options)),
    redactString: (text: string, options?: RedactOptions) =>
      Effect.sync(() => redactString(text, options)),
  });
}
