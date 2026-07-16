import { createFileRoute } from '@tanstack/react-router'

import { LoginPage } from './-login-page'

export const Route = createFileRoute('/login')({
  component: LoginPage,
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
})
