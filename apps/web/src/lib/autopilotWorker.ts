const trimTrailingSlash = (value: string) => value.replace(/\/+$/, '');

export function getAutopilotWorkerBaseUrl(): string {
  const envUrl = (import.meta as any).env?.VITE_AUTOPILOT_WORKER_URL;
  if (typeof envUrl === 'string' && envUrl.trim()) {
    return trimTrailingSlash(envUrl.trim());
  }
  if (typeof window !== 'undefined') {
    return trimTrailingSlash(window.location.origin);
  }
  return '';
}

export function buildAutopilotUrl(path: string): string {
  const base = getAutopilotWorkerBaseUrl();
  if (!base) return path;
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${base}${normalizedPath}`;
}
