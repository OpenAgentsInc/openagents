# 19. NIP-04 and why it is retired

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

NIP-04 was Nostr's first encrypted direct message. The pinned text marks it
`unrecommended` in favor of NIP-17. This chapter describes it exactly, states
its weaknesses, and records what this stack does with it.

Part V, Encryption. Result: NIP-04's AES-256-CBC with the shared x as key and
its `?iv=` suffix, its weaknesses (no authentication, metadata in the clear,
the key used as is), and the decision to read it only where a consumer needs
it.

## What this chapter teaches

- The format: kind 4, content `base64(ciphertext)?iv=base64(iv)`, AES-256-CBC
  with the raw ECDH x as the key.
- Weaknesses: no MAC, so ciphertexts can be altered; the sender, recipient and
  time are public; the key is not derived through a KDF.
- The stack's position (coax/nostr.md section 12): NIP-46 does not add NIP-04
  methods; unsupported legacy methods return an explicit error.
- If a consumer needs to read old messages: a decrypt-only reader, AES-256
  checked against FIPS-197 vectors, never used for new messages.

## What exists today

- Nothing in Coax. No AES implementation exists, and the plan says core Nostr
  does not need one.

## Dependencies

- [14. Shared secrets with ECDH](ch014-shared-secrets-with-ecdh.md)
- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Done means

- The chapter states every weakness above with a reference to the pinned text.
- If a reader is built: FIPS-197 and NIST CBC vectors pass, and the reader
  refuses to encrypt.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-04](../nips/official/04.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
