# OpenAgents mobile TestFlight build 124 release evidence

Date: 2026-07-22. This record captures the build-and-upload evidence for the
OpenAgents mobile iOS build 124.

## Why this build

The owner needed a current mobile build so that the owner can talk to Sarah in
the app. Sarah is now live: her hosted runtime runs, her memory is on, and her
autonomous tick acts. Build 124 comes from the current `main`, so it carries the
current Sarah conversation surface.

## Identity

- App: `apps/openagents-mobile`, name `OpenAgents`.
- iOS bundle identifier: `com.openagents.app`.
- Apple team: `HQWSG26L43`.
- App Store Connect app id: `6748620735`.
- Marketing version: `0.5.2` (unchanged).
- Build number: `124` (was `123`).

## Build evidence

- Source commit on `main`: `2b8580139b` (release identity commit for build 124).
- Native project: regenerated with `expo prebuild --clean --platform ios`.
  CocoaPods installed. Derived `Info.plist` reports `com.openagents.app`,
  `0.5.2`, and `124`.
- Archive: `xcodebuild ... archive` returned exit 0. The archive signed with
  Apple Development for the archive step, per the runbook.
- Export: `xcodebuild -exportArchive` with the App Store Connect method and the
  `com.openagents.app AppStore` profile. Export succeeded.
- IPA SHA-256: `7703adf0e05d91800f0b7b0d6bee3f2daa403df928b7736bd9397aad6386638d`.
- Upload: `xcrun altool --upload-app` returned `UPLOAD SUCCEEDED with no errors`.
- Delivery UUID: `6bb615eb-b864-4c9b-af7c-5e6c4af19c69`.

## Status

Uploaded to App Store Connect. The build then processes on Apple's side before
TestFlight shows Install. Do not call the release valid until App Store Connect
reports the build `VALID`.

## In-app path to Sarah

Sign in with the admitted owner account. The app bootstraps the stable
owner-private Sarah thread through `/api/mobile/sarah` and opens her
conversation. Send a message in the composer, or use the microphone. The hosted
runtime runs her turn and she replies. A long press on her reply plays her
voice.
