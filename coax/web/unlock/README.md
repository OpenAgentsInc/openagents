# Hidden presentations

A phrase typed on a Coaxial page opens a presentation that only a chosen key
can read. Neither the phrase nor the presentation is in this repository or
readable in the page.

## How it works

- **Sealing** (`seal.ts`): a deck is written as Markdown (`parseDeck` in
  `lib.ts`), put inline in an OpenAgents private artifact (kind 3188 with an
  `openagents.artifact-envelope.v1` body whose JCS bytes match its digest; see
  `nips/openagents/contracts.md`), NIP-44 v2 encrypted to the recipient,
  sealed by the author (kind 13) and gift-wrapped from a throwaway key
  (kind 1059, NIP-59). The wrap goes to relay.openagents.com, which serves it
  only to its recipient after NIP-42 AUTH.
- **Deploying** (`inject.ts`, run by `../deploy.sh`): for each entry of
  `COAXIAL_UNLOCKS`, index.html gets a random salt, a 64-bit check of the
  salted phrase, and the pointer (relay, event id, recipient) AES-GCM
  encrypted under PBKDF2-SHA-256 (210,000 rounds) of the phrase. The gate
  script is added; the opener module is a separate content-named file.
- **In the page** (`gate.ts`, then `page.ts`): the page's Coax code draws
  everything you see (`apps/openagents-web/ui.coax`): the access terminal,
  the key prompt and the slides. These scripts only answer it through the
  Coaxial host (`coaxial` events in, `coaxial.send` out). A code sent from
  the terminal is checked against the entries; a phrase that matches
  decrypts its pointer and loads the opener, which finds the recipient key on
  this device: a NIP-07 signer holding it, or a key pasted into the prompt
  once and kept in this site's IndexedDB. It fetches the wrap as that key,
  verifies every signature, the author chain, the recipient and the digest,
  and hands the slides to the page as deck lines (`deckLines` in `lib.ts`;
  arrow keys, click or swipe; N shows notes; Esc closes).

The phrase gate is a door, not a lock: anyone with the page can try phrases
against the check offline. The lock is the key the deck is sealed to.

## Use

```sh
cd coax/web/unlock && bun install
bun seal.ts keygen ~/work/.secrets/NAME.key          # prints the public key
bun seal.ts deck.md --author AUTHOR.key --to RECIPIENT_PUBKEY
# prints RELAY,EVENT_ID,RECIPIENT; then, in a gitignored env file or the environment:
COAXIAL_UNLOCKS="PHRASE=RELAY,EVENT_ID,RECIPIENT"     # several: newlines or ;
../deploy.sh ../../../apps/openagents-web SERVICE
bun test                                             # unlock.test.ts
```

`deploy.sh` reads `COAXIAL_UNLOCKS` from the environment, else from
`~/work/.secrets/coaxial-unlocks.env` (or `COAXIAL_UNLOCKS_FILE`). With none
set, pages ship without the gate. `coax serve` doesn't inject unlocks.

nostr-effect's `nip59` module leaves the nonce out of the NIP-44 MAC, so its
wraps don't open with spec NIP-44 or NIP-07 signers; `lib.ts` builds the seal
and wrap from nostr-effect's `nip44` and `pure` instead.
