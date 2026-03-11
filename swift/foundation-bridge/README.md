# Foundation Models HTTP Bridge

HTTP server that exposes Apple's Foundation Models via an OpenAI-compatible API.

## Requirements

- macOS 26+
- Apple Silicon Mac
- Apple Intelligence enabled in System Settings
- Xcode 26+

## Building

```bash
./build.sh
```

This compiles the binary to `../../bin/foundation-bridge`.

## Usage

```bash
./bin/foundation-bridge
./bin/foundation-bridge 8080
```

## API Endpoints

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`

The bridge is intended to be supervised by the desktop app as a localhost sidecar for Apple Foundation Models inference only. The desktop app discovers the binary automatically (from repo root, paths relative to the executable, or `OPENAGENTS_APPLE_FM_BRIDGE_BIN`) and will run `./build.sh` once if the binary is missing (requires Xcode/Swift). No manual steps required beyond enabling Apple Intelligence on macOS 26+ Apple Silicon.
