import { Schema as S } from "effect";
import { redactString } from "@openagentsinc/atif/redaction";
import { canonicalStringify } from "./internal/canonical.js";
import { sha256Hex } from "./internal/sha256.js";

/**
 * Formal NIP-AE engram schema and a strict pre-sign redaction boundary.
 *
 * This module defines the OpenAgents companion profile for NIP-AE kind 30174
 * addressable events. It treats engrams as immutable, signed records: a
 * correction never overwrites a prior engram; it appends a new engram that
 * explicitly references and supersedes the previous event id.
 *
 * Every engram value passes a strict zero-credential / zero-token redaction
 * filter before it can be signed. Tokens, private SSH/Nostr keys, private IPs,
 * and environment variables are detected, redacted, and cause the engram to be
 * rejected as hard-unsafe.
 */

/** NIP-AE addressable engram kind. */
export const ENGRAM_KIND = 30174 as const;

/** OpenAgents companion schema id written inside the engram body. */
export const COMPANION_SCHEMA_ID =
  "openagents.agent_experience_memory.nip_ae_companion.v1" as const;

/** NIP-31 alt text for engrams (NIP-AE default). */
export const ENGRAM_ALT = "encrypted agent memory record" as const;

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));

export const EngramSlug = S.String.check(S.isMinLength(1), S.isMaxLength(255));
export type EngramSlug = typeof EngramSlug.Type;

export const EngramAdmission = S.Literals(["admitted", "candidate", "rejected"]);
export type EngramAdmission = typeof EngramAdmission.Type;

export const EngramSourceRole = S.Literals([
  "turn_record",
  "tool_result",
  "owner_message",
  "import",
  "supersession",
]);
export type EngramSourceRole = typeof EngramSourceRole.Type;

export const EngramSourceEventRef = S.Struct({
  eventId: Hex64,
  role: EngramSourceRole,
});
export type EngramSourceEventRef = typeof EngramSourceEventRef.Type;

export const EngramRelationDirection = S.Literals(["out", "in", "both"]);
export type EngramRelationDirection = typeof EngramRelationDirection.Type;

export const EngramRelation = S.Struct({
  type: S.String.check(S.isMinLength(1), S.isMaxLength(64)),
  targetSlug: EngramSlug,
  direction: EngramRelationDirection,
});
export type EngramRelation = typeof EngramRelation.Type;

/** OpenAgents companion fields under the NIP-AE unknown-fields rule. */
export const EngramCompanion = S.Struct({
  schema: S.Literal(COMPANION_SCHEMA_ID),
  admission: EngramAdmission,
  entityId: S.String.check(S.isMinLength(1), S.isMaxLength(128)),
  contentDigest: S.String.check(S.isPattern(/^sha256:[0-9a-f]{64}$/)),
  sourceEventRefs: S.Array(EngramSourceEventRef),
  relations: S.Array(EngramRelation),
  derivedFromSlugs: S.Array(EngramSlug),
  /** The prior event id this engram supersedes, if any. */
  supersedes: S.optionalKey(Hex64),
});
export type EngramCompanion = typeof EngramCompanion.Type;

/** NIP-AE engram body with the OpenAgents companion profile. */
export const EngramBody = S.Struct({
  slug: EngramSlug,
  /** The plaintext value. `null` is the in-band tombstone. */
  value: S.NullOr(S.String),
  openagents: EngramCompanion,
});
export type EngramBody = typeof EngramBody.Type;

/** A Nostr-like signed engram event. */
export const EngramEvent = S.Struct({
  id: Hex64,
  pubkey: Hex64,
  created_at: S.Number,
  kind: S.Literal(ENGRAM_KIND),
  tags: S.Array(S.Array(S.String)),
  content: S.String,
  sig: S.String,
});
export type EngramEvent = typeof EngramEvent.Type;

/** A signer produces a signature over a 64-character hex event id. */
export type EngramSigner = (eventId: string) => string;

/**
 * Categories that make an engram hard-unsafe and therefore non-storable.
 * Tokens, keys, private paths, private IPs, Nostr private keys, and
 * environment variables are included. Soft PII (for example emails) is redacted
 * but does not block signing.
 */
