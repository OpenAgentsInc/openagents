// Re-export shim. The shared Foldkit UI kit now lives in `@openagentsinc/ui`.
// App call sites keep importing from `./ui` / `../ui` / `../../../ui`
// unchanged. App-local modules (tenant-theme, credits-panel) stay here.
export * from '@openagentsinc/ui'
export * from './tenant-theme'
export * from './credits-panel'
export * from './email-sequence-panel'
