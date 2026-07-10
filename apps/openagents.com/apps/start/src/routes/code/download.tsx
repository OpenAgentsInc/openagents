import { createFileRoute, redirect } from '@tanstack/react-router'

export const redirectRetiredCodeDownloadRoute = (): never => {
  throw redirect({
    to: '/promises',
    replace: true,
    statusCode: 308,
  })
}

export const Route = createFileRoute('/code/download')({
  beforeLoad: redirectRetiredCodeDownloadRoute,
})
