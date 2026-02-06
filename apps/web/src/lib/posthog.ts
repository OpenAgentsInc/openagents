/**
 * PostHog client helpers for the Effect Telemetry sink.
 * Client-only: no-op when window/posthog is unavailable (SSR or not loaded).
 */

export type PostHogClient = {
  capture: (event: string, properties?: Record<string, unknown>) => void;
  identify: (id: string, properties?: Record<string, unknown>) => void;
};

declare global {
  interface Window {
    posthog?: PostHogClient;
  }
}

export function getPostHog(): PostHogClient | null {
  if (typeof window === 'undefined') return null;
  if (!window.posthog || typeof window.posthog.capture !== 'function') return null;
  return window.posthog;
}

export function getPageContext(): Record<string, string> {
  if (typeof window === 'undefined') return {};
  return {
    path: window.location.pathname,
    search: window.location.search,
  };
}
