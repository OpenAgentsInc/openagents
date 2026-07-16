import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/new')({
  beforeLoad: () => {
    throw redirect({ to: '/splash' })
  },
})
