import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/blog/$slug')({
  beforeLoad: () => {
    throw redirect({ to: '/', replace: true })
  },
})
