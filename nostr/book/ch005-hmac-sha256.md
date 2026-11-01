# 5. HMAC-SHA256

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

HMAC turns SHA-256 into a keyed authenticator. NIP-44 uses HMAC-SHA256 to
authenticate each payload, and HKDF uses it to derive keys. This chapter
builds HMAC from the inner and outer pads.

Part II, Hashes and MACs. Result: HMAC-SHA256 with keys longer than a block
hashed first, checked against RFC 4231 cases 1–7 including the truncated case.

## What this chapter teaches

- The construction: `H((K ^ opad) || H((K ^ ipad) || m))` with a 64-byte
  block.
- Keys longer than 64 bytes are hashed; shorter keys are zero-padded.
- Constant-time tag comparison, and why it matters before any NIP-44
  decryption.
- RFC 4231 case 5, the 128-bit truncated tag.

## What exists today

- `coax/lib/crypto/hmac.coax` and `hmac_workspace.coax` implement it over
  caller workspaces.
- Record:
  [2026-10-29-hmac-workspace.md](../../docs/verification/2026-10-29-hmac-workspace.md).

## Dependencies

- [4. SHA-256](ch004-sha-256.md)

## Done means

- RFC 4231 SHA-256 cases 1–7 pass.
- A tag that differs in one bit is refused by the constant-time comparison.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
