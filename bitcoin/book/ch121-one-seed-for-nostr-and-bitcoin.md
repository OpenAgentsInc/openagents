# 121. One seed for Nostr and Bitcoin

An OpenAgents agent has a Nostr key, which is its name on relays, and
Bitcoin wallets, which hold what it earns. Both can come from one BIP-39
phrase: the Nostr book's
[chapter 46](../../nostr/book/ch046-keys-from-a-seed-phrase.md) takes the
Nostr key from the words at NIP-06's path `m/44'/1237'/0'/0/0`, and
[chapter 33](ch033-derivation-paths.md) takes wallet keys from the same
kind of seed at BIP-84's and BIP-86's paths. This chapter puts the two
together. One phrase gives the agent's `npub` and two wallets, and a
three-line card records which paths are in use. A restore rebuilds the
card from the words and the passphrase and compares it, so a wrong
passphrase, account or network shows up before any coins move. The
chapter then asks what one seed risks: why the Nostr key and the wallet
keys must sit on separate paths, and why separate paths are enough.

The worked examples use published vectors: NIP-06's first phrase gives
NIP-06's `npub`, BIP-84's and BIP-86's phrase gives their published
addresses and account `xpub`, and the same phrase with the passphrase
`TREZOR` gives BIP-39's vector 0 seed.

## What this chapter teaches

- One BIP-39 seed, three branches: NIP-06 at `m/44'/1237'/a'/0/0`, BIP-84
  at `m/84'/c'/a'` and BIP-86 at `m/86'/c'/a'`.
- The card: the `npub` with its path, and each wallet as a BIP-380
  descriptor with its key origin, `<0;1>/*` for its receive and change
  chains, and its checksum.
- A restore that compares cards, and what it catches: a wrong
  passphrase, account or network, and words with a wrong checksum.
- What the passphrase changes: every key, the Nostr key included.
- Why a key must not serve both protocols: a raw signature request to a
  Nostr signer can be a spend.
- Why separate hardened paths are enough, and what they don't protect.
- What an agent's backup contains, and what it can't rebuild from the
  words.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [oneseed.coax](../../coax/lib/bitcoin/oneseed.coax) | `oneseed_seed_ct(phrase, passphrase, work, seed)` | the words, checked, and the passphrase to the 64-byte seed |
| | `oneseed_nostr_ct(seed, account, work, key)` | the NIP-06 key of an account |
| | `oneseed_account_ct(seed, network, purpose, account, work, xpub, fingerprint)` | a BIP-84 or BIP-86 account `xpub` and the master fingerprint |
| | `oneseed_descriptor_ct(purpose, network, account, fingerprint, xpub, work, out)` | that account as a descriptor with its checksum |
| | `oneseed_card_ct(phrase, passphrase, network, account, work, out)` | the card of a phrase |
| | `oneseed_line_vartime(card, k)` | line `k` of a card |
| | `oneseed_restore_ct(phrase, passphrase, network, account, card, work)` | whether a phrase and passphrase give a card |
| [bip39.coax](../../coax/lib/crypto/bip39.coax) | `bip39_decode_ct`, `bip39_seed_ct` | chapter 31: the words to their entropy, and the seed |
| [bip32.coax](../../coax/lib/crypto/bip32.coax) | `bip32_master_ct`, `bip32_descend_ct`, `bip32_neuter_ct`, `bip32_fingerprint_ct`, `bip32_encode_ct` | chapter 32: the nodes and their text |
| [path.coax](../../coax/lib/bitcoin/path.coax) | `path_standard_vartime`, `path_format_vartime` | chapter 33: the standard paths and their text |
| [descriptor.coax](../../coax/lib/bitcoin/descriptor.coax) | `descriptor_checksum_vartime`, `descriptor_script_vartime` | chapter 30: the checksum, and a descriptor's scripts |
| [nip06.coax](../../coax/lib/nostr/nip06.coax) | `nip06_npub` | Nostr chapter 46: a key's `npub` |

The words, the passphrase, the seed and the private keys go only through
chapters 31 and 32's constant-time procedures, which leave their work
buffers as zeros. The card is public, so the code that writes and reads
it branches on it. Each procedure returns a count or one negative
refusal, and a refusal writes nothing.

## Refusals

<!-- source: coax/lib/bitcoin/oneseed.coax codes -->
```jai
OS_WORK_BYTES :: BIP39_WORK_BYTES;   // the larger of chapter 31's and chapter 32's work buffers
OS_CARD_MAX :: 512;                  // three lines: 88, about 160 and about 160 characters
OS_LINE_MAX :: 200;
OS_NOSTR_COIN :: 1237;               // SLIP-44's coin type for Nostr

// Refusals.
OS_WORDS :: -1;       // words chapter 31 refuses: an unknown word, a count it doesn't define, a wrong checksum
OS_FIELD :: -2;       // an account of 2^31 or more, a network outside 1-3, or a purpose other than 84 or 86
OS_CAPACITY :: -3;    // a buffer that is too small
OS_MISMATCH :: -4;    // a card that these words and this passphrase don't give
OS_DERIVE :: -5;      // a BIP-32 refusal on the way: an invalid child, odds about 1 in 2^127
```

`OS_DERIVE` is for completeness: BIP-32 refuses a child whose `I_L` is
`n` or more, or whose key is zero, and the odds of that on a given path
are about 1 in 2^127 (chapter 32).

## One seed, three branches

The seed is chapter 31's: PBKDF2-HMAC-SHA512 over the words, with the
salt `mnemonic` followed by the passphrase. Chapter 31's `bip39_seed_ct`
leaves the checksum to its caller, because a phrase with a wrong
checksum still has a seed. A wallet that restored from such a phrase
would show an empty wallet instead of an error, so `oneseed_seed_ct`
decodes the words first and refuses them:

