# OpenAgents 1.0 on TestFlight (external testers)

> Historical: this app was removed in #11901. Use the [current mobile app](../../apps/openagents/OPENAGENTS.md) and its [iOS release guide](../../apps/openagents/apps/ios/release/README.md).

Issue: [#11093](https://github.com/OpenAgentsInc/openagents/issues/11093).
App: `com.openagents.app`, App Store Connect app 6748620735, version 1.0.0.
External group: **OpenAgents Beta Testers**, public link
<https://testflight.apple.com/join/dvQdns5B>.

The build is the streamlined app (Coder, Wallet, Settings, and the plain
Grid in the Verse) from the [mobile 1.0 audit](1.0-audit.md), built without
`OPENAGENTS_MOBILE_PREVIEW`, so the Gym, Everglade, Trainer, Playtest, and
Tailnet are hidden and playtest logging is off.

## Sign-in

Optional. The app makes its own device key on first launch, and chat, the
wallet, the Verse, and Settings all work without an account. Settings has
**Log in** for openagents.com accounts (by invitation only for now), which
adds the account's chats, Projects, Running, Memory, and Scheduled prompts.
Apple's reviewer needs no demo account, and **Sign-in required** is **No**.

## For testers (share this)

> Join the OpenAgents beta on your iPhone:
>
> 1. Install TestFlight from the App Store:
>    <https://apps.apple.com/app/testflight/id899247664>
> 2. Open <https://testflight.apple.com/join/dvQdns5B> on the iPhone and tap
>    **Accept**, then **Install**.
> 3. Open OpenAgents. There is no sign-in: start typing in Chat.
>
> To reach your own computer from the phone, open **Account > Computers >
> Connect a computer** and follow the steps there (it starts with
> `curl -fsSL https://openagents.com/cli/install.sh | bash` on the computer).
>
> Something wrong? **Account > Report a problem**, or take a screenshot and
> send it as TestFlight feedback.

## What to Test (set on each build)

The build's What to Test text is `whatToTest` in
[`bins/openagents-ios/testflight.json`](https://github.com/OpenAgentsInc/openagents/blob/e6783578b3900f0d60dd0d5f9033ab26e9371c74/bins/openagents-ios/testflight.json)
(build 59's, below):

> Welcome to the OpenAgents beta. No sign-in is needed.
>
> New in this build:
> - The app opens where you left off: the chat you were in, or the first time, a welcome with Start a chat. A new chat is a plain field, with no cards.
> - Ask anything in Ask OpenAgents. The answer streams in, and the stop button stops it.
> - With an openagents.com account (Settings > Log in; accounts are by invitation for now): your chats list newest first with their project, Projects makes a project from just a name and starts a chat in it, and Running shows what Coder and Solver in the cloud are doing, with Stop.
>
> Also try:
> 1. Wallet: your balance, Receive, and Send. It is a real Bitcoin wallet, so keep only small amounts in it during the beta.
> 2. Verse: tap Verse at the top and walk around the Grid with your avatar; tap Coder to come back.
> 3. Settings: open each row. To reach your own computer, open Computers > Connect a computer and follow the steps there.
>
> Something wrong? Settings > Report a problem, or take a screenshot and send it as TestFlight feedback.

## Beta App Review information

Set from the same file (`review`):

- Contact: Christopher David, chris@openagents.com. The phone number already
  in App Store Connect is kept (the file leaves it out).
- Sign-in required: **No** (no demo account).
- Review notes:

> No sign-in, account, or demo login is needed. On first launch the app makes its own device key, and everything below works without an account. Settings has an optional Log in for openagents.com accounts, which are by invitation only for now; reviewing the app does not need it.
>
> 1. Coder (opens first): tap Start a chat, then type a question such as "What is OpenAgents?". The answer comes from our hosted assistant and streams in; the stop button stops it.
> 2. Verse: the switch at the top opens the Grid, a shared 3D world where you walk around as an avatar with the on-screen sticks; Coder comes back.
> 3. The menu button at the top left opens Computers, Wallet, Settings, and recent chats. Wallet is a self-custody Bitcoin wallet created on the device; reviewing it needs no funds, and Receive shows a request to be paid. Settings has Report a problem, Computers, Appearance, Your keys, Identity keys, About this device, Changelog, Source code, and Follow us on X.
>
> Computers is optional: it pairs the phone with a computer running our command-line app (install: curl -fsSL https://openagents.com/cli/install.sh | bash; then openagents connect invite shows a code to scan). Nothing else in the app needs a computer.
>
> Contact: chris@openagents.com

Test information for the app (also from the file): the beta description,
feedback email chris@openagents.com, marketing URL https://openagents.com,
and privacy policy https://openagents.com/privacy.

## Shipping a build to external testers

1. Gate: on a fresh simulator, run the live release gate, which asks the
   starting questions signed out, opens Wallet, and opens every Account row,
   saving a screenshot and the screen's text for each step to check by eye:

   ```sh
   bins/openagents-ios/build.sh sim   # builds the Rust library and the project
   xcrun simctl uninstall booted com.openagents.app
   TEST_RUNNER_OPENAGENTS_UITEST_LIVE=1 TEST_RUNNER_OPENAGENTS_UITEST_SHOTS=/tmp/gate \
     xcodebuild test -project bins/openagents-ios/host/OpenAgents.xcodeproj \
     -scheme OpenAgents -configuration Release -destination id=UDID \
     -only-testing:OpenAgentsUITests/ReleaseGateUITests \
     OPENAGENTS_RUST_LIBRARY_DIR=$CARGO_TARGET_DIR/aarch64-apple-ios-sim/debug CODE_SIGN_IDENTITY=-
   ```

2. Upload: raise the build number and add its Changelog entry
   (`AGENTS.md`), then `scripts/release/testflight.sh start` and
   `scripts/release/testflight.sh wait` until it ends.
3. Test information, once per change to the file:
   `python3 scripts/release/asc.py test-info`.
4. External testers and review:
   `python3 scripts/release/asc.py external --build N --after <upload start, ISO> --submit`
   sets the build's What to Test, adds it to **OpenAgents Beta Testers**, and
   sends it to Beta App Review. `python3 scripts/release/asc.py review-status
   --build N` shows where it is. Testers get it once Apple approves it.

`asc.py` reads the App Store Connect key from `ASC_API_KEY_ID`,
`ASC_API_ISSUER_ID`, and `ASC_API_PRIVATE_KEY_PATH` (the owner's
`~/work/.secrets/appstoreconnect.env`) and never prints it.
