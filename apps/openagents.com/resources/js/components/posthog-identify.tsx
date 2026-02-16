import { usePage } from '@inertiajs/react';
import { usePostHog } from '@posthog/react';
import { useEffect } from 'react';

type AuthUser = {
    id: number;
    email: string;
    name: string;
    avatar?: string | null;
};

type SharedAuth = {
    auth: { user: AuthUser | null };
};

/**
 * Identifies the current user with PostHog when auth.user is present.
 * Renders nothing. Use once inside the Inertia app (e.g. wrapped around the resolved page).
 */
export function PostHogIdentify() {
    const posthog = usePostHog();
    const { auth } = usePage<SharedAuth>().props;
    const user = auth?.user ?? null;

    useEffect(() => {
        if (!posthog || !user?.email) return;
        posthog.identify(user.email, {
            email: user.email,
            name: user.name ?? undefined,
        });
    }, [posthog, user?.email, user?.name]);

    return null;
}
