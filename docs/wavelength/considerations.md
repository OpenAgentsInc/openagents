# Wavelength: considerations

This is our assessment, not the vendor's. Facts are from the SDK at
`6f71128`, its docs, and the native docs summarized in
[native-ios-android.md](native-ios-android.md).

## Fit

- **Bitcoin only.** It uses Lightning and Ark, both Bitcoin, so it fits §26
  and the Bitcoin-only rule. There are no stablecoins or Taproot Assets, so
  it doesn't replace `tap-ldk` work.
- **Self-custodial with no backend.** The wallet runs on the user's device
  and we operate nothing. That suits wallets users own, such as paying for
  agents, tipping and receiving earnings.
- **Same API everywhere.** One daemon API covers web, React Native, Swift
  and Kotlin, so web and native integrations share concepts and tests.

## Risks

1. **Maturity.** The daemon and SDK are at 0.2.0, and the native wrappers
   aren't published. Mainnet has no preset and must be opted into
   (`allowMainnet: true`). Treat it as signet-only until Lightning Labs
   ships mainnet defaults.
2. **Operator trust and availability.** Every payment needs Lightning Labs'
   Ark server, and Lightning needs their swap server. The user can always
   exit to chain on their own, but day-to-day use depends on those services,
   and swaps cost fees. This is a different trust model from our
   MDK, LDK and Taproot Assets paths, and the product copy must say so
   accurately.
3. **Liveness.** VTXOs expire. A wallet must come online to refresh them,
   or exit before the unilateral-exit window closes.
   - That is fine for always-on agents.
   - It is dangerous for a phone app opened once a quarter.
   - It needs reminders, visible deadlines, and a tested "exit to chain"
     path before real funds.
4. **The app can steal the keys.** Their docs say any script on the
   wallet's web origin, or any code in the app process, can drive a passkey
   ceremony and read the root secret. On the web, put the wallet on its own
   origin with a strict CSP and no third-party scripts, not inside
   openagents.com's main bundle. On iOS, every linked dependency is inside
   that boundary.
5. **Crashes.** Go panics outside `start` and stream reads end the app.
6. **Binary supply chain.**
   - The web runtime is digest-checked against pinned values; keep that on,
     never `VITE_RUNTIME_INTEGRITY=off`, outside local development.
   - The native xcframework and aar are fetched with `gh`. We pin the tag,
     record their hashes and check them, the way Bitcoin Core treats its
     dependencies (see the [verification report](../verification/2026-10-27-formal-verification-brainstorm.md) §2).
7. **Units.** The API uses satoshis and our `Amount` uses millisatoshis.
   Convert in one place, with checked arithmetic, and test it with
   `#forall` and `coax mutate` if that boundary is in Coax.
8. **Not for our own money.** It is a user wallet. Treasury, payouts and
   provider settlement stay on their current paths (the MDK treasury
   container and the Worker ledger), and they don't move to an
   operator-dependent Ark wallet.

## Recommendation

A signet spike, scoped to user-held wallets:
1. **Web.** Create, back up and unlock a wallet, receive and pay a signet
   invoice, and show the VTXO expiry. Use a separate origin with a strict
   CSP and runtime integrity on.
2. **iOS.** The same flow in `apps/openagents-ios` through `WalletKit`, with
   the xcframework pinned by hash, and the start/stop lifecycle tested
   across suspend and resume.
3. **Boundary.** One sats↔msat conversion with exhaustive checks, and the
   wallet's payment results mapped onto `payment.v1` outcomes: succeeded,
   failed or unknown.
4. **Exit.** Exercise unilateral exit on signet and record how long it takes
   and what it costs.

Go to mainnet only after:
- Lightning Labs publishes mainnet defaults;
- the exit path and expiry warnings are tested;
- the owner accepts the operator trust model in writing.

Revisit when wavelength-mobile publishes a Swift package and when the daemon
reaches 1.0.
