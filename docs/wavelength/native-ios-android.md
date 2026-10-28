# Wavelength on native iOS and Android

Source: the docs site's
[native overview](https://wavelength.lightning.engineering/native-ios-android/overview/),
[architecture](https://wavelength.lightning.engineering/native-ios-android/architecture/)
and [quickstart](https://wavelength.lightning.engineering/native-ios-android/quickstart/).
The canonical, faster-moving material is in
[wavelength-mobile](https://github.com/lightninglabs/wavelength-mobile):
[API guide](https://github.com/lightninglabs/wavelength-mobile/blob/main/docs/api-guide.md),
[architecture](https://github.com/lightninglabs/wavelength-mobile/blob/main/docs/architecture.md),
[signet](https://github.com/lightninglabs/wavelength-mobile/blob/main/docs/signet.md),
and the
[full gomobile method list](https://github.com/lightninglabs/wavelength/blob/main/docs/wavewalletdk_mobile.md).

## What it is

- **iOS:** `ios/WalletKit` is a Swift `actor` with `async` methods, an
  `AsyncThrowingStream` for activity, and `Codable` models.
- **Android:** `android/walletkit` is a Kotlin library with `suspend`
  functions, a `Flow` for activity, and typed models.
- **Under both:** the gomobile bindings `Wavewalletdk.xcframework` and
  `Wavewalletdk.aar`, built by each wavelength release. They are the same
  daemon the React Native package embeds, without the JavaScript layer.

## Architecture

- **In process.** The bindings start the whole Go daemon on a background
  goroutine and talk to it over an in-memory gRPC channel. Nothing listens on
  the network and there is no sidecar process.
- **JSON across the boundary.** gomobile carries few types, so the facade's
  RPC verbs take and return JSON bytes. The wrappers decode them with
  `Codable` or `kotlinx.serialization`; there is no protobuf runtime. A few
  hot paths return scalars, such as the confirmed balance.
- **No callbacks.**
  - `start(config)` blocks until the daemon is serving; the wrappers run it
    off the main thread.
  - Go errors become Swift `throws` or Kotlin exceptions.
  - Streams are pull handles that the wrappers turn into
    `AsyncThrowingStream` or `Flow`.
- **One daemon per process.** A second `start` before `stop` throws. `stop`
  tears down, unblocks pending stream reads, and allows a later restart, for
  example after the OS suspends and resumes the app.
- **Panics.** A panic is recovered only in `start` and in the stream's
  `next`. A Go panic anywhere else ends the app.

## First run (Swift)

```swift
let client = WalletClient()                     // one per app; an actor, held in @State
let dir = FileManager.default
    .urls(for: .documentDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("wavewalletdk").path
try await client.start(.signet(dataDir: dir))   // public signet endpoints

let res = try await client.createWallet(walletPassword: Data(password.utf8))
// res.mnemonic: the 24 words; show once and have the user back them up
_ = try await client.unlockWallet(walletPassword: Data(password.utf8))  // later launches

let info = try await client.getInfo()           // block height, operator connectivity, readiness
if info.walletReady { let b = try await client.balance(); _ = b.confirmedSat }
```

Sending, receiving, activity streaming, exiting to chain and error handling
are in the upstream API guide.

## Getting the code

There is no published Swift package or Maven artifact yet.
1. Clone wavelength-mobile.
2. Stage the bindings with `scripts/fetch-xcframework.sh` and
   `scripts/fetch-aar.sh`. These download a GitHub release with the `gh`
   CLI, which must be signed in.
3. Add `ios/WalletKit` as a local Swift package. Its `Package.swift` has a
   binary target at `Frameworks/Wavewalletdk.xcframework`, which isn't
   committed. On Android, add `android/walletkit` as a Gradle module.

## Fit with `apps/openagents-ios`

The iOS home is native Swift and Metal around a Coax library, built with
XcodeGen. Its README promises that a build needs no package download or
backend account. Adding Wavelength changes that.

- **Binary and download.** A large prebuilt Go binary would be fetched per
  checkout. To keep builds reproducible, we would:
  - pin the wavelength release tag;
  - record the xcframework's SHA-256 and check it before linking, as the web
    SDK does for its wasm;
  - vendor or cache it so that `build.sh` doesn't depend on `gh` sign-in.

  The README must say so.
- **Where it lives.** `WalletClient` belongs in the Swift host, the native
  adapter. Coax keeps scene, HUD and admission logic. Wallet results that
  Coax displays cross as plain scalars or bounded bytes, never as JSON parsed
  in Coax.
- **Lifecycle.** Start the daemon off the main thread at launch or on first
  use. Stop it on background if the OS requires that, and handle `start`
  throwing after a resume. Because most panics aren't recovered, the
  wallet's failure modes are app crashes. Treat that as a release blocker to
  measure, not an edge case.
- **Storage.** Use a sandbox `dataDir` with iOS data protection, excluded
  from iCloud backup, because it holds the encrypted seed. Back up the
  mnemonic deliberately, not by accident.
- **Liveness.** iOS doesn't keep apps running. VTXO refresh happens only
  while the app is open, so the app must warn users before deadlines (see
  [considerations.md](considerations.md)).
- **App Review.** A self-custodial wallet in the app falls under App Store
  rules for crypto wallets. Check them before shipping beyond TestFlight.
