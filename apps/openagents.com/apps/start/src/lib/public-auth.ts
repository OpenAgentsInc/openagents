export type PublicAuthState =
  | Readonly<{ tag: 'anonymous' }>
  | Readonly<{ isAdmin: boolean; tag: 'authenticated' }>

type FetchLike = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

export const readPublicAuthState = async (
  fetcher: FetchLike = fetch,
): Promise<PublicAuthState> => {
  try {
    const response = await fetcher('/api/auth/session', {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (!response.ok) return { tag: 'anonymous' }

    const body: unknown = await response.json()
    if (
      typeof body === 'object' &&
      body !== null &&
      'authenticated' in body &&
      body.authenticated === true &&
      'bootstrap' in body &&
      typeof body.bootstrap === 'object' &&
      body.bootstrap !== null &&
      'isAdmin' in body.bootstrap &&
      typeof body.bootstrap.isAdmin === 'boolean'
    ) {
      return {
        isAdmin: body.bootstrap.isAdmin,
        tag: 'authenticated',
      }
    }
  } catch {
    // The public header remains usable if the fail-soft session probe fails.
  }

  return { tag: 'anonymous' }
}

export const publicAuthAction = (
  state: PublicAuthState,
): Readonly<{ href: string; label: string }> =>
  state.tag === 'authenticated'
    ? state.isAdmin
      ? { href: '/admin/analytics', label: 'Analytics' }
      : { href: '/portal', label: 'Portal' }
    : { href: '/login', label: 'Log In' }

export const publicLogoutAction = (
  state: PublicAuthState,
  showLogout: boolean,
): Readonly<{ href: string; label: string }> | null =>
  showLogout && state.tag === 'authenticated'
    ? { href: '/logout?returnTo=%2F', label: 'Log Out' }
    : null
