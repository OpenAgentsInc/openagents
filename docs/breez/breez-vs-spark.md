# Breez and Spark

Date: 2026-09-28

Scope: who builds and runs Spark and the Breez SDK, whether every use case on
Spark's [use cases page](https://docs.spark.money/start/use-cases) works
through the Breez SDK, what each SDK has that the other lacks, and which one
the OpenAgents Wallet should use. Sources are Spark's documentation, Spark's
repository (read-only clone of `buildonspark/spark` at `0b3a32a`,
2026-08-25, in `/Users/christopherdavid/work/projects/repos/buildonspark-spark`),
and the Breez repository (`breez/spark-sdk` at `754ae7959`, 2026-09-28).
**Unverified** marks claims this review could not confirm.

## Who builds and runs what

| Piece | Builder or operator | Evidence |
| --- | --- | --- |
| Spark protocol and operator server (Go) | Lightspark | Spark FAQ (<https://docs.spark.money/learn/faq>); `buildonspark/spark` is Apache-2.0 and its `MAINTAINERS` are Lightspark staff |
| Spark operators | Lightspark, Breez, and Flashnet: `0.spark.lightspark.com`, `spark-operator.breez.technology`, `2.spark.flashnet.xyz` | FAQ names the three founding operators. Spark's JS config (`sdks/js/packages/spark-sdk/src/services/wallet-config.ts`) and Breez's (`crates/spark-wallet/src/config.rs`) list the same endpoints. Breez sets a threshold of 2. |
| Spark service provider (SSP) | Lightspark (`api.lightspark.com`) | FAQ: "Lightspark is running the first SSP on Spark. Anyone can become an SSP." Breez's default points there. |
| Breez's own SSP (`sspd`, Rust) | Breez | `crates/spark-service-provider/sspd`, in active development. **Unverified:** whether it runs in production. It is not the default. |
| Spark's official SDKs | Lightspark: `@buildonspark/spark-sdk` 0.9.0 (Node, browser, React Native, Bare), `@buildonspark/issuer-sdk`, `@buildonspark/spark-mcp` | `sdks/js/packages/*`, Apache-2.0. All TypeScript. |
| Spark's Rust code | Lightspark: FROST signer and token primitives (`signer/`), with Swift and Python bindings used inside the JS SDK | No Spark-published Rust, Swift, or Kotlin **wallet** SDK was found. |
| Breez SDK (Spark) | Breez, MIT | Breez's own Rust client for the Spark protocol, begun 2025-06-16 by Breez engineers. It compiles copies of Spark's `.proto` files and uses Lightspark's FROST fork (`lightsparkdev/frost`). It is not a wrapper around the TypeScript SDK. Spark lists it as an integration (<https://docs.spark.money/integrations/breez>). |
| Cross-chain USDC and USDT, BTC and token swaps | Flashnet (Orchestra and its AMM) | `crates/flashnet`. See [stablecoin receive](stablecoin-receive.md). |

**The relationship in one sentence:** Lightspark designed Spark and runs its
coordinator operator and default SSP; Breez is an independent company that
runs one of the three operators and wrote its own Rust client and product
layer on top; Flashnet runs the third operator and the swap services both
SDKs use. The same network carries the money whichever SDK you pick.

## Use cases on Spark's page

| # | Spark use case | Through Breez | Breez API | Notes |
| --- | --- | --- | --- | --- |
| 1 | P2P and banking wallet | Yes | `connect`, `get_info`, `receive_payment`, `prepare_send_payment`/`send_payment`, `list_payments`, `claim_deposit`, `unilateral_exit`, contacts | Breez's main target. It adds Lightning addresses, LNURL, and passkey login. |
| 2 | Cross-border payments | Yes | BTC over Spark, Lightning, and on-chain; `get_cross_chain_routes` and cross-chain send and receive; `list_fiat_rates`; `buy_bitcoin` | Fiat ramps are third parties in both SDKs. |
| 3 | Social tipping | Yes | `register_lightning_address`, `lnurl_pay`, `receive_payment`, LNURL webhooks | Breez's LNURL server supports NIP-57 zaps (`crates/breez-sdk/lnurl/src/zap.rs`). |
| 4 | Global USD accounts | Yes | Stable Balance, USDB token payments, token conversion, cross-chain USDC and USDT | Dollars are USDB, swapped through Flashnet. USDB's issuer and backing are **unverified**. |
| 5 | DeFi wallet | Partial | Token balances, token sends, `send_batch`, conversion, HTLCs (`claim_htlc_payment`, HODL invoices) | No token allowances, which Spark's SDK has. No pool or liquidity management. |
| 6 | Trading platforms | Partial or no | Flashnet swaps; HTLCs for atomic swaps | No order book or pool APIs. Spark's wallet SDK lacks them too; a DEX needs Flashnet's SDK. |
| 7 | Token launchpad | Partial | `get_token_issuer()`: create, mint, burn, freeze, unfreeze; distribution with `send_batch` | One token per issuer wallet (use another account number). No holder-distribution query. |
| 8 | Bitcoin cashbacks | Yes | `send_payment`, `send_batch` for tokens | `send_batch` covers tokens only: "Sending sats to several recipients at once is not supported yet" (`batch_send.md`). |
| 9 | Play to earn | Yes | As 8, plus issuer API for a reward token | Same batch limit. |
| 10 | Referrals | Yes | `send_payment` to Spark address, Lightning address, or LNURL; webhooks | None. |
| 11 | Point conversions | Partial | Issuer API plus Flashnet conversion | Converting needs a Flashnet pool for the points token; Breez converts but does not create pools. |

## What Breez adds

- A native Rust core with UniFFI bindings for Swift, Kotlin, Kotlin
  Multiplatform, Python, Go, and C#, plus Flutter, React Native, and WASM.
- LNURL-pay, LNURL-withdraw, LNURL-auth, and an open-source Lightning-address
  server with NIP-57 zap support.
- Cross-chain USDC and USDT send and receive, Stable Balance, and token
  conversion inside one payment call.
- Passkey-derived seeds, Turnkey, external signers, and client signing
  (`build_unsigned_*_package`, `publish_signed_*_package`).
- Encrypted real-time sync across devices, and SQLite, Postgres, and MySQL
  storage with a multi-tenant server mode.
- Unilateral-exit tooling with state export and import.
- `PaymentObserver`, a hook that can veto any send.
- Contacts, fiat rates, fee recommendations, and `parse` for any payment
  input.

## What Spark's SDK has that Breez does not expose

- **Token allowances.** `createTokenAllowance`, `revokeTokenAllowance`,
  `queryTokenAllowances`, `startAllowancePull`, and `commitAllowancePull`
  (`sdks/js/packages/spark-sdk/src/services/tokens/allowances.ts`). An owner
  signs an allowance naming a spender key, a token, a per-transaction ceiling,
  a lifetime total, an expiry, and an optional recipient allowlist. It is
  replicated to every operator, and **every operator enforces it** when the
  spender pulls. Revocation is a permanent tombstone. At most one active
  allowance exists per owner, spender, and token. Breez's `crates/spark`
  compiles the `create_token_allowance` RPC from `spark_token.proto` but the
  SDK exposes none of it. This matters for agent grants; see
  [the wallet design](wallet-design.md#agent-wallets).
  **Unverified:** whether allowances are live on mainnet operators.
  **Limit:** tokens only. There is no allowance for BTC sats.
- Several tokens per issuer wallet and a holder-distribution query.
- Watchtower exited-leaf recovery (`recoverWatchtowerExitedLeaf`,
  `getWatchtowerExitedLeaves`).
- Lower-level calls such as `querySparkInvoices`, `getTokenL1Address`,
  `signTransaction`, `checkTimelock`, `claimMultiUtxoDeposit`, and
  `getSwapFeeEstimate`. Some exist in Breez's lower crates but not in
  `BreezSdk`. **Unverified:** a one-for-one match for each.
- An official MCP server (`@buildonspark/spark-mcp`).
- Neither SDK exposes a multisig wallet, although both carry
  `multisig.proto`.

## Recommendation

**Use the Breez SDK for every wallet in the OpenAgents app.** Reasons:

1. **It is the only option that fits the codebase.** Spark's wallet SDK is
   TypeScript and would need a JavaScript runtime inside the app. This
   repository does not allow TypeScript product code. The Breez crate links
   into `crates/openagents-mobile` directly, and the Swift and Kotlin hosts
   stay thin.
2. **It has the product layer we would otherwise build.** Lightning
   addresses, LNURL, zaps, sync, fee quotes, exit tooling, client signing,
   and a pre-send hook.
3. **The trust is the same.** Both SDKs talk to the same three operators and
   the same SSP.

**Borrow one thing from Spark:** token allowances. They are the only way to
give an agent bounded authority over the owner's funds that holds while the
phone is off, enforced by the operators rather than by our host software. Use
them for dollar (USDB) allowances once they are confirmed on mainnet. The
path is to call the RPC through Breez's lower `spark` crate, which already
compiles it, and to propose exposing it upstream in the Breez SDK. Do not
embed the TypeScript SDK to get it.

The cost of choosing Breez is a second implementation of the Spark client
protocol that must track Lightspark's changes. Breez has done so for over a
year, and it runs an operator, so it has reason to keep up.
