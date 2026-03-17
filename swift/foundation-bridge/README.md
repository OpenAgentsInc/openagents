# Foundation Models HTTP Bridge

HTTP server that exposes Apple's Foundation Models via an OpenAI-compatible API.

## Requirements

- macOS 26+ on Apple Silicon
- **Apple Intelligence enabled**: System Settings → Apple Intelligence (in the sidebar) → turn on Apple Intelligence
- **Swift compiler** (to build the bridge; the bridge is written in Swift): install Xcode from the App Store, or run `xcode-select --install` to install only the Command Line Tools (Swift without the full Xcode app)

## Building

```bash
./build.sh
```

This compiles the binary to `../../bin/foundation-bridge` and signs it for
local launch. When a real signing identity is available in the keychain,
`build.sh` prefers `Apple Development`, then `Developer ID Application`, then
`Apple Distribution`; if none are available it falls back to ad-hoc signing.
It also writes a signed `../../bin/FoundationBridge.app` bundle with an
`Info.plist` that includes Xcode and SDK metadata, which is the preferred
launch surface for the desktop app on Apple Intelligence systems.

## Usage

```bash
open -n -g ./bin/FoundationBridge.app --args 11435
```

For the desktop app and other bundled launches on macOS, prefer
`FoundationBridge.app`. Launching the inner Mach-O directly is still useful for
low-level debugging, but it is not the normal supported path because it can
trigger Apple Intelligence "app needs an update" warnings when Launch Services
never sees the bundle metadata.

## API Endpoints

- `GET /health`
- `POST /control/shutdown`
- `GET /v1/models`
- `GET /v1/adapters`
- `POST /v1/adapters/load`
- `DELETE /v1/adapters/{adapter_id}`
- `POST /v1/sessions`
- `GET /v1/sessions/{id}`
- `GET /v1/sessions/{id}/transcript`
- `POST /v1/sessions/{id}/adapter`
- `DELETE /v1/sessions/{id}/adapter`
- `POST /v1/sessions/{id}/responses`
- `POST /v1/sessions/{id}/responses/structured`
- `POST /v1/sessions/{id}/responses/stream`
- `POST /v1/sessions/{id}/reset`
- `DELETE /v1/sessions/{id}`
- `POST /v1/chat/completions`

The bridge is intended to be supervised by the desktop app as a localhost
sidecar for Apple Foundation Models inference plus adapter load/attach testing.
For full detail (architecture, discovery, shipping, user requirements, agent
workflow), see
**[OpenAgentsInc/psionic docs/FM_BRIDGE_CONSIDERATIONS.md](https://github.com/OpenAgentsInc/psionic/blob/main/docs/FM_BRIDGE_CONSIDERATIONS.md)**.

## Shipping the app (no build on user machines)

To ship the app so **users never need to build the bridge or install Xcode**:

1. **Build the bridge once** (on your machine or in CI): from the repo root run `cd swift/foundation-bridge && ./build.sh`. This produces both `bin/foundation-bridge` and `bin/FoundationBridge.app`.
2. **Include that binary in your app bundle** when you package the app:
   - **macOS .app**: bundle `FoundationBridge.app` under `YourApp.app/Contents/Helpers/` or `YourApp.app/Contents/Resources/`. The desktop app launches that helper bundle through Launch Services.
   - **Other layouts**: set `OPENAGENTS_APPLE_FM_BRIDGE_BIN` to the full path of the helper-bundle executable in your package. Raw-binary overrides are for developer debugging only.

Users then only need macOS 26+, Apple Silicon, and Apple Intelligence enabled—no Xcode or build step. If you don’t bundle the binary, the app will try to build it once (requires Swift on the user’s machine) or show an error that the app was not packaged with the bridge.
