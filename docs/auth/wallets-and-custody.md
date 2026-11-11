# Wallets and custody

Status: design, 2026-11-11. Part of [OpenAgents authentication](README.md).
Money is Bitcoin only: on-chain, Lightning, Spark and Taproot Assets.

## Where we start

| Piece | Today | Source |
| --- | --- | --- |
| A person's wallet | Breez Spark in the iOS app, words in the Keychain (`com.openagents.app.spark`, `seed-v1`, this device only) | [docs/lightning/](../lightning/README.md) |
| Spark's trust model | "Acceptable for a person's spending wallet with small balances ... Not acceptable for a treasury" (owner, 2026-09-28) | [breez-spark.md](../lightning/breez-spark.md) |
| Our own wallet | Not built: no descriptor wallet, no miniscript yet | [build-everything audit](../research/bitcoin/2026-11-11-build-everything-audit.md) §2 |
| One seed for Nostr and Bitcoin | Built and checked in Coax: NIP-06, BIP-84 and BIP-86 from one phrase, plus a public card | [book ch121](../../bitcoin/book/ch121-one-seed-for-nostr-and-bitcoin.md) |
| Agent treasuries | 2-of-3 (agent, owner, recovery) with a timelocked recovery path, plus a capped hot wallet; scripts and PSBT flow run, network half waits for the node | [book ch122](../../bitcoin/book/ch122-agent-treasuries.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md) |
| Encrypted personal data | NIP-VAULT: key slots (device, Nostr key, passkey PRF, recovery code) over one master key; tier `user` built in Coax | [NIP-VAULT](../../nostr/nips/openagents/NIP-VAULT.md), `coax/lib/nostr/vault.coax` |
| Custodial balances we hold | Inference credit in msat per key (`inference.credit`), and maker tips received into and paid out of our Spark wallet (`makers.tips`) | `coax/web/api/migrations/0066_inference_credit.sql`, `0073_maker_tips.sql` |
| Operator treasury | The MDK treasury container and payout dispatcher (#4698, #4699, #4700, closed) were an operator wallet, not user custody | GitHub issues |

## The options

| Model | Who can move the coins | What we store | If the person loses every device | If we're breached | Regulatory weight |
| --- | --- | --- | --- | --- | --- |
| **A. Self-custody, seed on device only** | The person | Nothing | Funds lost unless they wrote the words down | Nothing to take | Lightest |
| **B. Self-custody with an encrypted backup we can't open** | The person | Ciphertext of the seed, sealed under the person's own factors (passkey PRF, recovery code, Nostr key) | Restore with a synced passkey or the recovery code | Ciphertext only; strength depends on the person's factors | Light: we hold no key |
| **C. Collaborative custody (2-of-3, we hold one key)** | Any two of person, device, us | One key share and the wallet descriptor | Person + us recover, after a delay | One share is useless alone | Middle: we can block but not move funds alone |
| **D. Custodial balance (our ledger)** | Us | The balance; the coins are in our wallet | Log in by any method; balance intact | Everything in the hot wallet is at risk | Heavy if withdrawable or transferable |

"Semi-custodial" in the owner's note most closely matches **B** (an
encrypted seed we store but can't decrypt). That is still self-custody: we
are a blob store. **C** is the arrangement usually meant by collaborative
custody.

## Recommendation

1. **People: B by default, C later for large balances.** The wallet's seed
   is made on the device and never sent to us in the clear. We store a
   NIP-VAULT object holding it, with at least two key slots:
   - a **passkey PRF** slot, so a synced passkey (iCloud Keychain, Google
     Password Manager) restores the wallet on a new phone with one Face ID;
   - a **recovery** slot from the account's 24-word recovery code
     (scrypt, as NIP-VAULT specifies);
   - optionally a **Nostr key** slot (NIP-44 to self).

   NIP-VAULT already requires a second slot before it accepts data, because
   PRF support differs by platform. When our own wallet can run a
   2-of-3, offer C for balances above a threshold the person picks.
2. **Custodial only as service credit (D, narrowed).** Credit for
   inference and other OpenAgents services may be prepaid in sats and kept
   in our ledger, capped (say 100,000 sats an account, shown), spendable
   only on our services, never withdrawable or sendable to another person.
   That is prepaid service, not a wallet.
3. **Maker tips pay the maker directly (decision D6, decided).** Today a
   player pays our wallet and we pay the maker. That is accepting value
   from one person and transmitting it to another, which is what
   money-transmission rules cover. So tips and upgrade shares change to go
   straight from the player's wallet to the maker's Lightning Address,
   with us only fetching and showing the invoice. The first-tip bonus
   becomes a separate payment from our wallet to the maker.
4. **`name@openagents.com` Lightning Addresses, paying the person's own
   wallet.** Anyone can claim one in one tap. The invoice is made for the
   person's own Spark wallet, which receives even while the phone is
   offline, or the address forwards to a Lightning Address the person
   already has. We never receive or hold the money. See
   [Lightning Addresses](lightning-addresses.md).
5. **Agents: ch122's split, keys separate from the owner's.** An agent
   holds a hot wallet capped by its grant, and co-signs refills from a
   2-of-3 treasury (agent, owner, recovery key with a 30-day timelock).
   The agent's one-seed card (ch121) gives its Nostr name and wallets from
   one backup, which suits an agent; the owner's keys never enter the
   agent's host.

## One seed for login and wallet

Chapter 121 shows one BIP-39 phrase can give a Nostr key
(`m/44'/1237'/a'/0/0`) and wallets (`m/84'/c'/a'`, `m/86'/c'/a'`). The
hardened paths keep the branches apart: a leaked Nostr key or an `xpub`
doesn't expose the other branches. What it can't fix is that **whoever
has the words has everything**.

For people we don't make the login identity the wallet seed by default:

- In this design, the account isn't a key at all: it's a row with many
  authenticators. A Nostr login key gets used often (every signed request,
  every NIP-46 approval, maybe pasted into other Nostr apps). A wallet
  seed should be touched rarely.
- The pre-nuke design gave the 12 words to every linked computer, which
  made every computer a holder of both identity and wallet
  (`git show 0b401561a0^:docs/audits/2026-10-17-spark-wallet.md`). We
  don't repeat it: each device has its own device key.
- A raw-signature request to a Nostr signer can be a Bitcoin spend if the
  same key ever serves both (ch121), so a signer must never sign for both.

**Opt-in:** a person who wants one backup for their npub and their coins
can turn on "Use my wallet words for my Nostr identity". The app then
derives the NIP-06 key from the wallet seed and links it as a `nostr`
authenticator, and shows the single-point-of-failure warning once.

## Step-up for spends

| Action | Needs |
| --- | --- |
| Spend under the person's daily limit (default 20,000 sats, shown, changeable) | An unlocked app (device key) |
| Spend over the limit, or to a new destination over 100,000 sats | L2: Face ID on the device key, or a passkey |
| Raise the daily limit | `L2-fresh`; takes effect after 24 hours, with a notice on every channel |
| Show the seed or export the wallet | `L2-fresh` on the device that holds it |
| Create or widen an agent's spend grant | `L2-fresh`, human only |
| Agent spend | Within its grant's per-payment and per-day caps; refills by the ch122 rule |
| Any spend from a session that came from email alone on an account with stronger factors | Refused until the 72-hour recovery completes |

Spending service credit (model D) follows the same daily limit.

## Recovery when the device is lost

| They still have | Account | Wallet |
| --- | --- | --- |
| A synced passkey | Log in with it | PRF slot opens the backup; restore on the new device |
| The recovery code | Log in with it | Recovery slot opens the backup |
| The wallet's written words | Log in by email or Nostr key | Type the words |
| Only email | Log in at L1; recovery wait for stronger actions | **Lost**, unless they had a synced passkey or the code. We can't open the backup |
| Collaborative custody (C) | Any way in | We co-sign a sweep to a new wallet after the 72-hour wait |

The app asks for a backup the moment the wallet first holds money, not at
install: "Your wallet has 5,000 sats. If you lose this phone, you need a
backup to get them back." One tap saves a passkey slot.

## Regulatory notes

Not legal advice; a lawyer reviews this before any custodial feature
ships. FinCEN's 2019 guidance (FIN-2019-G001) treats providers of hosted
(custodial) wallets as money transmitters, and providers of unhosted
wallet software, including multisig providers that only add a second
authorization key, generally not
([summary, Jones Day](https://www.jonesday.com/en/insights/2019/06/fincen-consolidates-guidance)).
States add their own licensing on top. So:

| Feature | Risk |
| --- | --- |
| B: device seed, encrypted backup we can't open | Low |
| C: we hold one of three keys and can't move funds alone | Low to medium; keep our key's role to co-signing |
| D as non-withdrawable service credit | Low to medium (prepaid access rules) |
| D as a withdrawable or transferable balance | High: money transmission |
| Maker tips through our wallet | Medium to high; replaced by direct payment (D6) |
| `@openagents.com` addresses paying the person's own wallet, or forwarding to their own address | Low: we answer with an invoice someone else's wallet made; we never hold the money ([Lightning Addresses](lightning-addresses.md#money-transmission)) |
| `@openagents.com` addresses received into our wallet and held | High: refused |
