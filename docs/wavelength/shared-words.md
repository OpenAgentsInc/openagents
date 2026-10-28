# One set of 12 words for Nostr, Spark and Wavelength

*2026-10-27. Checked against `wavelength-sdk` at `6f71128`. This is a
proposal; nothing is built.*

## How Wavelength's own mnemonic works

Wavelength uses **aezeed**, lnd's cipher seed, not BIP-39.
- An aezeed phrase is always **24 words** from the BIP-39 English word list.
  It encodes a version, a wallet birthday, **16 bytes of entropy** and a
  random salt, enciphered with AEZ and checksummed. Those 16 bytes are the
  wallet's HD root.
- Because the salt is random, exporting the same wallet twice gives two
  different, equally valid phrases.
- An imported phrase must be aezeed. A BIP-39 phrase, of 12 or 24 words, is
  not one, so our words can't simply be typed into `createWallet`. The SDK
  calls the optional `seedPassphrase` a BIP-39 passphrase, but in aezeed a
  passphrase protects the enciphered phrase.
- Passkey wallets compute their 16 bytes as
  `HKDF-SHA256(PRF output, info "wavewalletdk:seed:v1")`, then wrap them in
  aezeed with a pinned version and a genesis birthday.

## The hook: external seeds

The daemon also opens a wallet from 16 bytes the host supplies, through
`startExternalSeedWallet({ seedEntropy, expectedIdentityPubKey,
recoverState, recoveryWindow })`. Its contract
(`ExternalSeedWalletRequest` in `packages/core/src/generated.ts`) says:

> Wavelength does not assign BIP39, BIP32, account-index, network, or
> storage-path semantics to those bytes; the host SDK owns that versioned
> derivation and selects Config.DataDir.

The result deliberately leaves out the internal aezeed phrase, because "the
host-owned external root and derivation contract are the wallet's backup
boundary." This is the same arrangement we had with Breez Spark, where the
12 words were handed to Spark as `Seed::Mnemonic`. The web and React Native
transports implement it. Check that the native `WalletKit` wrapper does
before relying on it; if it doesn't, the gomobile facade's
`startExternalSeedWallet` is the call to wrap.

## Our existing contract

From the identity work (#11992, #11995, #12004; profile frozen in IDR-00):
- 12 BIP-39 words, English list, **empty passphrase**, become a 64-byte seed.
- **Nostr identity (NIP-06):** BIP-32 at `m/44'/1237'/0'/0/0`
  ([nips/official/06.md](../../nips/official/06.md)).
- **Spark wallet:** the same words, given to Breez (Spark derives at
  `m/44'/0'/0'/0/0`).
- On iOS the words' entropy is Keychain item `com.openagents.app.spark`,
  account `seed-v1`, this device only. It is reused, never regenerated or
  overwritten. A restore replaces the wallet and the identity together.

## Proposal: derive Wavelength's 16 bytes with BIP-85

```
12 words --BIP-39, empty passphrase--> 64-byte seed --BIP-32--> master key
master key --BIP-85 HEX, m/83696968'/128169'/16'/0'--> 16 bytes = seedEntropy
```

- **Why BIP-85, not a path of our own or HKDF?**
  - BIP-85 is a published standard for deriving independent child entropy
    from a BIP-32 root.
  - Hardware wallets such as Coldcard implement it, so a user can rebuild
    the Wavelength entropy without our code.
  - The child is one-way: leaking it reveals neither the Nostr key nor the
    Spark keys.
- **Every step is hardened,** so it needs PBKDF2-HMAC-SHA512, HMAC-SHA512
  and addition mod n, but no elliptic-curve multiplication. That makes it a
  later candidate for Coax, tested against the published BIP-39, BIP-32 and
  BIP-85 vectors. To start, it runs in the Swift host with an audited
  library, and the words never enter Coax.
- **Version it.** Name the profile, for example
  `openagents.wavelength.bip85_hex16.v1`. Record it and the returned
  identity public key in the account's public manifest, and pass that key
  as `expectedIdentityPubKey` on every open. Then a wrong derivation or a
  wrong data directory fails loudly instead of opening an empty wallet.
- **Restore** means the same words, `recoverState: true` (server-assisted
  recovery), and a recovery window large enough for the wallet's history.

## Consequences

- **One backup.** The 12 words restore the Nostr identity, the Spark wallet
  and the Wavelength wallet. The existing rules apply unchanged: words stay
  in memory only, are shown once after a warning, and are never logged.
- **Portability.** Another Wavelength app can't open this wallet from the 12
  words unless it implements the same BIP-85 derivation. Mitigations:
  - document the profile publicly;
  - optionally offer an advanced aezeed export of the same 16 bytes. That
    needs an aezeed encoder, and the SDK doesn't return the phrase.
- **No passkey wallet.** Wavelength's passkey mode derives its own root from
  a PRF, which would be a second root. Don't use it. If we want passkey
  unlock, it should unlock the Keychain item that holds the words.
- **Database at rest.** The external-seed request has no password field.
  Confirm how the daemon protects the wallet database on that path
  (`walletPasswordFile` in the config is one candidate) before shipping.
- **Domain separation.** Nostr, Spark and Wavelength use separate subtrees.
  Per-device pairing, push and transport keys stay random and are never
  derived from the words.
