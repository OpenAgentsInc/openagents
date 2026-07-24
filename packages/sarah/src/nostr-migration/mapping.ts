/**
 * Stable identity mapping between Khala Sync thread refs and Nostr conversation
 * tags (SARAH-NR-00 §4.2 / SARAH-NR-08).
 *
 * | Legacy reference           | Nostr conversation tag value |
 * | thread.sarah.<digest>      | sarah.<digest>               |
 *
 * The raw owner id never enters a tag, event, or export.
 */

const DIGEST_RE = /^[0-9a-f]{24}$/;
const THREAD_RE = /^thread\.sarah\.([0-9a-f]{24})$/;
const CONVERSATION_RE = /^sarah\.([0-9a-f]{24})$/;

export const isSarahThreadRef = (value: string): boolean =>
  THREAD_RE.test(value);

export const isSarahConversationTag = (value: string): boolean =>
  CONVERSATION_RE.test(value);

/** Extract the 24-hex digest from either form, or null. */
export const extractSarahDigest = (value: string): string | null => {
  const thread = THREAD_RE.exec(value);
  if (thread?.[1] !== undefined) return thread[1];
  const conversation = CONVERSATION_RE.exec(value);
  if (conversation?.[1] !== undefined) return conversation[1];
  if (DIGEST_RE.test(value)) return value;
  return null;
};

/**
 * Map `thread.sarah.<digest>` → `sarah.<digest>`.
 * Returns null when the input is not a Sarah owner thread ref.
 */
export const conversationTagFromThreadRef = (
  threadRef: string,
): string | null => {
  const match = THREAD_RE.exec(threadRef);
  if (match?.[1] === undefined) return null;
  return `sarah.${match[1]}`;
};

/**
 * Map `sarah.<digest>` → `thread.sarah.<digest>`.
 * Returns null when the input is not a Sarah conversation tag.
 */
export const threadRefFromConversationTag = (
  conversationTag: string,
): string | null => {
  const match = CONVERSATION_RE.exec(conversationTag);
  if (match?.[1] === undefined) return null;
  return `thread.sarah.${match[1]}`;
};

/**
 * Resolve both forms from either input. Prefer the supplied form and derive
 * the other. Throws when the input is neither form.
 */
export const resolveSarahConversationIdentity = (
  threadRefOrConversationTag: string,
): {
  readonly threadRef: string;
  readonly conversation: string;
  readonly digest: string;
} => {
  const digest = extractSarahDigest(threadRefOrConversationTag);
  if (digest === null) {
    throw new Error(
      "sarah_nostr_migration: expected thread.sarah.<24 hex> or sarah.<24 hex>",
    );
  }
  return {
    threadRef: `thread.sarah.${digest}`,
    conversation: `sarah.${digest}`,
    digest,
  };
};
