import posthog from 'posthog-js';

type RuntimePostHogConfig = {
    key?: string;
    host?: string;
    disabled?: boolean;
};

const runtimeConfig: RuntimePostHogConfig | undefined =
    typeof window !== 'undefined' ? window.__OA_POSTHOG__ : undefined;

const key = import.meta.env.VITE_POSTHOG_KEY ?? runtimeConfig?.key;
const host =
    import.meta.env.VITE_POSTHOG_HOST ??
    runtimeConfig?.host ??
    'https://us.i.posthog.com';
const runtimeDisabled = runtimeConfig?.disabled === true;

// Never send client-side PostHog events from local/dev builds.
const shouldEnable =
    !import.meta.env.DEV &&
    !runtimeDisabled &&
    typeof key === 'string' &&
    key.length > 0;

if (shouldEnable) {
    posthog.init(key, {
        api_host: host,
        person_profiles: 'identified_only',
    });
}

export { posthog };
