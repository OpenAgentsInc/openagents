# Electron AI SDK harness test

## Owner direction

On 2026-07-17, the owner asked for a minimal Electron/Vite application under
`apps/` that can interact with the experimental AI SDK Codex harness through a
chat UI, then asked for it to be launched in development mode and merged into
`main`.

## Scope and boundaries

- This is an owner-local proof of concept, not a production agent surface.
- The harness runs only in Electron's main process; the renderer gets a narrow
  local HTTP endpoint through a context-isolated preload bridge.
- The local sandbox adapter is a developer fixture. It is not a containment or
  public multi-tenant security boundary.
- The app invokes the user's installed `codex` executable, never the obsolete
  adapter-bundled executable.

## Acceptance checks

- `pnpm --dir apps/electron-ai-sdk-test typecheck`
- `pnpm run check`
- Launch with `pnpm --dir apps/electron-ai-sdk-test dev`
- Send a small prompt to confirm an AI SDK `useChat` stream reaches the local
  Codex harness.
