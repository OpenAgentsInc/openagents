const FORGE_DOCUMENT_PREFIX = '/forge'
const FORGE_TENANT_REF = 'tenant.openagents'

const isForgeDocumentPath = (pathname: string): boolean =>
  pathname === FORGE_DOCUMENT_PREFIX ||
  pathname.startsWith(`${FORGE_DOCUMENT_PREFIX}/`)

const copySetCookies = (from: Headers, to: Headers): void => {
  const getSetCookie = (
    from as Headers & { getSetCookie?: () => ReadonlyArray<string> }
  ).getSetCookie
  const values =
    typeof getSetCookie === 'function'
      ? getSetCookie.call(from)
      : ([from.get('set-cookie')].filter(Boolean) as Array<string>)
  for (const value of values) {
    to.append('set-cookie', value)
  }
}

const redirectToLogin = (request: Request): Response => {
  const url = new URL(request.url)
  const returnTo = `${url.pathname}${url.search}`
  const headers = new Headers({
    'cache-control': 'no-store',
    location: `/login?returnTo=${encodeURIComponent(returnTo)}`,
  })
  return new Response(null, { headers, status: 302 })
}

export type ForgeUiMembershipCheck = (
  request: Request,
  tenantRef: string,
) => Promise<Response>

/**
 * Protect each Forge document before the Start handler returns its shell.
 * The membership API remains the policy authority. This gate only maps its
 * result to document behavior and carries refreshed session cookies forward.
 */
export const gateForgeDocumentRequest = async (
  request: Request,
  checkMembership: ForgeUiMembershipCheck,
  serveDocument: () => Promise<Response | undefined>,
): Promise<Response | undefined> => {
  if (!isForgeDocumentPath(new URL(request.url).pathname)) {
    return serveDocument()
  }

  const membership = await checkMembership(request, FORGE_TENANT_REF)
  if (membership.status === 401) {
    return redirectToLogin(request)
  }
  if (!membership.ok) {
    const headers = new Headers({
      'cache-control': 'no-store',
      'content-type': 'application/json; charset=utf-8',
    })
    copySetCookies(membership.headers, headers)
    return new Response(
      JSON.stringify({ error: 'forge_membership_required' }),
      {
        headers,
        status: membership.status === 403 ? 403 : 503,
      },
    )
  }

  const document = await serveDocument()
  if (document === undefined) {
    return undefined
  }
  const headers = new Headers(document.headers)
  copySetCookies(membership.headers, headers)
  return new Response(document.body, {
    headers,
    status: document.status,
    statusText: document.statusText,
  })
}
