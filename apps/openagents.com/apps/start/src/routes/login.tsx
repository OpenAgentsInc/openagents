import { createFileRoute } from '@tanstack/react-router'

import { LoginPage } from './-login-page'

export const Route = createFileRoute('/login')({
  component: LoginRoutePage,
  head: () => ({
    meta: [
      { title: 'Log in - OpenAgents' },
      {
        name: 'description',
        content:
          'OpenAgents login screen rendered through the TanStack Start staging app.',
      },
    ],
  }),
  validateSearch: (search: Record<string, unknown>) => ({
    returnTo: search.returnTo === '/app' ? ('/app' as const) : undefined,
  }),
})

function LoginRoutePage() {
  const { returnTo } = Route.useSearch()
  return <LoginPage {...(returnTo === undefined ? {} : { returnTo })} />
}
