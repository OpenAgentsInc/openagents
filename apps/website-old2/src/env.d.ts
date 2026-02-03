/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly CONVEX_URL?: string;
  readonly CONVEX_SITE_URL?: string;
  readonly VITE_BREEZ_API_KEY?: string;
  readonly PUBLIC_API_URL?: string;
  readonly PUBLIC_SITE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
