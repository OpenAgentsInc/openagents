// PORTAL-1 (#8652): browser entry for the /portal surface as served by the
// Cloud Run monolith (workers/api/src/cloudrun/portal-ui.ts). The monolith
// deploy script bundles this file with `vp pack --platform browser`; the same
// component also renders through the TanStack Start route shell
// (routes/portal.tsx) on the isolated Start service.
//
// Converted from an Effect Native DOM-renderer mount to a React root (#9325):
// this entry mounts exactly the PortalPage component the Start route uses, so
// both deployments render the identical surface. This file stays `.ts` (no
// JSX) — `createElement` keeps the pack step unchanged.

import { createElement } from 'react'
import { createRoot } from 'react-dom/client'

import { PortalPage } from './routes/-portal-page'

const boot = (): void => {
  const container = document.getElementById('portal-root')
  if (container === null) {
    return
  }
  const root = createRoot(container)
  window.addEventListener('pagehide', () => {
    root.unmount()
  })
  root.render(createElement(PortalPage))
}

if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
      boot()
    })
  } else {
    boot()
  }
}
