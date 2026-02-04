import { NBrowserSigner } from '@nostrify/nostrify';
import type { NPool, NostrEvent } from '@nostrify/nostrify';

type Tag = Array<string>;
type Tags = Array<Tag>;

function buildUnsignedEvent(
  kind: 7,
  content: string,
  tags: Tags,
): Omit<NostrEvent, 'id' | 'pubkey' | 'sig'> {
  return {
    kind,
    content,
    tags,
    created_at: Math.floor(Date.now() / 1000),
  };
}

/**
 * Publish a NIP-25 reaction (kind 7) to a target event.
 * content: "+" for upvote, "-" for downvote (or emoji).
 */
export async function publishReaction(
  pool: NPool,
  target: { id: string; pubkey: string },
  content: string,
): Promise<NostrEvent> {
  const signer = new NBrowserSigner();
  const tags: Tags = [
    ['e', target.id],
    ['p', target.pubkey],
  ];
  const unsigned = buildUnsignedEvent(7, content, tags);
  const signed = await signer.signEvent(unsigned);
  await pool.event(signed);
  return signed;
}
