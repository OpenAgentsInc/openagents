// PORTAL-1 (#8652): browser entry for the /portal Effect Native surface as
// served by the Cloud Run monolith (workers/api/src/cloudrun/portal-ui.ts).
// The monolith deploy script bundles this file with `vp build --target
// browser`; the same EN tree also mounts through the TanStack Start route
// shell (routes/portal.tsx) on the isolated Start service. No React here —
// the DOM renderer mounts the typed view program directly.

import { Effect, Exit, Scope } from '@effect-native/core/effect'

import { mountPortalSurface } from './routes/-portal-core'

const boot = async (): Promise<void> => {
  const root = document.getElementById('portal-root')
  if (root === null) {
    return
  }
  const scope = await Effect.runPromise(Scope.make())
  window.addEventListener('pagehide', () => {
    void Effect.runPromise(Scope.close(scope, Exit.void))
  })
  await Effect.runPromise(Scope.provide(scope)(mountPortalSurface(root)))
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    void boot()
  })
} else {
  void boot()
}
