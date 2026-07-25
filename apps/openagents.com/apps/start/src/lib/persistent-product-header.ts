export const usesPersistentProductHeader = (pathname: string): boolean =>
  pathname === '/' || pathname === '/splash' || pathname === '/admin/analytics'

export const showsProductLogout = (pathname: string): boolean =>
  pathname === '/admin/analytics'