<!-- source: coax/lib/bitcoin/oneseed.coax seed -->
```jai
// The seed of checked words and a passphrase into seed[0..<64]. Chapter 31
// leaves the checksum to the caller; a phrase with a wrong checksum still
// has a seed, so this procedure decodes the words first and refuses them.
// Returns 64 or a refusal. work (OS_WORK_BYTES) comes back as zeros.
oneseed_seed_ct :: (phrase: string, passphrase: string, work: [] u8, seed: [] u8) -> int {
  if work.count < OS_WORK_BYTES || seed.count < 64 { return OS_CAPACITY; }
  entropy: [32] u8;
  n := bip39_decode_ct(phrase, work, entropy[..]);
  for i: 0..<32 { entropy[i] = 0; }
  if n < 0 { return OS_WORDS; }
  return ifx bip39_seed_ct(phrase, passphrase, work, seed) == 64 then 64 else OS_WORDS;
}
```

Below the seed's master node, three branches start at the first,
hardened step:

```text
m ─┬─ 44'/1237'/a'/0/0   the Nostr key (NIP-06; 1237 is SLIP-44's Nostr)
   ├─ 84'/c'/a'          the P2WPKH wallet (BIP-84)
   └─ 86'/c'/a'          the key-path P2TR wallet (BIP-86)
```

`a` is the account: NIP-06 lets a client increment it for more
identities, and BIP-44's levels make each account a separate wallet. `c`
is the coin type, 0 on mainnet and 1 on every test network. Nostr has no
test network, so the Nostr path is the same in both cases.

<!-- source: coax/lib/bitcoin/oneseed.coax nostr -->
```jai
// The NIP-06 private key of account at m/44'/1237'/account'/0/0 into
// key[0..<32]. Returns 32 or a refusal. Nostr has no network: the path is
// the same for mainnet and testnet wallets.
oneseed_nostr_ct :: (seed: [] u8, account: int, work: [] u8, key: [] u8) -> int {
  if account < 0 || account >= BIP32_HARDENED { return OS_FIELD; }
  if key.count < 32 || work.count < BIP32_WORK_BYTES { return OS_CAPACITY; }
  path: [5] int;
  path[0] = 44 + BIP32_HARDENED; path[1] = OS_NOSTR_COIN + BIP32_HARDENED;
  path[2] = account + BIP32_HARDENED; path[3] = 0; path[4] = 0;
  node: [BIP32_NODE_BYTES] u8;
  r := bip32_master_ct(seed, false, work, node[..]);
  if r > 0 { r = bip32_descend_ct(node[..], path[..], work, node[..]); }
  if r > 0 { for j: 0..<32 { key[j] = node[HDK_KEY + 1 + j]; } }
  for j: 0..<BIP32_NODE_BYTES { node[j] = 0; }
  return ifx r > 0 then 32 else OS_DERIVE;
}
```

For account 0 this is chapter 46's `nip06_key`, and the library's checks
compare the two on NIP-06's first phrase. NIP-06 marks itself
`unrecommended: prefer a single nsec`; an agent that uses words anyway
gets one backup for its name and its coins.

The wallets keep only public nodes: the account `xpub`, and the master
key's fingerprint, which a descriptor's key origin carries so that a
signer can find its key (chapter 30):

<!-- source: coax/lib/bitcoin/oneseed.coax account -->
```jai
// The public account node m/purpose'/coin'/account' into xpub (78 bytes;
// an xpub, or a tpub for the test networks), and the master key's
// fingerprint into fingerprint[0..<4]. purpose is 84 (P2WPKH) or 86
// (key-path P2TR); coin is 0 on mainnet and 1 on the test networks.
oneseed_account_ct :: (seed: [] u8, network: int, purpose: int, account: int, work: [] u8, xpub: [] u8, fingerprint: [] u8) -> int {
  if purpose != 84 && purpose != 86 { return OS_FIELD; }
  indexes: [5] int;
  if path_standard_vartime(purpose, network, account, 0, 0, indexes[..]) != 5 { return OS_FIELD; }
  if xpub.count < BIP32_NODE_BYTES || fingerprint.count < 4 || work.count < BIP32_WORK_BYTES { return OS_CAPACITY; }
  master: [BIP32_NODE_BYTES] u8; node: [BIP32_NODE_BYTES] u8; fp: [4] u8;
  r := bip32_master_ct(seed, network != SW_MAINNET, work, master[..]);
  if r > 0 { r = bip32_fingerprint_ct(master[..], work, fp[..]); }
  if r > 0 { r = bip32_descend_ct(master[..], indexes[0..<3], work, node[..]); }
  if r > 0 { r = bip32_neuter_ct(node[..], work, node[..]); }
  if r > 0 {
    for j: 0..<BIP32_NODE_BYTES { xpub[j] = node[j]; }
    for j: 0..<4 { fingerprint[j] = fp[j]; }
  }
  for j: 0..<BIP32_NODE_BYTES { master[j] = 0; node[j] = 0; }
  return ifx r > 0 then BIP32_NODE_BYTES else OS_DERIVE;
}
```

## The card

A card is three lines: the Nostr path and the `npub`, then the two
wallets as BIP-380 descriptors. Each descriptor names the fingerprint and
path of its account key, the account `xpub`, and `<0;1>/*`: BIP-389's
multipath step for the receive chain (0) and the change chain (1), then
any index. The checksum is chapter 30's:

