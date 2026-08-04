# Boltz web-app UX parity teardown — the swap widget as a product

- Date: 2026-08-04
- Lane: Fast Follow research / product-surface teardown
- Owning issue: [openagents#9314](https://github.com/OpenAgentsInc/openagents/issues/9314)
  ("our core web UI must replicate what Boltz has")
- Disposition: adopt the interaction laws, the state coverage, the
  disabled-with-a-reason primary action, and the rescue *ceremony*.
  Reject the single-coordinator model, the raw-server-string error
  surface, the non-hardened key derivation, the xpub-to-coordinator
  discovery scan, and — most importantly — the Rescue page's silent
  dependency on the coordinator it claims to rescue you from.
- Primary source: `boltz-web-app` v2.2.1, commit
  `dd9c2df26db54a2554dc1e628b095ce856c0d9de` (2026-07-30), local clone
  `~/work/projects/repos/boltz/boltz-web-app/`. SolidJS + Vite +
  `@solidjs/router`, with a vendored SDK workspace package at
  `packages/boltz-swaps/`
- Secondary pins: `boltz/docs` `0d7cb95` (2026-04-24);
  `boltz-frontend` `e10a1e0` (2021-01-05, superseded — not evidence for
  current behaviour)
- Companions:
  [Boltz ecosystem and Nostr rebuild](./2026-08-03-boltz-ecosystem-nostr-rebuild-teardown.md)
  (§3.4 covers this repository in five lines — this document is that
  gap filled),
  [Satora/LendaSwap outage](./2026-08-04-satora-lendaswap-outage-teardown.md),
  [tbDEX liquidity protocol](./2026-08-04-tbdex-liquidity-protocol-teardown.md),
  [Market rails](./2026-08-04-ark-solver-mostro-cashu-rails-teardown.md)
- Target documents:
  [swap demo UI rollout plan](../markets/2026-08-04-swap-demo-ui-rollout-plan.md)
  (amended by this teardown), [`docs/nips/MKT.md`](../nips/MKT.md),
  [`docs/nips/MKT-SWP.md`](../nips/MKT-SWP.md)

## 0. Licensing boundary

`boltz-web-app` is **AGPL-3.0**. This teardown was produced by reading
that source tree read-only. **No code, no stylesheet, no i18n string, no
asset, and no test fixture from it is copied into this repository, and
none may be.** Identifier names, file paths, route paths, numeric
constants, and protocol status strings appear below as facts about the
system under study; every behavioural description is written in our own
words. Every component named in the build plan is authored against the
OpenAgents design system.

The same rule governed the earlier Boltz teardown and is restated here
because parity work creates the temptation the earlier lane did not.
When an implementer needs the shape of a flow, they read this document,
not the AGPL source.

No swap was created, no credential presented, no funds moved, and the
Boltz API was not called during this teardown.

Evidence labels follow
[the directory convention](./README.md#evidence-convention): `[source]`,
`[test]`, `[schema]`, `[public]`, `[inferred]`, `[limitation]`.

## Summary

Boltz's web app is a single-page, self-custodial swap client that has
been ground smooth by production. Its value to us is not its visual
design; it is the **completeness of its state coverage** and one
interaction law: *the primary action is always present, and when it
cannot proceed it says exactly why, in the units the user is currently
looking at*. Fifteen distinct labels can occupy that one button, and
fourteen of them are refusals. [source]

Three findings change what we should build.

**First, the parity target is not the whole of Boltz.** v2.2.1 has grown
far past BTC/Lightning/Liquid: it routes RBTC, wrapped BTC, and USDT0 and
USDC across roughly twenty-two chains through DEX hops and LayerZero/CCTP
bridges, with slippage tolerance, gas top-up, and commitment swaps.
MKT-SWP v1 supports exactly three shapes — submarine, reverse, and
chain — on Bitcoin and Lightning, and explicitly forbids a non-null
`evm_leg`. Feature parity with today's Boltz is therefore **not
achievable under the profile we have**, and pursuing it would mean
building UI for rails the protocol refuses to execute. The honest parity
target is *the BTC/Lightning subset of Boltz's product experience, at
full depth*, with the stablecoin surface arriving only with the reserved
`mkt-swp-evm` extension. [source] [schema]

**Second, Boltz's Rescue page is not coordinator-independent, and by
MKT-SWP's own definition its exit package is unusable.** The browser's
BIP39 mnemonic gives you private keys and — through a deliberate
`preimage = sha256(claim private key)` derivation — your preimages. It
does not give you the swap script. `lockupAddress`, `swapTree`, the
server public key, the Liquid blinding key, and `timeoutBlockHeight` are
fetched from the Boltz API's restore endpoint, keyed by the master
xpub. Signing and broadcast are genuinely client-side, with explorer
fallbacks for fee estimation, UTXO lookup, and broadcast. Discovery and
*parameters* are not. There is no import path for the history export
that would supply them — their own product note argues against building
one — and no test exercises recovery with the coordinator down. Under
[`MKT-SWP` §12](../nips/MKT-SWP.md), "an exit package that depends on a
vanished coordinator to obtain a signature, transaction template, fee
rule, timeout, or counterparty identity is `swp_exit_package_unusable`."
Boltz's rescue path meets that definition. This is the single place
where our parity must be *stronger than the thing we are copying*, and
it is also the strongest available validation of the doomsday drill.
[source] [inferred]

**Third, the widget's shape is single-coordinator all the way down and
ours cannot be.** Boltz has no quote object: limits and fees come from
three pair endpoints, the receive amount is recomputed reactively on
every dependency change, and there is no quote identity, no quote
expiry, and no countdown on the create form. Amount drift is handled
*after* funding, by a replacement-quote panel that asks the user to
accept a worse fill. NIP-MKT quotes are signed records with an
`expiration`, a `firm`/`indicative` class, and a `none`/`soft`/`hard`
reservation — so our widget needs an expiry countdown and a
re-quote path Boltz has never needed, and it can show *competing quotes
from independent providers*, which Boltz structurally cannot. The
compare table is not a nice-to-have decoration on a Boltz clone; it is
the part of the product that only exists because the market is on
relays. [source] [schema]

## 1. Source, evidence, and limits

| Property | Value |
| --- | --- |
| Repository | `BoltzExchange/boltz-web-app` |
| Pin | `dd9c2df26db54a2554dc1e628b095ce856c0d9de` |
| Version | v2.2.1 (2026-07-30) |
| License | AGPL-3.0 |
| Stack | SolidJS, Vite, SCSS, `@solidjs/router`, BigNumber.js, `@scure/bip32`/`bip39`, `@noble/hashes`/`curves`, `@scure/btc-signer`, `boltz-core`, `viem` |
| Internal SDK | `packages/boltz-swaps/` (network, status sources, refund/claim construction, LNURL) |
| Locales | 6 (`en`, `de`, `es`, `pt`, `zh`, `ja`), one 3 344-line dictionary module |
| Routes | 18 declared plus legacy redirects |
| Storage schema | version 7, with seven sequential migration steps |
| E2E | Playwright, including `e2e/rescue/*`, `e2e/urlParams.spec.ts`, `e2e/postMessage.spec.ts`, `e2e/locale.spec.ts` — all against a live regtest Boltz backend |

**Audit limits.** This is a source read at one pin, not a live product
session: no swap was created, so runtime timing, backend error bodies,
and real failure sequencing are inferred from code paths and tests
rather than observed. The vendored SDK is read as part of the app. The
Boltz backend is closed to us except through its published API surface,
so statements about what the server sends are read from client call
sites. [limitation]

## 2. The swap widget, state by state

### 2.1 Where the state lives

There is **no explicit state machine for the create form.** Its state is
the cross-product of roughly fourteen independent reactive signals —
pair, send amount, receive amount, which side the user last edited,
amount validity, address validity, invoice validity, invoice error,
quote loading, quote error, BOLT12 probe loading, limits loading,
destination locked, gas-token resolution — plus global online and pairs
signals. The single place that cross-product collapses into one
observable value is the effect that computes the primary button's label.
[source]

The in-flight side, by contrast, *is* a real string state machine, and it
is the backend's, not the client's. [source]

That asymmetry is itself the lesson: **the pre-funding form was allowed
to stay an implicit product of signals, and it is the part of the app
whose behaviour is hardest to state.** Our widget should carry an
explicit typed state, because the protocol already gives us one — the
session projection the engine exports — and because a behaviour contract
cannot be written against a cross-product.

### 2.2 The pre-creation states

Ordered as the label effect evaluates them; the first match wins, so this
is the effective precedence. [source]

| # | State | Trigger | What the user sees |
| --- | --- | --- | --- |
| 1 | API offline | pairs fetch threw | Danger-styled button naming the outage; page banner with a manual reload control |
| 2 | Pairs loading | boot, pairs undefined | Spinner in the button, disabled |
| 3 | WASM unsupported | feature probe fails | Whole create page replaced by a dedicated error page plus a banner |
| 4 | Unsupported send asset | route not found and the chosen send asset is configured non-sendable | Error-styled button naming the send asset as the problem |
| 5 | Unroutable pair | route not found otherwise | Error-styled button naming the pair; **the asset picker did not prevent this selection** |
| 6 | Empty form | no amount, no destination | Button reads as a minimum-amount refusal (zero is treated as below minimum); inputs get a validity message but deliberately **not** the red class |
| 7 | Below minimum | send amount below the resolved minimum | Button states the minimum with the amount and the current denomination; both amount inputs go invalid |
| 8 | Above maximum | send amount above the resolved maximum | Same treatment, error-styled |
| 9 | Zero quote | positive send, zero receive (fees exceed input) | Button states the condition, but is *not* error-styled |
| 10 | Quote failed — capacity | typed bridge-capacity error | Button states capacity; derived amount forced to zero |
| 11 | Quote failed — generic | quote threw | Button states "no quote" |
| 12 | Quote refreshing | 500 ms debounced re-quote, only on routes needing a network quote | Derived input shows a skeleton, is disabled and `aria-busy`; button spins |
| 13 | Limits loading | limits resolving | The MAX chip renders a skeleton and disables |
| 14 | MAX resolving | wallet balance probe running | Both amount inputs disabled |
| 15 | No destination | no address, invoice, LNURL, or offer | Button disabled regardless of label |
| 16 | Invalid address | non-submarine route, address rejected | Button names the asset; field goes invalid |
| 17 | Invalid invoice | submarine route, invoice rejected | Button carries the specific invoice error key when present (including the amountless-invoice case) |
| 18 | Offer probing | BOLT12 detection in flight | Button spins |
| 19 | Deferred destination ready | LNURL / lightning address / BOLT12 offer | Normal action label; the concrete invoice is fetched **on click** with a 25 s timeout |
| 20 | Destination amount out of range | LNURL min/max violated, discovered after click | Button relabels with the destination's own limit *and* a toast; the only place an explicit disable flag is set |
| 21 | Commitment ready | deferred-invoice ERC-20 path | Invoice field rendered disabled with a clear control |
| 22 | Gas top-up undecided | balance probe pending | Button disabled |
| 23 | Destination locked | URL-driven embed with a locked output | Asset selector, flip control, limits chip, and both amount inputs disabled; destination inputs hidden |
| 24 | Ready | everything valid | Plain action button, enabled |
| 25 | Creating | submit in flight | Spinner, disabled |
| 26 | Creation rejected, recovered | server rejected on stale limits or stale pair hash | Pairs refetched, **send amount snapped to the fresh limit**, toast, user stays on the form |
| 27 | Creation response failed validation | client re-derivation of the server's response mismatched | Hard navigation to a generic error page; the specific reason is logged, never shown |

**Absent by design: rate-stale, quote TTL, quote-expired, and any
countdown before re-quote.** Quotes are not objects; they are the current
value of a computation over the current pairs data. [source]

### 2.3 The primary-action pattern

This is the idea worth taking outright, and it decomposes into four
independent mechanisms that our implementation should keep separate:

1. **Label** — one signal holding a message key plus parameters, written
   by a single straight-line cascade. Exactly one reason surfaces at a
   time and precedence is positional. The min/max messages are
   parameterised with the amount *and* the denomination the user is
   currently viewing, so the refusal is stated in the units on screen.
2. **Colour** — a separate memo over a small allowlist of "this is the
   user's fault" keys. Notably asymmetric in the original: the maximum
   message is error-styled and the minimum is not, and neither address
   nor invoice invalidity is. We should not reproduce that asymmetry; we
   should decide it deliberately.
3. **Disabled** — a *separate* boolean expression, not derived from the
   label. A state can therefore be explained without being blocking, and
   blocked without a fresh explanation. Keeping these two independent is
   correct and worth copying.
4. **Content** — spinner replaces the label while loading, except when
   the pair is unroutable, so a permanent refusal never spins forever.

The law to carry into our design system: **the primary action is always
rendered, always states the single most proximate reason it cannot
proceed, and states it in the user's current units.** [source]

### 2.4 In-flight and terminal states

The swap page renders one status component chosen by a switch over the
backend status string, with a raw status chip above it. It is a
component switch, not a stepper. Six gates run before the switch:
swap-not-found, backup-required, chain-probe loading, already-refunded,
status-not-yet-fetched, and waiting-for-timeout. [source]

The backend vocabulary is twenty strings: ten pending, eight failure, two
success. [schema]

| Class | Strings |
| --- | --- |
| Pending (10) | invoice set, invoice paid, invoice pending, swap created, transaction mempool, transaction confirmed, zero-conf rejected, claim pending, server mempool, server confirmed |
| Failure (8) | swap expired, swap refunded, waiting for refund, invoice expired, invoice failed to pay, transaction failed, transaction lockup failed, transaction refunded |
| Success (2) | invoice settled, transaction claimed |

Three of the checklist states in the parity brief are **not backend
states at all** and are worth naming because we will have to decide the
same thing:

- **claiming** is a client-side overlay: a claim is in flight locally, or
  a claim transaction id is known.
- **refunding** is a local flag inside the refund control that disables
  the address field and the action.
- **refunded** is derived from a refund transaction id being present on
  the stored swap, and short-circuits the entire switch.

There is also a deliberate **renaming**: when a chain swap reports that
the *backend* refunded its own leg, the client rewrites the displayed
status so the user is not told "refunded" about someone else's money.
That instinct is right and, in our model, is already law — a Status is
one signer's claim, and per-signer sequences must be displayed
separately rather than merged into one narrative. [source]

Expiry is **block-height based, not wall-clock**. The client fetches the
tip, compares against the swap's timeout height, and on timeout stops
trusting the backend status entirely. The refund ETA is rendered as a
target block height plus a static localised timestamp — there is no
ticking countdown, and if the height lookup fails the code deliberately
allows the uncooperative refund anyway. MKT-SWP §6 requires exactly this
pairing and adds the rule Boltz leaves implicit: clients display both the
height bound and the time estimate, and never convert an estimated time
into consensus authority. [source] [schema]

### 2.5 The error taxonomy

Seven presentation channels, no error boundary, no central error type:
inline native field validity, the primary button label, a single
auto-dismissing toast (4 s), page banners, full-page error routes,
in-panel error blocks on the swap page, and no modals at all. [source]

Backend errors reach the user through three mappings:

- A recovery handler that does **substring matching on English server
  prose** to detect stale pair hashes and stale limits, then refetches
  and snaps the amount.
- A universal normaliser that detects wallet rejections by code or by a
  four-phrase substring list across eight nested error shapes, and
  otherwise unwraps to the raw upstream string.
- A small number of genuinely typed predicates imported from the SDK
  (LNURL amount errors, bridge capacity).

**Everything that is not a wallet rejection is shown to the user as the
raw upstream English string.** [source] This is the clearest anti-pattern
in the app and the one our protocol already solves: MKT-SWP §17 defines
fifty-plus stable lowercase error identifiers. Our UI maps identifiers to
localisable messages and never renders a counterparty's prose.

One more shape worth noting: six distinct destination-parse failure modes
are computed as a typed discriminated result and then **collapsed into a
single generic message**, with the discriminant used only for a debug
log. The typing was done and then discarded at the boundary. [source]

Separately, the client re-derives the server's swap response — swap tree,
lockup address, payment-request string, preimage hashes, and for EVM the
deployed contract code hash — and any mismatch throws, logs, and
redirects to a generic error page. The user never learns which check
failed. That is the right check and the wrong presentation: this is
precisely our verify-before-fund checklist, and ours renders each check
with an explicit pass/fail rather than one anonymous failure. [source]

## 3. Assets, pairs, rates, fees, units, destination

### 3.1 Asset and direction selection

Two side-labelled chips (send, receive) open **one** modal whose heading
is chosen by side. The modal is a two-column grid with full keyboard
traversal (vertical, horizontal, enter, escape) that skips disabled
entries. Picking, on one side, the asset already on the other side
**swaps them** rather than refusing. Selecting clears the invoice,
conditionally preserves the on-chain address when the destination asset
is unchanged, rebuilds the pair, and refetches. [source]

A hub asset with per-chain variants pushes a second-level searchable
network picker with a back control; variants are hidden from the
top-level list. A "Bitcoin only" setting turns the chip into a refusal
with a toast and continuously normalises any non-Bitcoin pair back. The
flip control sits between the rows, is hidden when the destination is
locked, always clears the on-chain address, and only physically exchanges
the two amounts when a token-unit asset is involved — for sat-denominated
pairs it lets the re-quote effect recompute. [source]

### 3.2 Pair discovery

Three parallel REST calls, one per swap type, assembled into a nested
`from → to` map carrying a rate, limits, and fees per direction. **The
swap type is inferred purely from where Lightning sits**: receive is
Lightning means submarine, send is Lightning means reverse, neither means
chain. Missing intermediate keys simply mean "unavailable". [source]

There is no polling. Pairs are refetched on create-screen mount, after
every asset or network selection, from the offline banner's reload
control, and after a rejected creation. [source]

**The swap type is never labelled.** It surfaces only as *which
destination field appears* — address, invoice, or connect-wallet — and as
whether the QR scanner and the zero-amount path are available. [source]

**An unroutable combination is only discoverable at the button.** The
asset modal filters on "can send" and "disabled" but does not pre-filter
to reachable counterparties. [source] Our Offerings make this strictly
better: a `39601` head carries one `sides` entry per ordered asset pair
with `min`, `max`, and `fee_bps`, and `max="0"` disables a side
explicitly. We can grey out unreachable directions *before* selection and
say which provider would need to appear to enable them.

### 3.3 Rate and fee display

There is no rate API and no rate line on the create form. The effective
rate is implied by the quote arithmetic: percentage fee plus miner fees
for Bitcoin-pegged pairs, live DEX quotes for token routes. A separate
price feed, cached five minutes, exists only to render an approximate
fiat total, with an explicit unavailable state. [source]

Fees are **collapsed by default** into one row showing an approximate
fiat total prefixed with a "roughly" marker. Expanded, in order: network
or miner fee, the service fee with its percentage inline beside the
absolute amount, bridge transfer fee, and bridge messaging fee. Outside
the collapse sits the submarine routing-fee ceiling, printed in parts per
million. [source]

The miner fee is adjusted in the client, not just displayed: a surcharge
for unconfidential Liquid destinations, a globally baked-in claim-fee
bump, and a relay gas surcharge that also fires a toast. [source]

**The UI never says whether the fee is a promise or an estimate.** The
only hedging is the approximation marker on the fiat line. Estimate-ness
is instead expressed through a user-configurable slippage tolerance and
through post-lockup panels that tell the user prices moved *while their
funds were already committed*. [source]

Our protocol removes the ambiguity and requires us to state it: MKT-SWP
§3.3 makes `fee_bps` "a provider promise for a conforming fill" and the
Quote's `output_amount` "the fill promise", which a provider may not
reduce after Order because its route or miner fee changed. The fee panel
must therefore say *promise* where Boltz says nothing, and must render
`provider_fee`, `miner_fee_budget`, and `lightning_routing_fee_budget`
separately with who pays each — a disclosure Boltz has no field for.
[schema]

### 3.4 Units and amount entry

Denomination is a persisted global preference defaulting to satoshis,
rendered as two explicit buttons on desktop and a single toggle on
mobile, and living inside the fee row. Decimal separator is a second
persisted preference initialised from the browser locale. [source]

Precision is per-asset: token-unit assets always render in their own
units and the sat/BTC toggle is a no-op for them. Satoshi formatting
inserts thin spaces every three digits. All arithmetic runs on BigNumber
over integer smallest-units, with **directional rounding that favours the
operator**: ceiling on fees and derived send amounts, floor on receive
amounts. [source]

Both amount fields are editable; a "which side did the user last touch"
signal decides which is authoritative. There is a MAX control and **no
minimum control**; MAX is wallet-aware, taking the lesser of the
connected balance (net of a gas reserve) and the protocol maximum.
[source]

Two footguns worth naming because we should not reproduce them:

- **Auto denomination switching**: typing a value below one flips
  satoshis to BTC, and typing ten or more flips BTC to satoshis. It
  silently reinterprets what the user is typing. [source]
- **The minimum is never shown proactively.** It appears only after the
  user has typed something out of range, and only via the button label
  and native field validity — which means the readable copy exists in
  exactly one place and the field-level message is a browser tooltip.
  [source]

Quote recomputation is debounced 500 ms with a monotonic request id so
superseded keystrokes are discarded — but only on routes that need a
network quote; simple routes recompute synchronously with no loading
state. Keystrokes are filtered by regex; pastes are validated against a
regex built from the digit count of the current maximum, rejected with a
toast, deduplicated, and replace rather than append. [source]

### 3.5 Destination entry and validation

One shared parser serves the address field, the invoice field, and the QR
scanner. Its order: empty check, URI extraction, **known-token-contract
rejection**, chain-specific validation for the current receive asset,
then a probe that tries the expected asset first and falls back through
Lightning, Liquid, and Bitcoin. Recognised URI prefixes cover Lightning
and two chain schemes; BIP-21 amounts are read case-insensitively and
malformed values are treated as absent rather than failing the URI.
[source]

**Pasting the wrong kind of destination switches the pair rather than
refusing it** — paste a Bitcoin address into a Liquid route and the route
changes, with a toast. The invoice field has its own parallel
implementation of the same behaviour. Every async step carries a
staleness closure and bails on superseded input. [source]

Lightning specifics that map directly onto our profile:

- **Amountless invoices are rejected outright**, matching MKT-SWP §7.2
  which declares them invalid in v1. [source] [schema]
- **An amount-locked invoice overrides the typed amount**, and a unified
  QR that carries both a BIP-21 amount and an amount-bearing invoice
  suppresses the BIP-21 value so two conflicting amounts cannot appear.
- **Editing the amount after pasting an invoice clears the invoice**,
  with a toast. Deferred destinations (LNURL, lightning address, BOLT12
  offer) survive, because their amount is not yet bound.
- Deferred destinations resolve to a concrete invoice **at submit time**,
  and their own min/max violations relabel the button with the
  *destination's* limit rather than the protocol's.

There is no clipboard read and no paste button on the destination fields;
QR scanning is mobile-only and gated on a camera probe. [source]

## 4. Rescue, refund, and the custody story

### 4.1 What the browser holds

One BIP39 12-word mnemonic per browser, generated automatically on first
load whether or not the user ever backs it up, stored in `localStorage`.
Every swap key derives from it. [source]

- Derivation paths are **non-hardened** for the UTXO branch, and per-chain
  for EVM. A non-hardened branch means the master public key plus any one
  child private key discloses the branch's other private keys.
- The preimage is **not random**: it is the SHA-256 of the derived claim
  private key, deliberately, so that preimages are recoverable from the
  mnemonic alone.
- Swaps store a derivation **index**, not a key. Raw key fields survive
  only as a legacy fallback.
- The key index counter lives in local storage and **is not resynchronised
  from restored swaps**; restoring into a fresh browser and then creating
  swaps there would restart at index zero. In practice the rescue flow
  keeps an imported mnemonic in a separate non-persisted context, which
  avoids the collision at the cost of "restore" never becoming your
  active wallet. [source] [inferred]
- Swap metadata is AES-GCM encrypted under a key derived from the
  mnemonic and uploaded to Boltz, bound to swap id and payment hash, so
  the coordinator can serve it back during restore without reading it.

### 4.2 The rescue-key ceremony

Two branches, both with real verification:

- **File**: download a JSON containing only the mnemonic, under a
  filename that shouts not to delete it. Verification requires
  **re-uploading** that file (or a photo of its QR), parsing it, and
  comparing it to the in-memory mnemonic.
- **Mnemonic**: a word-by-word quiz in three groups of four, one hidden
  word per group chosen from four candidates; failure returns to the
  display step.

Only after verification is the backup flag set. [source]

**The gate is in the wrong place, twice.** It runs on the swap status
page *after* the swap has been created, not before; and it is skipped
entirely for reverse swaps, where the user has no on-chain funds at risk
but does lose the claim if the key is lost. [source]

MKT-SWP §7.1 step 7 already puts the gate where it belongs: the exit
package is built, persisted, and digest-verified against the Swap
Contract pair **before** `verification_passed`, and funding is not
presented until every check passes. Our ceremony therefore precedes
funding by protocol law, and applies to both directions.

### 4.3 The Rescue page

Routes cover a scan entry point plus per-swap claim and refund pages,
with the older refund routes redirecting in. Accepted inputs: the rescue
JSON file, a photo of its QR, a manually typed 12-word mnemonic, or a
connected EVM wallet as a second method. Legacy per-swap refund JSON
files are **no longer accepted**. [source]

What the scan does:

1. Derives the master xpub and calls the Boltz **restore** endpoint with
   it, paginated.
2. Signs a challenge with the derived EVM account and posts it to the
   same endpoint.
3. Decrypts the returned metadata blobs with the mnemonic.
4. Derives candidate preimage hashes in a Web Worker — up to 100 000
   iterations, posted back in batches of 1 000 — to match on-chain EVM
   lockups.
5. Scans EVM contract logs over a plain RPC endpoint. **This branch does
   not need the Boltz API to find a lockup.**
6. Classifies each result as claim, refund, pending, successful, or
   failed using explorer UTXO data and the tip height.

Refund transactions are constructed, signed, and broadcast in the
browser. The cooperative path asks Boltz for a partial signature and, on
any failure, **falls back automatically to the script-path refund** with
the lock time set to the swap timeout — which then only relays after
expiry, so the client maps the resulting relay rejections to a
"locktime not satisfied" message and shows the ETA. [source]

### 4.4 The coordinator-dependency split — the central finding

| Capability | Truly client-side? |
| --- | --- |
| Mnemonic, key, xpub, and preimage derivation | Yes |
| Constructing and signing the uncooperative script-path refund | Yes |
| Fee estimation | Yes — Boltz first, then public explorers with per-asset floors |
| Broadcast | Yes — races the Boltz endpoint and a public explorer in parallel, takes whichever succeeds |
| UTXO lookup for a **known** lockup address | Yes — public explorers, including onion variants |
| Tip height and timeout ETA | Yes — explorer |
| EVM lockup discovery | Yes — contract log scan over a plain RPC |
| EVM timeout refund | Yes — direct contract call |
| **Discovering which swaps exist** | **No** — the restore endpoint, keyed by xpub |
| **Obtaining the swap script** (lockup address, swap tree, server public key, blinding key, timeout height) | **No** — read out of the restore response; there is no local re-derivation and no way to paste them in |
| Live swap status | No — the Boltz stream, except in the timeout-refund states where the client deliberately stops trusting it |
| Cooperative refund | No — requires the Boltz partial signature; without it you wait out the timelock |
| Claiming an already-locked output | No — restore for the tree and blinding key, status for the lockup transaction hex, pairs for the fee |
| Wrong-asset rescue | No — entirely coordinator endpoints |

The honest summary: **the signing half of self-custody is
coordinator-free; the data half is not.** The only genuinely Boltz-free
material is what already sits in this browser's IndexedDB — which does
contain the swap tree, claim public key, blinding key, and timeout height
from the original creation response — plus the mnemonic. Lose the browser
profile and you depend on the restore endpoint being up. [source]

The history export contains exactly that dataset. **There is no import
path**, and their own product note argues against building one on the
grounds that native rescue exists and re-import is niche. [source] The
one artifact that would make a user independent is write-only.

And there is no test of the failure it is named for: every rescue E2E
spec runs against a live regtest backend. There is no coordinator-down
case. [test]

### 4.5 Where our parity must be stronger

| Boltz | MKT-SWP requirement | Consequence for the UI |
| --- | --- | --- |
| Swap parameters fetched from the coordinator at rescue time | §12 exit package persisted **before** the funding broadcast, digest-bound by the Swap Contract pair, containing the funding template digest, script pubkey, exit template, timelock bounds, fee policy, and broadcast endpoints | The rescue artifact is self-sufficient by construction; the UI's job is to prove it exists and is verifiable, not to fetch it later |
| Backup gate after creation, skipped for reverse | §7.1 verification must pass, and the package must be built and digest-checked, before funding is offered | The custody ceremony is a step in the fund flow for every direction |
| Signed rescue file contains a mnemonic only | §12 forbids seeds, raw private keys, preimages, macaroons, NWC strings, bearer tokens, and MuSig2 secret nonces in the package; secrets stay in the local secret store behind a non-secret handle | Two artifacts, not one: a secret store the user backs up, and a package that is safe to persist and inspect |
| Rescue = "find swaps not in this browser" | §12.1 doomsday drill: every handler, provider API, socket, catalog, and coordinator removed **after funds move**, and both parties still reach a correct terminal state | Rescue is a *product surface for the drill*, and the drill is an acceptance test the UI must be able to pass |
| No coordinator-down test | §12.1 plus immortal#18 adversarial lab | A parity Rescue page ships with a coordinator-absent test or it is not done |
| Non-hardened derivation; xpub handed to the coordinator on every scan | §14 privacy classification: claim and refund public keys are pairwise-private | Discovery runs off our own persisted signed records, never off a master public key given to a provider |

## 5. History and resumability

Swap history lives in IndexedDB (with a local-storage fallback) keyed by
swap id, alongside four sibling stores for schema version, logs, wallet
identifiers, and a key-index counter. Preferences live separately in
plain local storage. [source]

The parts worth carrying:

- **Read-modify-write is serialised under a named lock per swap id**, to
  stop the status checker and the background execution worker losing each
  other's updates. Claims take a second global lock.
- **Resume is app-wide, not page-scoped.** A checker component mounted
  outside the router reads *every* stored swap on mount and subscribes to
  each one that is not final — plus final-but-unclaimed ones. A restart
  therefore re-attaches everything outstanding, and the swap page
  additionally fires a one-shot status fetch to catch up before relying
  on the stream.
- **A navigation guard** blocks reload while chain or reverse swaps are
  mid-flight, disabled in the regtest configuration.
- **Rows are sorted by actionability first**, then by date descending,
  with the per-row action computed asynchronously from explorer UTXO data
  and tip height rather than from the stored status alone.
- **Schema versioning is real**: seven sequential migrations, each
  rewriting every stored record, plus a one-way drain of a legacy
  local-storage array.

Per-row actions are view/resume, delete with a confirm, export the whole
history, and clear all. Ten rows per page, with a minimum height to
prevent layout shift. [source]

**The create form itself does not survive reload.** Only the chosen
assets persist; amounts, destination, and validity reset, and URL
parameters are consumed once and stripped. [source]

## 6. Status and progress

Transport is a **single multiplexed WebSocket with a REST-polling
fallback** — no server-sent events. [source]

| Mechanism | Value |
| --- | --- |
| Reconnect backoff | exponential, 1 s initial, 30 s max, factor 2, 50 % jitter |
| Connect timeout | 15 s |
| Application ping | every 15 s, force-reconnect on an unanswered ping (catches half-open sockets) |
| Stability reset | a frame arriving 10 s or more after open resets backoff and disengages the fallback |
| Degrade to polling | after 3 failed connect attempts |
| Poll interval | 5 s, bulk status request chunked at 64 ids |
| Bulk fallback | per-id requests, because one unknown id rejects the whole batch |
| Emission rule | the poller emits only when a serialised update actually changes, to match socket semantics |

Subscribing replays current status, so **events missed while the tab was
closed are covered by resubscription** rather than by a cursor. Late
handlers on an already-tracked id get the last update replayed. Stale
socket guards prevent a superseded connection from corrupting state.
[source]

Progress is rendered as a spinner inside whichever status component is
selected, not as a stepper. The only stepper-shaped UI in the app belongs
to the merchant-plugin marketing page and the rescue-key backup flow.
[source]

**Two divergent definitions of "final" exist in the same repository**:
the app's list omits waiting-for-refund, transaction-failed, and
lockup-failed so those stay watched, while the SDK's helper treats all
eight failure statuses as terminal. Anything migrating from the app's
checker to the SDK's watch helper would silently stop watching refundable
swaps. [source] That is a good argument for our single exported session
projection being the only definition of terminality.

**Mapping onto NIP-MKT Status.** Boltz's stream is one authority
reporting one string. Ours is per-signer: each author has its own `seq`
starting at zero and increasing by one, `previous` references chain them,
a missing number is a displayed gap (`swp_status_gap`), and two records
at one sequence from one author are a retained fork
(`swp_status_fork`) that clients show rather than resolve by arrival
time. The base `state` is derived from the profile `swp_state` through
the §9 table, and `contract_pending`/`contract_bound` are **local
projections with no Status mapping** — they are never established by a
claim. A `completed` Status is a claim until an admitted verifier raises
the evidence rung, and the rung is never inferred upward. [schema]

So the parity status view is not one lane with a spinner. It is two
lanes — requester and provider — with gaps and forks rendered as gaps and
forks, and a rung label that lags the claim.

## 7. Surrounding surfaces

| Surface | Boltz | Parity call |
| --- | --- | --- |
| Nav | Swap, Rescue, History, Products, Help (external), Docs (external), plus a locale dropdown and a mobile hamburger; hidden in embed mode | **Yes** for Swap, Rescue, History. Products and Docs map onto our existing public surfaces rather than new pages |
| Footer | Social row, a secondary row (partner, branding, status, regtest, onion), a legal row, and a version line linking the release tag and commit | **The version line, yes.** A swap surface should state exactly which build a user is trusting |
| Products | Three cards: merchant plugin, client daemon, and the pro fee tier | **Theirs.** Our equivalent is provider-side, and per the rollout plan the web never grows an operator view |
| Docs / Help | External links to a docs site and a support centre | Our docs live in the repository and on the public site; a link, not a page |
| i18n | 6 locales, one dictionary module, missing keys back-filled from English at load, locale from explicit setting → URL param → previous URL locale → browser language → default | **Scaffolding yes, six locales no.** The typed-key mechanism and the back-fill are the parts to copy |
| PWA | Manifest with a swap start URL, and a ten-line service worker that caches one path and has **no fetch handler** — installable, not offline-capable | **Defer.** An installability shim that cannot serve offline is worse than nothing for a funds surface |
| Support widget | Third-party chat, lazily injected, with a "send my logs" path that uploads a log JSON to the widget's API | **Reject the third-party widget.** Keep the local log export; a funds page must not load a third-party script |
| Settings | Bitcoin-only, slippage, gas top-up, denomination, fiat currency, decimal separator, privacy mode, zero-conf, rescue key, logs. No backend override, no theme toggle, no in-app network switch — network is a build-time config, Tor is auto-detected from an onion hostname | Denomination, separator, privacy mode, rescue key, and logs map across. Slippage and gas top-up are token-route concepts we do not have. **Relay selection is ours and has no Boltz analogue** |
| Embedding | `embedded=true` strips nav and footer, a strict-origin parent `postMessage` on terminal status, and a documented prefill parameter set with explicit precedence rules | **Later, and valuable** — this is how a multi-provider widget reaches merchants without an integration |
| Referral | A fixed build/platform string sent as a request header; the URL parameter exists but is unread | Not applicable — providers are discovered, not referred |

## 8. Gap analysis

The heart of the document. "`/demo` today" refers to
`apps/openagents.com/apps/market-demo/main.rs` (1 520 lines): a six-stage
scripted walkthrough at a fixed 100 000 sat amount, with no amount entry,
no destination entry, no keys, and one live NIP-11 relay probe. [source]

| Boltz capability | `/demo` today | What we must build | NIP-MKT / MKT-SWP concept |
| --- | --- | --- | --- |
| Send/receive asset selectors with a flip control | No — the pair is fixed in the script | Two selectors over discovered Offerings, plus a direction toggle; unreachable directions greyed before selection | `39601` Offering `sides`, ordered `[input_asset_id, output_asset_id]`, `max="0"` disables a side |
| Pair availability from an API | No | Availability folded from live `39601` heads across providers, with freshness and a paused-provider state | Offering head, provider `39600` availability label |
| Amount entry with MAX, both sides editable | No — fixed amount | Both fields editable, authoritative-side tracking, MAX bounded by the Offering maximum | `min`/`max` decimal strings per side; §3.2 canonical amounts |
| Min/max validation with a stated reason | No | The disabled-with-a-reason button, parameterised in the user's denomination | `swp_invalid_amount`, `swp_side_disabled` |
| Unit toggle (BTC/sats) | No — sats only, hard-coded | Persisted denomination and separator preferences over integer atomic units | §3.2: decimal-string satoshis, no float, no exponent, no suffix |
| Live rate and fee disclosure | Static labels | Per-quote fee breakdown: provider fee, miner-fee budget, routing-fee budget, who pays each, the rounding rule, and the amount equation | §3.3 fee fields; `amount_equation`, `floor_output_sats` |
| Fee framed as promise vs estimate | No | Explicit: the output amount is the fill promise and may not be reduced after Order | §3.3 output promise |
| Price-feed provenance | No | When a Quote pins a feed, show the URL, pointer, observation, age limit, and response digest, and refuse a substitute host | §3.4 exact price-feed pinning; `swp_price_feed_invalid`, `swp_price_feed_stale` |
| Destination address entry and validation | No | Address and invoice fields with per-asset validation, paste-driven route switching, and QR scan | §7.1 checks 3–5; `swp_invalid_asset_id`, `swp_script_commitment_mismatch` |
| Lightning invoice parsing and constraints | No | Local parse of network, payment hash, amount, expiry, minimum final CLTV, route-hint policy, and description commitment; amountless invoices refused | §7.2; `swp_invoice_invalid`, `swp_payment_hash_mismatch` |
| Quote lifetime | None — quotes are not objects | **Beyond parity**: quote identity, an expiry countdown, firm-vs-indicative, and a reservation class with its proof class | Quote `expiration`, `quote`, `reservation`, `reservation_terms.proof_class`; `swp_quote_expired` |
| Competing offers from independent providers | Two scripted quotes | **The differentiator**: a real compare table over Quotes from independent providers, sortable, best-execution first, with custody dimensions per row | `39605` Quote; MKT §"Roles and authority" |
| Custody disclosure | Static strip | Six dimensions per Quote plus the worst-case custody duration in both wall-clock and height terms | §6 custody and control dimensions |
| Verify-before-fund | Scripted checklist | Real checklist driven by the engine: signatures, references, script and tree parsed from bytes, re-derived output key, payment hash, timelock inequalities, exit package built and digest-checked, confirmation and RBF policy — funding disabled until all pass | §7.1–§7.4; `swp_funding_not_authorized` |
| In-browser key material | **None** | Per-session key and preimage generation, a secret store, and the rescue ceremony **before** funding | §12 exit package; §14 forbidden material list |
| Rescue page | No | A recovery surface that works with every provider and relay gone, from the persisted package plus the secret store | §12.1 doomsday drill; `swp_exit_package_unusable` |
| Cooperative and unilateral refund | No | Cooperative path first, unilateral script path as the guaranteed fallback, with the height bound and the time estimate shown separately | §8 timeout ladders; §9 recovery branches |
| History | No | Local signed-record store keyed by session, with resumable in-flight sessions and an **import path** | MKT §"State, replay, and recovery"; idempotency keys |
| Status streaming | Scripted tape | Relay subscription over the existing wasm-proven client, with EOSE snapshot then live fold | Existing Immortal client; NIP-42 for recipient-scoped reads |
| Status rendering | One lane with a scripted gap | Two lanes with per-signer sequences, gaps as gaps, forks as forks, and a rung label that never infers upward | §9.5; MKT Status `seq`/`previous`; provenance labels |
| Terminal outcome | Scripted Close plus receipt | Close with complete per-asset loss accounting, conflicting Closes both visible, and a redacted public receipt | §15 loss accounting; `39603` receipt; §14 receipt consent |
| Error presentation | None | Typed identifiers mapped to localisable messages; never a counterparty's prose | §17 error table |
| Nav: Swap, Rescue, History | No — one page | Three routes plus the existing public surfaces | — |
| i18n | No — English only, and the repository has no i18n infrastructure at all | Typed-key scaffolding, English first | — |
| Embed / prefill / parent messaging | No | Later phase; strict-origin only | — |

## 9. What parity must not copy

- **The single coordinator.** Boltz's API *is* the market: one host
  supplies pairs, limits, fees, status, cooperative signatures, and swap
  discovery. Ours is many providers publishing signed Offerings on
  relays. No page may be written in a way that assumes one operator, and
  no copy may say "our rate" or "our fee" when the rate and fee belong to
  a provider the user chose.
- **Any claim of settlement the evidence does not carry.** A `completed`
  Status is one signer's claim. Relay acceptance proves transport only.
  The rung renders at the narrowest level the exact evidence proves.
- **Raw upstream error prose, and substring matching on it.** We have
  fifty-plus stable error identifiers; the UI maps identifiers, never
  strings.
- **Discarding a typed discriminant at the presentation boundary.** Six
  parse failure modes collapsed into one message is a loss we compute and
  then throw away.
- **Non-hardened derivation, and handing a master public key to a
  provider to discover your own history.** Discovery runs off our own
  persisted signed records.
- **A rescue path that needs the coordinator for the swap script.** The
  exit package is complete before funding or it is
  `swp_exit_package_unusable`.
- **A write-only backup.** If we ship an export, we ship the import in
  the same issue.
- **Auto-switching the denomination while the user types.**
- **A third-party support script on a funds page.**
- **An installability shim with no offline capability.** Either the
  service worker serves the swap surface offline or we do not register
  one.
- **The full asset matrix.** Stablecoins, DEX hops, bridges, slippage
  tolerance, and gas top-up are outside MKT-SWP v1, which requires
  `evm_leg` to be absent or null and Offering `evm_extension` to be
  `unsupported`. Building that UI now would be building for a rail the
  protocol refuses to execute.

## 10. The architecture question parity forces

Parity turns the demo into a product surface, and that exposes a
contradiction the rollout plan did not have to face. It should be
recorded rather than smoothed over.

**The facts.** `@openagentsinc/nip-mkt` already implements the NIP-MKT
**base** in Effect/TypeScript — per-kind schemas, tag grammar, NIP-59
transport, idempotency, quote and reservation projections, per-signer gap
and fork detection, expiry. It does **not** implement MKT-SWP: profile
content is opaque to it. The profile — script and tree parsing, output-key
re-derivation, invoice checks, MuSig2 transcript checks, timeout ladders,
exit packages, the typestate fund-authorisation flow, and the keyless
broadcaster — lives in the Immortal client crate at roughly ten thousand
lines of Rust, builds without tokio for `wasm32`, and takes secret keys
as bytes with no randomness source of its own. [source]

**The contradiction.** The plan's thesis is one GPUI component set on two
surfaces. A GPUI/WebGPU canvas is right for a demo and wrong for the page
a user moves money on: it has no DOM, therefore no screen reader, no
browser text selection, no browser translation, no indexable content, and
no rendering at all where WebGPU is unavailable. The repository's own
mandate also puts product UI on Effect Native. You cannot simultaneously
have an Effect Native product swap surface and one GPUI component set on
both surfaces. Something gives.

**What must not give** is having verify-before-fund implemented twice. A
TypeScript re-implementation of MKT-SWP would be a second chance to be
wrong about the exact thing that authorises spending money, and the wrong
copy would be the one gating the fund button.

**Recommendation, for owner confirmation.** One engine, two renderers:

- The Immortal client crate compiles to `wasm32` and is exposed to
  JavaScript through a thin binding whose surface is an Effect Schema
  contract. **Everything that can authorise funding lives behind that
  boundary**: profile validation, verification, exit-package construction
  and digest binding, and transaction construction.
- The Effect Native host owns rendering, storage, entropy (the engine has
  no RNG, so key and preimage material is seeded from WebCrypto in the
  host), and the relay transport already implemented in
  `@openagentsinc/nip-mkt`.
- The unifying artifact across web and Omega becomes the **exported typed
  session view-model**, not shared Rust widgets. Omega's `market_ui` GPUI
  components and the web's Effect Native components render the same
  contract, and behaviour contracts hold them to the same laws.
- `/demo` keeps its current job unchanged: the protocol walkthrough, in
  the `/dh` lineage, gated as it is today.

**The rejected alternative, stated honestly.** Promote the GPUI wasm
document to the product surface at a swap route. It preserves one
component set and reuses the engine natively with no binding work. It
costs accessibility, text selection, translation, indexability, and every
browser without WebGPU — on the one page where a user is about to commit
funds — and it contradicts the Effect Native product mandate. It is the
faster path and the wrong one.

**A boundary consequence to state plainly in the product.** A browser
holding keys is a weaker custody environment than Omega: any script
injection on the origin reaches the key material, and Boltz has the same
exposure without saying so. Our surface should carry a strict
content-security posture, load no third-party script, and be honest that
the desktop path exists — not as a disclaimer nobody reads, but as a
visible option at the point the amount stops being small.

## 11. Harvest map

| Item | What it is | Where it lands |
| --- | --- | --- |
| Disabled-with-a-reason primary action | One always-present button; label, colour, disabled, and content computed independently; the refusal states the amount in the user's current denomination | The swap widget's action control, and a design-system law for every gated action |
| Complete state enumeration | 27 pre-creation states and 20 backend statuses, with claiming/refunding/refunded correctly identified as client-derived rather than reported | The typed widget state and the session projection; the source of the behaviour-contract list |
| Verification of the counterparty's response | Client re-derives tree, address, payment string, and hashes and refuses on mismatch | Already law for us (§7.1); harvest the *habit*, reject the anonymous failure page |
| Real backup verification | Re-upload the file, or pass a word quiz — not a checkbox | The rescue-key ceremony, moved before funding and applied to both directions |
| Deterministic preimage derivation | Preimage as a function of the derived claim key, so it survives with the seed | A candidate for `preimage_recovery_ref`; note that it makes the preimage a function of a key, which is a deliberate trade |
| Explorer-diverse broadcast and fee estimation | Races the coordinator and a public explorer, takes whichever succeeds; per-asset fee floors | The exit package already carries `broadcast.esplora_urls` and `minimum_agreeing_sources`; harvest the racing pattern |
| Automatic cooperative-to-unilateral fallback | Cooperative co-sign first, script path on any failure, with relay rejections mapped to a wait-for-expiry state | The refund flow, with our height-plus-estimate display rule |
| Named per-record locks | Serialised read-modify-write per swap id, plus a global claim lock | The local session store, where a status fold and a background task will race |
| App-wide resume | Subscriptions re-attached for every non-final stored swap on mount, plus a one-shot catch-up fetch | Session resume; ours also has EOSE-snapshot-then-live already proven |
| Socket discipline | Ping-based half-open detection, jittered backoff, stability reset, degrade-to-polling after three failures, change-only emission | The relay client's reconnect posture |
| Storage schema versioning | A version key and sequential migrations that rewrite every record | The local session store from day one |
| Actionability-first history sort | Rows needing a claim or refund float above chronology, computed from chain state | The History surface |
| Destination paste that switches the route | Pasting the wrong kind of destination changes the pair instead of refusing | Asset selection, with our Offering-derived availability check |
| Typed-key i18n with English back-fill | Every call site type-checked against the English key set; missing translations fall back rather than blank | i18n scaffolding, English-only first |
| Embed mode | Nav and footer stripped, strict-origin parent messaging, documented prefill precedence | A later distribution phase, alongside the BTCPay/WDK surfaces already recorded in the Satora teardown |
| Version in the footer | Release tag and commit linked from the page | Any surface that holds keys should say which build it is |

## 12. Routing

| Finding | Owning issue |
| --- | --- |
| Widget shell, typed state, and the engine boundary decision (§2.1-§2.3, §10) | SWAP-0 — openagents#9315 |
| Asset and direction selection from Offerings; rate and fee panel (§3.1-§3.4) | SWAP-1 — openagents#9316 |
| Destination entry and validation, including invoice constraints (§3.5, §2.5) | SWAP-2 — openagents#9317 |
| Multi-provider quote comparison, expiry, reservation and custody disclosure, verify-before-fund (§2.2, §3.3) | SWAP-3 — openagents#9318 |
| Key generation, rescue ceremony, and the Rescue page (§4) | SWAP-4 — openagents#9319 |
| History, resumable sessions, and import/export (§5) | SWAP-5 — openagents#9320 |
| Status and progress against per-signer NIP-MKT Status (§6, §2.4) | SWAP-6 — openagents#9321 |
| Nav, routes, build provenance, and the surrounding surfaces (§7) | SWAP-7 — openagents#9322 |
| i18n scaffolding and the typed error-message table (§2.5, §7) | SWAP-8 — openagents#9323 |
| Custody-boundary and component-set amendments | [rollout plan](../markets/2026-08-04-swap-demo-ui-rollout-plan.md), amended by this teardown |
| Which parity components belong to the shared GPUI crate | omega#244 |
| Engine capabilities the UI depends on | immortal#12 (landed), #14, #15, #18 |
| Stablecoin and EVM surface | deferred to a future `mkt-swp-evm` extension; not parity scope |

The index also lives on
[openagents#9314](https://github.com/OpenAgentsInc/openagents/issues/9314).
