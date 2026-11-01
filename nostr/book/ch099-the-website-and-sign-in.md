# 99. The website and sign-in

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

This chapter follows a login from the page to the API and names each part that
is Coax and each part that is not yet.

Part XII, Nostr in OpenAgents. Result: Logging in to openagents.com with a
NIP-98 signed request from a words account (NIP-06) or a browser signer
(NIP-07), and the cutover of the C verifier to Coax.

## What this chapter teaches

- The words account: 12 BIP-39 words, the NIP-06 key, a NIP-98 login; keys are
  zeroed before each call returns.
- The browser signer path through NIP-07.
- The API's verifier in `coax/web/api/nostr.c` and the parity and cutover
  tasks that move it to Coax.
- Sessions: fixation protection, rotation, expiry and sign-out.

## What exists today

- `coax/lib/nostr/weblogin.coax` and `apps/openagents-web/nip01-verify.coax`;
  the server verifier is C ([docs/web-log-in.md](../../docs/web-log-in.md)).

## Dependencies

- [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md)
- [49. HTTP auth](ch049-http-auth.md)

## Done means

- The Coax verifier and the C verifier agree on every recorded request.
- A login event reused after its window refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-98](../nips/official/98.md)
- [NIP-06](../nips/official/06.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
