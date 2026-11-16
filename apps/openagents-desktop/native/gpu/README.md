# Compiled desktop SDK qualification

This target links the phone's actual compiled page and shared app core into
an AppKit/Metal carrier. It qualifies the G6 host work for #13645. It does
not replace the shipping native desktop client yet: that still owns the
protected local-service adapter and migration surface.

```sh
apps/openagents-ios/generate.sh
coax/remote/remote.sh run --local -- coax/web/tool.sh apps/openagents-desktop/native/gpu/build.coax
coax/remote/remote.sh run --local -- coax/web/tool.sh apps/shared/native/identity-check.coax
```

The build tool creates `build/GPU/OpenAgents.app` and runs only its hidden
`--inspect-controls` mode. Other launch modes refuse. No account credentials,
Keychain item, visible window, clipboard, browser or resident agent process
is touched. View state stays in an isolated in-memory store.

The real SDK check opens GPU Settings/login by committed row ID, edits through
AppKit's text system, holds provisional input until commit, replaces the
virtual field with the active editor in accessibility, then blurs and retires
it. The identity check runs the iOS test source on the standalone Mac XCTest
runner with controlled HTTP and private storage, plus the actual private-file
adapter. Seven tests cover the existing identity cases, restart/exact replay
from a protected file, atomic public-view storage, permissions and symlink
refusal. Production view records use the registered Coax codec in
`coax/native/account-view-wire.coax`; no credentials or pending commands enter
that record.

`AppCoreC` and `AppRuntime` are shared with iOS. `KitEditorMac` owns active
AppKit composition and selection; `KitPasskey` uses AuthenticationServices'
platform anchor. Coax owns state, admission, codecs, scheduling and paint.

Remaining before this becomes the desktop application: protected local RPC
and resident-service discovery, existing preference/reference migration,
production signing/passkey ceremonies, real VoiceOver and closing the GUI during a running turn. Linux
still needs its shared-core service adapter and AT-SPI bridge. Do not delete
the existing desktop clients on the strength of this qualification target.
