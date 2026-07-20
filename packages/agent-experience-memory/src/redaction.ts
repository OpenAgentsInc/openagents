import { redactString } from "@openagentsinc/atif/redaction";

/**
 * The single redaction boundary for owner-local memory.
 *
 * Memory reuses the existing ATIF redaction service (`@openagentsinc/atif`)
 * rather than a parallel scrubber, so one rule set governs traces and memory.
 * Redaction runs BEFORE storage and AGAIN before a recalled slice enters a
 * prompt (defense in depth). A secret, wallet or payment value, local path,
 * token, or email is scrubbed in both directions, so it never enters a stored
 * record and never leaves in a recall result.
 *
 * The redaction categories that mark a value UNSAFE to store at all — a value
 * that must be rejected, not merely scrubbed — are the hard categories below.
 * These match the ATIF trace tripwire finding classes.
 */
export const HARD_UNSAFE_CATEGORIES = [
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
] as const;

export type MemorySafetyVerdict = Readonly<{
  /** The scrubbed text. Always safe to store or return. */
  redacted: string;
  /** True when the input needed no redaction at all. */
  clean: boolean;
  /** True when the input carried no hard-unsafe (secret / wallet / path) material. */
  storable: boolean;
  /** The redaction categories that fired. */
  categories: ReadonlyArray<string>;
  /** The total number of redactions applied. */
  total: number;
}>;

/**
 * Guard a candidate memory string. Returns the scrubbed text plus a verdict.
 *
 * `storable` is false when the original text carried hard-unsafe material
 * (secrets, wallet or payment material, local paths). The store rejects those
 * records outright; it never keeps even a scrubbed shell of a secret-bearing
 * fact. Soft categories (for example an email) are scrubbed but do not block
 * storage of the redacted fact.
 */
export const guardMemoryText = (
  input: string,
  usernames?: ReadonlyArray<string>,
): MemorySafetyVerdict => {
  const { value, report } = redactString(input, usernames ? { usernames } : {});
  const categories = Object.keys(report.counts);
  const hard = new Set<string>(HARD_UNSAFE_CATEGORIES);
  const storable = categories.every((category) => !hard.has(category));
  return {
    redacted: value,
    clean: report.total === 0,
    storable,
    categories,
    total: report.total,
  };
};

/**
 * Assert a string is free of any redactable material. Used on recall output as a
 * backstop: a recalled slice that still contains anything redactable is a defect
 * and this throws rather than returning it.
 */
export const assertRecallClean = (input: string): string => {
  const verdict = guardMemoryText(input);
  if (!verdict.clean) {
    throw new Error(
      `recall output contained redactable material (${verdict.categories.join(", ")})`,
    );
  }
  return input;
};
