// Re-export shim. The icon catalog now lives in `@openagentsinc/ui` (see
// packages/ui/src/icon.ts) so the shared UI kit (e.g. workroom.ts) can depend
// on it. App call sites keep importing from `./icon` / `../icon` unchanged.
export * from '@openagentsinc/ui/icon'
