import { createFileRoute, redirect } from '@tanstack/react-router'

// Retired with the Effect Native framework removal (#9325). `/landing-en` was
// an unlinked landing re-authoring whose purpose was the framework itself; its
// copy never cleared owner sign-off (#8565). The live landing is `/` (and
// `/splash`).
export const Route = createFileRoute('/landing-en')({
  beforeLoad: () => {
    throw redirect({ to: '/splash' })
  },
})
