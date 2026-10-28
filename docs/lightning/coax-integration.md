# The Spark wallet in the iOS app, with Coax

`apps/openagents-ios` is a Swift and UIKit shell around Coax. Its home
(`metareal/home.coax`) compiles to C11 and a header in an Xcode build
phase. Swift calls the `#export_c` procedures, and Coax calls back through
scalar `#foreign` hooks (`host_load`, `host_dot`, `host_output_byte`, …)
that are implemented in `metareal/platform/host.c` and the Swift platform
layer. The wallet uses the same split.

## Who does what

```
UIKit screens (Swift)                 show balance, QR, quote, confirm, history
      │  touches, lifecycle
      ▼
metareal/wallet.coax  #authority      decides: may this send go? what state next?
      ▲  scalars over #export_c        what the HUD shows, what record to write
      │  / #foreign
SparkHost.swift (Swift)               does: Keychain, BreezSdkSpark calls,
      │                                events, timers, network
      ▼
BreezSdkSpark (Swift package, Rust)   Spark operators, SSP, chain service
```

- **Swift does the effects.** `SparkHost.swift` is the only file that
  imports `BreezSdkSpark`. Its jobs:
  - read the entropy from the Keychain;
  - build the SDK;
  - call `parse`, `prepareSendPayment`, `sendPayment` and `receivePayment`;
  - listen for events;
  - save the exit data after each sync;
  - disconnect when the app goes to the background.

  It passes Coax plain numbers, enum codes and byte views. **The words and
  keys never cross into Coax.** Coax needs none of them, so no secret sits
  in Coax memory or its records.
- **Coax decides.** `metareal/wallet.coax` is an `#authority` file. Every
  procedure in it must be run by a check, and the checks run on every
  build. It decides:
  - whether a quote may be paid;
  - how the payment state moves;
  - sats↔msat conversion;
  - spend limits and fee caps;
  - when the expiry and backup warnings show;
  - what the HUD shows.
- **The SDK parses Lightning.** Coax doesn't re-parse BOLT11 or LNURL.
  Swift hands Coax the fields the SDK parsed:
  - amount;
  - network;
  - expiry;
  - the quoted fee;
  - whether the destination is a Spark address.

  Coax checks them against policy. A later step can re-check the invoice's
  network prefix and amount in Coax as a second opinion.

## The authority

A first cut of the send decision and the outcome mapping. It is checked
with `coax0 0.5.1`: `coax test` passes 5 checks, and `coax mutate` catches
33 of 33 mutants.

```coax
#import "Basic";
#authority;

MAX_SAT :: 2_100_000_000_000_000;

// The SDK counts satoshis; payment.v1 counts millisatoshis.
sat_to_msat :: (sat: int) -> int {
  if sat < 0 || sat > MAX_SAT { return -1; }
  return sat * 1000;
}

Pay :: enum u8 { idle; quoted; sending; succeeded; not_sent; unknown; }
Verdict :: enum u8 { send; over_cap; over_limit; expired; bad_amount; busy; }

// Whether a confirmed quote may be paid. Every input comes from the host.
may_send :: (state: Pay, amount: int, fee: int, cap: int, left: int,
             now: int, expires: int) -> Verdict {
  if state != Pay.quoted { return Verdict.busy; }
  if amount <= 0 || fee < 0 || amount > MAX_SAT { return Verdict.bad_amount; }
  if now >= expires { return Verdict.expired; }
  if fee > cap { return Verdict.over_cap; }
  if amount > left - fee { return Verdict.over_limit; }
  return Verdict.send;
}

#forall "may_send" state: Pay, amount: -1..<6, fee: -1..<4, cap: 0..<3, left: 0..<8, late: bool {
  v := may_send(state, amount, fee, cap, left, ifx late then 10 else 9, 10);
  if state != Pay.quoted { #assert v == Verdict.busy; }
  else if amount <= 0 || fee < 0 { #assert v == Verdict.bad_amount; }
  else if late { #assert v == Verdict.expired; }
  else if fee > cap { #assert v == Verdict.over_cap; }
  else if amount + fee > left { #assert v == Verdict.over_limit; }
  else { #assert v == Verdict.send; }
}

// What the SDK reported. An unknown outcome never becomes a failure here.
Report :: enum u8 { sent; failed_before_send; timed_out; }

after_send :: (state: Pay, report: Report) -> Pay {
  if state != Pay.sending { return state; }
  if report == Report.sent { return Pay.succeeded; }
  if report == Report.failed_before_send { return Pay.not_sent; }
  return Pay.unknown;
}

#forall "after_send" state: Pay, report: Report {
  next := after_send(state, report);
  if state != Pay.sending { #assert next == state; }
  else if report == Report.sent { #assert next == Pay.succeeded; }
  else if report == Report.failed_before_send { #assert next == Pay.not_sent; }
  else { #assert next == Pay.unknown; }
}
```

