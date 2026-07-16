import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/install')({
  beforeLoad: () => {
    throw redirect({ to: '/download', replace: true, statusCode: 301 })
  },
})
