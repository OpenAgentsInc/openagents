# AI SDK Harness POC

This is a Vite+-bootstrapped Node 24 proof of concept for AI SDK 7 harnesses.
It provides a tiny HTTP server around the experimental Codex harness adapter.

It deliberately uses the authenticated local Codex home at `~/.codex` and
places harness workspaces in `.harness-workspaces/`. This is owner-local test
code, not a production sandbox.

The bridge uses your installed `codex` command rather than the adapter's
bundled Codex executable. It defaults to `gpt-5.6-sol`. Set `CODEX_BIN` or
`CODEX_MODEL` to override either.

```sh
pnpm --dir apps/ai-sdk-harness-poc dev

curl http://127.0.0.1:8787/health

curl -X POST http://127.0.0.1:8787/api/run \
  -H 'content-type: application/json' \
  -d '{"sessionId":"poc-1","prompt":"Create a short README for this workspace."}'
```

Reuse `sessionId` for subsequent turns. Omit it to start a new Codex session.
Use `PORT` to change the listener and `HARNESS_WORKSPACE_ROOT` to place the
local harness workspaces elsewhere.