The checked sketch also has `#test` cases at `MAX_SAT` and a `#forall` over
`sat_to_msat`. Those are left out above.

## Payment states

| State | Entered when | Leaves to |
| --- | --- | --- |
| `idle` | Nothing pending | `quoted` after `prepareSendPayment` returns |
| `quoted` | A quote with amount, fee and expiry is on screen | `sending` when the person confirms and `may_send` says `send`; `idle` on cancel or expiry |
| `sending` | `sendPayment` was called with a fresh idempotency key | `succeeded`, `not_sent` or `unknown` |
| `succeeded` | The SDK reported success; for Lightning, a preimage | `idle` |
| `not_sent` | The SDK failed before any leaf moved | `idle`; may quote again |
| `unknown` | Timeout or crash after the call | `succeeded` or `not_sent` only on a later SDK event or `getPayment` under the **same** key. Never resent with a new key. |

Rules this table makes checkable:

- One send per quote.
- The fee is never above the quote or the cap.
- The amount plus the fee never exceeds what is left of a limit.
- An unknown outcome is never shown as a failure.
- A retry reuses the idempotency key.

## Records

`payment.v1` (spec §26.5) has `WalletView`, `Invoice`, `PaymentRef`,
`PaymentEvidence`, `PaymentReceipt`, `SpendGrant`, `SpendRequest` and
`SpendReceipt`. Its status is Partial: there is no `proto/openagents/payment/`
module yet. When it lands:

- Add a selected `payment` profile and import it with `#import_proto` into
  a **separate** file, `metareal/wallet_records.coax`. Today `#authority`
  covers every generated API and host-address operation in a file (see
  `coax/README.md`, "Limits of the verification gates"), so the codecs
  can't share the authority file yet.
- The authority returns plain values. `wallet_records.coax` encodes them as
  `WalletView` (rail `RAIL_SPARK`, balance and pending in msat), `PaymentRef`
  (`payment_hash` or `spark_transfer`) and `PaymentReceipt`.
- The SDK's own types never become protocol types. That is the spec's rule
  for LDK, Spark and Breez (§13.2).

## Build and packaging

- **Package.** Add `breez-sdk-spark-swift` to `apps/openagents-ios/project.yml`
  under `packages`, at an exact version. Commit `Package.resolved`, which
  records the xcframework checksum. Bump it by hand, reading the release
  notes, the way Bitcoin Core treats its dependencies.
- **Coax.** The existing build phase gets a second library:
  - `coax c metareal/wallet.coax --library --header`;
  - the checks run as part of that compile, because the file is
    `#authority`;
  - a failing check stops the app build.
- **Size.** The xcframework is a Rust static library and adds megabytes to
  the app. Measure it in the first build and write it down.
- **API key.** The committed Breez key goes in `SparkHost.swift`'s
  configuration, as before. It isn't a secret.

## Lifecycle on iOS

- **Foreground.** Show the last balance from Coax's state at once. Connect,
  sync, then update it.
- **Background.** Disconnect. Receiving still works, because the SSP holds
  the Lightning side. The next sync claims what arrived.
- **After each sync**, save `exportUnilateralExitState` encrypted in the
  app's container, with an export to Files. Coax tracks when it was last
  saved and warns when it is stale.
- **Restore** replaces the identity and the wallet together, from the same
  words.
- **Suspension during `sending`.** The state is `unknown` until the next
  launch, which then asks `getPayment` under the same key.

## Order of work

1. `SparkHost.swift` and screens with no Coax logic, to prove the SDK opens
   from the existing Keychain item on device.
2. `metareal/wallet.coax` with `#authority`. Move every decision Swift made
   in step 1 into it, check-first.
3. Exit-data backup and the recoverable-funds display.
4. `wallet_records.coax` once `payment.v1` exists.
5. Lightning address and LNURL, then agent spend grants through
   `PaymentObserver`.
