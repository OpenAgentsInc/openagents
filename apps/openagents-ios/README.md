# OpenAgents for iOS

The iOS app is one screen: the chat page. Today it is a title over the
composer. You can type in the composer; Send does nothing yet. The page is compiled Coax drawn
by the shared GPU component kit in one Metal view. The app compiles nothing
to JavaScript or WebAssembly, has no web view and no Updates path. The plan
is in [components/gpu-components.md](../../components/gpu-components.md#the-ios-app-grown-screen-by-screen):
the app grows one screen at a time from this page.

Bundle ID `com.openagents.app`, team `HQWSG26L43`, version 1.0.0.

## Files

| File | What it is |
| --- | --- |
| `chat.coax` | The chat page: state, layout, paint and semantic rows, with its check |
| `app.coax` | The root that `generate.sh` compiles to C: the chat page and the atlas tables |
| `host.c`, `host.h` | The page's GPU carrier, included by `components/gallery/linux/main.c` |
| `ComponentsHost.m` | Compiles the shared Apple GPU host into the app |
| `App.swift` | App delegate and chat screen: display link, platform editor, VoiceOver |
| `Bridge.h` | The C functions Swift sees |
| `project.yml` | The XcodeGen project: the `OpenAgents` app |
| `generate.sh` | Coax to C and Coax to Metal, in one remote job |
| `build.sh` | Generate, `xcodegen generate`, then a simulator build |
| `check.sh` | Fails if the built app holds any JavaScript path |
| `release.sh` | Archive and upload to App Store Connect; `tools/app_store.py` is its API client |
| `OpenAgents/` | Icons, `Info.plist` and entitlements |

Shared code the app builds: `apps/shared/native/KitEditor.swift`,
`apps/shared/host/apple/ui-host.m` and `components/`.

## Build and run

```sh
apps/openagents-ios/build.sh -destination 'platform=iOS Simulator,name=Coax Native Starter'
xcrun simctl install booted apps/openagents-ios/build/Build/Products/Debug-iphonesimulator/OpenAgents.app
xcrun simctl launch booted com.openagents.app
apps/openagents-ios/check.sh
```

## Check

```sh
coax/remote/remote.sh run -- coax/bin/coax test apps/openagents-ios/chat.coax
```

## Release

```sh
apps/openagents-ios/release.sh /absolute/fresh/output/directory
python3 apps/openagents-ios/tools/app_store.py status --build BUILD
```

The version and build number are `MARKETING_VERSION` and
`CURRENT_PROJECT_VERSION` in `project.yml`. The script reads App Store
Connect credentials from `~/work/.secrets/appstoreconnect.env`.
