const trimTrailingSlash = (value: string) => value.replace(/\/+$/, '');

export function getLiteclawWorkerBaseUrl(): string {
  const envUrl = (import.meta as any).env?.VITE_LITECLAW_WORKER_URL;
  if (typeof envUrl === 'string' && envUrl.trim()) {
    return trimTrailingSlash(envUrl.trim());
  }
  if (typeof window !== 'undefined') {
    return trimTrailingSlash(window.location.origin);
  }
  return '';
}

export function buildLiteclawUrl(path: string): string {
  const base = getLiteclawWorkerBaseUrl();
  if (!base) return path;
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${base}${normalizedPath}`;
}
