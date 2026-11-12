# Nostr Lab

An unlisted learning world at https://openagents.com/worlds/nostr-lab, made
for Episode 295, *Nostr From Scratch*. You walk your Everglade character down
a lit lane on the Grid. Four stations stand beside it, in order:

1. **Key pair**: a secret key's 32 bytes, never shown, and the public key it
   makes, as hex and as an `npub`.
2. **An event**: a kind 1 note taking shape field by field: `pubkey`,
   `created_at`, `kind`, `tags` and `content`.
3. **The ID**: the event serialized as NIP-01's
   `[0, pubkey, created_at, kind, tags, content]`, those bytes into SHA-256,
   and the 32-byte event ID out.
4. **The signature**: the BIP-340 signature over the ID, and its check. Then
   one letter of the content changes: the bytes hash to a different ID, and
   the same signature fails.

Every key, byte, hash and signature is computed in Coax as the page runs, by
the Nostr library (`coax/lib/nostr/`) and the shared crypto. The secret key
is BIP-340's published test vector 1, the key the book's examples sign with;
it is not a real key and the world never shows it.

Read more in [Nostr From Scratch](https://github.com/OpenAgentsInc/openagents/blob/main/nostr/book/README.md):
[chapter 16, NIP-19 entities](https://github.com/OpenAgentsInc/openagents/blob/main/nostr/book/ch016-nip-19-entities-and-nostr-uris.md),
[chapter 17, JSON you can sign](https://github.com/OpenAgentsInc/openagents/blob/main/nostr/book/ch017-json-you-can-sign.md) and
[chapter 21, events, IDs and signatures](https://github.com/OpenAgentsInc/openagents/blob/main/nostr/book/ch021-events-ids-and-signatures.md).
Relays, a second relay holding the same event and an encrypted message
between two keys come later.

Use WASD or the arrow keys to walk, Shift to walk slowly and Space to jump;
drag to turn the follow camera. On touch, drag the left side to walk and the
right side to look. WORLDS (top right) goes back to [Worlds](/worlds).
