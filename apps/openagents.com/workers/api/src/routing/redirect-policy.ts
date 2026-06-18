export const githubWriteResultRedirectLocation = (appOrigin: string): string =>
  appOrigin

export const cleanProductRouteRedirectLocation = (
  url: URL,
): string | undefined => {
  if (url.search === '') {
    return undefined
  }

  if (url.pathname === '/login') {
    return `${url.origin}/`
  }

  if (
    url.pathname === '/' ||
    url.pathname === '/billing' ||
    url.pathname === '/onboarding' ||
    url.pathname === '/order' ||
    url.pathname.startsWith('/orders/') ||
    url.pathname.startsWith('/share/')
  ) {
    return `${url.origin}${url.pathname}`
  }

  return undefined
}
