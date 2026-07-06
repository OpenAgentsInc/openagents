import { createFileRoute } from '@tanstack/react-router'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AIUR_LOGIN_START_PATH, AIUR_LOGOUT_PATH } from '@/auth/routes'
import { TokensServedPanel } from '@/dashboard/tokens-served-panel'

import { useAiurAccess } from './-use-aiur-access'

export const Route = createFileRoute('/')({
  component: AiurShell,
  head: () => ({
    meta: [{ title: 'Aiur' }],
  }),
})

function SignedOutView() {
  return (
    <section
      data-route="aiur-sign-in"
      className="flex min-h-dvh w-full items-center justify-center bg-khala-void px-6 text-khala-text"
    >
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Aiur</CardTitle>
          <CardDescription>
            Owner-only admin panel for the Khala Code mobile MVP.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button asChild>
            <a href={AIUR_LOGIN_START_PATH}>Sign in with GitHub</a>
          </Button>
        </CardContent>
      </Card>
    </section>
  )
}

function DeniedView({ login }: { login: string | undefined }) {
  return (
    <section
      data-route="aiur-denied"
      className="flex min-h-dvh w-full items-center justify-center bg-khala-void px-6 text-khala-text"
    >
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Access denied</CardTitle>
          <CardDescription>
            {login ? `@${login} is` : 'This account is'} not on the Aiur owner
            allowlist. Aiur fails closed by design — this is expected unless
            you are the OpenAgents owner.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button asChild variant="secondary">
            <a href={AIUR_LOGOUT_PATH}>Sign out</a>
          </Button>
        </CardContent>
      </Card>
    </section>
  )
}

function OwnerDashboard({ login }: { login: string | undefined }) {
  return (
    <section
      data-route="aiur-dashboard"
      className="mx-auto flex min-h-dvh w-full max-w-3xl flex-col gap-6 bg-khala-void px-6 py-10 text-khala-text"
    >
      <header className="flex items-center justify-between">
        <div>
          <h1 className="m-0 font-mono text-2xl font-semibold text-white">Aiur</h1>
          <p className="m-0 text-sm text-khala-text-muted">
            Signed in{login ? ` as @${login}` : ''}.
          </p>
        </div>
        <div className="flex gap-2">
          <Button asChild size="sm" variant="secondary">
            <a href="/credits">Credits console</a>
          </Button>
          <Button asChild size="sm" variant="ghost">
            <a href={AIUR_LOGOUT_PATH}>Sign out</a>
          </Button>
        </div>
      </header>
      <TokensServedPanel />
    </section>
  )
}

function AiurShell() {
  const access = useAiurAccess()

  if (access.kind === 'loading') {
    return (
      <section className="flex min-h-dvh w-full items-center justify-center bg-khala-void text-khala-text-faint">
        <p className="font-mono text-sm">Loading Aiur...</p>
      </section>
    )
  }

  if (access.kind === 'signed_out') {
    return <SignedOutView />
  }

  if (access.kind === 'denied') {
    return <DeniedView login={access.user.login} />
  }

  return <OwnerDashboard login={access.user.login} />
}
