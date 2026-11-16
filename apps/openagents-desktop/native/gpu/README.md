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
`--inspect-controls` mode. That mode touches no account credentials,
Keychain item, visible window, clipboard, browser or resident agent process
and keeps view state in an isolated in-memory store. Normal launch is an
opt-in compiled GPU client with the existing desktop token/device stores,
the shared account/identity services, public view persistence and the local
resident-service adapter. The shipping `native/build.sh` remains separate
until the replacement gates below pass.

The real SDK check opens GPU Settings/login by committed row ID, edits through
AppKit's text system, holds provisional input until commit, replaces the
virtual field with the active editor in accessibility, then blurs and retires
it. The identity check runs the iOS test source on the standalone Mac XCTest
runner with controlled HTTP and private storage, plus the actual private-file
adapter. Ten tests cover the existing identity cases, restart/exact replay
from a protected file, atomic public-view storage, permissions and symlink
refusal. Production view records use the registered Coax codec in
`coax/native/account-view-wire.coax`; no credentials or pending commands enter
that record.

`AppMacDevice` uses the installed resident environment CLI for public-key
discovery and protected local RPC, and delegates remote calls to the existing
paired-device adapter. Discovery is asynchronous through the same effect
outbox. SDK tests cover process limits/deadlines, start failure, protected
request files, exact request carriage, unknown outcomes and no local fallback
for remote calls. `--inspect-resident` is a separate explicit, hidden check:
it discovers the installed service and makes only DescribeEnvironment and
ListSessions requests, decoded by the existing Coax codec. On 2026-11-16 it
authenticated to the actual Mac service and read 16 session summaries; no
turn was started and no transcript or private bytes were logged. `--inspect-resident-turn` is a separate explicit opt-in check that sends one
marker-only Claude turn through the shared core, closes the hidden GUI while
the actual service reports busy, then opens a new GUI/core and rejoins the
retained session. That real installed-service check passed on 2026-11-16;
it requested no tool calls and logs no transcript or account credentials.

`AppCoreC` and `AppRuntime` are shared with iOS. `KitEditorMac` owns active
AppKit composition and selection; `KitPasskey` uses AuthenticationServices'
platform anchor. Coax owns state, admission, codecs, scheduling and paint.

The GPU drawer now reads all paired computers independently of the active
conversation using the shared `AppNavigation` cache, with one outstanding
read, per-computer pagination and visible-only scheduling. Account chats
share the same virtualized, fixed-width window. An unanswered write blocks
navigation rather than losing its sole owner. Row IDs include the full
host/session identity and remain stable through catalog reorder.

Remaining before this becomes the default desktop application: existing preference/reference migration,
production signing/passkey ceremonies, real VoiceOver. Running-turn GUI detach/rejoin now passes its explicit live check. Linux
still needs its shared-core service adapter and AT-SPI bridge. Do not delete
the existing desktop clients on the strength of this qualification target.

## Compiled Linux qualification

`linux-build.coax` builds the actual shared page/core on the same atlas carrier
and Vulkan shader source. It uses the common Coax native SDK linker; the
older generic gallery is not its entry. The executable is explicitly
`OpenAgentsGPUCheck`, not a replacement desktop launcher.

```sh
OA_REMOTE=1 coax/remote/remote.sh run -- nix-shell -p gcc pkg-config at-spi2-core \
  vulkan-headers vulkan-loader wayland wayland-scanner wayland-protocols libxkbcommon \
  --run 'coax/web/tool.sh apps/openagents-desktop/native/gpu/linux-build.coax'
```

`--sanitized` builds `OpenAgentsGPUCheckASan`. Both rendered the real page on
Lavapipe and passed semantic Settings/login activation, a committed field
value, actual animation settling and blur; PNGs match exactly. See the
[evidence and picture](../../../../docs/verification/gpu-native-13645/README.md).
The canvas-host editor now also passes actual Vulkan calls for committed
typing, provisional IME isolation, undo, Tab/Shift-Tab and stale-field refusal,
normally and with sanitizers. It uses the component library's existing
composer body and edit buffer; no second parser or wrapper is introduced.
The production Wayland protocol listeners pass buffered commit/preedit and
owner/cursor refusal checks normally and instrumented. Third-party compositor
IME, active text/selection through AT-SPI, actual account/device services and
an Orca ceremony are still required before normal launch replaces GTK.