<!-- source: coax/lib/bitcoin/oneseed.coax descriptor -->
```jai
// Text into out at at; the new end, or -1 when it doesn't fit.
os_put :: (out: [] u8, at: int, text: string) -> int {
  if at < 0 || at + text.count > out.count { return -1; }
  for k: 0..<text.count { out[at + k] = text[k]; }
  return at + text.count;
}
// The descriptor of an account: wpkh() for BIP-84 or tr() for BIP-86, the
// key's origin [fingerprint/purpose'/coin'/account'] with h for hardened,
// the account xpub, <0;1>/* for the receive and change chains (BIP-389),
// then # and the checksum. Returns its length or a refusal.
oneseed_descriptor_ct :: (purpose: int, network: int, account: int, fingerprint: [] u8, xpub: [] u8, work: [] u8, out: [] u8) -> int {
  if purpose != 84 && purpose != 86 { return OS_FIELD; }
  indexes: [5] int;
  if path_standard_vartime(purpose, network, account, 0, 0, indexes[..]) != 5 { return OS_FIELD; }
  if fingerprint.count != 4 || xpub.count != BIP32_NODE_BYTES || work.count < BIP32_WORK_BYTES { return OS_CAPACITY; }
  text: [OS_LINE_MAX] u8; n := 0; digits := "0123456789abcdef";
  n = os_put(text[..], n, ifx purpose == 84 then "wpkh([" else "tr([");
  for j: 0..<4 { text[n] = digits[cast(int) (fingerprint[j] >> 4)]; text[n + 1] = digits[cast(int) (fingerprint[j] & 15)]; n += 2; }
  path: [32] u8;
  m := path_format_vartime(indexes[0..<3], 'h', path[..]);
  n = os_put(text[..], n, cast(string) path[1..<m]);           // the path without its m
  n = os_put(text[..], n, "]");
  if bip32_encode_ct(xpub, work, text[n..<n + BIP32_TEXT]) != BIP32_TEXT { return OS_DERIVE; }
  n = os_put(text[..], n + BIP32_TEXT, "/<0;1>/*)");
  sum: [DS_CHECKSUM_CHARS] u8;
  #assert descriptor_checksum_vartime(cast(string) text[0..<n], sum[..]) == DS_CHECKSUM_CHARS;
  n = os_put(text[..], n, "#");
  n = os_put(text[..], n, cast(string) sum[..]);
  if out.count < n { return OS_CAPACITY; }
  for k: 0..<n { out[k] = text[k]; }
  return n;
}
```

<!-- source: coax/lib/bitcoin/oneseed.coax card -->
```jai
// The card of a phrase for one account on one network, three lines, each
// ending in a newline:
//
//   nostr m/44h/1237h/<account>h/0/0 <npub>
//   wpkh([<fingerprint>/84h/<coin>h/<account>h]<xpub>/<0;1>/*)#<checksum>
//   tr([<fingerprint>/86h/<coin>h/<account>h]<xpub>/<0;1>/*)#<checksum>
//
// Everything on it is public: the npub is the agent's name, and the
// descriptors let a watch-only wallet see its coins. Returns its length or
// a refusal. work (OS_WORK_BYTES) comes back as zeros.
oneseed_card_ct :: (phrase: string, passphrase: string, network: int, account: int, work: [] u8, out: [] u8) -> int {
  if network < SW_MAINNET || network > SW_REGTEST || account < 0 || account >= BIP32_HARDENED { return OS_FIELD; }
  if work.count < OS_WORK_BYTES { return OS_CAPACITY; }
  seed: [64] u8; key: [32] u8; card: [OS_CARD_MAX] u8; n := 0;
  r := oneseed_seed_ct(phrase, passphrase, work, seed[..]);
  if r > 0 { r = oneseed_nostr_ct(seed[..], account, work, key[..]); }
  if r > 0 {
    path: [5] int; text: [64] u8;
    path[0] = 44 + BIP32_HARDENED; path[1] = OS_NOSTR_COIN + BIP32_HARDENED;
    path[2] = account + BIP32_HARDENED; path[3] = 0; path[4] = 0;
    n = os_put(card[..], 0, "nostr ");
    n = os_put(card[..], n, cast(string) text[0..<path_format_vartime(path[..], 'h', text[..])]);
    n = os_put(card[..], n, " ");
    npub: [64] u8;
    m := nip06_npub(key[..], npub[..]);
    r = ifx m > 0 then 32 else OS_DERIVE;
    if r > 0 { n = os_put(card[..], n, cast(string) npub[0..<m]); n = os_put(card[..], n, "\n"); }
  }
  purposes := [2] int.[84, 86];
  for purpose: purposes {
    xpub: [BIP32_NODE_BYTES] u8; fp: [4] u8;
    if r > 0 { r = oneseed_account_ct(seed[..], network, purpose, account, work, xpub[..], fp[..]); }
    if r > 0 { r = oneseed_descriptor_ct(purpose, network, account, fp[..], xpub[..], work, card[n..<OS_CARD_MAX]); }
    if r > 0 { n = os_put(card[..], n + r, "\n"); }
  }
  for j: 0..<64 { seed[j] = 0; }
  for j: 0..<32 { key[j] = 0; }
  for j: 0..<work.count { work[j] = 0; }
  if r < 0 { return r; }
  if out.count < n { return OS_CAPACITY; }
  for k: 0..<n { out[k] = card[k]; }
  return n;
}
// Line k of a card (0, 1 or 2) without its newline; empty when there is
// no such line.
oneseed_line_vartime :: (card: string, k: int) -> string {
  start := 0; line := 0;
  for i: 0..<card.count {
    if card[i] == '\n' {
      if line == k { return card[start..<i]; }
      line += 1; start = i + 1;
    }
  }
  return card[0..<0];
}
```

### Worked example: NIP-06's first phrase

The phrase `leader monkey parrot ring guide accident before fence cannon
height naive bean`, no passphrase, account 0 on mainnet:

```text
nostr m/44h/1237h/0h/0/0 npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu
wpkh([ae957285/84h/0h/0h]xpub6Byf6wvtJs2NobE6DV9fCPmNXCa5fxZ6EhcEhgXpJKfqr2Bzyeznmq26kTxUCyNgUS9ZQKXPUTFsWMqcpEuHaAsGmoYRbkF2ZXxim5fsGT1/<0;1>/*)#mcpgfhwv
tr([ae957285/86h/0h/0h]xpub6BkvVbW4aaRfBmgjc4Yg1oG7hDMs1sL6a3P6pP1LJtRQKB9fe22RCbp3ifSSMye9ziPQUH2YRk38oqzKUiXoKCb1R6keV3X57qBFDx4e7eG/<0;1>/*)#a8gvn82u
```

