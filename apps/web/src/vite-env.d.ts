/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_CONVEX_URL: string;
  readonly VITE_POSTHOG_KEY?: string;
  /** Prelaunch mode: "1" or "true" to show countdown and disable non-home routes. */
  readonly VITE_PRELAUNCH?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
