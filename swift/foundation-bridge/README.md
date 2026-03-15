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

This compiles the binary to `../../bin/foundation-bridge` and ad-hoc signs it so
macOS will launch the rebuilt sidecar locally.

## Usage

```bash
./bin/foundation-bridge
./bin/foundation-bridge 8080
```

## API Endpoints

- `GET /health`
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
**[crates/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md](../../crates/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md)**.

## Shipping the app (no build on user machines)

To ship the app so **users never need to build the bridge or install Xcode**:

1. **Build the bridge once** (on your machine or in CI): from the repo root run `cd swift/foundation-bridge && ./build.sh`. This produces `bin/foundation-bridge`.
2. **Include that binary in your app bundle** when you package the app:
   - **macOS .app**: put `foundation-bridge` next to your main executable, e.g. `YourApp.app/Contents/MacOS/foundation-bridge`, or in `YourApp.app/Contents/Resources/foundation-bridge`. The app looks in both places.
   - **Other layouts**: set `OPENAGENTS_APPLE_FM_BRIDGE_BIN` to the full path of the binary in your package.

Users then only need macOS 26+, Apple Silicon, and Apple Intelligence enabled—no Xcode or build step. If you don’t bundle the binary, the app will try to build it once (requires Swift on the user’s machine) or show an error that the app was not packaged with the bridge.
