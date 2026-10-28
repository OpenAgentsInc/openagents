# Breez and Spark in this repository

Breez or Spark has come and gone in this repository since March 2025. Each
round below names the commits to read. The detailed studies were deleted by
the Nuke (`0b401561a0`, 2026-10-27). Read them from its parent:

```sh
git show 0b401561a0^:docs/breez/history.md
git show 0b401561a0^:docs/breez/sdk-review.md
git show 0b401561a0^:docs/breez/wallet-design.md
git show 0b401561a0^:docs/breez/spend-protocol.md
git show 0b401561a0^:docs/audits/2026-10-17-spark-wallet.md
```

## Rounds

| When | What | How it ended |
| --- | --- | --- |
| Mar–Jun 2025 | Onyx and web wallets on Breez's Liquid SDK | Repository reset |
| Nov 2025 | Breez Spark chosen on paper | — |
| Dec 2025–Feb 2026 | `crates/spark` in Rust | Carried into the next round |
| Feb–May 2026 | Desktop wallet pane, Pylon's Spark runtime, and the Nexus treasury paying Pylon providers on mainnet | Removed (#4497, #4500, #4505) after operational failures |
| Jun–Jul 2026 | TypeScript Spark rail for Pylon payouts and offline receive. Real Tassadar training settlements of 1,005 sats; Forum tips | Scope cut: payments left the product (#8795); about 652 sats awaited a sweep |
| Sep 2026 | `openagents wallet` built on `ldk-node` instead (#9777) | Spark can't be an x402 receiver |
| 2026-09-28 → 10-27 | Phone wallet on `breez-sdk-spark` 0.26.0, mainnet, then Spark on every device and agent payments | Removed by the Nuke, not for a wallet fault |

### The May 2026 failure

The Nexus treasury was one long-lived server wallet paying many providers.
The removal audit (`7679951e05`) says Spark "repeatedly put slow wallet
sync, stale history, and leaf spendability on the operational critical
path." The failures:

- Funding timeouts of 10, 20, 180 and 600 seconds.
- History that stayed empty while balances changed.
- Leaves stuck as `SplitLocked` or `TransferLocked`.
- `TreeServiceError(InsufficientFunds)` despite a nominal balance.
- A pin far behind upstream: 0.6.6 read 0 sats where 0.12.2 read 138,877
  sats (#4198).
- An on-chain deposit that never credited (#4189).
- Background sync poisoning (#4321).

### The last round, 2026-09-28 to 2026-10-27

| Commit | What |
| --- | --- |
| `b99e9d224a` | Plan to re-add the Breez SDK as the app's Spark wallet (#9854) |
| `7bc872cfed` | Phone wallet on Spark mainnet. Its features: <br>• the balance shown at once from the last read <br>• receive by BOLT11, Spark address or Bitcoin address, with QR <br>• send only a confirmed quote, once, with a fresh idempotency key <br>• history <br>• automatic deposit claims <br>• MoonPay and Cash App buying <br>• recovery words behind a warning <br>• the seed as BIP-39 entropy in its own Keychain item, leaving Rust only in direct replies |
| `e56d173480` | Android, with a Keystore-wrapped seed |
| `f0466f1b15` | Pay by npub, Lightning address or QR; publish the Spark address |
| `c26e37c366` | An agent asks the phone to pay; the owner approves each payment |
| `d4310fc4fa`, `315de9957c` | Spark on every device (#10202): `openagents wallet` is the phone's wallet, from the same seed |
| `29f3ac33d6`, `30a3b74866`, `7ee084c9c2` | Payouts R5 to R7: a destination resolver (NIP-A3 Spark, `lud16`), a payout worker, and a daily reconciliation |
| `631232dee6`, `69152fef0d` | x402 buyers pay from the person's wallet; the default fee cap refused a 5-sat call with a 3-sat Spark fee |
| `4b2738fcd1` | Typed pay failures that separate "not sent" from "outcome unknown" |
| `67c75999ee` | The wallet back on the rewritten phone app, keeping the old seed (#11999) |
| `a42b0ed43e`, `6faabe671f` | The same 12 words become the Nostr identity (#11995, #12004) |
| `90ae2804bb` | The audit of the wallet against the SDK and x402 |
| `dd9457ab7a`, `b86ce5645e` | Pylon 1.0 sells into, pays from, and is paid to the Spark wallet (#12193, #12197) |
| `c720d485bd`, `a6077b636a` | `name@openagents.com` Lightning addresses on a self-hosted LNURL server with zaps, and an opt-in browser wallet (#12307) |
| `473cd69be5` | The phone approves and pays the browser's payment requests |

The code lived in:

- `crates/spark-wallet` (package `openagents-spark`), the only caller of the
  SDK, with these files:
  - `spark.rs` — the node;
  - `model.rs` — the shared `Node` trait;
  - `seed.rs`;
  - `store.rs` — a JSON store, because one Cargo lock can link only one
    SQLite;
  - `computer.rs`;
  - `link.rs` — the NIP-44 seed hand-off;
- `crates/openagents-mobile/src/wallet.rs`, and later
  `apps/openagents/crates/mobile/src/wallet/`;
- the iOS screens in `apps/openagents/apps/ios/OpenAgents/Wallet/`.

## Owner decisions that still apply

From `docs/breez/README.md` (2026-09-28 and 2026-10-02):

1. Spark's trust model is acceptable for a person's main wallet, with a
   plain-language trust note.
2. Separate agent seeds only when needed.
3. Code isn't blocked on a NIP; the spend grant, request and receipt formats
   are the contract.
4. No regulatory review.
5. The Breez API key is committed in source. It replaced the old "never
   embed the key" lesson, because the Breez team confirmed it is a basic
   validation key that can't be hidden.
6. Mainnet on the phone. Regtest is for automated tests only.
7. (Buying through MoonPay and Cash App, #9865.) Revisit it: the Cash App
   bridges are USDC/USDT, which the 2026-10-09 money rule bans.
8. Spark on every device, from the same seed, for one balance everywhere.

## Lessons

1. **Keep Spark off paths that must answer within seconds.** Treat balance,
   history and spendable leaves as eventually consistent. Show the last
   balance at once with a "connecting" state. Reserve before sending, and
   show pending states.
2. **Stay on a current tag.** Most incidents came from an old pin.
3. **Keep a person's wallet apart from a treasury.** The failures came from
   one hot server wallet paying many people. Agent spending under grants
   brings that load back.
4. **One set of words.** The 0.26 phone app could make a second seed when
   the identity existed first (audit finding 1).
5. **Back up the exit data.** The words restore the balance only while
   Spark works. The earlier app saved `export_unilateral_exit_state`
   encrypted after each sync and exported it to Files; the rewrite dropped
   it.
6. **Narrow what computers hold.** `openagents wallet link` sent the whole
   seed, which became the Nostr identity too after #11995. Send the account
   key instead.
7. **A pending agent payment needs follow-up.** Wait on `PaymentSucceeded`
   or poll `getPayment` under the same idempotency key, rather than failing
   the call.
8. **Write the retirement path before launch.** Two removals stranded sats.
9. **Decide x402 receiving separately.** Spark can't be the receiver.
