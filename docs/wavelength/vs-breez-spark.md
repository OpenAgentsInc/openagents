# Wavelength or Breez Spark for the iOS wallet

*Notes from 2026-10-27. Reviewed against `breez/spark-sdk` `main` at
`83c8122` (latest tag `0.27.2`), including its Swift CLI at
`crates/breez-sdk/bindings/examples/cli/langs/swift/`, and against
`lightninglabs/wavelength-sdk` at `6f71128` (daemon `v0.2.0`). Our Breez
history comes from git, from the Nuke commit's parent (`0b401561a0^`).
Nothing was built or run against either network for this note.*

## Verdict

**Use Breez Spark for the first wallet in `apps/openagents-ios`.**
Wavelength stays a signet spike, per [considerations](considerations.md).
Wavelength's trust model is better. Spark wins today because it is ready to
ship on iOS and we have already run it:

1. **Mainnet now.** Spark has a published Swift package and a mainnet
   default. Our phone ran it on mainnet from 2026-09-28 until the Nuke.
   Wavelength has no mainnet preset, and its Swift wrapper isn't published.
2. **The same 12 words, with no extra step.** Spark takes the BIP-39
   mnemonic as is (`Seed.mnemonic`). Our NIP-06 identity already uses that
   mnemonic. Wavelength needs the BIP-85 step in
   [shared-words.md](shared-words.md) to open from the same words.
3. **The phone can be offline when money arrives.** Spark's service
   provider holds the Lightning side, so an invoice or Lightning address
   gets paid while the app is closed. Wavelength's VTXOs expire, so a phone
   that is rarely opened has to refresh them or exit before the deadline.