The first line ends in NIP-06's published `npub`. `ae957285` is the
first 4 bytes of HASH160 of the master public key. Chapter 30's
`descriptor_script_vartime` expands the second line at branch 0, index 0
to `0014` and a 20-byte key hash, which is
`bc1q8zxz5kl6q30y2mzhx86gcwcz0t0hgzl2f2jpm5`, with no seed at all.

### Worked example: BIP-84's and BIP-86's phrase

The phrase `abandon abandon … abandon about` (eleven `abandon`) is the
one BIP-84 and BIP-86 publish their vectors for. Its card's descriptors
expand to their published addresses:

| Line | Branch/index | Address | Published in |
| --- | --- | --- | --- |
| `wpkh` | 0/0 | `bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu` | BIP-84 |
| `wpkh` | 0/1 | `bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g` | BIP-84 |
| `wpkh` | 1/0 | `bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el` | BIP-84 |
| `tr` | 0/0 | `bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr` | BIP-86 |
| `tr` | 0/1 | `bc1p4qhjn9zdvkux4e44uhx8tc55attvtyu358kutcqkudyccelu0was9fqzwh` | BIP-86 |
| `tr` | 1/0 | `bc1p3qkhfews2uk44qtvauqyr2ttdsw7svhkl9nkm9s9c3x4ax5h60wqwruhk7` | BIP-86 |

The `tr` line's `xpub` is BIP-86's published account key, and its
fingerprint `73c5da0a` is the one both BIPs' phrase is known by. BIP-84
publishes its account key as a `zpub`; chapter 33 shows it is the same
78 bytes with another version.

## A restore

A restore takes the words and the passphrase, rebuilds the card for the
account and network, and compares it byte for byte with the card it was
given:

<!-- source: coax/lib/bitcoin/oneseed.coax restore -->
```jai
// Whether phrase and passphrase give card for account on network: OS_OK
// (0), OS_MISMATCH, or a refusal from the words. A wrong passphrase has
// no checksum to catch it; it makes another seed and so another card,
// which is how a restore finds it.
OS_OK :: 0;
oneseed_restore_ct :: (phrase: string, passphrase: string, network: int, account: int, card: string, work: [] u8) -> int {
  mine: [OS_CARD_MAX] u8;
  n := oneseed_card_ct(phrase, passphrase, network, account, work, mine[..]);
  if n < 0 { return n; }
  if n != card.count { return OS_MISMATCH; }
  for k: 0..<n { if mine[k] != card[k] { return OS_MISMATCH; } }
  return OS_OK;
}
```

What each mistake gives:

| Restore with | Result | Why |
| --- | --- | --- |
| the same words and passphrase | the same card | the seed, and every key below it, is the same |
| the words and passphrase `x` | mismatch | another salt, another seed: every line changes |
| account 1 | mismatch | other hardened children on all three branches |
| the testnet card for mainnet | mismatch | coin type 1 and `tpub` on the wallet lines; the Nostr line stays |
| `… naive beauty` for `… naive bean` | words refused | the last word carries the checksum, and `beauty`'s bits don't match |

A wrong passphrase has no checksum to catch it: BIP-39 accepts every
passphrase and gives each its own seed. Without the card, a restore with
a mistyped passphrase opens an empty wallet and a stranger's `npub`. With
the card, it stops.

### The passphrase changes every key

With the passphrase `TREZOR`, the abandon phrase gives BIP-39's vector 0
seed, `c55257c3…7463b04`, and a card with another fingerprint
(`b4e3f5ed`), other wallets and another `npub`. NIP-06 doesn't mention a
passphrase, and chapter 46's `nip06_key` uses none. A Nostr client that
imports NIP-06 words without asking for a passphrase derives the empty
passphrase's key, so an agent with a passphrase must restore its Nostr key
with a tool that asks for one.

## Separate paths

### One key in both protocols

A Nostr event is signed with BIP-340 over its 32-byte id. A Taproot spend
is signed with BIP-340 over a 32-byte signature hash (chapter 43). The
signature doesn't say which protocol it was made for. A Nostr event id is
a SHA-256 of the event's serialization, so a signer that signs only
events it serializes itself can't be steered to a chosen sighash. But a
signer that signs 32 bytes a caller hands it, as some remote and browser
signers offer, signs whatever those bytes are.

The example signs a TapSighash-tagged hash with the Nostr key, as such a
request would:

| Valid under | Result | When that key is |
| --- | --- | --- |
| the Nostr key | true | a key in a `<key> OP_CHECKSIG` leaf of a script tree (chapter 42) |
| the Nostr key's BIP-86 output key | false | a BIP-86 internal key: the key path needs the tweaked key |
| the wallet key at `m/86'/0'/0'/0/0` | false | a separate key |
| the wallet key's BIP-86 output key | false | a separate key, tweaked |

So reuse is worst in a script leaf, where the key signs untweaked: the
leaf form of chapter 122's 2-of-3 is that case. Chapter 41's tweak
protects a key-path output only because the signer would have to sign
with the tweaked key, and a signer that offers raw signing for one key
may well offer it for another. Reuse also links: the `npub` is public, so
a coin locked to the same key is the agent's coin for anyone who looks.
The rule is simple: the Nostr key signs Nostr events, and wallet keys
sign transactions.

### Why separate hardened paths are enough

Every branch starts with a hardened step from the master node, and a
hardened child is HMAC-SHA512 of the parent's chain code over the
parent's private key (chapter 32). Nothing on one branch, its `xpub`, its
addresses, or even its private keys, gives any key on another without
the master private key, as long as HMAC-SHA512 behaves as a random
function. That covers the cases that matter here:

