# Nostr from scratch

A short demo of Nostr, shown as a chat. It plays on its own when the page opens, and **Replay** starts it again. Nothing is typed and no model answers: the questions are fixed, and each answer is a few tool calls you can open, with what each one was given and what came back.

It goes through six steps:

1. **A key pair.** A demo seed makes a key, shown as an `nsec` (the secret) and an `npub` (who you are), as NIP-19 writes them.
2. **A note.** A kind 1 event: the exact bytes that are hashed, their SHA-256 (the event's ID), and a BIP-340 Schnorr signature over the ID.
3. **A relay.** The note goes to a relay as `EVENT` and comes back as `OK`; a `REQ` reads it back, then `EOSE`. We check the signature ourselves.
4. **One letter changed.** The bytes hash to another ID, the signature no longer fits, and both our check and the relay refuse it.
5. **A private message.** Alice encrypts a message to us with NIP-44, and we decrypt it. A changed letter, or someone else's key, does not open it.
6. **A gift wrap.** The same message sealed by Alice and wrapped by a key used once (NIP-59, as NIP-17 direct messages send it), so a relay can't see who it is from.

Every value is worked out in the page, when its step comes, by our Nostr library, written from scratch in Coax (`coax/lib/nostr/`). The relay is the library's own relay code running inside the page, with no network: the page says so. If the library refuses a step, the page says where and stops.

The keys are published test keys, so nothing here is secret, and the page uses fixed bytes where a real client draws random ones, so it reads the same each time. Never use these keys or fixed bytes for anything real.

The same steps, and a station for every NIP we have built, are in [Nostr Lab](/worlds/nostr-lab). The chat's parts are on [/components](/components/index.md).
