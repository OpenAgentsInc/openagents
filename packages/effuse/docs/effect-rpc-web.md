# Effect RPC (apps/web)

This document describes the Effect RPC surface added to `apps/web` and how to call it from loaders/components.

## Mount Point

- **HTTP route:** `POST /api/rpc`
- **TanStack Start route file:** `apps/web/src/routes/api.rpc.tsx`
- **Handler:** `RpcServer.toWebHandler(...)` using:
  - `disableFatalDefects: true`
  - the shared server `MemoMap` (from `apps/web/src/effect/serverRuntime.ts`)
  - `RpcSerialization.layerNdjson`

## RPC Definition

- **RPC group:** `AgentRpcs` in `apps/web/src/effect/api/agentRpc.ts`
- **Handlers:** `AgentRpcsLive` in `apps/web/src/effect/api/agentRpcHandlers.ts`

The current procedures wrap the existing `AgentApiService` boundary to `/agents/*` so call sites can migrate incrementally without removing the legacy HTTP client.

## Client Usage

- **Client service:** `AgentRpcClientService` in `apps/web/src/effect/api/agentRpcClient.ts`
- **Wired into app layer:** `apps/web/src/effect/layer.ts` (so it is available via `context.effectRuntime`)

Example (from `apps/web/src/routes/modules.tsx`):

```ts
Effect.gen(function* () {
  const rpc = yield* AgentRpcClientService
  return yield* rpc.agent.getModuleContracts({ chatId: userId })
})
```

## Notes

- RPC is additive: do not remove `AgentApiService` usage until RPC equivalents are verified and migrated.
- `MemoMap` sharing matters when RPC handlers and route loaders/serverFns use the same Effect layers: we pass the same memoMap to `RpcServer.toWebHandler` so layer resources are memoized consistently across these entry points.
