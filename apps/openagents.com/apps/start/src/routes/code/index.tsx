import { createFileRoute, redirect } from '@tanstack/react-router'

export const redirectRetiredCodeRoute = (): never => {
  throw redirect({
    to: '/app',
    replace: true,
    statusCode: 308,
  })
}

export const Route = createFileRoute('/code/')({
  beforeLoad: redirectRetiredCodeRoute,
})
