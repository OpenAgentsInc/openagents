# Lightning Addresses

Status: design, 2026-11-11. Part of [OpenAgents authentication](README.md).
Decided by the owner the same day (decision D6).

> yes good to pay peoples lightning addresses but lets ALSO offer
> @openagents.com lightning addresses and let people easily opt to get
> that from us.

## In one page

- **Anyone with an account can claim `name@openagents.com`** in one tap,
  in the app or on the web. It is off until they claim it. Agents can have
  one too.
- **The money lands in the person's own wallet, never in ours.** By
  default the invoice is made for the person's own Spark wallet, which
  receives even while the phone is off. If they already have a Lightning
  Address somewhere else, ours forwards to it instead. We never receive
  and hold money for anyone.
- **We pay makers' own Lightning Addresses directly.** A tip or an upgrade
  goes from the player's wallet to the maker's wallet. It doesn't pass
  through ours.
- **Names are permanent once paid.** A name that has ever received a
  payment is never given to anyone else, so a saved contact can't start
  paying a stranger.
- **Changing where the money goes is the most guarded action we have.**
  It needs a fresh strong login, a proof from the new wallet, and a
  24-hour wait with a notice on every channel.
- **Every limit is on the claim card** before the person needs it.
- **Bitcoin only:** Lightning invoices paying bitcoin, nothing else.

## How a Lightning Address works

