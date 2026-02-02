type PostHogClient = {
  capture: (event: string, properties?: Record<string, unknown>) => void;
  identify: (id: string, properties?: Record<string, unknown>) => void;
};

declare global {
  interface Window {
    posthog?: PostHogClient;
  }
}

function getPostHog(): PostHogClient | null {
  if (typeof window === "undefined") return null;
  if (!window.posthog || typeof window.posthog.capture !== "function") return null;
  return window.posthog;
}

function getPageContext() {
  if (typeof window === "undefined") return {};
  return {
    path: window.location.pathname,
    search: window.location.search,
  };
}

export function posthogCapture(event: string, properties?: Record<string, unknown>) {
  const posthog = getPostHog();
  if (!posthog) return;
  try {
    posthog.capture(event, { ...getPageContext(), ...(properties ?? {}) });
  } catch {
    // ignore analytics failures
  }
}

export function posthogIdentify(id: string, properties?: Record<string, unknown>) {
  const posthog = getPostHog();
  if (!posthog) return;
  try {
    posthog.identify(id, { ...getPageContext(), ...(properties ?? {}) });
  } catch {
    // ignore analytics failures
  }
}