- the card, which publishes the `npub` and both account `xpub`s, says
  nothing about the Nostr private key;
- a leaked Nostr key costs the agent its name, not its coins;
- chapter 32's leak, a wallet `xpub` with one of its address keys, gives
  that account's private key and stops there (chapter 33): the Nostr key
  and the other wallet are on other hardened branches.

Separate paths don't protect against the one thing they share: whoever
has the words and the passphrase has every key. One seed trades two
backups for one single point of failure, and the backup must be kept
accordingly.

## What an agent's backup contains

| Part | Secret | Rebuilt from the words |
| --- | --- | --- |
| the 12 or 24 words | yes | — |
| the passphrase, if any, kept apart from the words | yes | — |
| the card: account, network, `npub`, descriptors | no, but it shows every coin | yes, given the account and network |
| the wallets' birthday, the height to rescan from | no | no: a wallet on our node keeps it (chapter 99) |
| the agent's relay list (NIP-65) and its other Nostr state | no | no: it lives on relays |
| the treasury's other keys and policy | no | no: chapter 122 |

The card holds no secret, but its descriptors let anyone watch every
address of both wallets, so it is shared with the owner, not published
on a relay; the `npub` on its first line is the only part meant to be
public. The restore compares against it, so it must be kept where the
words are restored.

## The example

[bitcoin/examples/one-seed.coax](../examples/one-seed.coax) prints the
card of NIP-06's first phrase and the addresses its descriptors give,
restores it four ways, prints the card of the BIP-84 and BIP-86 phrase
with their published addresses, the `TREZOR` seed and card, the testnet
card, and the raw signature request:

<!-- source: bitcoin/examples/one-seed.coax example -->
```jai
#load "../../coax/lib/bitcoin/oneseed.coax";
#load "../../coax/lib/crypto/schnorr_sign.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
REFUSALS :: [5] string.["words refused", "bad field", "capacity", "mismatch", "derivation"];
result :: (code: int) -> string { return ifx code == OS_OK then "the same card" else REFUSALS[-code - 1]; }
// The address of a card's descriptor line at branch and index.
address :: (line: string, network: int, branch: int, index: int, out: [] u8) -> string {
  script: [AD_SCRIPT_MAX] u8;
  n := descriptor_script_vartime(line, network, branch, index, 0, script[..]);
  #assert n > 0;
  m := address_encode_vartime(network, script[0..<n], out);
  #assert m > 0;
  return cast(string) out[0..<m];
}
// Print a card's addresses: two receiving and one change, per descriptor.
addresses :: (card: string, network: int) {
  addr: [90] u8;
  for d: 1..<3 {
    line := oneseed_line_vartime(card, d);
    name := ifx d == 1 then "wpkh" else "tr";
    print("% 0/0: %\n", name, address(line, network, 0, 0, addr[..]));
    print("% 0/1: %\n", name, address(line, network, 0, 1, addr[..]));
    print("% 1/0: %\n", name, address(line, network, 1, 0, addr[..]));
  }
}
// The private key at a path string below a seed.
key_at :: (seed: [] u8, path: string, key: [] u8) {
  work: [BIP32_WORK_BYTES] u8; node: [BIP32_NODE_BYTES] u8;
  #assert path_derive_ct(seed, false, path, work[..], node[..]) == BIP32_NODE_BYTES;
  for j: 0..<32 { key[j] = node[HDK_KEY + 1 + j]; }
}
// A BIP-340 signature of 32 bytes by a key, with zero aux.
sign :: (key: [] u8, message: [] u8, sig: [] u8) {
  w: [1] SignWork; aux: [32] u8; status := 0;
  for k: 0..<6 { status = schnorr_sign_step(key, aux[..], message, w[..], sig, false); }
  #assert status == SIGN_DONE;
}
// The x-only public key of a private key.
xonly :: (key: [] u8, out: [] u8) {
  pub: [33] u8;
  #assert bip32_public(key, pub[..]);
  for j: 0..<32 { out[j] = pub[1 + j]; }
}

NIP06_WORDS :: "leader monkey parrot ring guide accident before fence cannon height naive bean";
NIP06_NPUB :: "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
ABANDON :: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

main :: () {
  work: [OS_WORK_BYTES] u8; card: [OS_CARD_MAX] u8; other: [OS_CARD_MAX] u8;

  // One phrase, three keys: NIP-06's first phrase gives its published
  // npub and two wallets.
  print("The card of NIP-06's first phrase, account 0\n");
  n := oneseed_card_ct(NIP06_WORDS, "", SW_MAINNET, 0, work[..], card[..]);
  c := cast(string) card[0..<n];
  print("%", c);
  first := oneseed_line_vartime(c, 0);
  print("npub is NIP-06's: %\n", ost_same(first[first.count - NIP06_NPUB.count..<first.count], NIP06_NPUB));
  print("Addresses from the card alone\n");
  addresses(c, SW_MAINNET);

  // A restore rebuilds the card from the words and compares.
  print("Restore\n");
  print("same words, no passphrase: %\n", result(oneseed_restore_ct(NIP06_WORDS, "", SW_MAINNET, 0, c, work[..])));
  print("same words, passphrase x: %\n", result(oneseed_restore_ct(NIP06_WORDS, "x", SW_MAINNET, 0, c, work[..])));
  print("same words, account 1: %\n", result(oneseed_restore_ct(NIP06_WORDS, "", SW_MAINNET, 1, c, work[..])));
  print("last word beauty: %\n", result(oneseed_restore_ct(
    "leader monkey parrot ring guide accident before fence cannon height naive beauty", "", SW_MAINNET, 0, c, work[..])));

  // BIP-84's and BIP-86's phrase: the card's descriptors give their
  // published addresses.
  print("The card of BIP-84's and BIP-86's phrase\n");
  n = oneseed_card_ct(ABANDON, "", SW_MAINNET, 0, work[..], card[..]);
  c = cast(string) card[0..<n];
  print("%", c);
  addresses(c, SW_MAINNET);

  // The passphrase is part of the seed: with TREZOR the phrase gives
  // BIP-39's vector 0 seed, and another npub and other wallets.
  print("The passphrase TREZOR\n");
  seed: [64] u8;
  #assert oneseed_seed_ct(ABANDON, "TREZOR", work[..], seed[..]) == 64;
  show("seed", seed[..]);
  m := oneseed_card_ct(ABANDON, "TREZOR", SW_MAINNET, 0, work[..], other[..]);
  print("%", cast(string) other[0..<m]);

  // Testnet: the wallets move to coin type 1 and tpub; the npub stays.
  print("The same phrase on testnet\n");
  m = oneseed_card_ct(ABANDON, "", SW_TESTNET, 0, work[..], other[..]);
  t := cast(string) other[0..<m];
  print("%", t);
  print("npub line unchanged: %\n", ost_same(oneseed_line_vartime(t, 0), oneseed_line_vartime(c, 0)));

  // Separate paths: the Nostr key and a wallet key share only m.
  print("Separate keys\n");
  #assert oneseed_seed_ct(ABANDON, "", work[..], seed[..]) == 64;
  nostr: [32] u8; wallet: [32] u8; np: [32] u8; wp: [32] u8; wq: [32] u8; nq: [32] u8;
  #assert oneseed_nostr_ct(seed[..], 0, work[..], nostr[..]) == 32;
  key_at(seed[..], "m/86'/0'/0'/0/0", wallet[..]);
  xonly(nostr[..], np[..]); xonly(wallet[..], wp[..]);
  show("nostr key m/44'/1237'/0'/0/0", np[..]);
  show("wallet key m/86'/0'/0'/0/0", wp[..]);
  #assert taproot_output_key_vartime(wp[..], wp[0..<0], wq[..]) >= 0;
  show("the wallet key's BIP-86 output key", wq[..]);
  #assert taproot_output_key_vartime(np[..], np[0..<0], nq[..]) >= 0;
  show("the nostr key's BIP-86 output key", nq[..]);

  // A 32-byte message signed on request: here a TapSighash-tagged hash
  // standing for a sighash.
  print("A raw 32-byte signature request to the Nostr key\n");
  spend := "a spend the agent never approved";
  bytes: [64] u8;
  for j: 0..<spend.count { bytes[j] = spend[j]; }
  h: [32] u8; sig: [64] u8;
  #assert bitcoin_tagged_checked(BH_TAPSIGHASH, bytes[0..<spend.count], h[..], false) == SHA_OK;
  show("message", h[..]);
  sign(nostr[..], h[..], sig[..]);
  print("valid under the nostr key, as in a <key> OP_CHECKSIG leaf: %\n", schnorr_verify_vartime(h[..], np[..], sig[..]));
  print("valid under the nostr key's output key: %\n", schnorr_verify_vartime(h[..], nq[..], sig[..]));
  print("valid under the wallet key: %\n", schnorr_verify_vartime(h[..], wp[..], sig[..]));
  print("valid under the wallet key's output key: %\n", schnorr_verify_vartime(h[..], wq[..], sig[..]));
  for j: 0..<32 { nostr[j] = 0; wallet[j] = 0; }
  for j: 0..<64 { seed[j] = 0; }
}
```

