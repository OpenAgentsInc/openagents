# Khala Mobile - build 22 iOS TestFlight receipt (2026-07-08)

Fresh TestFlight build cut from `main` at `99098e4b15`
(`khala-mobile: bump iOS TestFlight build 22`), after the CX-2..CX-8 mobile
Codex cutover slices landed on main.

## Upload

- App: `Khala Code` (`com.openagents.khala.mobile`), version `0.1.0`, iOS
  build `22`, Team `HQWSG26L43`.
- Source change: `clients/khala-mobile/app.json` `buildNumber` `21` -> `22`.
- Archive: `xcodebuild -workspace clients/khala-mobile/ios/KhalaCode.xcworkspace
  -scheme KhalaCode -configuration Release -destination 'generic/platform=iOS'
  -archivePath /tmp/KhalaCode-build22.xcarchive archive`.
- Export: manual App Store signing with the installed
  `com.openagents.khala.mobile AppStore` provisioning profile.
- IPA: `/tmp/KhalaCode-build22-export/KhalaCode.ipa` (31 MB).
- Upload: `UPLOAD SUCCEEDED with no errors`.
- Delivery UUID: `f30897f8-ee48-448f-b7bd-f37aa51dc626`.
- App Store Connect API check: latest TestFlight upload for `0.1.0` is build
  `22`.

## Verification

- `bun install --frozen-lockfile`.
- Pre-push Khala mobile release gate: `PASS` (`440 pass`, `0 fail`).
- `expo prebuild --platform ios`: succeeded.
- Xcode archive: `** ARCHIVE SUCCEEDED **`.
- Xcode export: `** EXPORT SUCCEEDED **`.
- `.env.local` was absent before and after export; no local session or seeded
  credential was baked into the shippable artifact.

## What to test in TestFlight build 22

Use a real phone and your own account; do not import or touch a live desktop
`~/.codex`. The intended path is device-auth into the mobile custody rail only.

1. Fresh launch and GitHub sign-in: install build 22, launch, complete normal
   GitHub sign-in, and confirm the app opens the signed-in Khala Code surface
   without Tailnet/local-desktop fallback.
2. Codex account connect: open the account/model-preference surface, start the
   Codex connect flow, complete provider device auth on the phone, and confirm
   the account row shows readiness/quota state without exposing secrets.
3. Account-targeted task: select the connected Codex account target and start a
   real coding task on one of our repos. Confirm the thread shows queued/running
   status and does not silently fall back to a hosted model.
4. Steering controls: while a turn is active, test interrupt, append/steer,
   resume, and retry. The controls should keep the same turn lane/account.
5. Multi-account/concurrency truth: if you have more than one account connected,
   start work that exercises one-account serialization and separate-account
   concurrency. Busy/quota states should be explicit.
6. Repo/writeback loop: pick a repo, let the turn produce a branch/writeback/PR
   path if available, and verify the phone deep link or receipt points to the
   resulting branch/PR.
7. Claude parity smoke: if a Claude account is available, connect/select it and
   verify it follows the same readiness and explicit fallback rules.
8. Dogfood receipt: record at least five real mobile-Codex tasks, any billing
   sanity observations, and every friction item that would have forced a
   desktop fallback.

