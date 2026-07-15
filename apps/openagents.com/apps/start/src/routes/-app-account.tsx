'use client'

import { Button } from '@/components/ui/button'
import { useEffect, useState } from 'react'

export type AppSessionIdentity = Readonly<{
  avatarUrl: string | null
  email: string | null
  login: string | null
  name: string | null
}>

type AppAccountState =
  | Readonly<{ phase: 'loading' }>
  | Readonly<{ phase: 'signed_out' }>
  | Readonly<{ identity: AppSessionIdentity; phase: 'signed_in' }>

const nullableString = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() !== '' ? value : null

export const readAppSessionIdentity = async (
  fetchFn: typeof fetch = fetch,
): Promise<AppSessionIdentity | null> => {
  try {
    const response = await fetchFn('/api/auth/session', {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (!response.ok) return null

    const body = (await response.json()) as {
      authenticated?: unknown
      bootstrap?: {
        session?: {
          avatarUrl?: unknown
          email?: unknown
          login?: unknown
          name?: unknown
        }
      }
    }
    if (body.authenticated !== true) return null

    const session = body.bootstrap?.session
    return {
      avatarUrl: nullableString(session?.avatarUrl),
      email: nullableString(session?.email),
      login: nullableString(session?.login),
      name: nullableString(session?.name),
    }
  } catch {
    return null
  }
}

const initialsFor = (identity: AppSessionIdentity): string => {
  const source = identity.name ?? identity.login ?? identity.email ?? 'OA'
  const words = source.trim().split(/\s+/).filter(Boolean)

  return (
    words.length > 1
      ? `${words[0]?.[0] ?? ''}${words[1]?.[0] ?? ''}`
      : source.slice(0, 2)
  ).toUpperCase()
}

export function AppAccount({
  initialIdentity,
}: Readonly<{ initialIdentity?: AppSessionIdentity | null }> = {}) {
  const [state, setState] = useState<AppAccountState>(() =>
    initialIdentity === undefined
      ? { phase: 'loading' }
      : initialIdentity === null
        ? { phase: 'signed_out' }
        : { identity: initialIdentity, phase: 'signed_in' },
  )

  useEffect(() => {
    if (initialIdentity !== undefined) return

    let active = true
    void readAppSessionIdentity().then(identity => {
      if (!active) return
      setState(
        identity === null
          ? { phase: 'signed_out' }
          : { identity, phase: 'signed_in' },
      )
    })

    return () => {
      active = false
    }
  }, [initialIdentity])

  if (state.phase === 'loading') {
    return (
      <div
        aria-live="polite"
        className="hidden min-h-10 items-center gap-2 border border-border/20 px-3 text-xs text-muted-foreground sm:flex"
        data-app-account="loading"
      >
        <span className="size-2 rounded-full bg-muted-foreground/45" />
        Checking session…
      </div>
    )
  }

  if (state.phase === 'signed_out') {
    return (
      <Button asChild size="sm" variant="secondary">
        <a href="/login?returnTo=%2Fapp">Session expired · Log in</a>
      </Button>
    )
  }

  const { identity } = state
  const primary =
    identity.name ?? identity.login ?? identity.email ?? 'OpenAgents account'
  const secondary =
    identity.login === null ? identity.email : `@${identity.login}`

  return (
    <div
      aria-label={`Signed in as ${primary}`}
      className="flex min-h-11 items-center gap-2.5 border border-border/20 bg-background px-2 py-1.5"
      data-app-account="signed-in"
    >
      <span className="relative grid size-8 shrink-0 place-items-center overflow-hidden rounded-full border border-border/25 bg-muted text-xs font-semibold text-foreground">
        <span aria-hidden="true">{initialsFor(identity)}</span>
        {identity.avatarUrl === null ? null : (
          <img
            alt=""
            className="absolute inset-0 size-full object-cover"
            referrerPolicy="no-referrer"
            src={identity.avatarUrl}
          />
        )}
      </span>

      <span className="hidden min-w-0 flex-col leading-tight lg:flex">
        <span className="truncate text-xs font-semibold text-foreground">
          {primary}
        </span>
        <span className="truncate text-[11px] text-muted-foreground">
          Signed in{secondary === null ? '' : ` · ${secondary}`}
        </span>
      </span>

      <Button asChild className="ml-1" size="sm" variant="ghost">
        <a href="/logout">Log out</a>
      </Button>
    </div>
  )
}
