/**
 * Lightweight memory-value safety gate for the Nostr engram write path.
 *
 * Full graph-memory redaction lives in `@openagentsinc/agent-experience-memory`
 * (`guardMemoryText`). This module stays free of that dependency so adapters
 * can reject secret-shaped values before encryption without a live NIP pin.
 *
 * Fail closed: reject hard-unsafe material rather than scrub into storage.
 */

export type SarahMemoryValueVerdict = Readonly<{
  /** True when the value may be stored as a durable engram. */
  storable: boolean;
  /** Categories that fired (empty when clean). */
  categories: ReadonlyArray<string>;
  /** The input when storable; undefined when rejected. */
  value?: string;
}>;

const HARD_PATTERNS: ReadonlyArray<{ category: string; re: RegExp }> = [
  { category: "private_key", re: /\bnsec1[a-z0-9]{20,}\b/i },
  {
    category: "private_key",
    re: /\b(private[_-]?key|seckey|secret[_-]?key)\b\s*[:=]\s*[0-9a-fA-F]{64}\b/i,
  },
  {
    category: "mnemonic",
    re: /\b(mnemonic|seed phrase|recovery phrase)\b/i,
  },
  {
    category: "bearer",
    re: /\b(bearer\s+[a-z0-9._\-]{20,}|authorization:\s*bearer\s+\S+)/i,
  },
  {
    category: "provider_key",
    re: /\b(sk-[a-zA-Z0-9]{20,}|xai-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{20,}|gho_[a-zA-Z0-9]{20,})\b/,
  },
  {
    category: "oa_token",
    re: /\b(OPENAGENTS_AGENT_TOKEN|oa_agent_[a-zA-Z0-9_\-]{16,})\b/,
  },
  {
    category: "wallet_or_payment",
    re: /\b(lnbc[0-9a-z]+|bc1[a-z0-9]{25,}|npub1[a-z0-9]{20,})\b/i,
  },
  {
    category: "home_path",
    re: /(?:^|[\s"'`])(\/Users\/[^\s"'`]+|\/home\/[^\s"'`]+|~\/\.[^\s"'`]+)/,
  },
  {
    category: "secrets_path",
    re: /\.secrets\/|\.env\b|auth\.json\b/i,
  },
  {
    category: "jwt",
    re: /\beyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\b/,
  },
];

/**
 * Guard a candidate memory value before encryption.
 * Hard-unsafe material is rejected outright (not scrubbed into a durable record).
 */
export const guardSarahMemoryValue = (input: string): SarahMemoryValueVerdict => {
  const categories: string[] = [];
  for (const { category, re } of HARD_PATTERNS) {
    if (re.test(input) && !categories.includes(category)) {
      categories.push(category);
    }
  }
  if (categories.length > 0) {
    return { storable: false, categories };
  }
  return { storable: true, categories: [], value: input };
};

/** Throw when a value must not enter an engram. */
export const assertSarahMemoryValueStorable = (input: string): string => {
  const verdict = guardSarahMemoryValue(input);
  if (!verdict.storable) {
    throw new Error(
      `sarah_nostr_memory: value rejected (${verdict.categories.join(", ")})`,
    );
  }
  return input;
};