4. **We know where it breaks.** Five rounds of Breez in this repository
   left a list of failures and fixes ([below](#our-history-with-breez)).
   Wavelength is new to us.

Wavelength wins on trust and the shape of the stack. Lightning Labs runs it
as one Go daemon with one API for web, React Native and native code. An Ark
exit has no "operators deleted their old key shares" assumption. Revisit
the choice when Wavelength publishes mainnet defaults and a Swift package.
Plan for both behind one boundary (see [One boundary for both](#one-boundary-for-both)).

## Side by side

| | Breez Spark | Wavelength |
| --- | --- | --- |
| **Balance model** | Spark: off-chain leaves co-signed with FROST by you plus 2 of 3 operators (Lightspark, Breez, Flashnet) | Ark: VTXOs coordinated by Lightning Labs' Ark server, backed by pre-signed exit transactions |
| **Main trust assumption** | At least one operator deleted its old key share after each transfer. This can't be proven. | The Ark server stays honest and online for normal use. The exit doesn't depend on a deletion promise. |
| **Lightning** | Through Lightspark's service provider; atomic against the preimage | Through Lightning Labs' swap server |
| **If the operators stop** | Unilateral exit: needs exit data synced earlier, a confirmed native SegWit UTXO for CPFP fees, and days of timelocks. The SDK builds and signs; you broadcast. | Unilateral exit before the VTXO expiry window closes |
| **Liveness** | Receive works with the app closed. A previous owner's old exit must be answered within its timelock; the SDK's watchtower fee ladder pre-signs refunds. | VTXOs expire. The app must come online to refresh them or exit. |
| **Networks** | Mainnet (Breez API key), regtest, signet with a custom operator config. Spark has no hosted Lightning test network. | Signet, testnet, testnet4 presets. Mainnet only with `allowMainnet: true`. |
| **iOS package** | `breez-sdk-spark-swift` on Swift Package Manager (binary xcframework built with UniFFI from Rust) | `WalletKit` Swift actor over a gomobile xcframework, fetched from releases; not published as a package |
| **API shape** | Typed async Swift: `SdkBuilder(config:seed:)`, `connect`, `prepareSendPayment` then `sendPayment`, `receivePayment`, events | JSON bytes through gomobile calls, wrapped in a Swift actor |
| **Seed** | BIP-39 mnemonic or raw entropy. Keys at `m/8797555'/{account}'` (mainnet account 1). Passkey (PRF) seeds and external signers are also supported. | aezeed (24 words), or `startExternalSeedWallet` with 16 bytes of entropy we derive |
| **Lightning features** | BOLT11 send/receive, HODL invoices, LNURL-pay, -withdraw, -auth, Lightning addresses (Breez's server or our own domain), on-chain deposits with automatic claims | BOLT11 send/receive through swaps; see [README](README.md) |
| **Extras to keep off** | Stable Balance (USDB), USDC/USDT cross-chain, Cash App bridges, MoonPay | None found |
| **Cost to us** | Free SDK; end users pay Spark and Lightning fees; 5 bps integrator fee on Flashnet conversions (off if conversions are off) | Swap fees |
| **Maturity** | 0.27.x. Breez ships weekly; the Swift CLI is an automated port of the Rust CLI. | 0.2.0 |
| **x402** | Can pay. Can't receive: every Spark invoice is signed by the shared provider node, which NIP-X402's exclusive-payee rule forbids. | Not studied; probably the same receiver problem, since a swap server issues the invoice |

## What the Swift CLI shows

The CLI is Breez's reference for a Swift integration. It is about 5,500
lines, and most of that is the REPL, argument parsing and a 2,048-word
BIP-39 list.

- **Setup.** It builds a config with `defaultConfig(network:)` and sets
  `config.apiKey` (required on mainnet). The seed is
  `Seed.mnemonic(mnemonic:passphrase: nil)`. It calls
  `SdkBuilder(config:seed:)`, can add a REST chain service (Esplora or
  mempool.space) or a Postgres/MySQL store, then connects. The default
  store is SQLite in the data directory.
- **Payments.** `receive` takes a method: `sparkaddress`, `sparkinvoice`,
  `bitcoin`, `bolt11` (optionally HODL), or `crosschain`. `pay` calls
  `prepareSendPayment`, shows the fee, and calls `sendPayment` with an
  idempotency key. LNURL, Lightning address and deposit commands are
  separate.
- **Mainnet adds cross-chain.** On mainnet the CLI sets
  `config.crossChainConfig = CrossChainConfig()`, which turns on USDC/USDT.
  We must not copy that line. Stable Balance tokens are opt-in flags.
- **The CLI's seed handling isn't ours to copy.** It writes the mnemonic to
  a plain `phrase` file, and its file-based passkey PRF secret also sits on
  disk. Our rule stays: the words' entropy lives in the this-device-only
  Keychain item, is held in memory only, is never logged, and is shown once
  behind a warning.
- **Passkeys are a later option.** `resolvePasskeySeed` derives a seed from
  a WebAuthn PRF. It would replace the 12 words, so it conflicts with "one
  set of words for Nostr and money." Leave it out.

## Our history with Breez

Breez or Spark has come and gone in this repository since March 2025.
Details are in `0b401561a0^:docs/breez/history.md`, `sdk-review.md`, and
`docs/audits/2026-10-17-spark-wallet.md`.

| When | What | How it ended |
| --- | --- | --- |
| Mar–Jun 2025 | Onyx and web wallets on Breez Liquid | Repository reset |
| Feb–May 2026 | `crates/spark` in Rust; Nexus treasury paid Pylon providers on mainnet | Removed (#4497–#4505): slow sync, stale history, leaves stuck `SplitLocked`/`TransferLocked`, and `InsufficientFunds` despite a balance, all on a hot server wallet with an old pin |
| Jun–Jul 2026 | TypeScript Spark rail for Pylon payouts and offline receive; 1,005 sats of real training settlements | Scope cut (#8795); about 652 sats left awaiting a sweep |
| Sep 2026 | `openagents wallet` on `ldk-node` instead (#9777) | Chosen because Spark can't be the x402 receiver |
| 2026-09-28 → 10-27 | Phone Wallet on `breez-sdk-spark` 0.26.0 mainnet (`7bc872cfed`), Android with a Keystore-wrapped seed (`e56d173480`), Spark on every device (#10202), Pylon payouts to Spark (#12193), `name@openagents.com` Lightning addresses (#12307), the phone paying the browser's requests (`473cd69be5`) | Removed by the Nuke (`0b401561a0`), not for a wallet fault |

The owner decisions from 2026-09-28 and 2026-10-02 still describe the
setup:

- Spark's trust model is acceptable for a person's spending wallet, with a
  plain-language trust note in the app.
- The Breez API key is committed in source. The Breez team confirmed it is
  a basic validation key that can't be hidden.
- The phone wallet runs on mainnet.
- Treasury money stays off Spark.

What carries over:

1. **Keep Spark off paths that must answer within seconds.** Show the last
   balance at once with a "connecting" state. Treat history and spendable
   leaves as eventually consistent. Pay only a confirmed quote, once, with a
   fresh idempotency key.
2. **Pin a current tag.** Most past incidents came from an old pin. Start on
   `0.27.2` or newer. Before choosing, check which tag first carries
   `prepare_recover_funds`/`recover_funds` (on `main` now), so a person can
   see and recover leaves the operators moved on-chain.
3. **One set of words.** The 0.26 phone app could make a second seed when
   the identity came first (audit finding 1). The wallet must open from the
   identity's entropy in `com.openagents.app.spark`/`seed-v1`, never from
   `Seed::generate`.
4. **Back up exit data.** The words restore the balance only while Spark
   works. The earlier app saved `export_unilateral_exit_state` encrypted
   after each sync and exported it to Files. Bring that back before telling
   anyone the wallet survives an outage.
5. **Narrow what computers hold.** Linked computers got the whole seed,
   which after the 12-words change is also the Nostr identity. Send the
   Spark account key (`m/8797555'/1'`) or another account number instead.
6. **Keep x402 receiving off Spark.** Paying works with a fee cap, an
   invoice-derived idempotency key, a checked preimage, and `preferSpark`
   off.
7. **Write the retirement path before launch.** Two earlier removals
   stranded sats.

## Bitcoin-only rails

Spark moves bitcoin over Spark and Lightning, which fits the rule. Its token
features don't, unless the owner decides otherwise:

- Keep `crossChainConfig` unset. It covers USDC/USDT on Ethereum, Base,
  Solana, Tron and others, which the rule bans outright.
- Keep Cash App bridges off.
- Keep `stableBalanceConfig` unset. USDB is a Spark token, not a Taproot
  Asset; treat it as out of scope.
- Keep MoonPay off; it was on before (#9865).

Wavelength has none of these features.

## One boundary for both

The Coax plan for Wavelength works for Spark unchanged, so whichever wallet
ships first doesn't decide the second.

- **Coax `#authority`** (`metareal/wallet.coax`) holds the policy:
  - payment states;
  - quote checks;
  - expiry warnings;
  - sats↔msat conversion;
  - spend limits;
  - the HUD;
  - `payment.v1` outcomes (succeeded, failed, unknown).

  Spark's "not sent" versus "outcome unknown" (`AgentPayFailure`) maps onto
  the last two.
- **Swift does the effects** (`WalletHost`): Keychain, the SDK actor or
  builder, network calls and events. It hands Coax scalars over the
  `#foreign`/`#export_c` boundary, the way `home.coax` does.
- **Each backend is one Swift file.** `SparkHost.swift` wraps
  `BreezSdkSpark`; `WavelengthHost.swift` would wrap `WalletKit`. Both open
  from the same Keychain entropy: Spark directly, Wavelength through BIP-85.
  The shared words make a move possible. A person can open both wallets at
  once and send the Spark balance to a Wavelength invoice.

The Spark package is a prebuilt binary built from Rust outside our
toolchain, like Wavelength's gomobile framework. Pin its version and its
xcframework checksum (Swift Package Manager records it in
`Package.resolved`). Treat updates the way Bitcoin Core treats its
dependencies.

## First steps

1. Add `breez-sdk-spark-swift` pinned to the chosen tag in
   `apps/openagents-ios/project.yml`. Use the committed API key.
   Cross-chain, Stable Balance and real-time sync stay off.
2. Write `SparkHost.swift`:
   - open from the existing Keychain item;
   - make a new one only if none exists, and then make it from the identity's
     words;
   - show balance, receive (BOLT11, Spark address, with QR), send with a
     confirmed quote, and history.
3. Write `metareal/wallet.coax` with `#authority`. Its checks cover:
   - unit conversion and quote acceptance, exhaustive with `#forall`;
   - every outcome mapping;
   - `coax mutate` with no surviving mutants.
4. Bring back the exit-data backup, and show recoverable funds once the pin
   has them.
5. Smoke-test on mainnet with a throwaway seed and a few sats, never the
   owner's wallet. Regtest is for automated tests only.
