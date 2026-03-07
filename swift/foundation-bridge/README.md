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

The bridge is intended to be supervised by the desktop app as a localhost sidecar for Apple Foundation Models inference only.
