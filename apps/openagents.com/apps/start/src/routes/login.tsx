import { createFileRoute } from '@tanstack/react-router'

import { LoginPage } from './-login-page'

type LoginSearch = Readonly<{ returnTo?: string }>

export const Route = createFileRoute('/login')({
  validateSearch: (search): LoginSearch =>
    typeof search.returnTo === 'string'
      ? { returnTo: search.returnTo }
      : {},
  component: LoginRoute,
  head: () => ({
    meta: [
      { title: 'Early access - OpenAgents' },
      {
        name: 'description',
        content: 'Log in with an approved OpenAgents early-access account.',
      },
    ],
  }),
})

function LoginRoute() {
  const { returnTo } = Route.useSearch()
  return <LoginPage {...(returnTo === undefined ? {} : { returnTo })} />
}
