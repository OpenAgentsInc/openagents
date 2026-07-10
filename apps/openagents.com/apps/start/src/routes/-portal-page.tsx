// PORTAL-1 (#8652): thin React route-shell host for the /portal Effect
// Native surface. All portal content lives in the EN tree in
// -portal-core.ts; React only mounts it (the EN adapter rule).

import { Effect, Exit, Scope } from '@effect-native/core/effect'
import { useEffect, useRef } from 'react'

import { mountPortalSurface } from './-portal-core'

export function PortalPage() {
  const rootRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const root = rootRef.current
    if (root === null) {
      return undefined
    }

    let disposed = false
    let closeScope: (() => void) | undefined

    void Effect.runPromise(Scope.make())
      .then((scope) => {
        const close = () => {
          void Effect.runPromise(Scope.close(scope, Exit.void))
        }
        closeScope = close
        if (disposed) {
          close()
          return undefined
        }
        return Effect.runPromise(
          Scope.provide(scope)(mountPortalSurface(root)),
        )
      })
      .catch(() => undefined)

    return () => {
      disposed = true
      closeScope?.()
    }
  }, [])

  return (
    <main
      aria-label="OpenAgents client portal"
      className="portal-host"
      data-route="portal"
      data-portal=""
    >
      <div ref={rootRef} data-portal-root="" />
    </main>
  )
}
