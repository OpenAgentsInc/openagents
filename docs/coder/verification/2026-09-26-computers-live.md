# Computers screens on live hosts verification

Date: September 26, 2026. Issue:
[#9715](https://github.com/OpenAgentsInc/openagents/issues/9715), part of
[#9704](https://github.com/OpenAgentsInc/openagents/issues/9704). It follows
[#9712](https://github.com/OpenAgentsInc/openagents/issues/9712) (host serve)
and [#9713](https://github.com/OpenAgentsInc/openagents/issues/9713)
(Computers screens).

This record covers the live Computers service, host presence telemetry,
host-observed last-seen in NIP-HOST device listing, the invitation QR code,
and lifecycle signals from iOS and Android. Evidence is split into synthetic
Rust checks, simulator and emulator runs, and physical-device checks. No
physical-device or production-relay check ran.

## What landed

- `coder-computers`: `live::Live`, a `ComputersService` over
  `coder_host::client`, the portable `coder-access` device client,
  `coder-reach` presence, and one `coder-link` registry. A background task
  drains connector reports and ticks supervisor timers; data freshness is
  reported separately after each catch-up (presence, activity summaries,
  device list, enrollment requests). A signed `revoked` refusal or a channel
  the host closed as revoked blocks the supervisor and persists as revoked.
  `live::Store` keeps the saved grants; `live::FileStore` and
  `load_or_create_key` keep them owner-only for the terminal slice.
  `ComputersService::application` carries foreground and background, and
  `qr` renders invitations locally.
- `coder-mobile`: the normal app uses the live service instead of
  `Unavailable`. Its grants live in a separate cache directory encrypted
  under the device key from the platform's protected store, so erasing the
  chats pairing does not erase them. New `lifecycle` request, new
  `computers_qr` packet field, and a test-launch-only `loopback_test` config
  flag. The synthetic service remains for tests.
- `coder-host`: presence telemetry (CPU count, load per CPU, available
  memory; `--no-telemetry` withholds it), last-seen recording for direct
  channels, and a default `host` feature so clients build only `client`.
- `coder-access`: `DeviceEntry::last_seen`, recorded on every admitted
  request and through `Host::touch`. NIP-HOST documents the device-listing
  fields.
- `coder-connect`: a `qr` feature and `qr_modules_prefixed` for local QR
  rendering without the host store.
- iOS and Android glue: invitation QR drawing from the packet modules,
  polling while the Computers screens show, lifecycle signals from the scene
  phase and `onResume`/`onPause`, and the `--loopback-test` launch argument
  or `loopback_test` debug extra.
- Terminal slice: `cargo run -p coder-computers --example terminal -- --live
  DIR` runs the live service and draws the QR code as half-block text.

## Synthetic acceptance (one Mac, local relay)

`crates/coder-mobile/src/computers_live_tests.rs` drives the mobile
application state (`coder_mobile::App`, the same state the iOS and Android
hosts call through the C ABI and JNI) against the `coder host serve` command
path (`coder_host::cli::run`) on the synthetic NIP-42 relay fixture, with an
in-memory task owner. Each step asserts the screen text:

1. The operator initializes the host and serves it; `invite --rights all`
   equivalent creates an invitation.
2. The app pastes the invitation. The notice reads `Added Computer <key>.`
   The saved record is encrypted; it contains neither the host key nor the
   grant schema in clear.
3. First run continues; the row reads `Online on this computer. Up to date.`
4. Background and return: the supervisor probes and the row returns to
   online.
5. The Access screen lists this device as `Active, joined by invitation,
   last seen just now`; no row reads `last seen: unknown`.
6. Leaving out terminals and reviews, the app creates an invitation. Its
   detail reads `Grants: View sessions and tasks, Run and steer tasks.` The
   packet's QR modules equal the local renderer's. Another device redeems it
   and holds exactly `observe, operate`, issued by the app's key.
7. That device creates a task. The Activity screen shows `Task queued`; the
   title the other device sent does not appear.
8. A third device with an all-rights grant revokes the app. The row reads
   `Revoked: this computer removed this device's access. Add it again with a
   new invitation.` A new app instance over the same protected state starts
   revoked.

The run takes about 4 seconds. A diagnostic variant that waited 150 seconds
before revoking, with the Computers screen polled every 5 seconds, also
passed.

## Targeted checks

- `cargo test -p coder-computers` (23 unit tests, including the live
  service's route classes, error mapping, and owner-only file store; the QR
  renderer; and lifecycle on the fixture).
- `cargo test -p coder-mobile` (31 tests, including the acceptance run and
  the normal app's live-service validation; the device-run fixture is
  ignored by default).
- `cargo test -p coder-host` (8 unit tests including telemetry, the
  end-to-end scenario now asserting telemetry and placement, and 3 serve
  tests).
- `cargo test -p coder-access` (14 + 2 CLI; the listing test asserts
  host-observed last-seen and `touch` throttling).
- `cargo test -p coder-connect --lib qr`.
- Clippy with `-D warnings` for `coder-access`, `coder-connect`,
  `coder-host`, `coder-computers`, and `coder-mobile` (all targets), plus
  `coder-host` and `coder-computers` with `--no-default-features`.
- `cargo fmt --check` for the same crates.

The workspace gate did not run.

## Simulator and emulator runs

Both runs used the ignored fixture `serve_a_host_for_a_device_run`, which
serves a host on the Mac, writes the invitation, and plays the other
devices: after the app enrolls it creates a task from a narrowed device,
and after a delay it revokes the app from an administrator device.

To reproduce, start the fixture, then pass `invitation.txt` to the platform
test: `TEST_RUNNER_CODER_LIVE_INVITATION` for `xcodebuild test
-only-testing:CoderUITests/ComputersLiveUITests`, or the
`coderLiveInvitation` instrumentation argument for Android after
`adb reverse tcp:PORT tcp:PORT` for the ports in `relay-port.txt` and
`host-port.txt`:

```sh
CODER_COMPUTERS_FIXTURE_DIR=/private/tmp/computers-run \
  cargo test -p coder-mobile --lib serve_a_host_for_a_device_run -- --ignored --nocapture
```

The `loopback_test` flag admits only `ws://` loopback relays and exists for
test launches: the iOS `--loopback-test` launch argument, which only a
developer or test launch passes, as with `--synthetic`, and the Android
`loopback_test` extra, which only debug builds honor.

- **iOS Simulator** (iPhone 17 Pro, iOS 26.5, debug Rust library):
  `ComputersLiveUITests` launched with `--synthetic --loopback-test` (the
  synthetic world and reader, the live Computers service) and passed in
  about 101 seconds: pasted enrollment, online, last seen, a narrowed
  invitation with its QR code drawn natively, `Task queued` activity, and
  the revoked status with its reason. `ComputersUITests` over the fixture
  passed in the same run.
- **Android emulator** (API 35, arm64, `adb reverse` for the relay and host
  ports): `computersLiveEnrollInviteActivityAndRevocation` passed in about
  96 seconds with the same steps; the QR view was asserted present, and the
  Access screenshot shows the listing above it.
  `computersScreensShowStatusesAccessAndPastedInvitation` passed in the same
  run.

The first simulator run found a real defect: the iOS panel's 5-second
`Timer.publish` was recreated on every world redraw and never fired, so the
Computers screen stopped refreshing on its own. Polling now runs in a
`.task` tied to the view's identity.

Screenshots: [online](2026-09-26-computers-live/ios-simulator-online.png),
[last seen](2026-09-26-computers-live/ios-simulator-last-seen.png),
[invitation QR code](2026-09-26-computers-live/ios-simulator-invitation-qr.png),
[activity](2026-09-26-computers-live/ios-simulator-activity.png),
[revoked](2026-09-26-computers-live/ios-simulator-revoked.png),
[Android access](2026-09-26-computers-live/android-emulator-access.png), and
[Android revoked](2026-09-26-computers-live/android-emulator-revoked.png).

## Physical devices and production relays

None ran. Pending owner steps:

1. Run `coder host serve` on a computer with a production relay, create an
   invitation with `coder host invite`, and scan its QR code with the iPhone
   and Android apps.
2. Confirm the row reaches **Online** over the relay or a LAN hint (start
   the host with `--listen` on a LAN address and `--allow-nonloopback`),
   then background the app for more than five minutes and confirm it
   reconnects.
3. On the **Access** screen, create an invitation and scan its QR code from
   a second device.
4. Revoke a device from another device and confirm the revoked status.

## Limits

- The live service does not start SSH hosts, read the owner directory, or
  approve reverse enrollment in a tested flow; approval and denial are
  implemented but only unit-checked through the controller.
- A host's label is `Computer` and a key prefix.
- A relay-route client detects revocation on its next catch-up or operation
  (every 30 seconds by default, sooner while a screen polls), not
  instantly.
- CPU use is the load average per CPU, not measured utilization.
- The `coder` binary crate, a direct consumer of `coder-host`, was checked
  (`cargo check -p coder --bins --test host_serve --test host_cli`) but its
  tests did not run; its host scenario shares the updated `scenario.rs`.
