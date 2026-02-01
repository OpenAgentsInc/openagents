import type { NostrEvent } from "@nostrify/nostrify";
import { NBrowserSigner } from "@nostrify/nostrify";
import type { NPool } from "@nostrify/nostrify";
import { createPostTags, createReplyTags } from "@/lib/clawstr";

function buildUnsignedEvent(
  kind: 1111,
  content: string,
  tags: string[][]
): Omit<NostrEvent, "id" | "pubkey" | "sig"> {
  return {
    kind,
    content,
    tags,
    created_at: Math.floor(Date.now() / 1000),
  };
}

/**
 * Publish a top-level kind 1111 post. Uses browser extension (window.nostr) to sign.
 * @throws If extension not available or relay rejects.
 */
export async function publishPost(
  pool: NPool,
  content: string,
  subclaw: string,
  includeAILabel = true
): Promise<NostrEvent> {
  const signer = new NBrowserSigner();
  const tags = createPostTags(subclaw, includeAILabel);
  const unsigned = buildUnsignedEvent(1111, content, tags);
  const signed = await signer.signEvent(unsigned);
  await pool.event(signed);
  return signed;
}

/**
 * Publish a kind 1111 reply. Uses browser extension (window.nostr) to sign.
 * @throws If extension not available or relay rejects.
 */
export async function publishReply(
  pool: NPool,
  content: string,
  subclaw: string,
  parentEvent: NostrEvent,
  includeAILabel = true
): Promise<NostrEvent> {
  const signer = new NBrowserSigner();
  const tags = createReplyTags(subclaw, parentEvent, includeAILabel);
  const unsigned = buildUnsignedEvent(1111, content, tags);
  const signed = await signer.signEvent(unsigned);
  await pool.event(signed);
  return signed;
}

/** Check if Nostr extension (window.nostr) is available. */
export function hasNostrExtension(): boolean {
  return typeof (globalThis as { nostr?: unknown }).nostr !== "undefined";
}
