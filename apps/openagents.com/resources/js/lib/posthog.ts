import posthog from 'posthog-js';

const key = import.meta.env.VITE_POSTHOG_KEY;
const host = import.meta.env.VITE_POSTHOG_HOST ?? 'https://us.i.posthog.com';

// Never send client-side PostHog events from local/dev builds.
const shouldEnable = !import.meta.env.DEV && typeof key === 'string' && key.length > 0;

if (shouldEnable) {
    posthog.init(key, {
        api_host: host,
        person_profiles: 'identified_only',
    });
}

export { posthog };
