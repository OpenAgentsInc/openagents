# Experimental phrase gate

This is the consumer of the reviewed [v2 format](../../../docs/protocol/2026-10-28-phrase-gate-v2.md). It is a separate public-fixture/staging path. The enabled production AES-GCM reader, real entries and sign-in/mobile integration are unchanged.

All new format and admission decisions run in Coax:

- `phrase-gate-profile.coax`: fixed framing, ASCII phrase normalization, trusted context hashing, canonical locator and the selected DNS-only wss URI profile.
- `phrase-gate-entry.coax`: bounded original JSON entry projection and canonical padded base64. The original decoded cryptogram is the MAC input.
- `phrase-gate-crypto.coax`: PBKDF2/HKDF, separated encryption/MAC keys, counter-one ChaCha20, full HMAC before decryption.
- `phrase-gate-state.coax`: bounded resumable job, progress, cancellation, cleanup acknowledgement and terminal output admission.
- `platform/phrase-gate-worker.c`: fixed caller storage and observable identified-buffer erasure; scalar ABI and byte transport only.
- `platform/phrase-gate-worker.js`: dedicated-worker scheduling and WebCrypto entropy effect. No JS KDF, cipher, MAC, JSON admission or URL authority.

`coax/web/unlock/experimental/client.js` explicitly schedules serial legacy-first attempts and then raw v2 entries. It takes a trusted legacy adapter instead of importing the production auto-arming entry point. The fixture build derives that adapter byte-for-byte from the frozen legacy source prefix. No configuration element implicitly arms this client, and no new path fetches an event or imports a presentation module.

## Caller contract

The payload cipher slot is exactly the input width; unlike a generic cipher buffer, it cannot borrow tag or adjacent capacity. The pure functions borrow caller-owned storage. Their overlap flag must truthfully describe aliasing; the actual worker has fixed disjoint regions. A job requires stable original input bytes until it finishes. The fixed workspace is 22,128 bytes, with 832 bytes reserved for PBKDF2. The C face additionally owns bounded phrase/context/entry/cryptogram/relay/recipient/entropy/output arrays. It erases copied inputs after initialization, erases registered private work on every terminal path, and withholds output until Coax accepts the cleanup acknowledgement. Only an admitted locator or sealed entry, progress or refusal crosses back to the UI. There is no intermediate key-export API.

The experimental client snapshots mutable trusted context, relay and recipient buffers at construction. It requires an exact limits reply before any legacy match, serializes superseding attempts, and cancels cooperatively. A dead or inactive worker terminates within a bounded deadline and refuses outstanding/subsequent work. Termination does **not** acknowledge successful erasure.

V2 accepts 16–40 normalized ASCII letters/digits from at most 128 raw UTF-8 bytes. It ignores ASCII spaces/hyphens and folds only ASCII A–Z. Legacy retains its previous JavaScript normalization, exact-terminal matching and truncated cheap check. These are separate dispatch rules; there is no attempt to reinterpret one cryptogram as the other.

The selected locator policy requires an exact caller-supplied relay and recipient. Relay syntax uses the shared `net/uri.coax` parser, restricted to `wss` and DNS authorities; IP literals, fragments, credentials, malformed escapes and encoded controls refuse. This is syntax and local policy, not DNS/TLS/SSRF, Nostr event-signature or relay-authorization proof. A v2 authenticated locator must be the exact canonical three-field JSON bytes, without reserialization before MAC verification.

## Memory and security limits

The erasure observations apply only to the identified caller buffers. Compiler stack temporaries, registers, JIT/GC/OS copies and immutable UI strings are not comprehensively erased or proven absent. The composition is experimental; it is not the RFC 8439 AEAD format or NIP-44 and has no independent production security approval. A legacy entry continues to expose its cheap offline phrase oracle. Rewrapping the same phrase in v2 does not remove a retained legacy copy or that weakness.

The 30-day minimum legacy rollback window in the reviewed format remains an operational requirement for any future real migration. This fixture demonstrates dual reading and byte-exact rollback; it neither starts nor automatically completes a production migration window.

See [verification](../../../docs/verification/coax-phrase-gate-2026-10-28.md) and [fixture tools](../../../tools/coax-nostr/phrase-gate/README.md).
