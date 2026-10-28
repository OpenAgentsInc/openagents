# OpenAgents iOS

A minimal native home, ported from the engine's native iOS app into **Coax**.
It opens directly onto the monochrome animated graph, drifting dust and
chamfered HUD bar. Touch nodes to light them; **Start** replays the animation
and emits an actual `openagents.engine.v1.HudInvocation`. There is no next
screen in this reduced shell yet.

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
need neither protoc nor a Python protobuf installation. Xcode compiles the
product C for its target SDK. Metal compiles the bundled shader source at
runtime; the optional command-line Metal toolchain component is not needed.

The bundle identifier is `com.openagents.app`. This is a local development
shell, version 0.1.0 (1); no archive, upload or TestFlight release is included.
No login, chat, worlds, network services, microphone, FM or payments code is
restored. Their implementation can be added deliberately later.

## Ownership

| Source | Responsibility |
| --- | --- |
| `../../metareal/home.coax` | Graph generation, dust, pulses, animation, touch ownership, HUD layout, protobuf reader/writer |
| `../../metareal/light.coax` | Scalar shader functions: sky, dots, threads, glass bar, bloom, tone mapping |
| `../../metareal/platform/host.c` | Bounded resident storage, generation-checked handles, packed draw packets |
| `OpenAgents/App.swift`, `HomeViewController.swift` | UIKit lifecycle, safe areas, orientation, reduced motion, accessibility, native touch forwarding |
| `OpenAgents/Platform/` | CAMetalLayer/Metal resources, triple-buffer transfers, CoreText glyph raster, texture/stage bindings |
| `../../proto/openagents/metareal/home/v1alpha1/` | HomeConfiguration in the root protocol tree; imports the existing engine HudAction |
| `../../metareal/home.textproto` | Human-readable resource; `scripts/proto.sh generate` produces `Home.pb` |

This is a port of the native engine home from source commit
`69848ca` (the engine repository is identified in the existing Metareal
migration registry), not the retired OpenAgents web/mobile app. Only the
home setup/graph/HUD ideas and the OFL Michroma font are carried over. Scene
storage is flat and bounded because coax0 does not yet have aggregate types.
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
