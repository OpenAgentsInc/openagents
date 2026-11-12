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
| Custodial balances we hold | Inference credit in msat per key (`inference.credit`). Maker tips no longer pass through our Spark wallet: since #13210 they go from the player's wallet to the maker's own Lightning Address | `coax/web/api/migrations/0066_inference_credit.sql`, `0154_maker_direct_pay.sql` |
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
   - a **recovery** slot from a 24-word recovery code (scrypt, as
     NIP-VAULT specifies). As built, this is the wallet's own code, made
     on the phone, not the account's
     ([why](#the-recovery-code-is-the-wallets-own));
   - optionally a **Nostr key** slot (NIP-44 to self).

   NIP-VAULT already requires a second slot before it accepts data, because
   PRF support differs by platform. When our own wallet can run a
   2-of-3, offer C for balances above a threshold the person picks.
2. **Custodial only as service credit (D, narrowed).** Credit for
   inference and other OpenAgents services may be prepaid in sats and kept
   in our ledger, capped (say 100,000 sats an account, shown), spendable
   only on our services, never withdrawable or sendable to another person.
   That is prepaid service, not a wallet.
3. **Maker tips pay the maker directly (decision D6, built in #13210).**
   Before, a player paid our wallet and we paid the maker. That is
   accepting value from one person and transmitting it to another, which
   is what money-transmission rules cover. Tips and upgrade purchases now
   go straight from the player's wallet to the maker's Lightning Address,
   with us only fetching, checking and showing the invoice. The first-tip
   bonus is a separate payment from our wallet to the maker.
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

## What is built

Built in #13217. The format is
[NIP-VAULT's wallet seed backup](../../nostr/nips/openagents/NIP-VAULT.md#wallet-seed-backup);
the code is `coax/lib/nostr/vault-wallet.coax` (the apps seal and open
with it, and the API reads public fields with it), `coax/web/api/wallet.c`
with its rules in `wallet-policy.coax`, and `verse/wallet-backup.coax` (with `verse/wallet-vault.coax`) for
what the iPhone app shows and does.

### What our database holds

One row for an account's backup (`auth.wallet_backups`, migration 0163):

| Column | What | Secret? |
| --- | --- | --- |
| `account`, `created_at`, `updated_at` | Whose it is and when | No |
| `vault` | A random id the phone made | No |
| `epoch` | A counter that goes up by one with each change | No |
| `object` | The seed and a small record, AES-256-GCM under a random key (`DEK`) made on the phone | Ciphertext |
| `slots` | For each way to open it: its id, method, label, time and public parameters, and the vault key (`VMK`) under a key derived from that method's secret | Ciphertext and public fields |

There is no column, here or in any other table, log or notice, for a PRF
output, a recovery code, a hash of one, or any key. That is why we can't
open a backup:

- The seed is under the `DEK`. The `DEK` is under a key derived from the
  `VMK` (HKDF-SHA256). Both are 32 random bytes from the phone.
- The `VMK` is under each slot's key: HKDF-SHA256 of the method's secret
  with the slot's random id as salt.
- A **passkey** slot's secret is the passkey's PRF output: 32 bytes the
  authenticator computes inside itself (HMAC-SHA256 under a key that
  never leaves it). We verify a passkey's signature; a signature says
  nothing about its PRF output. The PRF input is public and fixed.
- A **recovery code** slot's secret is scrypt of 24 words carrying 256
  random bits. The phone makes the words, shows them once and forgets
  them. We store the scrypt salt and cost, which are public: they slow a
  guess, and there are 2^256 codes to guess.
- So a search for any slot's key is a search of 2^256 values, and nothing
  we hold shortens it. `coax/web/api/tests/wallet_checks.coax` holds the
  database to this: the row's columns are exactly the ones above, and the
  test's PRF output, recovery secret, seed and vault key appear in no row
  of the backup, the audit log or a notice.

Database backups keep an old row until they age out (35 days of
point-in-time recovery, and the dumps). An old row is the same
ciphertext, under the same rule.

### The recovery code is the wallet's own

The design above said the recovery slot would use the account's 24-word
recovery code. It can't: we make that code and we see it each time it is
typed, so a slot under it would be one we could open. The wallet backup
has **its own recovery code**: 24 words made on the phone from the
system's randomness, shown once, never sent to us and never asked for on
the website. A person with both keeps two codes; the app says which is
which.

### Slots

A backup counts with a recovery-code slot and a passkey slot. A Nostr-key
slot (32 random bytes sealed with NIP-44 to the person's own key) is in
the format, the library and the server, with vectors. The iPhone app
doesn't offer it: its Nostr key lives in the same phone's Keychain, so a
slot under it is lost with the phone. A wallet whose Nostr key lives
somewhere else (a signer on another device) can add one.

Adding a slot opens the `VMK` with an existing slot and seals the record
again at the next epoch; the seed never leaves the phone in the clear,
and we never see the `VMK`. Removing a slot for good means a new backup
(a new `VMK` and a new recovery code): the old slot then opens nothing.
If a slot's secret may have leaked, the safe step is a new wallet.

### What needs a fresh proof that it is you

| Action | Needs |
| --- | --- |
| See the limit, what was spent, and the backup's public facts | Any login |
| Read the backup's ciphertext, store or change a backup, delete it | `L2-fresh` (the gate and the policy both), and every way in is told |
| Lower the daily limit | Any login that isn't an inbox-only one; it counts at once |
| Raise the daily limit | `L2-fresh`; it counts 24 hours later (the database refuses sooner), every way in is told with a cancel link, and any login of the account cancels it |
| A payment within the limit | Nothing more than the phone's own Face ID check |
| A payment past the limit, or over 100,000 sats to somewhere new | `L2-fresh`: the passkey prompt |

### Where the limit is enforced, and where it isn't

The wallet's keys are on the phone. A payment is signed there and goes to
the Lightning or Bitcoin network without passing us, so **we can't stop
one**. What happens instead:

- **The app enforces the limit.** Before each payment it asks
  `POST /api/wallet/spend`. The server keeps the limit and the last 24
  hours' count, and answers "go" or "prove it is you first". The app
  obeys. Past the limit it shows the passkey prompt, whose user
  verification the phone's system does and our server checks.
- **When the app can't reach us** it uses its own count of the last 24
  hours against the last limit it heard, and doesn't pay past it.
- **A phone that has had a limit doesn't pay while logged out.** Freezing
  an account ends every login, so a stolen phone's app stops paying. The
  last limit is kept in the Keychain, so deleting the app and installing
  it again doesn't forget it.
- **The server enforces who changes the limit**: a raise needs
  `L2-fresh`, waits 24 hours and tells every way in.

What this doesn't hold: someone with the wallet's words (they can use any
wallet software), or changed software on the phone that skips the check.
It also doesn't hold against us: a compromised server could answer "go"
to every payment, which turns the limit off. It still can't move money,
because it never has the keys.
The limit protects against someone with the unlocked phone or the login,
not against someone with the seed. The app and
[/settings/wallet](https://openagents.com/settings/wallet/index.md) say
so. The limit is also not applied to service credit (model D) yet; that
has its own per-key accounting (#13330).

### Threats and their checks

| Threat | What holds | Check |
| --- | --- | --- |
| Our database is stolen | Only ciphertext and public fields are in it; each slot is a 2^256 search | `wallet_checks.coax` "our database stolen" (three checks); the vectors |
| A malicious server serves another account's backup, or an old slot | The apps refuse a record for another account (`WB_ACCOUNT`) and one opened through a slot it doesn't list (`WB_UNLISTED`) | `vault-wallet.coax`; `wallet_checks.coax` "a swapped vault"; the iPhone unit tests |
| A malicious server serves changed JavaScript | The website never handles the seed, the PRF output or the wallet's recovery code: it shows the limit and the backup's public facts, and says it never asks for the code. Sealing and opening happen only in the app | The page (`apps/openagents-web/wallet.coax`) has no input for a code and no call that reads ciphertext |
| A lost passkey | The recovery code opens the backup; the wallet on the phone keeps working; a new backup replaces the old | iPhone unit tests; `wallet_checks.coax` "a lost passkey" |
| A lost recovery code | The passkey opens the backup; "Back up again" makes a new code and a new vault | iPhone unit tests |
| Both lost, and the phone too | The money is lost. Nobody can open the backup, us included; the app says so before the code is dismissed | By design |
| A stolen unlocked phone | Every payment needs Face ID or the passcode (as before); past the daily limit, the passkey; freezing the account ends the login and the app stops paying | `verse/wallet-backup.coax`; iPhone unit tests |
| A rollback | The server stores a backup only at the next epoch (the database refuses otherwise); the apps refuse an epoch that isn't the one named, and one older than they have seen. A new phone can't tell an old epoch the server kept: it restores the same seed | `wallet-policy.coax`, `wallet_checks.coax` "a rollback" (four checks), `vault-wallet.coax` |
| A slot added by someone with a session and no step-up | Storing needs `L2-fresh`; and a slot is only made by someone who opened the `VMK` | `wallet_checks.coax` "a slot added without step-up" |
| A session thief raises the limit | `L2-fresh`, 24 hours, a notice to every way in with a cancel link | `wallet_checks.coax` "limit raising by a session thief" and the six checks after it |
| An inbox thief | An email login on an account with a stronger way in can't spend, store, read ciphertext or raise | `wallet_checks.coax` "an inbox thief" |

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

Built in #13217: the switch is in the app's Wallet safety screen, off
unless turned on. Coax derives the key (`m/44'/1237'/0'/0/0`), signs the
link challenge and wipes the key (`verse/wallet-vault.coax`); Swift never
holds it. Turning it off removes that key from the account. Agents use
one seed by default: `openagents nostr new` makes 12 words and derives the
agent's NIP-06 key from them, and the same words derive its wallets
([ch121](../../bitcoin/book/ch121-one-seed-for-nostr-and-bitcoin.md)'s
card), so one backup holds an agent's identity and its coins.

## Step-up for spends

| Action | Needs |
| --- | --- |
| Spend under the person's daily limit (default 20,000 sats, shown, changeable) | An unlocked app, and the phone's Face ID check before each payment |
| Spend over the limit, or to a new destination over 100,000 sats | `L2-fresh`: the passkey prompt for this payment |
| Raise the daily limit | `L2-fresh`; takes effect after 24 hours, with a notice on every channel |
| Show the seed or export the wallet | `L2-fresh` on the device that holds it |
| Create or widen an agent's spend grant | `L2-fresh`, human only |
| Agent spend | Within its grant's per-payment and per-day caps; refills by the ch122 rule |
| Any spend from a session that came from email alone on an account with stronger factors | Refused until the 72-hour recovery completes |

Spending service credit (model D) follows the same daily limit.

## Recovery when the device is lost

| They still have | Account | Wallet |
| --- | --- | --- |
| A synced passkey | Log in with it | The same prompt opens the backup: the wallet is back on the new device |
| The account's recovery code and the wallet's recovery code | Log in with the account's | Type the wallet's code in the app: it opens the backup |
| A Nostr key and the wallet's recovery code | Log in with the key | Type the wallet's code in the app |
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
