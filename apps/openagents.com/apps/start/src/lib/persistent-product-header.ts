export const usesPersistentProductHeader = (pathname: string): boolean =>
  pathname === '/' || pathname === '/splash' || pathname === '/admin/analytics'