Run it:

```shell
coax/bin/coax run bitcoin/examples/one-seed.coax --run-steps 250000000
```

It prints:

```text
The card of NIP-06's first phrase, account 0
nostr m/44h/1237h/0h/0/0 npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu
wpkh([ae957285/84h/0h/0h]xpub6Byf6wvtJs2NobE6DV9fCPmNXCa5fxZ6EhcEhgXpJKfqr2Bzyeznmq26kTxUCyNgUS9ZQKXPUTFsWMqcpEuHaAsGmoYRbkF2ZXxim5fsGT1/<0;1>/*)#mcpgfhwv
tr([ae957285/86h/0h/0h]xpub6BkvVbW4aaRfBmgjc4Yg1oG7hDMs1sL6a3P6pP1LJtRQKB9fe22RCbp3ifSSMye9ziPQUH2YRk38oqzKUiXoKCb1R6keV3X57qBFDx4e7eG/<0;1>/*)#a8gvn82u
npub is NIP-06's: true
Addresses from the card alone
wpkh 0/0: bc1q8zxz5kl6q30y2mzhx86gcwcz0t0hgzl2f2jpm5
wpkh 0/1: bc1qc42v7hwyvumnwglrlrr3xufwjj7qhmq7xhzay8
wpkh 1/0: bc1qspfxfzq76qdh3xlcanasx94ag9t0qxm8jsapmf
tr 0/0: bc1ppp2h2mqq4r8al4n8y800qfe53hvuamg3eqz4kz299eyqx6u7c9esfpualz
tr 0/1: bc1pqdqz3cgeqze7mj4c3ru3ltt6yh9ac2qpsxyeufzc5t5pnx5tmrvqcq3czh
tr 1/0: bc1ps5t2n3nq5mk2qxrgmzstehn7fyljftxwnt27ah3lne04uvd0nslshqxr0r
Restore
same words, no passphrase: the same card
same words, passphrase x: mismatch
same words, account 1: mismatch
last word beauty: words refused
The card of BIP-84's and BIP-86's phrase
nostr m/44h/1237h/0h/0/0 npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7
wpkh([73c5da0a/84h/0h/0h]xpub6CatWdiZiodmUeTDp8LT5or8nmbKNcuyvz7WyksVFkKB4RHwCD3XyuvPEbvqAQY3rAPshWcMLoP2fMFMKHPJ4ZeZXYVUhLv1VMrjPC7PW6V/<0;1>/*)#qf45pmyh
tr([73c5da0a/86h/0h/0h]xpub6BgBgsespWvERF3LHQu6CnqdvfEvtMcQjYrcRzx53QJjSxarj2afYWcLteoGVky7D3UKDP9QyrLprQ3VCECoY49yfdDEHGCtMMj92pReUsQ/<0;1>/*)#xf07c0qd
wpkh 0/0: bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu
wpkh 0/1: bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g
wpkh 1/0: bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el
tr 0/0: bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr
tr 0/1: bc1p4qhjn9zdvkux4e44uhx8tc55attvtyu358kutcqkudyccelu0was9fqzwh
tr 1/0: bc1p3qkhfews2uk44qtvauqyr2ttdsw7svhkl9nkm9s9c3x4ax5h60wqwruhk7
The passphrase TREZOR
seed: c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04
nostr m/44h/1237h/0h/0/0 npub17v46v50fwtwq5rdc6xqxjznk6w2pqrk2k6u4jjcf2r5h5ua7k7hq2q3erf
wpkh([b4e3f5ed/84h/0h/0h]xpub6Crgkie5Rb7wDabkf4Uf6A2qnuERMA3p2QrnmHNQDrsXTaGvz9zugU38Apne8WqrcbSjdLwbhtfHrzWjNCJPVAkkNoQhMfzhBm8rKMA8KxH/<0;1>/*)#72x53tc6
tr([b4e3f5ed/86h/0h/0h]xpub6CvFPYhCVDRVnfFhxGZH5WXD79vRUoR1Mqh7o1uSaG7Qya2Z4vm67vihWr8io7zcfy4ehfAuZ5k2Bzk8pT1wAehqhZVEyfHwsWVMud7omCf/<0;1>/*)#nh3grzr2
The same phrase on testnet
nostr m/44h/1237h/0h/0/0 npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7
wpkh([73c5da0a/84h/1h/0h]tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*)#lxek0ef2
tr([73c5da0a/86h/1h/0h]tpubDDfvzhdVV4unsoKt5aE6dcsNsfeWbTgmLZPi8LQDYU2xixrYemMfWJ3BaVneH3u7DBQePdTwhpybaKRU95pi6PMUtLPBJLVQRpzEnjfjZzX/<0;1>/*)#ewv9va06
npub line unchanged: true
Separate keys
nostr key m/44'/1237'/0'/0/0: e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f
wallet key m/86'/0'/0'/0/0: cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115
the wallet key's BIP-86 output key: a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c
the nostr key's BIP-86 output key: 6d90a6e15606a12b3b1ca1271744bdbf7c062fb0092bf3afa051b859761d0927
A raw 32-byte signature request to the Nostr key
message: 852311f6554abb9a0dbde68a35c13fbbfdf67003bda3764c64839a445bb13869
valid under the nostr key, as in a <key> OP_CHECKSIG leaf: true
valid under the nostr key's output key: false
valid under the wallet key: false
valid under the wallet key's output key: false
```

