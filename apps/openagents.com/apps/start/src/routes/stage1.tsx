import { createFileRoute, redirect } from '@tanstack/react-router'

// Retired with the Effect Native framework removal (#9325). `/stage1` existed
// only to validate the framework itself: an unlinked, noindex landing surface
// that was never the live homepage. The live landing is `/` (and `/splash`).
export const Route = createFileRoute('/stage1')({
  beforeLoad: () => {
    throw redirect({ to: '/splash' })
  },
})
