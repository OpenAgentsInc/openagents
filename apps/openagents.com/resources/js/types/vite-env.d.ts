/// <reference types="vite/client" />

interface ImportMetaEnv {
    readonly VITE_APP_NAME?: string;
    /** PostHog project API key (frontend). Use same project as backend (POSTHOG_API_KEY). */
    readonly VITE_POSTHOG_KEY?: string;
    /** PostHog host (default https://us.i.posthog.com). */
    readonly VITE_POSTHOG_HOST?: string;
}

interface ImportMeta {
    readonly env: ImportMetaEnv;
}

declare global {
    interface Window {
        __OA_POSTHOG__?: {
            key?: string;
            host?: string;
            disabled?: boolean;
        };
    }
}
