# Amounts: BIP 177 with a legacy BTC toggle

OpenAgents shows bitcoin amounts as [BIP 177](https://bips.dev/177/)
integers: one **bitcoin** is the base unit that used to be called a satoshi,
so `0.00010000 BTC` shows as `₿10,000`. The legacy BTC code keeps its
meaning (1 BTC = 100,000,000 base units) and stays one tap away. Owner
decision, 2026-09-28 ([#9881](https://github.com/OpenAgentsInc/openagents/issues/9881)).

## The rule

- **One formatter.** `crates/bitcoin-amount` shows and reads every amount.
  `Format::Bip177` (the default) renders `₿12,345` and speaks "12,345
  bitcoin"; `Format::LegacyBtc` renders `0.00012345 BTC`. Grouping is by
  thousands in both (`21,000,000.00000000 BTC`).
- **Integers only.** Amounts are `u64` base units in memory, in storage, and
  across the app boundary. The legacy decimal is built from and parsed into
  integer digits; no `f32`/`f64` is involved.
- **Typed amounts follow the format.** BIP 177 mode takes whole base units
  (`1000`, `₿1,000`, `1,000 bitcoin`) and refuses a decimal point. Legacy mode
  takes decimal BTC with at most eight decimals (`0.00001`, `1.5 BTC`, `.25`).
  Both refuse zero, anything smaller than one base unit, anything above
  21,000,000 BTC, and anything that overflows `u64`.
- **One choice, everywhere.** The phone saves the choice as
  `amount-format` (`bip177` or `btc`) in the app's state directory and
  applies it to the Wallet and every other surface through the packet's
  `amounts` view. The command line reads `OPENAGENTS_AMOUNT_FORMAT` for its
  text output.
- **Transitional note.** Until the person reads it, the Wallet shows a short
  note explaining the change, with the format choice beside it. The balance
  always shows both forms (the chosen one large, the other below it), which
  is BIP 177's transitional dual display. The **Show amounts as** control
  stays in the Wallet after the note is gone.
- **No "sat" in product copy.** Screens say "bitcoin", `₿`, or BTC.

## Names that keep `sat` and `msat`

Machine-readable names are not display, and renaming them would break
protocols, stored data, or scripts. They keep their names and units:

| Name | Unit | Why it stays |
| --- | --- | --- |
| BOLT11 `amount_msat`, LNURL `minSendable`/`maxSendable`, `--msat`/`--max-msat`/`--max-fee-msat` flags, x402 `amount_msat` | millisatoshi | Lightning's wire unit is finer than one base unit; BIP 177 integers cannot hold it. The phone rounds LNURL bounds inward to whole base units. |
| x402 `asset: "BTC"` with `amount` in msat | millisatoshi | Fixed by the x402 exact-Lightning scheme and NIP-X402. |
| Rust fields and JSON keys ending `_sats` (`balance_sats`, `amount_sats`, `fee_sats`, `onchain_total_sats`, the Breez SDK's `amount_sat`) | base units | Internal and machine-readable; the value already is a BIP 177 integer. |
| CLI flags `--sats`, `--lsp-sats`, `--our-sats` | base units | Scripts depend on them; `openagents wallet` help says they take base units. |
| Agent spend protocol `unit: "msat"`, `amount_msat`, `fee_max_msat`, `period_msat`, `total_msat` ([spend protocol](spend-protocol.md)); the wallet design's `unit: "sat"` | millisatoshi / base units | Wire fields for grants, requests, and receipts; the approval sheet shows them through the formatter (`Format::show_msat`: `₿N` when whole, `N msat` otherwise). |
| NIP-60/61 Cashu `unit: "sat"`, NIP-15 `currency: "SAT"` | base units | Published Nostr protocol values. |
| Fee rates `sat/vB` | base units per virtual byte | A rate, not an amount; the SDK's and Bitcoin's own name. |
| MoonPay's decimal BTC in its purchase URL | BTC | The provider's parameter. |

## Audit (2026-09-28)

Changed:

| Surface | Where | What |
| --- | --- | --- |
| Wallet balance, dual display, spoken form | `crates/openagents-mobile/src/wallet.rs`, `WalletTab.swift` | `balance` in the chosen format, `balance_alternate` in the other, `balance_spoken` for VoiceOver; `balance_btc` removed. |
| Receive: invoice amount entry and caption | same | Entry parsed in the chosen format; "Lightning invoice for ₿1,000". |
| Send: amount entry, LNURL range, confirmation amount, fee, total, "Send ₿…" button | same | All through the formatter; LNURL range asks name ₿ or BTC. |
| Buy (MoonPay, Cash App) amount entry | same | Parsed in the chosen format; "Enter how much bitcoin to buy." |
| Deposits and claim fees | `wallet.rs`, `spark.rs` | Deposit problems are typed (`DepositProblem`) and worded at render time, so the fee follows the format. |
| History rows and fees | `wallet.rs` | `+₿1,000`, `₿3 fee`. |
| Balance warning (#9858) | `wallet.rs` | "more than ₿1,000,000". |
| Agent payment approval sheet, computer grants, history, and notices (#9863) | `crates/openagents-mobile/src/spend.rs` | Amount, fee, fee ceiling, and what a grant has left follow the saved format; `docs/breez/spend-protocol.md` defaults read ₿. |
| Deposit refunds and on-chain send speeds (#9862) | `wallet.rs` | Refund amount, refund fee, and each speed's fee through the formatter; the fee rate stays `sat/vB`. |
| Android Wallet (#9861) | `bins/openagents-android/.../WalletScreen.kt`, `bins/openagents-android/README.md` | Same Rust views: `balance_alternate`, `balance_spoken`, the note, **Show amounts as**, and unit-named amount fields with a decimal keyboard in legacy mode. |
| Amount format setting and transitional note | `crates/openagents-mobile/src/amounts.rs`, `app.rs`, `WalletTab.swift`, `MobileBridge.swift` | `amount_format` and `amount_note_acknowledge` requests; packet `amounts`. |
| iOS amount fields | `WalletTab.swift` | Placeholders name ₿ or BTC; number pad in BIP 177 mode, decimal pad in legacy mode. |
| `openagents wallet info` text | `crates/openagents-cli/src/wallet.rs` | On-chain and Lightning balances as `₿N`; usage explains the flag units. |
| `openagents` help for x402 | `crates/openagents-cli/src/main.rs` | "for an exact bitcoin amount". |
| Docs | `docs/breez/wallet-design.md`, `docs/game/playtesting.md` (wallet session script), `INVARIANTS.md`, `bins/openagents-ios/README.md` | New terms; the LNURL invariant says base units. |

Left as they are, with reasons:

| Surface | Reason |
| --- | --- |
| x402 CLI text (`paid N msat`, `serving … for N msat`), `wallet` channel `inbound/outbound N msat`, x402 policy errors | Millisatoshi amounts, below one base unit; showing them as `₿` would round. They are developer surfaces priced in the wire unit. |
| Coder, Gym, Verse, XP screens | Show no bitcoin amounts; XP is not money (NIP-XP). |
| Design and history docs (`docs/verse/gdd.md`, `docs/verse/agent-trainer-leveling.md`, `docs/coder/*`, `docs/bitcoin/*`, `docs/breez/history.md`, `docs/cli/README.md` transcripts of past runs, `docs/transcripts/`) | Prose about plans and past events, not product UI; they quote amounts as they were recorded. New product copy follows this page. |
| `docs/breez/sdk-review.md`, `breez-vs-spark.md` | Quote the Breez SDK's own names and documentation. |
