// Vendored from packages/atif/src/redaction.ts by scripts/vendor-memory.mjs — do not edit here.
// The drift guard (test/vendored-memory-drift.test.ts) fails when this copy
// no longer matches the canonical source.
import { Context, Effect, Layer } from "effect";

import { BIP39_ENGLISH_WORDS } from "./bip39-wordlist.js";

export type RedactionCategory =
  | "private_key"
  | "mnemonic"
  | "jwt"
  | "bearer"
  | "provider_key"
  | "oa_agent_token"
  | "x_code"
  | "oa_token"
  | "machine_token"
  | "aws_key"
  | "google_key"
  | "slack_token"
  | "github_token"
  | "gitlab_token"
  | "owner_id"
  | "env_secret"
  | "wallet_or_payment"
  | "secrets_path"
  | "home_path"
  | "file_url"
  | "email"
  | "phone"
  | "ssn"
  | "date_of_birth"
  | "medical_record_id"
  | "ip"
  | "long_blob"
  | "username";

export type RedactOptions = Readonly<{
  usernames?: ReadonlyArray<string>;
}>;

export type RedactionSurface = "corpus_ingestion" | "trace_capture";

export type RegulatedVertical = "legal" | "health" | "other_regulated";

export type ExternalInferenceRedactionOptions = RedactOptions &
  Readonly<{
    surface: RedactionSurface;
    regulatedVertical?: RegulatedVertical;
  }>;

export type RedactionReport = Readonly<{
  counts: Readonly<Record<string, number>>;
  total: number;
}>;

export type RedactionResult<T> = Readonly<{
  value: T;
  report: RedactionReport;
}>;

export type ExternalInferenceRedactionResult<T> = RedactionResult<T> &
  Readonly<{
    policy: Readonly<{
      serviceRef: typeof REDACTION_SERVICE_REF;
      surface: RedactionSurface;
      regulatedVertical?: RegulatedVertical;
      appliedBeforeExternalInference: true;
    }>;
    safeForExternalInference: true;
  }>;

export type TraceRedactionCategory = RedactionCategory;
export type TraceRedactionReport = RedactionReport;
export type TraceRedactionResult<T> = RedactionResult<T>;

export type TraceRedactorShape = Readonly<{
  redact: <T>(value: T, options?: RedactOptions) => Effect.Effect<RedactionResult<T>>;
  redactString: (text: string, options?: RedactOptions) => Effect.Effect<RedactionResult<string>>;
  redactText: (text: string, options?: RedactOptions) => Effect.Effect<RedactionResult<string>>;
  redactForExternalInference: <T>(
    value: T,
    options: ExternalInferenceRedactionOptions,
  ) => Effect.Effect<ExternalInferenceRedactionResult<T>>;
  redactTextForExternalInference: (
    text: string,
    options: ExternalInferenceRedactionOptions,
  ) => Effect.Effect<ExternalInferenceRedactionResult<string>>;
  redactTrajectory: <T>(
    trajectory: T,
    options?: RedactOptions,
  ) => Effect.Effect<RedactionResult<T>>;
}>;

export const REDACTION_SERVICE_REF = "@openagentsinc/atif/redaction";

const ALLOWLIST_EXACT: ReadonlyArray<string> = ["openagents/khala"];

