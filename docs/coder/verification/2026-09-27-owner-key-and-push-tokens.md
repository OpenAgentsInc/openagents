# Owner key entry and native push tokens — September 27, 2026

This record covers two client items of
[issue #9723](https://github.com/OpenAgentsInc/openagents/issues/9723):
**Owner key entry on phones** and **Native push token plumbing**. It keeps
simulator and emulator evidence separate from physical-device and live-push
evidence, none of which ran.

## Delivered behavior

- **Secret input requests.** `rust_native::InputRequest<P>`
  ([`crates/rust-native/src/input.rs`](../../../crates/rust-native/src/input.rs))
  is a product-neutral request for one value a view can't collect. It carries
  the application's purpose type and a required `secret` flag. `validate`
  checks the token, label, text bounds, and a value bound of 1 byte to
  64 KiB. `accept` checks an answer's token and length, and its errors never
  contain the value. The [spec](../../../crates/rust-native/docs/spec.md#input-requests)
  states the adapter rule for a secret: a masked field whose value is never
  echoed, logged, persisted, autofilled, or suggested.
- **Computers.** `coder_computers::InputRequest` is now that type with the
  Computers purposes, so the serialized shape is unchanged. Answers go
  through `accept`. Phones offer **Enter owner key**; before, only the
  desktop and terminal clients did. The NIP-REACH owner-authority rule is
  unchanged: the live service accepts a key only when a held grant names it
  as owner, and it keeps the key in `live::Saved::owner`, the same encrypted
  store as the grants. A key that isn't the owner now gets its own refusal,
  which never repeats what was entered. The synthetic fixture accepts its own
  public test owner key (`0e` repeated 32 times) so UI tests can enter one.
- **iOS.** A secret request renders as a SwiftUI `SecureField` with
  autocorrection off and `privacySensitive()`. The value lives only in the
  view's binding and is cleared after submitting and when the field goes
  away. Push is opt-in: with `CODER_PUSH_RELAY_URL`,
  `CODER_PUSH_GATEWAY_URL`, and `CODER_PUSH_APP_PROFILE` in the Info.plist,
  the app asks for notification permission, calls
  `registerForRemoteNotifications`, and passes the APNs token to `push_token`
  as lowercase hex. A declined permission or failed registration shows as the
  wake status. The `aps-environment` entitlement is in
  `bins/coder-ios/host/Push/Coder-Push.entitlements` and applies only with
  `CODER_IOS_PUSH=development` or `production`.
- **Android.** A secret request renders as a single-line
  `TYPE_TEXT_VARIATION_PASSWORD` field with no suggestions, autofill, or saved
  state. Push compiles in only with `host/app/google-services.json`: the Google
  services plugin loads through a conditional `buildscript` classpath,
  Firebase Messaging is a conditional dependency, and `src/push` replaces the
  `src/nopush` stub. The messaging service in the manifest is enabled only in
  that build. With the push settings also set, the app requests
  `POST_NOTIFICATIONS` on Android 13 and later, fetches the FCM token at each
  launch, and passes it and every `onNewToken` rotation to `push_token`.

## Checks

| Check | Result |
| --- | --- |
| `cargo test -p rust-native --lib input` | 4 passed: the secret flag serializes and round-trips; a missing flag or unknown field is refused; token, label, text, and value bounds; stale and oversized answers, with no value in the error. |
| `cargo test -p coder-computers --lib` | 29 passed, including `owner_key_import_is_offered_on_every_platform_and_needs_a_computer` (a phone projects **Enter owner key**, its request is secret and validates) and `a_phone_enters_the_owner_key_the_fixture_grants_name` (a wrong key refused without an echo, stale token and oversized value refused, the fixture key accepted and the directory read). |
| `cargo test -p coder-computers --all-features --tests --examples` | Passed, including `tests/live.rs` against a real host on the synthetic relay. |
| `cargo test -p coder-mobile --lib` | 41 passed, 1 ignored. `a_phone_enters_the_owner_key_through_a_masked_request` checks that the packet the adapters decode has `"secret": true`, that the key is accepted, and that no later packet contains it. |
| `cargo clippy -p rust-native -p coder-computers --all-targets --all-features -- -D warnings`, `cargo fmt --check` | Passed. |
| iOS default simulator build (`scripts/build-coder-mobile.sh sim-build`) | Passed. The simulated entitlements hold only `application-identifier`; the three push keys in the Info.plist are empty. |
| iOS push simulator build (`CODER_IOS_PUSH=development` and loopback push settings) | Passed. The simulated entitlements hold `aps-environment` = `development`. |
| `OwnerKeyUITests` (dedicated iOS 26.5 iPhone 17 Pro simulator, Xcode 26.6) | Passed. The field is a secure text field, not a text field; its value isn't the typed key; a wrong key shows "isn't the owner key"; the fixture key shows "now holds your owner key" and "Your directory is empty"; no element's label or value contains either key. |
| `PushRegistrationUITests` with `TEST_RUNNER_CODER_PUSH_TEST=1` on the push build | Passed. The notification prompt was allowed, the simulator issued an APNs token, and the wake status changed from "Wakes off" to "Wakes unavailable: push setup could not connect: error sending request for url (http://127.0.0.1:9/)". That status comes from Rust after it received the token and failed to reach the absent loopback gateway. |
| Full iOS UI suite, default build | 18 tests: 15 passed, 2 skipped (the live test, which needs a fixture, and the push test), 1 failed. `VerseUITests.testComputerAndBackgroundResumeWithoutBreakingChats` fails at its second `openWorldComputer` after background resume. It fails the same way with `CoderApp.swift` reverted to its previous version, and none of the other changes run in a default build, so it's not caused by this work. |
| Android default build, `check` (lint and unit tests) | Passed. The default APK's manifest has the messaging service with `enabled=false` and no Firebase components. |
| Android instrumentation suite, default build (dedicated `coder_ownerkey_9723` AVD: Pixel 7, API 35, ARM64, emulator 36.6.11.0) | 18 tests, 0 failures, 2 skipped (the live test and the push test). `ownerKeyIsMaskedAndAcceptedOnlyWhenAGrantNamesIt` checks the password input variation, the password transformation, and no saved state, then the same wrong-key and accepted-key outcomes as iOS. |
| Android push build with a placeholder `google-services.json` and loopback push settings | Built. The manifest's messaging service is enabled. `pushConfiguredBuildReportsItsWakeStatus` passed: the placeholder project has no valid API key, so FCM refused the token request and the app showed "Couldn't register for wakes: Please set a valid API key…". The placeholder was deleted afterwards and was never committed. |

The existing Computers UI tests on both platforms still expected a disabled
SSH control on the phone. Phones stopped showing the SSH section in
`e98b2f8883`, and these tests hadn't run since. They now check that the
section is absent.

## Limits

- No physical device ran. The owner-key entry is simulator and emulator
  evidence only.
- No real APNs or FCM token reached a gateway. iOS proved the token handoff
  into Rust with a loopback gateway that wasn't running. Android proved only
  failure reporting, because it had no real Firebase project.
- A wake doesn't trigger a reconnect on Android. `onMessageReceived` does
  nothing, so the app reconnects at its next launch or foreground. On iOS,
  the system shows the alert and the app does nothing more.
- The phone adapters, like the terminal, rely on each adapter to mask a
  secret request. Nothing in `Capabilities` states that an adapter masks
  inputs; the desktop has no adapter for the Computers screens yet.
- A default Android build declares `POST_NOTIFICATIONS`, and it never
  requests it.

## Owner steps

1. **iOS push capability.** In the Apple Developer portal, turn on **Push
   Notifications** for `com.openagents.coder`, then edit and save the
   **OpenAgents Coder App Store** profile (and any development profile) and
   download it. Build with `CODER_IOS_PUSH=production` and the three push
   settings, as `bins/coder-ios/README.md` describes. Until then, leave
   `CODER_IOS_PUSH` unset so TestFlight signing is unchanged.
2. **Firebase app.** In the Firebase console, add the Android app
   `com.openagents.coder` and put `google-services.json` at
   `bins/coder-android/host/app/google-services.json` on the build machine.
   Don't commit it.
3. **Gateway and relay.** Deploy the push gateway with APNs and FCM
   credentials and matching app profiles, per
   [the runbook](../../deployment/push-gateway.md).
4. **Devices.** Install a push build on an iPhone and an Android phone, allow
   notifications, and confirm a wake arrives. Record it separately from this
   record.