A Lightning Address looks like an email address and is a pointer to an
LNURL-pay endpoint ([LUD-16](https://github.com/lnurl/luds/blob/luds/16.md),
[lightningaddress.com](https://lightningaddress.com), reference repo
`projects/repos/lightning-address`). Each payment gets a fresh invoice,
so reusing one address is safe
([Strike](https://strike.me/learn/what-is-a-lightning-address/)).

1. **Lookup.** A wallet paying `alice@openagents.com` sends
   `GET https://openagents.com/.well-known/lnurlp/alice`. We answer
   ([LUD-06](https://github.com/lnurl/luds/blob/luds/06.md) step 3):

   ```json
   {
     "tag": "payRequest",
     "callback": "https://openagents.com/lnurlp/alice/invoice",
     "minSendable": 1000,
     "maxSendable": 1000000000,
     "metadata": "[[\"text/plain\",\"Pay alice@openagents.com\"],[\"text/identifier\",\"alice@openagents.com\"]]",
     "commentAllowed": 280,
     "payerData": {"name": {"mandatory": false}, "pubkey": {"mandatory": false}},
     "allowsNostr": true,
     "nostrPubkey": "<our zap receipt key, hex>"
   }
   ```

   Amounts are in millisatoshis. `metadata` must carry one `text/plain`
   entry, and LUD-16 requires a `text/identifier` entry (not
   `text/email`: the name isn't a mailbox). We send no image.
2. **Callback.** The wallet calls
   `GET <callback>?amount=<msat>&comment=<text>` (comment:
   [LUD-12](https://github.com/lnurl/luds/blob/luds/12.md); optional payer
   name or pubkey: [LUD-18](https://github.com/lnurl/luds/blob/luds/18.md)).
   We check the amount against the limits and answer:

   ```json
   {
     "pr": "lnbc...",
     "routes": [],
     "successAction": {"tag": "message", "message": "Paid alice@openagents.com"},
     "verify": "https://openagents.com/lnurlp/verify/<payment hash>"
   }
   ```

   `successAction` is the `message` kind only
   ([LUD-09](https://github.com/lnurl/luds/blob/luds/09.md)): a wallet must
   refuse to pay when it doesn't know a tag, and every wallet knows
   `message`. `verify` lets anyone check settlement
   ([LUD-21](https://github.com/lnurl/luds/blob/luds/21.md)).
3. **Invoice.** The invoice is BOLT11, mainnet, for exactly the amount
   asked, expiring in 10 minutes, with its description hash set to
   SHA-256 of the `metadata` string. The current LUD-06 text no longer
   requires that hash, but many wallets still check it, so we always set
   it. The wallet checks the amount and pays.
4. **Zaps** ([NIP-57](../../nostr/nips/official/57.md)). If the callback
   carries a `nostr` parameter, it is a signed kind 9734 zap request. We
   validate it as Appendix D says (signature, one `p` tag, at most one `e`,
   `amount` equal to the query's), make the invoice's description that
   request alone, and once the invoice is paid, publish a kind 9735 zap
   receipt signed by `nostrPubkey` to the relays the request named. Our
   Coax library already reads and checks both events
   (`coax/lib/nostr/nip57.coax`). A zap receipt is our server's claim that
   it saw the payment, not a proof, and NIP-57 says so.

Errors use LUD-06's form, `{"status": "ERROR", "reason": "..."}`, with a
reason a person can read: "This address isn't receiving yet",
"The most this address takes in one payment is 1,000,000 sats".

## Where the money goes

This is the hard part. Under our [custody model](wallets-and-custody.md)
(decision D4), people hold their own coins, we keep only backups we can't
open, and the only balances we hold are capped service credit that can't
be withdrawn. Someone else's incoming payment can't be service credit: it
is their money. So an `@openagents.com` address must put the money in a
wallet the person controls.

### The options

| Option | How | Phone offline | What we hold | Trust and trade-offs |
| --- | --- | --- | --- | --- |
| **A. Forward to the person's own address** | The person enters a Lightning Address they already have (say at their own node, Alby or Strike). Our lookup fetches theirs and passes its answer through; our callback asks their callback for the invoice and returns it unchanged | Works if their provider is online | Nothing | Their provider makes the invoice, so its custody model applies. Their `metadata` must pass through byte for byte, because the invoice's description hash commits to it, so the payer's wallet shows *their* identifier in the details. Zaps work only if their provider allows them (we pass through `allowsNostr` and `nostrPubkey`; their key signs the receipt) |
| **B. An invoice for the person's own Spark wallet** | The app's wallet registers its Spark identity key with our LNURL server. On each callback the server asks the Spark service provider for an invoice paying that key. The service provider takes the Lightning payment and hands the sats to the person's Spark wallet; the phone claims them next time it opens | Works: this is Spark's main advantage for phones | Nothing. Our server holds the name, the Spark public key and payment metadata | Spark's trust model, which the owner accepted for a person's spending wallet with small balances ([breez-spark.md](../lightning/breez-spark.md)). We run Breez's open-source LNURL server, so we depend on Breez and the Spark service provider for receiving |
| **C. An invoice from the person's own Lightning node** | Our server asks the app for an invoice when the phone is online; for an offline phone, our own LSP holds the payment until the phone wakes (async payments) | Needs our LSP | Nothing, beyond a payment held in flight | The long-term home (below). Needs our own node and LSP, which don't exist yet |
| **D. We receive and hold** | Payments land in our wallet; we keep a balance per person | Works | Everyone's money | Custodial: a hot-wallet breach takes everyone's money, and a withdrawable balance held for others is money transmission. It breaks decision D4 |

### Recommendation

1. **B for anyone with the OpenAgents wallet, as the default.** Tapping
   "Claim" in the app points the address at that phone's Spark wallet.
   It receives while the phone is off, and the money is the person's.
2. **A for anyone who already has a Lightning Address**, or who logs in
   only on the web without our wallet. "Send to an address I already
   have" is one field.
3. **No wallet yet:** the name can be claimed and held, and the address
   answers "This address isn't receiving yet" until the person picks B or
   A. We never accept a payment with nowhere of theirs to put it.
4. **Never D.** Not even briefly to "fix" a missing wallet.
5. **C when our own node and LSP pass their gates,** replacing B without
   changing anyone's address.

### Money transmission

Not legal advice; counsel reviews this before launch, as decision D6
already asks. In A and B we answer a wallet's question with an invoice
that someone else's wallet made, and we never have the money or a key
that can move it. FinCEN's 2019 guidance (FIN-2019-G001) ties money
transmission to accepting value and transmitting it, and treats providers
of unhosted wallet software as generally outside it
([summary, Jones Day](https://www.jonesday.com/en/insights/2019/06/fincen-consolidates-guidance)).
That is our reading for A and B: we are a directory and an invoice relay.
What we can't settle ourselves:

- In B, the Spark service provider does take part in the Lightning leg.
  Its own regulatory position is Breez's and the provider's; we should
  read their terms and keep a note of it.
- States add their own rules; the opinion should cover the states we
  expect users in.
- D, and any later "we hold it for you" feature, is high risk and
  refused here.

### Who can change where the money goes

Pointing a name at a different wallet redirects every future payment, so
an account takeover would aim there first. So:

| Action | Needs |
| --- | --- |
| First claim, pointing at this phone's wallet (B) | L1 and a signature by the wallet's Spark identity key over the account id, name and revision |
| First claim, forwarding (A) | L1; we fetch the target address and show what it returned before saving |
| Any later change of destination | `L2-fresh`, a proof from the new wallet (B), then a **24-hour wait** with a notice by email, push and Nostr direct message, cancellable from any of them. An account with only email waits 72 hours instead, like recovery |
| Moving a B address to a new phone | Both wallets sign the transfer (Breez requires it). If the old phone is lost, the 72-hour recovery path ([identity](identity-and-methods.md#recovery)) authorizes the new wallet |
| Turn the address off | L1, at once. The name stays yours |

Agents can never do any of these: they are human-only routes. An invoice
already issued keeps its original recipient whatever changes later.

## Names

| Rule | Detail |
| --- | --- |
| Characters | `a` to `z`, `0` to `9`, and `.`, `-`, `_` (LUD-16's set). The separators can't start or end a name or come twice in a row |
| Length | 3 to 32 characters. One- and two-character names are reserved for now |
| Case | Lowercase only. We lowercase what's typed and fold any case in lookups, since some wallets send capitals. Non-ASCII letters are refused, not transliterated, so look-alike letters can't sneak in |
| Uniqueness | One account per name, by a unique index. A name is also refused when its **skeleton** matches an existing name: drop `.`, `-` and `_`, and map `0`→`o`, `1`→`l`, `i`→`l`. So `chris.d`, `chrisd` and `chr1sd` are one name |
| Tags | `alice+tips@openagents.com` reaches `alice` (LUD-16 tags); the tag goes into `text/tag` and is shown to the recipient. A tag never selects a different person or wallet |
| Reserved | `_` (the LUD-16 default identifier, which we leave unused), system words (`admin`, `root`, `api`, `www`, `mail`, `postmaster`, `hostmaster`, `webmaster`, `abuse`, `security`, `support`, `help`, `billing`, `pay`, `payments`, `wallet`, `lnurl`, `noreply`), our names (`openagents`, `oa`, `team`, `staff`, `official`, `verse`, `everglade`, `coder`, `foreman`) and the team's own names. The list is a file changed only by a deploy, like `admins.conf` |
| How many | One name per person account. Each agent can have one, up to 10 agent names per owner. Shown on the claim card |
| Ties to | The account, not a key or a wallet. Changing keys or wallets doesn't move the name |
| Renaming | Once every 30 days. The old name keeps paying the same wallet unless you turn it off, and stays held by your account |
| Held forever | A name that has ever received a payment is held by its account for good: after a rename, after you turn it off, after the account is deleted (the name becomes a tombstone that issues no invoices). It is never given to another account |
| Never-paid names | A name that never received a payment, on an account that hasn't logged in for 12 months, is released after a notice and 30 days' grace |
| Impersonation | Names that pose as a person, a company or our staff are refused when they hit the reserved list or skeleton rule, and can be reported. An admin can **suspend** a name: it stops issuing invoices ("This address is suspended"), the holder is told why, and the action is in the audit log. A suspended name that was ever paid is never reassigned. A never-paid one can go to a verified claimant after 30 days |
| Nostr | The same claim can publish `alice@openagents.com` as a NIP-05 name for the account's Nostr key, and add `lud16` to the person's profile; the person's key signs that profile, never ours |

## The one-tap claim

**In the app**, the wallet screen shows a card: "Get a Lightning Address:
alice@openagents.com", with a suggested name from the display name (if
it's free) and an edit field that checks as you type. One button,
**Claim**. The wallet signs its proof in the background, and the card
turns into the address with **Copy**, **Share** and a QR. Under it, one
line of limits: "Receives 1 to 1,000,000 sats a payment. Notes up to 280
characters."

**On the web**, `/settings/lightning` shows the same card. With no app
wallet it offers "Send to an address I already have" (option A) and "Get
the app to receive in your own wallet" (option B, with the install link).

**Where we offer it:** the first time the wallet opens, in the maker's
claim flow (`/realms/<realm>/claim` asks for a Lightning Address today;
it gets a "Use alice@openagents.com" button), and on the profile page.
Never auto-assigned: a person who doesn't tap has no address.

Turning it off is one tap on the same card; turning it back on needs no
new claim.

## Limits and abuse

Every limit below is on the claim card or the settings page, listed on
`/docs/limits`, and named in the error a wallet shows.

| Limit | Value | Why |
| --- | --- | --- |
| Smallest payment | 1 sat (`minSendable` 1,000 msat) | LUD-06 floor |
| Largest payment | 1,000,000 sats by default; the person can set it lower. Agents: their grant's receive cap | Spark is for small balances |
| Note with a payment | 280 characters (`commentAllowed`) | Fits in the callback URL, which LUD-12 warns should stay near 2,000 characters |
| Zap request | 8 KB | `N57_MAX_DESCRIPTION` in `nip57.coax` |
| Invoice expiry | 10 minutes | Long enough to approve on a phone; short enough that a stale invoice isn't paid |
| Lookups | 30 a minute per IP | Ingress limit |
| Invoice requests | 120 a minute per name, 30 a minute per IP | Stops invoice flooding |
| Names | 1 per person, 1 per agent, 10 agent names per owner | Squatting |
| Renames | 1 every 30 days | Churn and confusion |

**Abuse we expect, and the answer:**

- **Invoice flooding** costs the payee nothing in B, but loads the Spark
  service provider and our server: the per-name and per-IP limits.
- **Scams using a look-alike name:** the skeleton rule, the reserved
  list, reports and suspension.
- **Spam notes:** notes are shown only in the recipient's wallet, as
  text, never as HTML; a person can turn notes off (`commentAllowed: 0`).
- **Privacy:** our server sees amounts, notes and the payer's IP for each
  invoice request. We keep IPs only as the keyed hash in
  [data](data-and-backups.md#encryption-at-rest), delete notes 30 days
  after the wallet has read them, and in option A the upstream provider
  sees our server, not the payer. We ask only for an optional payer name
  or pubkey (LUD-18), never an email.

## Paying makers' own addresses

Decision D6 moves tips and upgrade purchases off our wallet:

1. The tip page asks our API for the maker's invoice. The API fetches the
   maker's Lightning Address (whether ours or elsewhere) with the amount,
   and returns the invoice and the maker's metadata.
2. The player's wallet pays it: the app's wallet directly, or any wallet
   by QR.
3. We count the tip on the realm only when it is confirmed: by LUD-21
   `verify` when the maker's service offers it, or by the preimage the
   app's wallet reports (checked against the payment hash). Otherwise the
   page says "Sent from your wallet" and doesn't claim the maker got it.
4. The first-tip bonus becomes a separate payment from our own wallet to
   the maker's address: our money, not the player's.
5. Upgrades sold by makers are paid the same way. They can no longer be
   bought with service credit, because credit is spendable only on our
   own services (decision D4).

This replaces the payout runner in `makers.tips` and `makers.payouts`
([makers](../verse/makers.md)).

## Agents

- **An agent can have a name**, claimed by its owner at L2 (on
  `/settings/agents` or with `openagents agent address claim`). The
  metadata says it's an agent: "Pay builder-bot@openagents.com, an agent
  run by alice@openagents.com".
- **It receives into the agent's own wallet,** made from its own seed
  (decision D5's agent default), never the owner's wallet.
- **Its hot wallet has a ceiling** set by the grant. `maxSendable` is the
  smaller of the grant's receive cap and the room left under the
  ceiling, so the hot wallet never grows past it. Above the ceiling, the
  [ch122 treasury split](../../bitcoin/book/ch122-agent-treasuries.md)
  sweeps to the 2-of-3 treasury once that exists.
- **Spending what it received** stays inside the grant's per-payment and
  per-day caps ([wallets](wallets-and-custody.md#step-up-for-spends)).
  Paying another Lightning Address is an ordinary capped spend, and the
  agent's own Nostr key signs its zap requests.
- **What the agent can't do:** claim, rename, re-point or turn off its own
  address. Those are human-only routes.

## Bitcoin only

Lightning invoices paying bitcoin, on mainnet (staging uses its own
domain and regtest or signet). No fiat or stablecoin conversion on
receive: Breez's Flashnet conversions stay off. Taproot Assets don't ride
on these addresses until our own node carries them. This follows the
Bitcoin-only rule in [wallets](wallets-and-custody.md).

## The long-term home

Spark stays the person's wallet until our own Lightning node passes its
gate ([build-everything audit](../research/bitcoin/2026-11-11-build-everything-audit.md),
decision 0). Then:

- **Our own LSP** (audit decision 11, items E13 and E15: LSPS0, 1, 2 and 5,
  and async payments with static invoices) makes option C possible: an
  offline phone's payment waits at our LSP and completes when the phone
  wakes, without us owning the money.
- **BOLT 12 offers** (item E8) can be published for the same names as
  BIP-353 DNS payment instructions, alongside LNURL.
- The LNURL server becomes Coax, like the rest of the API.

Names, limits and the claim flow don't change when the backend does.

## Data

| Table | Columns that matter |
| --- | --- |
| `pay.addresses` | `name` (unique), `skeleton` (unique), `account`, `mode` (spark, forward, none), `spark_pubkey`, `forward_to`, `state` (active, off, suspended, retired), `revision`, `max_msat`, `comments_on`, `created_at`, `renamed_from` |
| `pay.address_changes` | Pending destination changes: `name`, `new_mode`, `new_target`, `proof`, `requested_session`, `ready_at`, `cancelled_at`, `applied_at` |
| `pay.name_holds` | `name`, `account`, `since`, `reason` (paid, renamed, deleted, suspended). Rows are never deleted |
| `pay.invoices` | `payment_hash`, `name`, `revision`, `amount_msat`, `expires_at`, `zap_request`, `comment` (deleted 30 days after read), `settled_at` |

Breez's LNURL server keeps its own Postgres (registrations and payment
metadata); it goes into the same dumps and drills as everything else
([backups](data-and-backups.md#backups)), with its service keys sealed
to the offline backup key. Every claim, rename, change, suspension and
transfer goes into `auth.audit`.

## What came before

On 2026-10-20 we built `name@openagents.com` on a self-hosted Breez LNURL
server with zaps, before the nuke (#12307; `c720d485bd`, `a6077b636a`).
Read its design with
`git show c720d485bd:docs/payments/self-hosted-lightning-addresses.md` and
its runbook with
`git show c720d485bd:docs/payments/lightning-address-operations.md`. This
design keeps its permanent payment-name holds, its wallet-signed
enrollment, its two-wallet transfer and its activation checks, and adds
forwarding, the skeleton rule, the destination-change delay, agents and
direct maker payment.