const ALLOWLIST_PATTERNS: ReadonlyArray<RegExp> = [
  /https?:\/\/openagents\.com\/[^\s"'`)<>]*/g,
  /https?:\/\/(?:www\.)?github\.com\/OpenAgentsInc\/[^\s"'`)<>]*/g,
  /#\d{1,6}\b/g,
];

const SENT_OPEN = "\uE000";
const SENT_CLOSE = "\uE001";

const escapeRegExp = (s: string): string => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const tag = (cat: RedactionCategory): string => `[REDACTED:${cat}]`;

const maskAllowlist = (text: string): { masked: string; originals: Array<string> } => {
  const originals: Array<string> = [];
  let masked = text;
  const stash = (m: string): string => {
    const idx = originals.length;
    originals.push(m);
    return `${SENT_OPEN}${idx}${SENT_CLOSE}`;
  };

  for (const exact of ALLOWLIST_EXACT) {
    masked = masked.replace(new RegExp(escapeRegExp(exact), "g"), (m) => stash(m));
  }
  for (const re of ALLOWLIST_PATTERNS) {
    re.lastIndex = 0;
    masked = masked.replace(re, (m) => stash(m));
  }

  return { masked, originals };
};

const unmaskAllowlist = (masked: string, originals: ReadonlyArray<string>): string =>
  masked.replace(
    new RegExp(`${SENT_OPEN}(\\d+)${SENT_CLOSE}`, "g"),
    (_m, idx: string) => originals[Number(idx)] ?? "",
  );

/**
 * One redaction rule. `replace` returns the substitution, or the match unchanged
 * to decline — a decline is not counted, which is what lets the mnemonic rule
 * gate a shape match against the BIP39 wordlist.
 *
 * Exported because this list is the authoritative one. Every other redaction
 * path in the repo consumes it rather than restating the patterns; restating
 * them is what let `oa_pat_` and `smct_` leak past a redaction that reported
 * success.
 */
export type Rule = Readonly<{
  category: RedactionCategory;
  pattern: RegExp;
  replace: (match: string, ...groups: Array<string>) => string;
}>;

// A candidate BIP39 seed phrase is a run of 12/15/18/21/24 lowercase words.
// This regex only FINDS candidates cheaply; `mnemonicReplace` then confirms
// every word is an actual BIP39 word before redacting, so ordinary English
// prose (which is full of non-wordlist words like "the", "roadmap", "ide") is
// left intact. Real seed phrases are all-wordlist by definition and still redact.
const MNEMONIC = /\b(?:[a-z]{3,8} ){11}[a-z]{3,8}(?:(?: [a-z]{3,8}){3})*\b/g;

// Shortest real BIP39 mnemonic. Runs of consecutive wordlist words below this
// length are treated as coincidental prose, not a seed phrase.
const MIN_MNEMONIC_WORDS = 12;

/**
 * Redact only the ACTUAL seed phrase inside a shape-matched candidate: the
 * longest run of CONSECUTIVE BIP39 English words. If that run is at least a
 * 12-word mnemonic it is replaced with the tag while any surrounding prose words
 * (which are not in the wordlist) are preserved; otherwise the whole match is
 * returned unchanged. This keeps genuine seed phrases fully redacted — even when
 * they sit next to ordinary words — without redacting prose that merely happens
 * to be a run of short lowercase words.
 */
const mnemonicReplace = (match: string): string => {
  const words = match.split(" ");
  let bestStart = -1;
  let bestLen = 0;
  let curStart = 0;
  let curLen = 0;
  for (let i = 0; i < words.length; i += 1) {
    if (BIP39_ENGLISH_WORDS.has(words[i] as string)) {
      if (curLen === 0) curStart = i;
      curLen += 1;
      if (curLen > bestLen) {
        bestLen = curLen;
        bestStart = curStart;
      }
    } else {
      curLen = 0;
    }
  }
  if (bestLen < MIN_MNEMONIC_WORDS) return match;
  const before = words.slice(0, bestStart).join(" ");
  const after = words.slice(bestStart + bestLen).join(" ");
  return [before, tag("mnemonic"), after].filter((part) => part !== "").join(" ");
};

const RULES: ReadonlyArray<Rule> = [
  {
    category: "private_key",
    pattern: /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----/g,
    replace: () => tag("private_key"),
  },
  {
    // Bech32 and base58 SECRET keys. `wallet_or_payment` below carries the
    // matching PUBLIC forms (`xpub`, `bc1`, an invoice); these are the spend
    // and signing halves, so they run first and are their own category.
    // `npub` is deliberately absent: it is the public name.
    category: "private_key",
    pattern:
      /\b(?:nsec1[02-9ac-hj-np-z]{50,}|(?:xprv|yprv|zprv|tprv|uprv|vprv)[1-9A-HJ-NP-Za-km-z]{50,})\b/g,
    replace: () => tag("private_key"),
  },
  { category: "mnemonic", pattern: MNEMONIC, replace: mnemonicReplace },
  {
    category: "jwt",
    pattern: /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{6,}\b/g,
    replace: () => tag("jwt"),
  },
  {
    category: "wallet_or_payment",
    pattern:
      /\b(?:lnbc[0-9][a-z0-9]{20,}|lntb[0-9][a-z0-9]{20,}|lno1[a-z0-9]{20,}|bc1[a-z0-9]{20,}|(?:xpub|ypub|zpub|tpub)[1-9A-HJ-NP-Za-km-z]{20,})\b/gi,
    replace: () => tag("wallet_or_payment"),
  },
  {
    category: "aws_key",
    pattern: /\bAKIA[0-9A-Z]{16}\b/g,
    replace: () => tag("aws_key"),
  },
  {
    category: "google_key",
    pattern: /\bAIza[0-9A-Za-z_-]{20,}\b/g,
    replace: () => tag("google_key"),
  },
  {
    category: "slack_token",
    pattern: /\bxox[baprs]-[A-Za-z0-9-]{8,}\b/g,
    replace: () => tag("slack_token"),
  },
  {
    // `github_pat_` first: the general `gh[pousr]_` shape does not reach it,
    // because a fine-grained PAT spells the prefix out in full.
    category: "github_token",
    pattern: /\bgithub_pat_[A-Za-z0-9_]{20,}\b/g,
    replace: () => tag("github_token"),
  },
  {
    category: "github_token",
    pattern: /\bgh[pousr]_[A-Za-z0-9]{16,}\b/g,
    replace: () => tag("github_token"),
  },
  {
    category: "gitlab_token",
    pattern: /\bglpat-[A-Za-z0-9_-]{16,}\b/g,
    replace: () => tag("gitlab_token"),
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
    pattern:
      /\b(?:sk-(?:or-|proj-|ant-)?[A-Za-z0-9_-]{8,}|(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{8,})\b/g,
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
    pattern: /\boa_(?:live|test|sk|key|secret|tok|token|pat)?_?[A-Za-z0-9]{12,}\b/g,
    replace: () => tag("oa_token"),
  },
  {
    // Machine tokens minted by computer pairing. They carry a hyphen, which
    // every rule above stops at, so they survived an ATIF export intact.
    category: "machine_token",
    pattern: /\bsmct_[A-Za-z0-9_-]{6,}\b/g,
    replace: () => tag("machine_token"),
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
    category: "file_url",
    pattern: /\bfile:\/\/[^\s"'`)<>]*/g,
    replace: () => tag("file_url"),
  },
  {
    category: "home_path",
    pattern: /\/Users\/[^\s"'`)<>]*/g,
    replace: () => tag("home_path"),
  },
  {
    category: "home_path",
    pattern: /\/home\/[^\s"'`)<>]*/g,
    replace: () => tag("home_path"),
  },
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
    category: "ssn",
    pattern: /\b(?:SSN|social security(?: number)?)\s*[:#=]?\s*\d{3}-\d{2}-\d{4}\b/gi,
    replace: () => `SSN ${tag("ssn")}`,
  },
  {
    category: "date_of_birth",
    pattern:
      /\b(?:DOB|date of birth|birth date)\s*[:#=]?\s*(?:\d{1,2}[/-]\d{1,2}[/-]\d{2,4}|[A-Z][a-z]{2,8}\s+\d{1,2},\s+\d{4})\b/g,
    replace: () => `DOB ${tag("date_of_birth")}`,
  },
  {
    category: "medical_record_id",
    pattern: /\b(?:MRN|medical record(?: number)?|patient id)\s*[:#=]?\s*[A-Za-z0-9-]{6,}\b/gi,
    replace: () => `MRN ${tag("medical_record_id")}`,
  },
  {
    category: "phone",
    pattern:
      /(^|[^\dA-Za-z])(?:\+?1[-.\s]?)?(?:\([2-9]\d{2}\)|[2-9]\d{2})[-.\s]?[2-9]\d{2}[-.\s]?\d{4}\b/g,
    replace: (_m, prefix: string) => `${prefix}${tag("phone")}`,
  },
  {
    category: "ip",
    pattern:
      /\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|100\.\d{1,3}\.\d{1,3}\.\d{1,3})\b/g,
    replace: () => tag("ip"),
  },
  {
    category: "long_blob",
    pattern: /\b[A-Fa-f0-9]{40,}\b/g,
    replace: () => tag("long_blob"),
  },
  {
    // A contiguous base64-ish blob. `/` is intentionally EXCLUDED from the class:
    // it made slash-separated PROSE (e.g. "candidate/shadow/released/active" or a
    // "schema/service/IPC/process/PTY" enum list) match as a fake blob. Real
    // base64 secrets are caught by the specific secret rules above and the
    // server tripwire; this catch-all only needs contiguous non-slash runs.
    category: "long_blob",
    pattern: /\b[A-Za-z0-9+]{48,}={0,2}\b/g,
    replace: () => tag("long_blob"),
  },
];

/**
 * How each category is classified for the purposes of the CLI redaction floor.
 *
 * `"credential"` means the match is key material or an access token: something
 * that grants access if it escapes. Every credential category is a floor that
 * the `openagents trace redact` path must also cover, and
 * `test/redaction-parity.test.ts` fails when one of them has no coverage there.
 *
 * `"other"` means the match is a location, an identifier, or a PII/PHI shape.
 * Those matter for a corpus export but are not access-granting, and the trace
 * path deliberately treats some of them differently -- it rewrites a home path
 * to `~` rather than to a tag, and it keeps `long_blob` off so a public `npub`
 * survives a redaction.
 *
 * This map is exhaustive by construction: `Record<RedactionCategory, ...>` will
 * not compile once a new category is added and left unclassified, which is the
 * first link in the chain that stops the two rule lists from drifting again.
 */
export const REDACTION_CATEGORY_CLASS: Record<RedactionCategory, "credential" | "other"> = {
  private_key: "credential",
  mnemonic: "credential",
  jwt: "credential",
  bearer: "credential",
  provider_key: "credential",
  oa_agent_token: "credential",
  x_code: "credential",
  oa_token: "credential",
  machine_token: "credential",
  aws_key: "credential",
  google_key: "credential",
  slack_token: "credential",
  github_token: "credential",
  gitlab_token: "credential",
  env_secret: "credential",
  wallet_or_payment: "credential",
  secrets_path: "credential",
  owner_id: "other",
  home_path: "other",
  file_url: "other",
  email: "other",
  phone: "other",
  ssn: "other",
  date_of_birth: "other",
  medical_record_id: "other",
  ip: "other",
  long_blob: "other",
  username: "other",
};

/** True when a category's match is key material or an access token. */
export const isCredentialCategory = (category: RedactionCategory): boolean =>
  REDACTION_CATEGORY_CLASS[category] === "credential";

/**
 * Every rule this module applies, in the order it applies them.
 *
 * This is the authoritative list. Consumers import it instead of restating the
 * patterns: `oa_pat_` and `smct_` both leaked because a second hand-written
 * list forgot them, and forgetting produced no error -- it produced a redaction
 * that reported success over a file full of live tokens.
 */
export const atifRedactionRules: ReadonlyArray<Rule> = RULES;

/**
 * The subset of {@link atifRedactionRules} whose matches are credentials.
 *
 * This is what the `openagents trace redact` path folds in on top of its own
 * trace-specific rules, so a token family added here is removed there too.
 */
export const atifCredentialRules: ReadonlyArray<Rule> = RULES.filter((rule) =>
  isCredentialCategory(rule.category),
);

const collectUsernames = (text: string): Set<string> => {
  const names = new Set<string>();
  for (const m of text.matchAll(/\/Users\/([A-Za-z0-9._-]+)/g)) {
    if (m[1] && m[1] !== "Shared") {
      names.add(m[1]);
    }
  }
  for (const m of text.matchAll(/\/home\/([A-Za-z0-9._-]+)/g)) {
    if (m[1]) {
      names.add(m[1]);
    }
  }
  for (const m of text.matchAll(/-Users-([A-Za-z0-9._]+?)-/g)) {
    if (m[1] && m[1] !== "Shared") {
      names.add(m[1]);
    }
  }
  return names;
};

const mergeReports = (into: Record<string, number>, from: RedactionReport): void => {
  for (const [cat, n] of Object.entries(from.counts)) {
    into[cat] = (into[cat] ?? 0) + n;
  }
};

export const redactString = (
  input: string,
  options: RedactOptions = {},
): RedactionResult<string> => {
  const counts: Record<string, number> = {};
  const bump = (cat: RedactionCategory): void => {
    counts[cat] = (counts[cat] ?? 0) + 1;
  };

  const { masked, originals } = maskAllowlist(input);
  let working = masked;

  for (const rule of RULES) {
    rule.pattern.lastIndex = 0;
    working = working.replace(rule.pattern, (...args: Array<unknown>) => {
      const match = args[0] as string;
      if (match.includes(SENT_OPEN)) {
        return match;
      }
      const groups = args.slice(1, -2) as Array<string>;
      const replaced = rule.replace(match, ...groups);
      // Only count a real redaction. A rule whose `replace` returns the match
      // unchanged (e.g. the mnemonic wordlist gate rejecting prose) is a no-op
      // and must not inflate the report.
      if (replaced !== match) {
        bump(rule.category);
      }
      return replaced;
    });
  }

  for (const name of options.usernames ?? []) {
    if (name === "") {
      continue;
    }
    const re = new RegExp(escapeRegExp(name), "g");
    working = working.replace(re, (m) => {
      if (m.includes(SENT_OPEN)) {
        return m;
      }
      bump("username");
      return "[REDACTED:home]";
    });
  }

  const value = unmaskAllowlist(working, originals);
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  return { value, report: { counts, total } };
};

export const redactTraceString = redactString;

type Json = unknown;

export const redactValue = <T extends Json>(
  value: T,
  options: RedactOptions = {},
): RedactionResult<T> => {
  const counts: Record<string, number> = {};
  const usernames = new Set<string>(options.usernames ?? []);

  const scan = (v: Json): void => {
    if (typeof v === "string") {
      for (const name of collectUsernames(v)) {
        usernames.add(name);
      }
      return;
    }
    if (Array.isArray(v)) {
      v.forEach(scan);
      return;
    }
    if (v !== null && typeof v === "object") {
      Object.values(v as Record<string, Json>).forEach(scan);
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
    if (Array.isArray(v)) {
      return v.map(walk);
    }
    if (v !== null && typeof v === "object") {
      const out: Record<string, Json> = {};
      for (const [k, child] of Object.entries(v as Record<string, Json>)) {
        out[k] = walk(child);
      }
      return out;
    }
    return v;
  };

  const redacted = walk(value) as T;
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  return { value: redacted, report: { counts, total } };
};

export const redactTraceValue = redactValue;

const externalInferencePolicy = (
  options: ExternalInferenceRedactionOptions,
): ExternalInferenceRedactionResult<unknown>["policy"] => ({
  serviceRef: REDACTION_SERVICE_REF,
  surface: options.surface,
  ...(options.regulatedVertical === undefined
    ? {}
    : { regulatedVertical: options.regulatedVertical }),
  appliedBeforeExternalInference: true,
});

export const redactForExternalInference = <T extends Json>(
  value: T,
  options: ExternalInferenceRedactionOptions,
): ExternalInferenceRedactionResult<T> => {
  const { surface: _surface, regulatedVertical: _regulatedVertical, ...redactOptions } = options;
  const redacted = redactValue(value, redactOptions);
  return {
    ...redacted,
    policy: externalInferencePolicy(options),
    safeForExternalInference: true,
  };
};

export const redactStringForExternalInference = (
  text: string,
  options: ExternalInferenceRedactionOptions,
): ExternalInferenceRedactionResult<string> => {
  const { surface: _surface, regulatedVertical: _regulatedVertical, ...redactOptions } = options;
  const redacted = redactString(text, redactOptions);
  return {
    ...redacted,
    policy: externalInferencePolicy(options),
    safeForExternalInference: true,
  };
};

export const makeTraceRedactor = (): TraceRedactorShape => ({
  redact: (value, options) => Effect.sync(() => redactValue(value, options)),
  redactString: (text, options) => Effect.sync(() => redactString(text, options)),
  redactText: (text, options) => Effect.sync(() => redactString(text, options)),
  redactForExternalInference: (value, options) =>
    Effect.sync(() => redactForExternalInference(value, options)),
  redactTextForExternalInference: (text, options) =>
    Effect.sync(() => redactStringForExternalInference(text, options)),
  redactTrajectory: (trajectory, options) => Effect.sync(() => redactValue(trajectory, options)),
});

export class TraceRedactor extends Context.Service<TraceRedactor, TraceRedactorShape>()(
  "@openagentsinc/atif/TraceRedactor",
) {
  static readonly Default = Layer.succeed(TraceRedactor, makeTraceRedactor());
}

export const TraceRedactorLive = TraceRedactor.Default;