export const ENGRAM_HARD_UNSAFE_CATEGORIES = [
  "private_key",
  "mnemonic",
  "jwt",
  "bearer",
  "provider_key",
  "oa_agent_token",
  "oa_token",
  "aws_key",
  "google_key",
  "slack_token",
  "github_token",
  "env_secret",
  "wallet_or_payment",
  "secrets_path",
  "home_path",
  "file_url",
  "ip",
  "private_ip",
  "nostr_private_key",
  "environment_variable",
] as const;

type RedactionRule = Readonly<{
  category: string;
  pattern: RegExp;
  replace: (match: string, ...groups: Array<string>) => string;
}>;

const tag = (category: string): string => `[REDACTED:${category}]`;

const EXTRA_RULES: ReadonlyArray<RedactionRule> = [
  {
    category: "nostr_private_key",
    pattern: /\bnsec1[ac-hj-np-z02-9]{50,90}\b/g,
    replace: () => tag("nostr_private_key"),
  },
  {
    category: "private_ip",
    pattern:
      /\b(?:127\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)|169\.254\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)|0\.0\.0\.0|255\.255\.255\.255)\b/g,
    replace: () => tag("private_ip"),
  },
  {
    category: "environment_variable",
    pattern: /\b(export\s+)([A-Z_][A-Z0-9_]*)(\s*=\s*)([^\s;'"`]+)/g,
    replace: (_match, exportDecl: string, name: string) =>
      `${exportDecl}${name}=${tag("environment_variable")}`,
  },
  {
    category: "environment_variable",
    pattern: /^([ \t]*[A-Z_][A-Z0-9_]*)(\s*=\s*)([^\n"']*)/gm,
    replace: (_match, name: string) => `${name}=${tag("environment_variable")}`,
  },
];

/**
 * Redact an engram value through the ATIF boundary plus engram-specific rules.
 *
 * Returns the redacted text, the categories that fired, whether the result is
 * safe to sign, and the total redaction count. A hard-unsafe category makes
 * `storable` false.
 */
export const redactEngramContent = (
  input: string,
): Readonly<{
  redacted: string;
  storable: boolean;
  categories: ReadonlyArray<string>;
  total: number;
}> => {
  const counts: Record<string, number> = {};
  const bump = (category: string): void => {
    counts[category] = (counts[category] ?? 0) + 1;
  };

  // Run the engram-specific rules first so targeted credentials (for example a
  // Nostr nsec) are caught before the ATIF long_blob heuristic swallows them.
  let working = input;
  for (const rule of EXTRA_RULES) {
    rule.pattern.lastIndex = 0;
    working = working.replace(rule.pattern, (match, ...args) => {
      const groups = args.slice(0, -2) as Array<string>;
      const replaced = rule.replace(match, ...groups);
      if (replaced !== match) {
        bump(rule.category);
      }
      return replaced;
    });
  }

  // Then run the full ATIF redaction boundary for tokens, private keys, paths,
  // payment material, and standard private IP ranges.
  const atif = redactString(working);
  for (const [category, count] of Object.entries(atif.report.counts)) {
    counts[category] = (counts[category] ?? 0) + count;
  }

  const categories = Object.keys(counts);
  const hard = new Set(ENGRAM_HARD_UNSAFE_CATEGORIES as unknown as ReadonlyArray<string>);
  const storable = categories.every((category) => !hard.has(category));
  const total = Object.values(counts).reduce((a, b) => a + b, 0);

  return { redacted: atif.value, storable, categories, total };
};

/** Pre-sign redaction verdict for an engram value. */
export type EngramContentVerdict = Readonly<{
  /** The redacted value. `null` for a tombstone. */
  redacted: string | null;
  storable: boolean;
  categories: ReadonlyArray<string>;
  total: number;
}>;

/**
 * Guard a candidate engram value. Tombstones are always safe. Any other value
 * is rejected as non-storable when it contains a hard-unsafe category.
 */
export const guardEngramContent = (value: string | null): EngramContentVerdict => {
  if (value === null) {
    return { redacted: null, storable: true, categories: [], total: 0 };
  }
  const { redacted, storable, categories, total } = redactEngramContent(value);
  return { redacted, storable, categories, total };
};

/**
 * Build a validated engram body. The schema id is always the canonical
 * companion id; callers do not need to supply it.
 */
export const buildEngramBody = (
  slug: EngramSlug,
  value: string | null,
  companion: Omit<EngramCompanion, "schema">,
): EngramBody =>
  S.decodeUnknownSync(EngramBody)({
    slug,
    value,
    openagents: { ...companion, schema: COMPANION_SCHEMA_ID },
  });

/** SHA-256 content digest over the canonicalized engram value. */
export const engramContentDigest = (value: string | null): string =>
  `sha256:${sha256Hex(canonicalStringify({ value }))}`;

/**
 * Build a companion body that supersedes a prior engram. The prior engram is
 * not modified; the new body carries the corrected value and a reference to the
 * prior event id.
 */
export const buildSupersedingBody = (
  prior: EngramBody,
  newValue: string | null,
  priorEventId: string,
  contentDigest?: string,
): EngramBody => {
  const { schema: _, ...companion } = prior.openagents;
  return buildEngramBody(
    prior.slug,
    newValue,
    {
      ...companion,
      contentDigest: contentDigest ?? engramContentDigest(newValue),
      supersedes: priorEventId,
      sourceEventRefs: [
        ...companion.sourceEventRefs,
        { eventId: priorEventId, role: "supersession" },
      ],
    },
  );
};

/**
 * Compute the NIP-01-style event id for an unsigned engram event: the SHA-256
 * digest of the canonicalized [0, pubkey, created_at, kind, tags, content]
 * tuple.
 */
export const computeEngramEventId = (
  event: Pick<EngramEvent, "pubkey" | "created_at" | "kind" | "tags" | "content">,
): string =>
  sha256Hex(
    canonicalStringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]),
  );

/**
 * Sign an engram event. The id is derived deterministically from the event
 * fields; the supplied signer produces the signature over that id.
 */
export const signEngramEvent = (
  event: Pick<EngramEvent, "pubkey" | "created_at" | "kind" | "tags" | "content">,
  sign: EngramSigner,
): EngramEvent => {
  const id = computeEngramEventId(event);
  const sig = sign(id);
  return S.decodeUnknownSync(EngramEvent)({ ...event, id, sig });
};

/**
 * Build and sign a fresh NIP-AE engram event. The `dTag` is the blinded
 * address; the `alt` tag is set to the canonical engram alt text.
 */
export const buildEngramEvent = (
  pubkey: string,
  created_at: number,
  dTag: string,
  content: string,
  sign: EngramSigner,
): EngramEvent =>
  signEngramEvent(
    {
      pubkey,
      created_at,
      kind: ENGRAM_KIND,
      tags: [
        ["d", dTag],
        ["alt", ENGRAM_ALT],
      ],
      content,
    },
    sign,
  );

/**
 * Build and sign a superseding engram. The new event reuses the prior `d` tag,
 * carries a greater `created_at`, and records `supersedes: prior.id` in its
 * companion body. The prior event is left untouched.
 */
export const signSupersedingEngram = (
  prior: EngramEvent,
  newValue: string | null,
  created_at: number,
  pubkey: string,
  sign: EngramSigner,
): EngramEvent => {
  if (created_at <= prior.created_at) {
    throw new Error("superseding engram must have a greater created_at");
  }
  const dTag = prior.tags.find((tag) => tag[0] === "d")?.[1];
  if (dTag === undefined) {
    throw new Error("prior engram is missing a d tag");
  }
  const priorBody = S.decodeUnknownSync(EngramBody)(JSON.parse(prior.content));
  const newContent = JSON.stringify(buildSupersedingBody(priorBody, newValue, prior.id));
  return buildEngramEvent(pubkey, created_at, dTag, newContent, sign);
};

/** Verify that an event id matches the canonical content digest of its fields. */
export const verifyEngramEventId = (event: EngramEvent): boolean =>
  event.id === computeEngramEventId(event);

/**
 * Verify a chain of superseding engrams. Each event id must be valid, each
 * `created_at` must be strictly increasing, and each event's companion body
 * must explicitly reference the previous event id in `supersedes`.
 */
export const verifySupersessionChain = (events: ReadonlyArray<EngramEvent>): boolean => {
  for (let i = 0; i < events.length; i += 1) {
    const event = events[i];
    if (!verifyEngramEventId(event)) {
      return false;
    }
    if (i === 0) {
      continue;
    }
    const prior = events[i - 1]!;
    if (event.created_at <= prior.created_at) {
      return false;
    }
    let body: EngramBody;
    try {
      body = S.decodeUnknownSync(EngramBody)(JSON.parse(event.content));
    } catch {
      return false;
    }
    if (body.openagents.supersedes !== prior.id) {
      return false;
    }
  }
  return true;
};
