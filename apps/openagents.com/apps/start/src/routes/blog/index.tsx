import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/blog/')({
  beforeLoad: () => {
    throw redirect({ to: '/', replace: true })
  },
})
