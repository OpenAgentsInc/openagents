import { createFileRoute } from '@tanstack/react-router'

import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AIUR_LOGIN_START_PATH, AIUR_LOGOUT_PATH } from '@/auth/routes'
import { OpsConsole } from '@/ops/ops-console'

import { useAiurAccess } from './-use-aiur-access'

export const Route = createFileRoute('/ops')({
  component: OpsPage,
  head: () => ({
    meta: [{ title: 'Aiur — Ops' }],
  }),
})

function OpsPage() {
  const access = useAiurAccess()

  if (access.kind === 'loading') {
    return (
      <section className="flex min-h-dvh w-full items-center justify-center bg-khala-void text-khala-text-faint">
        <p className="font-mono text-sm">Loading Aiur...</p>
      </section>
    )
  }

  if (access.kind === 'signed_out') {
    return (
      <section
        data-route="aiur-ops-sign-in"
        className="flex min-h-dvh w-full items-center justify-center bg-khala-void px-6 text-khala-text"
      >
        <Card className="w-full max-w-md">
          <CardHeader>
            <CardTitle>Aiur</CardTitle>
            <CardDescription>Sign in to view ops.</CardDescription>
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

  if (access.kind === 'denied') {
    return (
      <section
        data-route="aiur-ops-denied"
        className="flex min-h-dvh w-full items-center justify-center bg-khala-void px-6 text-khala-text"
      >
        <Card className="w-full max-w-md">
          <CardHeader>
            <CardTitle>Access denied</CardTitle>
            <CardDescription>This account is not on the Aiur owner allowlist.</CardDescription>
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

  return (
    <section
      data-route="aiur-ops"
      className="mx-auto flex min-h-dvh w-full max-w-3xl flex-col gap-6 bg-khala-void px-6 py-10 text-khala-text"
    >
      <header className="flex items-center justify-between">
        <div>
          <h1 className="m-0 font-mono text-2xl font-semibold text-white">Ops</h1>
          <p className="m-0 text-sm text-khala-text-muted">
            Who signed up, what did they run, and is the stack up.
          </p>
        </div>
        <div className="flex gap-2">
          <Button asChild size="sm" variant="ghost">
            <a href="/">Dashboard</a>
          </Button>
          <Button asChild size="sm" variant="ghost">
            <a href={AIUR_LOGOUT_PATH}>Sign out</a>
          </Button>
        </div>
      </header>
      <OpsConsole />
    </section>
  )
}
