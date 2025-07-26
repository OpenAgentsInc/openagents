/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_OPENAUTH_URL?: string
  readonly VITE_CONVEX_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
