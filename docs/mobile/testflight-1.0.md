# OpenAgents 1.0 on TestFlight (external testers)

Issue: [#11093](https://github.com/OpenAgentsInc/openagents/issues/11093).
App: `com.openagents.app`, App Store Connect app 6748620735, version 1.0.0.
External group: **OpenAgents Beta Testers**, public link
<https://testflight.apple.com/join/dvQdns5B>.

The build is the streamlined three-tab app (Chat, Wallet, Account) from the
[mobile 1.0 audit](1.0-audit.md), built without `OPENAGENTS_MOBILE_PREVIEW`,
so the Verse, the Gym, Trainer, Playtest, and Tailnet are hidden and playtest
logging is off.

## Sign-in

None. The app has no login: it makes its own device key on first launch, and
chat, the wallet, and Account all work without an account. Apple's reviewer
needs no demo account, and **Sign-in required** is **No**.

## For testers (share this)

> Join the OpenAgents beta on your iPhone:
>
> 1. Install TestFlight from the App Store:
>    <https://apps.apple.com/app/testflight/id899247664>
> 2. Open <https://testflight.apple.com/join/dvQdns5B> on the iPhone and tap
>    **Accept**, then **Install**.
> 3. Open OpenAgents. There is no sign-in: start typing in Chat.
>
> To reach your own computer from the phone, get OpenAgents at
> <https://openagents.com/download>, run `openagents connect invite` on the
> computer, and scan the code from **Account > Computers**.
>
> Something wrong? **Account > Report a problem**, or take a screenshot and
> send it as TestFlight feedback.

## What to Test (set on each build)

The build's What to Test text is `whatToTest` in
[`bins/openagents-ios/testflight.json`](../../bins/openagents-ios/testflight.json):

> Welcome to the OpenAgents beta. No sign-in is needed.
>
> 1. Chat: ask anything, or tap a suggested question. The answer streams in.
> 2. Wallet: your balance, Receive, and Send. It is a real Bitcoin wallet, so
>    keep only small amounts in it during the beta.
> 3. Account: open each row. To reach your own computer, get OpenAgents at
>    https://openagents.com/download, run openagents connect invite on it, and
>    scan the code from Account > Computers.
>
> Something wrong? Account > Report a problem, or take a screenshot and send
> it as TestFlight feedback.

## Beta App Review information

Set from the same file (`review`):

- Contact: Christopher David, chris@openagents.com. The phone number already
  in App Store Connect is kept (the file leaves it out).
- Sign-in required: **No** (no demo account).
- Review notes:

> No sign-in, account, or demo login is needed: the app has no login. On
> first launch it makes its own device key.
>
> 1. Chat tab (opens first): type a question such as "What is OpenAgents?" or
>    tap a suggested question. The answer comes from our hosted assistant and
>    streams in.
> 2. Wallet tab: a self-custody Bitcoin wallet created on the device.
>    Reviewing it needs no funds; Receive shows a request to be paid.
> 3. Account tab: Report a problem, Computers, Appearance, Your keys, Identity
>    keys, About this device, Changelog, Source code, and Follow us on X.
>
> Computers is optional: it pairs the phone with a computer running
> OpenAgents (https://openagents.com/download, then openagents connect invite
> shows a code to scan). Nothing else in the app needs a computer.

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