The expected output is in
[bitcoin/fixtures/one-seed.out](../fixtures/one-seed.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/one-seed.coax](../examples/checks/one-seed.coax),
which loads the example:

- the Nostr key of accounts 0, 1 and 2 is chapter 33's key at
  `m/44'/1237'/a'/0/0`;
- each descriptor's receive and change scripts are chapter 33's
  `path_account_script_ct` scripts from the same account `xpub`, for
  `wpkh` and for `tr`;
- a signature by the Nostr key verifies under the Nostr key and never
  under a BIP-84 or BIP-86 wallet key or its output key.

<!-- source: bitcoin/examples/checks/one-seed.coax check -->
```jai
#load "../one-seed.coax";

// BIP-84's and BIP-86's seed (chapter 33's), so the checks below skip
// PBKDF2.
ABANDON_SEED :: "5eb00bbddcf069084889a8ab9155568165f5c453ccb85e70811aaed6f6da5fc19a5ac40b389cd370d086206dec8aa6c43daea6690f20ad3d8d48b2d2ce9e38e4";
seed_bytes :: (out: [] u8) -> [] u8 {
  digits := "0123456789abcdef";
  for i: 0..<64 {
    hi := 0; lo := 0;
    for d: 0..<16 { if digits[d] == ABANDON_SEED[2 * i] { hi = d; } if digits[d] == ABANDON_SEED[2 * i + 1] { lo = d; } }
    out[i] = cast(u8) (hi * 16 + lo);
  }
  return out[0..<64];
}

#forall "the Nostr key of account a is the key at m/44'/1237'/a'/0/0" a: 0..<3 #steps 1000000000 {
  sb: [64] u8; work: [OS_WORK_BYTES] u8; key: [32] u8; want: [32] u8;
  seed := seed_bytes(sb[..]);
  #assert oneseed_nostr_ct(seed, a, work[..], key[..]) == 32;
  path := ifx a == 0 then "m/44'/1237'/0'/0/0" else ifx a == 1 then "m/44'/1237'/1'/0/0" else "m/44'/1237'/2'/0/0";
  key_at(seed, path, want[..]);
  for j: 0..<32 { #assert key[j] == want[j]; }
}
// The script of a card descriptor at branch and index, and chapter 33's
// script of the same account xpub; true when they are the same.
descriptor_is_path :: (purpose: int, branch: int, index: int) -> bool {
  sb: [64] u8; work: [OS_WORK_BYTES] u8; xpub: [BIP32_NODE_BYTES] u8; fp: [4] u8; line: [OS_LINE_MAX] u8;
  s1: [AD_SCRIPT_MAX] u8; s2: [AD_SCRIPT_MAX] u8;
  seed := seed_bytes(sb[..]);
  #assert oneseed_account_ct(seed, SW_MAINNET, purpose, 0, work[..], xpub[..], fp[..]) == BIP32_NODE_BYTES;
  n := oneseed_descriptor_ct(purpose, SW_MAINNET, 0, fp[..], xpub[..], work[..], line[..]);
  a := descriptor_script_vartime(cast(string) line[0..<n], SW_MAINNET, branch, index, 0, s1[..]);
  b := path_account_script_ct(purpose, xpub[..], branch, index, work[..], s2[..]);
  if a <= 0 || a != b { return false; }
  for j: 0..<a { if s1[j] != s2[j] { return false; } }
  return true;
}
#forall "the wpkh descriptor's receive and change scripts are chapter 33's" branch: 0..<2 #steps 4000000000 {
  #assert descriptor_is_path(84, branch, 3 * branch + 1);
}
#forall "the tr descriptor's receive and change scripts are chapter 33's" branch: 0..<2 #steps 4000000000 {
  #assert descriptor_is_path(86, branch, 3 * branch + 1);
}
#forall "a signature by the Nostr key is never valid under a wallet key" i: 0..<2 #steps 8000000000 {
  sb: [64] u8; work: [OS_WORK_BYTES] u8; nostr: [32] u8; wallet: [32] u8; np: [32] u8; wp: [32] u8; wq: [32] u8;
  h: [32] u8; sig: [64] u8;
  seed := seed_bytes(sb[..]);
  #assert oneseed_nostr_ct(seed, 0, work[..], nostr[..]) == 32;
  path := ifx i == 0 then "m/84'/0'/0'/0/0" else "m/86'/0'/0'/0/0";
  key_at(seed, path, wallet[..]);
  xonly(nostr[..], np[..]); xonly(wallet[..], wp[..]);
  #assert taproot_output_key_vartime(wp[..], wp[0..<0], wq[..]) >= 0;
  for j: 0..<32 { h[j] = cast(u8) (j * 7 + i); }
  sign(nostr[..], h[..], sig[..]);
  #assert schnorr_verify_vartime(h[..], np[..], sig[..]);
  #assert !schnorr_verify_vartime(h[..], wp[..], sig[..]) && !schnorr_verify_vartime(h[..], wq[..], sig[..]);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/one-seed.coax --run-steps 250000000
```

