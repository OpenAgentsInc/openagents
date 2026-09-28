# OpenAgents for iOS

OpenAgents is a new iPhone app that reuses Coder's Rust Native approach under
its own identity. Rust builds each screen as a
[Rust Native](../../crates/rust-native/) view in the `openagents-mobile`
crate, and a thin SwiftUI host decodes and renders it. The host shares
Coder's renderer, [`NativeView.swift`](../coder-ios/host/App/NativeView.swift),
instead of copying it.

The home screen lists the devices on your tailnet, with each device's
name, operating system, tailnet address, and online state.

## Tailnet devices

iOS does not let one app read another app's Tailscale state, and a tailnet
has no discovery broadcast, so the phone's own Tailscale connection cannot
list its peers. Instead, the app uses Tailscale's Rust control client,
[tailscale-rs](https://github.com/tailscale/tailscale-rs) (`ts_control`
0.6.1), to register as its own tailnet node named `openagents-ios`, then
reads one netmap from Tailscale's control server. It never joins the data
plane or carries traffic.

1. On first launch, the control server returns a sign-in URL. The app shows
   **Connect to a tailnet** and **Sign in with Tailscale**.
2. The button opens the URL in Safari. After you approve the device, the app
   reads the netmap and shows the device list.
3. The node keys stay in the app's Application Support directory, so later
   launches skip the sign-in until the node key expires.

If the tailnet has no other devices, the app shows **Connect to a tailnet**
with a **Refresh** button. tailscale-rs is pre-1.0 and unaudited; the app
uses only its control-plane client.

## App identity

| Setting | Value |
| --- | --- |
| Project, scheme, target, product | `OpenAgents` |
| Bundle identifier | `com.openagents.app` |
| App Store Connect app | `6748620735` (**OpenAgents**) |
| Development team | `HQWSG26L43` |
| Marketing version and build | `1.0.0` / `2` |
| Minimum OS and device family | iOS 17 / iPhone |
| Archive signing | Manual, Apple Distribution, `OpenAgents App Store` profile |

The App Store Connect record also holds `0.x` builds from an earlier app on
this bundle identifier. Build numbers only need to be unique within one
version, so `1.0.0` started at build `1`. Build `1` (hello world) is on TestFlight. Raise the build number for every
upload; set it in `host/project.yml` or with `OPENAGENTS_IOS_BUILD_NUMBER`.

The `OpenAgents App Store` profile uses the same Apple Distribution
certificate as Coder's profile. The older `com.openagents.app AppStore`
profile names a different certificate, and Xcode rejects it when both
certificates are in the keychain.

The icon is the black-and-white power symbol from the earlier Khala app
(commit `8a54389bd1`).

## Build

Prerequisites: the pinned Rust toolchain with the `aarch64-apple-ios` and
`aarch64-apple-ios-sim` targets, Xcode with the iOS SDK, and `xcodegen`.

```sh
# Build, install, and launch on a simulator (default: the booted one).
OPENAGENTS_IOS_DEVICE=<simulator-udid> bins/openagents-ios/build.sh sim

# Signed App Store archive. Does not upload.
bins/openagents-ios/build.sh archive

# Upload the archive to TestFlight with an App Store Connect API key.
ASC_API_KEY_ID=... ASC_API_ISSUER_ID=... ASC_API_PRIVATE_KEY_PATH=... \
  bins/openagents-ios/build.sh upload
```

Build products go to `$CARGO_TARGET_DIR/openagents-ios`. The archive command
records the source commit and workspace status beside the archive.
