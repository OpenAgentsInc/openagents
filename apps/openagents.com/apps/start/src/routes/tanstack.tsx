import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/tanstack')({
  beforeLoad: () => {
    throw redirect({ to: '/astro', replace: true })
  },
})
