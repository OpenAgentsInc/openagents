import posthog from 'posthog-js';

const key = import.meta.env.VITE_POSTHOG_KEY;
const host = import.meta.env.VITE_POSTHOG_HOST ?? 'https://us.i.posthog.com';

if (typeof key === 'string' && key.length > 0) {
    posthog.init(key, {
        api_host: host,
        person_profiles: 'identified_only',
    });
}

export { posthog };