`coax test` runs 403 checks, including the loaded files' own checks, in
about 135 CPU seconds on 2026-11-03.

### The library's own checks

`oneseed.coax` is an `#authority` file. Its checks run NIP-06's two
vectors, BIP-39's vector 0 with `TREZOR`, BIP-86's account `xpub` and
fingerprint, the six published BIP-84 and BIP-86 addresses from the card
alone, the five restores, and every refusal of every procedure, with
nothing written on a refusal.

```shell
coax/bin/coax test coax/lib/bitcoin/oneseed.coax --run-steps 250000000
```

`coax test` runs 271 checks, including the loaded files' own checks, in
about 100 CPU seconds on 2026-11-03. Each restore check builds two cards,
so that it stays inside the compile-time work limit. Mutation testing: not
run (owner speed order).

### A Python implementation

[oneseed.py](../../coax/tests/bitcoin/oneseed.py) checks the hash of
every pinned document it reads (BIP-39's vectors and word list, BIP-84,
BIP-86, NIP-06 and the secp256k1lab `bip340.py` pinned with chapter 102)
and that the published values above appear in them. It then writes the
example's transcript with code that shares nothing with Coax: the BIP-39
checksum and `hashlib.pbkdf2_hmac`, chapter 32's and chapter 29's Python
references for the nodes and addresses, chapter 30's for the descriptor
checksum, a bech32 encoder for the `npub`, and secp256k1lab's BIP-340.

```shell
python3 -I coax/tests/bitcoin/oneseed.py --coax coax/bin/coax
```

On 2026-11-03 it passes: every pinned document and published value is
there, its transcript and the fixture agree on all 48 lines, and the
example prints them in debug and release builds.

## What this chapter leaves out

- Other Nostr key paths. NIP-06 allows clients other paths "for their
  own other purposes"; this chapter uses only `m/44'/1237'/a'/0/0`.
- Rescanning the chain for a restored wallet's coins, which needs a node:
  chapter 99.
- The treasury's other signers: chapter 122.
- NFKD normalization of non-ASCII words and passphrases, which chapter 31
  refuses.

## Summary

- One BIP-39 phrase gives an agent's Nostr key at `m/44'/1237'/a'/0/0`
  and its wallets at `m/84'/c'/a'` and `m/86'/c'/a'`.
- The card records the `npub` and both wallets as descriptors with key
  origins and checksums; it holds no secret but shows every coin.
- A restore rebuilds the card and compares: a wrong passphrase, account
  or network gives another card, and a wrong checksum refuses the words.
- One key must not sign for both protocols: a raw 32-byte signature
  request to a Nostr signer can be a script-path spend.
- Hardened branches from the master keep the branches independent; only
  the words and the passphrase give them all.

## Dependencies

- [31. Seed phrases](ch031-seed-phrases.md)
- [33. Derivation paths](ch033-derivation-paths.md)
- Nostr book: [46. Keys from a seed phrase](../../nostr/book/ch046-keys-from-a-seed-phrase.md)

## Sources

- [BIP-39](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
  (mnemonic code), with its vectors, pinned in
  [coax/tests/vectors/upstream/bip39/](../../coax/tests/vectors/upstream/bip39/bip-0039.mediawiki).
- [BIP-84](https://github.com/bitcoin/bips/blob/master/bip-0084.mediawiki)
  and [BIP-86](https://github.com/bitcoin/bips/blob/master/bip-0086.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip84/](../../coax/tests/vectors/upstream/bip84/bip-0084.mediawiki)
  and
  [coax/tests/vectors/upstream/bip86/](../../coax/tests/vectors/upstream/bip86/bip-0086.mediawiki).
- [NIP-06](../../nostr/nips/official/06.md) (basic key derivation from
  mnemonic seed phrase).
- [BIP-380](https://github.com/bitcoin/bips/blob/master/bip-0380.mediawiki)
  (descriptors) and
  [BIP-389](https://github.com/bitcoin/bips/blob/master/bip-0389.mediawiki)
  (multipath descriptor key expressions).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
