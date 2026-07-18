# Electron AI SDK test

A deliberately small Electron/Vite proof of concept for the experimental AI SDK
Codex harness. The renderer uses `useChat` and `DefaultChatTransport`; Electron's
main process owns the harness, session state, and a loopback-only streaming route.

## Run

```sh
pnpm --dir apps/electron-ai-sdk-test dev
```

The first chat turn uses the locally installed and signed-in `codex` CLI. Override
its path with `CODEX_BIN=/absolute/path/to/codex` if needed.

## Safety boundary

This is an owner-local development fixture. The local sandbox adapter provides a
workspace-shaped environment for the harness but is not a production or
multi-tenant containment boundary. The Electron renderer has no Node integration
and receives only the loopback chat endpoint through the preload bridge.
