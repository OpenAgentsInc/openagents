const PUBLIC_SITE_HOSTS = new Set(['openagents.com', 'www.openagents.com'])

export const isPublicSiteRootRequest = (url: URL): boolean =>
  PUBLIC_SITE_HOSTS.has(url.hostname) && url.pathname === '/'
