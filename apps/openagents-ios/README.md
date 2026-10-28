# OpenAgents iOS

A minimal native home, ported from the engine's native iOS app into **Coax**.
It opens directly onto the monochrome animated graph, drifting dust and
chamfered HUD bar. Touch nodes to light them; **Start** emits an actual
`openagents.engine.v1.HudInvocation` and opens **the Grid**, the bare Verse
world: ground lines out to 264 m and a wire figure standing in for your
character. Drag on the left half to walk (push past the ring to run), drag on
the right half to look, and tap the right half to jump.

## Build and run

Requires macOS, Xcode with an iOS SDK/simulator, a C++20 host compiler, and
XcodeGen (`xcodegen`). No package download or backend account is required.

```sh
apps/openagents-ios/build.sh
xcrun simctl install booted apps/openagents-ios/build/Build/Products/Debug-iphonesimulator/OpenAgents.app
xcrun simctl launch booted com.openagents.app
```

Or open `OpenAgents.xcodeproj`, choose a simulator and Run. The checked-in
project has a build phase that builds coax0 when needed, generates the C11
library/header, and combines generated scalar Metal functions with native
stage bindings. Normal app builds use the checked-in protobuf resource and
need neither Python nor protoc. The native C++ importer in coax0 expands the
`ios-home` descriptor into generated Coax codecs, using the runtime embedded
in the compiler binary. No helper script/runtime files are loaded and no
protobuf or Python runtime is bundled with the app. Test/schema and App Store
administration scripts still use Python separately. Xcode compiles the
product C for its target SDK. If it rebuilds coax0, the bootstrap is explicitly
targeted at the host Mac with its macOS SDK, so inherited iOS deployment
settings cannot turn the build tool into an iOS executable. Metal compiles the bundled shader source at
runtime; the optional command-line Metal toolchain component is not needed.

The bundle identifier is `com.openagents.app`. This is a minimal home
shell. Distribution continues the existing OpenAgents App Store Connect record
at version 1.0.0; the current build is 1047.
The home shows the balance of the person's existing Spark wallet. It opens
the wallet from the Keychain seed the earlier app saved and only reads the
balance, through Breez's Spark SDK (`breez-sdk-spark-swift` 0.27.2, pinned in
`Package.resolved`). See [`docs/lightning/`](../../docs/lightning/README.md).
No login, chat, microphone or FM code is restored, the Grid has no buildings,
robots or other players yet, and no payments are sent or received yet.

## Ownership

| Source | Responsibility |
| --- | --- |
| `../../metareal/home.coax` | Graph generation, dust, pulses, animation, touch ownership, HUD layout, closed resource admission and generated proto codec calls |
| `../../metareal/grid.coax` | The Grid: projecting and fading its ground lines, the stand-in figure, the move/look/jump touch policy |
| `../../metareal/player.coax` | Walking, running, jumping and the third-person camera, ported from aethyr-cpp |
| `../../metareal/light.coax` | Scalar shader functions: sky, dots, threads, glass bar, bloom, tone mapping |
| `../../metareal/platform/host.c` | Bounded resident storage, generation-checked handles, packed draw packets |
| `OpenAgents/App.swift`, `HomeViewController.swift` | UIKit lifecycle, safe areas, orientation, reduced motion, accessibility, native touch forwarding; routes Start to the Grid and passes numbers between `grid.coax` and `player.coax` each frame |
| `OpenAgents/Platform/` | CAMetalLayer/Metal resources, triple-buffer transfers, CoreText glyph raster, texture/stage bindings |
| `../../proto/openagents/metareal/home/v1alpha1/` | HomeConfiguration in the root protocol tree; imports the existing engine HudAction |
| `../../metareal/home.textproto` | Human-readable resource; `scripts/proto.sh generate` produces `Home.pb` |

This is a port of the native engine home from source commit
`69848ca` (the engine repository is identified in the existing Metareal
migration registry), not the retired OpenAgents web/mobile app. Only the
home setup/graph/HUD ideas and the OFL Michroma font are carried over. Scene
storage remains flat and bounded from the initial port; the newer Coax
aggregate types have not replaced that resident graph storage path. Config
parsing and HudInvocation writing now use the generated `ios-home` profile.
Configuration views borrow a bounded local copy; the native bridge retains
only copied label bytes and scalar settings. No stack view escapes init.
The closed resource policy still rejects unknown/duplicate fields and
non-minimal varints; the general importer remains permissive about legal
non-minimal encodings unless its minimal-varint decoder is selected.
The default graph uses seed 42727, 4000 motes and eight satellites; exact native
floating-point pixels/RNG draw ordering are not a compatibility contract.

The Metal implementation uses one HDR scene target and two quarter-resolution
blur targets. It recreates targets on resize, holds in-flight resources until
GPU completion and skips frames when all three upload slots are busy. Reduce
Motion shows the settled graph without drift/rotation/pulses. Backgrounding
stops the display link and clears touch ownership; resuming cannot jump time.
Start uses the same Coax HUD activation policy for touch and VoiceOver.

## Checks

```sh
coax/build.sh test
export PROTOC=/path/to/protoc-29.3/bin/protoc
PROTO_PYTHON=/path/to/python-with-protobuf-5.29.3 scripts/proto.sh check
python3 metareal/test_home.py
metareal/test_sanitized.sh
xcodebuild -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -destination 'platform=iOS Simulator,name=OpenAgents iPhone' \
  -derivedDataPath apps/openagents-ios/build CODE_SIGNING_ALLOWED=NO \
  -parallel-testing-enabled NO test
```

Tests compile real Coax output and verify canonical protobuf interoperability,
unknown/duplicate/truncated resource rejection, bounded finite draw packets,
deterministic initialization, reduced motion, resume, stale handles, cancelled
presses and exact-once invocations. UI tests launch the real Metal app and tap
the rendered HUD through the native touch path in portrait and landscape.

## TestFlight

The rebuild retains `com.openagents.app`, team HQWSG26L43 and the existing app
icon. Increment `CURRENT_PROJECT_VERSION` above the record's highest build,
then regenerate the Xcode project. The release path targets only OpenAgents.
Credentials come from the owner's local `~/work/.secrets/appstoreconnect.env`
or ASC_API_* environment variables; private keys, tokens, profiles and output
archives stay outside the repository.

```sh
apps/openagents-ios/release.sh /absolute/fresh/output-directory
# Once Apple reports VALID, set notes and assign the existing internal group:
python3 apps/openagents-ios/tools/app_store.py status --build 1046
python3 apps/openagents-ios/tools/app_store.py testflight --build 1046 \
  --group Owner --notes-file apps/openagents-ios/release-notes/1046.txt
```

The API commands require ASC_API_* variables in the current environment. The
release script uses manual distribution signing, preserves the checked-in
version/build during export and uploads the exported IPA. It returns after
upload; status and group assignment are explicit steps. It never creates
testers or external groups and never submits a public App Store release.
